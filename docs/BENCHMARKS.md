# Benchmarks

Only measured numbers appear here. Each entry states build type, method and date. Targets from the brief
(cold start < 2 s, idle RAM < 150 MB) are **not yet measured on a release build** — see §4.

## Test environment

| | |
|---|---|
| CPU / RAM | Intel Core i5-13450HX · 15.6 GB |
| OS | Windows 11 Home, build 10.0.26300, x64 |
| WebView2 | 152.0.4191.66 |
| Tools | yt-dlp 2026.10.21 · FFmpeg 9.0.2 (gyan full, dev only) · Node 24.22.1 (JS runtime, dev only) |
| Network | Home broadband, not characterised yet (no controlled link) |

## 1. Extraction latency (2026-10-22)

`yt-dlp -J` on a public YouTube video (`jNQXAC9IVRw`, 19 s), wall clock, single run each:

| Configuration | Time | Formats returned |
|---|---|---|
| No JavaScript runtime | **24 752 ms** | 24 (yt-dlp warns that formats may be missing) |
| `--js-runtimes node` | **4 958 ms** | 25 |
| Rabbit `analyze` (yt-dlp + node + thumbnail fetch + normalise), run 1 | 5 694 ms | 10 options |
| Rabbit `analyze`, run 2 | 5 908 ms | 8 options |

**Decision driven by this measurement:** a JavaScript runtime is required in the shipped app for YouTube. Bundling
choice (Deno vs Node vs QuickJS) is open; it affects installer size and must be benchmarked the same way.

## 1b. Native extractors — analysis latency (2026-10-23)

`cargo test --test native_platforms analysis_latency -- --ignored --nocapture`: three `analyze` runs per link in
one process (run 1 includes TLS setup and, for Bilibili, the visitor session + signing key; later runs reuse
them). Includes thumbnail fetch and normalisation.

| Link | Run 1 | Run 2 | Run 3 |
|---|---|---|---|
| Bilibili `BV1efJk6aEdA` | 1 884 ms | 585 ms | 500 ms |
| RedNote a RedNote video share link (xhslink.com) | 2 631 ms | 1 754 ms | 1 171 ms |
| Kuaishou a Kuaishou share link (v.kuaishou.com) | 3 543 ms | 1 660 ms | 1 596 ms |
| Kuaishou `kuaishou.com/short-video/3xjegz725scedme` | 3 478 ms | 1 065 ms | 1 010 ms |

For comparison, `yt-dlp -J` on the same Bilibili link took **4 286 ms to fail**. The native route spawns no
process. Transfers use the existing native engine unchanged (segmented ranges; Bilibili's DASH video + audio in
parallel, then an FFmpeg stream-copy merge). Observed in the live app: 8.1 MB/s from Bilibili's CDN.

## 2. Transfer engine — correctness under fault injection (not throughput)

`cargo test --test http_engine` against an in-process HTTP/1.1 server on loopback (debug build). These verify
behaviour; the timings are not throughput claims.

| Case | Result |
|---|---|
| 10 MB + 12 345 B, ranges on, 4 segments | byte-exact; 4 requests total (probe reused as segment 0) |
| 300 KB file | 1 request, single stream |
| Server without ranges | single stream, byte-exact |
| First 4 connections cut at 700 KB | segments resumed from last byte, byte-exact |
| Non-resumable server cut at 500 KB | restarted cleanly, progress not double-counted |
| HTTP 403 | reported as "link expired" |
| Cancel after 300 ms on a throttled 64 MB transfer | stopped in < 3 s |

## 3. Live observations (not controlled benchmarks)

| Observation | Value | Notes |
|---|---|---|
| User-initiated YouTube 1080p download (HLS route, before the format-preference fix) | ~1.3 MB/s displayed | Path was yt-dlp-managed HLS; native DASH is now preferred. Needs a controlled re-test |
| YouTube 1080p60 H.264 (275 MB), native DASH, 8 segments + parallel audio, debug build | 5.5 MB/s at 6 s; 9.3 MB/s after resume | Same session, same network, single sample each — indicative only |
| Live e2e: 720p60 (181 MB) resumed run incl. ~5.5 s re-analysis | 44.6 s wall clock | ≈ 4 MB/s effective including analysis and merge |
| Debug build, idle, Home view, Vite dev server | rabbit.exe 32 MB working set; WebView2 6 processes, 193 MB private bytes total | **Not representative**: unminified React dev build + StrictMode + HMR client. Release measurement pending |

## 4. Not yet measured

Cold/warm start (release), idle RAM (release), CPU while idle and during downloads, segmented vs single-stream
throughput, segment-count sweep (1/2/4/8/16), native vs yt-dlp-managed vs aria2, disk write rate, UI frame time
during 3 concurrent downloads, library with 10 000 rows, installer size.
