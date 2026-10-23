//! Signed-in sessions: a user's cookies reach only the hosts allowed for their site
//! (docs/DESIGN_SIGNED_IN_SESSIONS.md §3). A local server records every Cookie header it receives.

use std::sync::{Arc, Mutex};

use rabbit_lib::extract::bilibili::SESSION_HOSTS;
use rabbit_lib::extract::web::{SessionCookies, Web};
use rabbit_lib::sys::cookies::{parse_netscape, Site};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serves `{}` to every request and records (path, Cookie header) pairs.
async fn recording_server() -> (String, Arc<Mutex<Vec<(String, Option<String>)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let mut got = 0;
                while !buf[..got].windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = sock.read(&mut buf[got..]).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    got += n;
                }
                let req = String::from_utf8_lossy(&buf[..got]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
                let cookie = req.lines().find_map(|l| l.strip_prefix("cookie: ").or_else(|| l.strip_prefix("Cookie: "))).map(str::to_string);
                log.lock().unwrap().push((path, cookie));
                let _ = sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").await;
            });
        }
    });
    (format!("http://127.0.0.1:{}", addr.port()), seen)
}

const JAR: &str = "#HttpOnly_.bilibili.com\tTRUE\t/\tTRUE\t4102444800\tSESSDATA\tuser-secret\n.google.com\tTRUE\t/\tTRUE\t4102444800\tSID\tgoogle-secret\n";

#[tokio::test]
async fn session_goes_only_to_allowed_hosts() {
    let (base, seen) = recording_server().await;
    let web = Web::new().unwrap();
    let cookies = parse_netscape(JAR, Site::Bilibili.domains());

    // The real Bilibili allow-list: a request to any other host is refused before it is sent.
    let bili = SessionCookies::new(Site::Bilibili, &cookies, SESSION_HOSTS).unwrap();
    let e = web.get_json_with_session(&format!("{base}/x/web-interface/nav"), "UA", &[], &bili).await.unwrap_err();
    assert!(e.to_string().contains("refusing to send your Bilibili session"), "{e}");
    assert!(seen.lock().unwrap().is_empty(), "nothing reached a host outside the allow-list");
    for host in ["https://upos-hz-mirrorakam.akamaized.net/x.m4s", "https://i0.hdslb.com/a.jpg", "https://bilibili.com.evil.test/", "https://evilbilibili.com/"] {
        assert!(!bili.allows(host), "{host}");
    }
    assert!(bili.allows("https://api.bilibili.com/x/web-interface/nav") && bili.allows("https://www.bilibili.com/"));

    // With the local server allowed, the request carries exactly the site's cookies — never google.com's.
    let local = SessionCookies::new(Site::Bilibili, &cookies, &["127.0.0.1"]).unwrap();
    web.get_json_with_session(&format!("{base}/allowed"), "UA", &[], &local).await.unwrap();
    // A plain request (no session) carries no cookies at all.
    web.get_json(&format!("{base}/plain"), "UA", &[]).await.unwrap();
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen, [("/allowed".to_string(), Some("SESSDATA=user-secret".to_string())), ("/plain".to_string(), None)]);
}

#[test]
fn debug_output_never_shows_values() {
    let cookies = parse_netscape(JAR, Site::Bilibili.domains());
    let s = SessionCookies::new(Site::Bilibili, &cookies, SESSION_HOSTS).unwrap();
    let dbg = format!("{s:?} {cookies:?}");
    assert!(!dbg.contains("user-secret"), "{dbg}");
    assert!(SessionCookies::new(Site::Bilibili, &[], SESSION_HOSTS).is_none(), "no cookies, no session");
}
