//! Engine tests against a deterministic in-process HTTP/1.1 server (R-NET in TEST_PLAN.md).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rabbit_lib::engine::http::{build_headers, fetch, fetch_mirrored, Counters, Request, ResumeState};
use rabbit_lib::error::AppError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy)]
struct Behaviour {
    ranges: bool,
    /// Close the connection after this many body bytes, for the first N requests.
    drop_after: Option<(u64, usize)>,
    status: u16,
    /// Delay between 64 KiB chunks.
    throttle: Option<Duration>,
    /// Answer 503 while this many connections are already being served (a per-client cap).
    max_conns: Option<usize>,
}

fn byte_at(i: u64) -> u8 {
    (i % 251) as u8
}

async fn serve(len: u64, b: Behaviour) -> (String, Arc<AtomicUsize>) {
    let (url, requests, _) = serve_counting(len, b).await;
    (url, requests)
}

/// Like `serve`, also counting body bytes actually written to sockets.
async fn serve_counting(len: u64, b: Behaviour) -> (String, Arc<AtomicUsize>, Arc<AtomicU64>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let bytes_out = Arc::new(AtomicU64::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();
    let bytes_total = bytes_out.clone();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            let n = counter.fetch_add(1, Ordering::SeqCst);
            let bytes_total = bytes_total.clone();
            let active = active.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let mut got = 0;
                loop {
                    let r = sock.read(&mut buf[got..]).await.unwrap_or(0);
                    if r == 0 {
                        return;
                    }
                    got += r;
                    if buf[..got].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let req = String::from_utf8_lossy(&buf[..got]).to_ascii_lowercase();
                if b.status != 200 {
                    let _ = sock.write_all(format!("HTTP/1.1 {} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n", b.status).as_bytes()).await;
                    return;
                }
                let serving = active.fetch_add(1, Ordering::SeqCst);
                struct Leave(Arc<AtomicUsize>);
                impl Drop for Leave {
                    fn drop(&mut self) {
                        self.0.fetch_sub(1, Ordering::SeqCst);
                    }
                }
                let _leave = Leave(active.clone());
                if b.max_conns.is_some_and(|m| serving >= m) {
                    let _ = sock.write_all(b"HTTP/1.1 503 Busy\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
                    return;
                }
                let range = req.lines().find_map(|l| l.strip_prefix("range: bytes=")).map(|r| {
                    let (a, z) = r.trim().split_once('-').unwrap();
                    let a: u64 = a.parse().unwrap();
                    let z: u64 = if z.is_empty() { len - 1 } else { z.parse().unwrap() };
                    (a, z.min(len - 1))
                });
                let (start, end, head) = match (b.ranges, range) {
                    (true, Some((a, z))) => (a, z, format!("HTTP/1.1 206 Partial Content\r\ncontent-range: bytes {a}-{z}/{len}\r\ncontent-length: {}\r\naccept-ranges: bytes\r\nconnection: close\r\n\r\n", z - a + 1)),
                    _ => (0, len - 1, format!("HTTP/1.1 200 OK\r\ncontent-length: {len}\r\nconnection: close\r\n\r\n")),
                };
                if sock.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                let limit = match b.drop_after {
                    Some((bytes, first_n)) if n < first_n => Some(bytes),
                    _ => None,
                };
                let mut pos = start;
                let mut sent = 0u64;
                while pos <= end {
                    let chunk_end = (pos + 65536).min(end + 1);
                    let chunk: Vec<u8> = (pos..chunk_end).map(byte_at).collect();
                    if let Some(l) = limit {
                        if sent + chunk.len() as u64 > l {
                            let take = (l - sent) as usize;
                            let _ = sock.write_all(&chunk[..take]).await;
                            return; // abrupt close mid-body
                        }
                    }
                    if sock.write_all(&chunk).await.is_err() {
                        return;
                    }
                    bytes_total.fetch_add(chunk.len() as u64, Ordering::SeqCst);
                    sent += chunk.len() as u64;
                    pos = chunk_end;
                    if let Some(d) = b.throttle {
                        tokio::time::sleep(d).await;
                    }
                }
            });
        }
    });
    (format!("http://{addr}/media.bin"), requests, bytes_out)
}

async fn run(url: &str, segments: usize, cancel: &CancellationToken) -> (Result<u64, AppError>, Arc<Counters>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (r, c) = run_in(dir.path(), url, segments, cancel, false).await;
    (r, c, dir)
}

async fn run_in(dir: &Path, url: &str, segments: usize, cancel: &CancellationToken, with_state: bool) -> (Result<u64, AppError>, Arc<Counters>) {
    let client = reqwest::Client::new();
    let req = Request { client: &client, url, headers: build_headers(&BTreeMap::new()), max_segments: segments, max_attempts: rabbit_lib::engine::http::MAX_ATTEMPTS };
    let counters = Arc::new(Counters::default());
    let state = dir.join("out.state.json");
    let r = fetch(&req, &dir.join("out.bin"), counters.clone(), cancel, with_state.then_some(state.as_path())).await;
    (r, counters)
}

fn assert_content(dir: &tempfile::TempDir, len: u64) {
    assert_content_at(dir.path(), len);
}

fn assert_content_at(dir: &Path, len: u64) {
    let data = std::fs::read(dir.join("out.bin")).unwrap();
    assert_eq!(data.len() as u64, len);
    assert!(data.iter().enumerate().all(|(i, b)| *b == byte_at(i as u64)), "content mismatch");
}

const BASE: Behaviour = Behaviour { ranges: true, drop_after: None, status: 200, throttle: None, max_conns: None };

#[tokio::test]
async fn segmented_download_is_byte_exact() {
    let len = 20 * 1024 * 1024 + 12345;
    let (url, requests) = serve(len, BASE).await;
    let (r, c, dir) = run(&url, 4, &CancellationToken::new()).await;
    assert_eq!(r.unwrap(), len);
    assert_eq!(c.segments.lock().unwrap().len(), 4);
    assert_eq!(requests.load(Ordering::SeqCst), 4, "probe response is reused as segment 0");
    assert_content(&dir, len);
}

#[tokio::test]
async fn small_files_use_one_stream() {
    let len = 300_000;
    let (url, requests) = serve(len, BASE).await;
    let (r, _, dir) = run(&url, 8, &CancellationToken::new()).await;
    assert_eq!(r.unwrap(), len);
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert_content(&dir, len);
}

#[tokio::test]
async fn servers_without_ranges_fall_back_to_single_stream() {
    let len = 6 * 1024 * 1024;
    let (url, _) = serve(len, Behaviour { ranges: false, ..BASE }).await;
    let (r, c, dir) = run(&url, 8, &CancellationToken::new()).await;
    assert_eq!(r.unwrap(), len);
    assert_eq!(c.segments.lock().unwrap().len(), 1);
    assert_content(&dir, len);
}

#[tokio::test]
async fn dropped_connections_resume_from_the_last_byte() {
    let len = 16 * 1024 * 1024;
    // The first 4 connections die after 700 KB; segments must resume, not restart.
    let (url, requests) = serve(len, Behaviour { drop_after: Some((700_000, 4)), ..BASE }).await;
    let (r, _, dir) = run(&url, 4, &CancellationToken::new()).await;
    assert_eq!(r.unwrap(), len);
    assert!(requests.load(Ordering::SeqCst) >= 8);
    assert_content(&dir, len);
}

#[tokio::test]
async fn non_resumable_server_restarts_after_drop() {
    let len = 2 * 1024 * 1024;
    let (url, _) = serve(len, Behaviour { ranges: false, drop_after: Some((500_000, 1)), ..BASE }).await;
    let (r, c, dir) = run(&url, 4, &CancellationToken::new()).await;
    assert_eq!(r.unwrap(), len);
    assert_eq!(c.done.load(Ordering::Relaxed), len, "progress must not double-count the failed attempt");
    assert_content(&dir, len);
}

#[tokio::test]
async fn forbidden_is_reported_as_expired_link() {
    let (url, _) = serve(1000, Behaviour { status: 403, ..BASE }).await;
    let (r, _, _dir) = run(&url, 4, &CancellationToken::new()).await;
    assert!(matches!(r, Err(AppError::Expired(403))), "{r:?}");
}

#[tokio::test]
async fn pause_then_resume_from_state_file() {
    let len = 24 * 1024 * 1024 + 777;
    let (url, _, bytes_out) = serve_counting(len, Behaviour { throttle: Some(Duration::from_millis(40)), ..BASE }).await;
    let dir = tempfile::tempdir().unwrap();

    // "Pause": cancel part-way through.
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        c2.cancel();
    });
    let (r, _) = run_in(dir.path(), &url, 4, &cancel, true).await;
    assert!(matches!(r, Err(AppError::Cancelled)), "{r:?}");
    let state = ResumeState::load(&dir.path().join("out.state.json")).expect("state written on pause");
    let persisted: u64 = state.segments.iter().map(|s| s.2).sum();
    assert!(!state.complete);
    assert!(persisted > 0 && persisted < len, "persisted {persisted}");
    let first_run_bytes = bytes_out.load(Ordering::SeqCst);

    // Resume: only the missing bytes (plus the discarded probe) are fetched again.
    let (r, c) = run_in(dir.path(), &url, 4, &CancellationToken::new(), true).await;
    assert_eq!(r.unwrap(), len);
    assert_eq!(c.done.load(Ordering::Relaxed), len);
    assert_content_at(dir.path(), len);
    let second_run_bytes = bytes_out.load(Ordering::SeqCst) - first_run_bytes;
    assert!(second_run_bytes < len - persisted / 2, "resume re-downloaded too much: {second_run_bytes} of {len}, persisted {persisted}");
    assert!(ResumeState::load(&dir.path().join("out.state.json")).unwrap().complete);
}

#[tokio::test]
async fn changed_resource_restarts_instead_of_resuming() {
    let dir = tempfile::tempdir().unwrap();
    let (url_a, _) = serve(12 * 1024 * 1024, Behaviour { throttle: Some(Duration::from_millis(40)), ..BASE }).await;
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        c2.cancel();
    });
    let _ = run_in(dir.path(), &url_a, 4, &cancel, true).await;
    assert!(dir.path().join("out.state.json").exists());

    // Same state file, but the server now serves a file of a different size.
    let len_b = 9 * 1024 * 1024 + 5;
    let (url_b, _) = serve(len_b, BASE).await;
    let (r, _) = run_in(dir.path(), &url_b, 4, &CancellationToken::new(), true).await;
    assert_eq!(r.unwrap(), len_b);
    assert_content_at(dir.path(), len_b);
}

#[tokio::test]
async fn completed_stream_is_not_downloaded_again() {
    let len = 6 * 1024 * 1024;
    let (url, requests) = serve(len, BASE).await;
    let dir = tempfile::tempdir().unwrap();
    let (r, _) = run_in(dir.path(), &url, 4, &CancellationToken::new(), true).await;
    assert_eq!(r.unwrap(), len);
    let after_first = requests.load(Ordering::SeqCst);
    let (r, c) = run_in(dir.path(), &url, 4, &CancellationToken::new(), true).await;
    assert_eq!(r.unwrap(), len);
    assert_eq!(c.done.load(Ordering::Relaxed), len);
    assert_eq!(requests.load(Ordering::SeqCst), after_first, "no new requests for a completed stream");
}

#[tokio::test]
async fn cancellation_stops_promptly() {
    let len = 64 * 1024 * 1024;
    let (url, _) = serve(len, Behaviour { throttle: Some(Duration::from_millis(20)), ..BASE }).await;
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        c2.cancel();
    });
    let started = std::time::Instant::now();
    let (r, c, _dir) = run(&url, 4, &cancel).await;
    assert!(matches!(r, Err(AppError::Cancelled)), "{r:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(c.done.load(Ordering::Relaxed) < len);
}

#[tokio::test]
async fn backs_off_when_the_server_caps_connections() {
    // A CDN that refuses a 4th parallel connection (observed on Bilibili's progressive MP4 host).
    let len = 24 * 1024 * 1024;
    let (url, requests) = serve(len, Behaviour { max_conns: Some(3), throttle: Some(Duration::from_millis(2)), ..BASE }).await;
    let (r, _, dir) = run(&url, 8, &CancellationToken::new()).await;
    assert_eq!(r.unwrap(), len, "the transfer completes at the width the server allows");
    assert_content(&dir, len);
    let n = requests.load(Ordering::SeqCst);
    assert!(n <= 20, "after the first refusals the engine stops opening connections it will lose ({n} requests)");
}
#[tokio::test]
async fn falls_over_to_a_mirror_that_serves_the_file() {
    // Primary cuts every connection after 270 KB (Bilibili's Akamai mirror for HD); the mirror is healthy.
    let len = 12 * 1024 * 1024;
    let (bad, bad_requests) = serve(len, Behaviour { drop_after: Some((270_000, usize::MAX)), ..BASE }).await;
    let (good, _) = serve(len, BASE).await;
    let dir = tempfile::tempdir().unwrap();
    let client = reqwest::Client::new();
    let t = std::time::Instant::now();
    let r = fetch_mirrored(&client, &[&bad, &good], &build_headers(&BTreeMap::new()), 4, &dir.path().join("out.bin"), Arc::new(Counters::default()), &CancellationToken::new(), None).await;
    assert_eq!(r.unwrap(), len);
    assert_content(&dir, len);
    println!("primary requests before giving up: {}; total {:.1}s", bad_requests.load(Ordering::SeqCst), t.elapsed().as_secs_f64());
    assert!(t.elapsed() < Duration::from_secs(30), "a failing host is abandoned quickly when a mirror exists");
    assert!(bad_requests.load(Ordering::SeqCst) <= 12, "the cutting host is not milked 270 KB at a time");
}