# Test Plan

Result vocabulary: **Pass (auto)** — automated test run, log cited · **Pass (manual)** — performed by hand, date
and steps recorded · **Fail** · **Not run**. Nothing is marked Pass without a recorded run.

## 0e. Platform compatibility and quality — 2026-10-23 (0.6.0)

Results: [TEST_RESULTS.md](TEST_RESULTS.md) (58-URL highest-quality matrix with FFprobe data per file, suites,
transfer benchmark). Status per platform: [PLATFORM_COMPATIBILITY.md](PLATFORM_COMPATIBILITY.md).

| Suite / scenario | Result |
|---|---|
| Rust unit | **Pass (auto)** 61/61 |
| Engine (+ server capping parallel connections) | **Pass (auto)** 11/11 |
| Native live / YouTube e2e live | **Pass (auto)** 7/7, 4/4 |
| Platform matrix, highest quality | 43/58 pass; 15 failures all explained (EXTRACTION_FAILURES) |
| Release build UI smoke (RedNote image note, X mixed post, Suno, Instagram photo, Pinterest story pin) | **Pass** |

## 0d. Bilibili / Kuaishou / RedNote — 2026-10-23 (0.5.0)

| Suite / scenario | Result |
|---|---|
| Rust unit (45: + WBI mixin key and `w_rid` against the published reference values, `encodeURIComponent` encoding, DASH → formats with Referer, address recognition for all three platforms, obfuscated Kuaishou state, RedNote stream mapping, `__INITIAL_STATE__` parsing with `undefined` / `new Set`, image sniffing, dates, portrait labels, progressive-only answers) | **Pass (auto)** 45/45 |
| Engine / updater (offline) | **Pass (auto)** 10/10, 4/4 |
| `tests/native_platforms.rs` (live, 7 tests) | **Pass (auto)** 7/7 — Bilibili video + M4A + MP3 + cover; multipart `?p=2` title/id and `?p=999` → input error; **pause → restart → resume** of a 68 MB Bilibili download re-analyzed through the native extractor; RedNote video + MP3 + cover; Kuaishou short + direct link name the same post, video + MP3; specific messages for an expired b23.tv code, an unknown BV id, a token-less RedNote link, an expired xhslink, kwai.com, a Kuaishou profile |
| `platform_matrix` (43 URLs) | Bilibili 6/8, Kuaishou 5/6, RedNote 1/5 (all failures have the platform's actual reason); regressions: Instagram 2/3, X 3/5, Facebook 2/3, Pinterest 3/3 — same as Phase 6. TikTok/Douyin/Suno unchanged (network-bound / sign-in / excluded) |
| `e2e_network` (YouTube, 4 tests incl. restart recovery) | **Pass (auto)** 4/4 |
| Live dev app over CDP (`research/tools/ui-analyze.mjs`, `ui-download.mjs`, screenshots `research/shots/ui-*`) | **Pass** — each of the user's three links: Analyze → preview (title, uploader, duration, thumbnail, options) → Download → queue progress → **Verified**; files decoded fully by FFmpeg with 0 errors |
| TypeScript (`tsc --noEmit`), Vitest | **Pass** 4/4 |
| Release build (`tauri build --no-bundle`, 0.5.0) driven over CDP, before FFmpeg was bundled | RedNote and Kuaishou videos verified; Bilibili offered only M4A + cover (release builds never use FFmpeg from PATH) |
| Release build with **bundled FFmpeg** (BtbN LGPL shared n9.0.2): sidecars, 7 DLLs and `licenses/` land beside `rabbit.exe`; Settings → Tools lists the bundled paths | **Pass** — Bilibili 480p merge, Bilibili MP3 (libmp3lame), YouTube "Me at the zoo" merge: all Verified; bundled ffmpeg decodes each output with 0 errors. NSIS installer not built (its tooling download wasn't approved), so installer size is unmeasured |
| RedNote image notes / galleries / Live Photos | **Not run** — no public image-note share link available |
| Kuaishou photo albums | **Not run** (declined with a message; no sample) |

The first full matrix run hit "paging file is too small" while the dev app was rebuilding in parallel; the affected
platforms were re-run and match Phase 6.

## 0c. yt-dlp updater — 2026-10-23

| Suite / scenario | Result |
|---|---|
| Rust unit (29: + tag validation, version order incl. nightly suffixes, SHA2-256SUMS parsing, manifest confinement to its folder) | **Pass (auto)** 29/29 |
| `tests/updater.rs` against a local fake GitHub serving the real binary | **Pass (auto)** 4/4 — verified install switches the manager live and survives restart; revert restores bundled and deletes copies; **tampered checksum installs nothing**; binary reporting a different version than its tag is rejected; malicious API tag (`../../../evil`) refused |
| Live GitHub (`--ignored`) | **Pass** — Stable 2026.10.21 = current (no update offered); Nightly 2026.10.21.233146 installed and verified in 15.5 s; Bilibili still fails with it (upstream fix not yet published) |
| Real app via REA (`research/app/a11.*`) | **Pass** after fixes — Nightly → Check → Update showed real progress (7.6 / 22.3 MB) → "updated · nightly" + verified toast → revert → bundled, update folder empty |

Bugs found in the real-app run and fixed: the status call (runs yt-dlp, ~1–2 s) could reset a channel the user had
just chosen, so *Update* requested a nightly tag from the stable repository (HTTP 404); *Update* now installs the
checked channel, 404s explain themselves, and revert shows progress.

## 0b. Phase 6 run — 2026-10-23 (platforms)

| Suite / scenario | Result |
|---|---|
| Rust unit (25: + silent sources, unknown audio, page-without-media, error classification for link cards / liability / image-only / same-site redirects) | **Pass (auto)** 25/25 |
| Engine | **Pass (auto)** 10/10 |
| `platform_matrix` (35 URLs, network) | Ran twice; final per-platform results in FEATURE_MATRIX §1 |
| Live app via REA: Suno link → "Not supported" panel; silent Instagram post → preview with "no audio track" note | **Pass** (`research/app/a10.*`) |

Bugs found and fixed: video-only formats of silent sources were dropped; verification demanded audio the source
never declared; MP3 offered from a guessed audio track; the generic extractor's HTML page was downloaded as a
"video" (b23.tv); misleading "needs FFmpeg" error; link-card posts reported as "unsupported address"; same-site
redirects reported as external; duplicated error prefixes.

## 0a. Phase 5 run — 2026-10-22

| Suite / scenario | Result |
|---|---|
| Rust unit (21: + library filters/sort/facets, settings defaults, task persistence) | **Pass (auto)** 21/21 |
| Engine (10: + pause→resume from state file byte-exact, changed resource restarts, completed stream skipped) | **Pass (auto)** 10/10 |
| Live `interrupted_download_resumes_after_restart` — 181 MB 720p60 BBB, second manager reads the DB mid-file | **Pass (auto)** — restored Paused/interrupted with 5.7 MB saved; resumed with re-analysis; ffprobe 634.6 s, 1 video + 1 audio; no temp files |
| Live `pause_resume_and_cancel_cleanup` | **Pass (auto)** — pause keeps 4 temp files; cancel deletes them; remove deletes the DB record |
| TypeScript 4 + typecheck | **Pass (auto)** |
| Real app via REA (CDP) — download → **force-kill rabbit.exe** → relaunch | **Pass** — no orphaned yt-dlp/ffmpeg; state files held 117.5 MB; Queue showed "Interrupted 112.1 / 275.0 MB"; Resume continued from ~112 MB (131.7 MB at 9.3 MB/s after 11 s incl. re-analysis); Pause; Cancel deleted all temp files |
| Real app via REA — Library filter chips + sort, Settings notifications switch | **Pass** (screenshots `research/app/a08.*`) |
| Notifications | **Not run** — window focus can't be removed from a CDP scenario; needs a manual check |

Bugs found and fixed in this run: test binaries failed to start (0xC0000139, missing manifest); restart reported 0 saved bytes (persisted counter only advanced on explicit flush); cancelled tasks still displayed discarded bytes; cancelled counted as "Failed"; a removed task could be re-saved by its winding-down run.

## 0. Phase 4 run — 2026-10-22

| Suite | Command | Result |
|---|---|---|
| Rust unit (19) | `cargo test --lib` | **Pass (auto)** — 19/19 |
| Transfer engine (7) | `cargo test --test http_engine` | **Pass (auto)** — 7/7 |
| Live E2E (2, network) | `cargo test --test e2e_network -- --ignored --test-threads=1` | **Pass (auto)** — 2/2; YouTube single-file 240p, native merge 144p (DASH video + AAC, FFmpeg stream copy), MP3 conversion, cover JPG; ffprobe duration ±2 s; no temp files left; unavailable video → clean `extractor` error |
| TypeScript unit (4) | `npx vitest run` | **Pass (auto)** — 4/4 |
| TypeScript typecheck | `npx tsc --noEmit` | **Pass (auto)** |
| UI smoke (REA CDP scenarios on the running dev app) | `research/app/a0*.json` | Home, Library, Settings render with live data; found and fixed sidebar overflow and Settings path overflow |
| E-4 manual | user pasted a YouTube link and downloaded 1080p in the dev app | **Pass (manual, user-performed; output verified by ffprobe: H.264 1920×1080 + AAC, 320.1 s)** |

Bugs found by these runs and fixed: balanced-paren URL trimming; FFmpeg could not infer container from `.part`
temp names (MP3 + merge paths); muxed formats mis-detected as video-only (yt-dlp `audio_ext: none` on muxed);
DRC audio preferred over original; HLS preferred over direct DASH.

## 1. Automated — Rust (`cargo test`)

| ID | Area | Cases | Result |
|---|---|---|---|
| R-ING-1 | Share-text URL extraction | every example URL in REFERENCE_RESEARCH §4 embedded in realistic share text; trailing punctuation; CJK text; multiple URLs; duplicates | Not run |
| R-ING-2 | URL validation | non-http schemes, credentials in URL, >2 048 chars, loopback/private hosts | Not run |
| R-PLT-1 | Platform detection | positive table per platform + negatives (look-alike hosts such as `youtube.com.evil.test`) | Not run |
| R-FS-1 | Filename sanitisation | reserved names (CON, NUL, COM1…), `<>:"/\|?*`, control chars, trailing dot/space, 400-char titles, emoji, RTL text | Not run |
| R-FS-2 | Path containment | `..` segments, absolute paths in titles, symlink/junction escape | Not run |
| R-FS-3 | Collision handling | existing file → ` (2)`; concurrent finalise of identical names | Not run |
| R-NRM-1 | yt-dlp JSON normalisation | recorded real `-J` outputs (fixtures captured from live runs, provenance noted) | Not run |
| R-NET-1..9 | NativeHttp against local test server | ranges on/off; unknown length; mid-transfer disconnect; 429 with Retry-After; 403 → re-extract hook; ETag change on resume; stall; speed limit accuracy ±5 %; cancellation removes temps | Not run |
| R-QUE-1 | Scheduler | concurrency limit respected; per-host limit; failure isolation; restart recovery from DB snapshot | Not run |
| R-DB-1 | Store | migrations, FTS search, 10 000-row paging | Not run |

## 2. Automated — TypeScript (Vitest)

| ID | Area | Result |
|---|---|---|
| T-FMT-1 | Byte/speed/ETA formatters (tabular, unit boundaries) | Not run |
| T-STORE-1 | Progress coalescing and per-row selectors don't re-render siblings | Not run |
| T-IPC-1 | Generated types match Rust (`types.gen.ts` freshness check) | Not run |

## 3. End-to-end (tauri-driver + WebDriver, real binaries)

| ID | Scenario | Result |
|---|---|---|
| E-1 | Paste invalid text → inline validation, no backend call | Not run |
| E-2 | Paste unsupported host → "Other (via yt-dlp)" or clear unsupported message | Not run |
| E-3 | Analyze a real public video → preview shows real title/duration/formats | Not run |
| E-4 | Download selected format → file exists, ffprobe verifies, Library row added | Not run |
| E-5 | Three concurrent downloads with limit 2 → third waits | Not run |
| E-6 | Pause → resume completes with identical SHA-256 to an uninterrupted download | Not run |
| E-7 | Kill app mid-download → restart → task restored as Paused, resumes | Not run |
| E-8 | Settings persist across restart | Not run |

## 4. Platform compatibility (manual, per release)

One stable public URL per platform and media type; results feed FEATURE_MATRIX. URLs are chosen from public,
non-personal content and listed in `tests/platform-urls.json` with the date checked.

## 5. Windows integration (manual)

DPI 100/125/150/200 %; multi-monitor move between different DPIs; snap layouts; window state restore; drag-drop
from Edge/Chrome/Firefox; Windows toast click opens Rabbit; installer install/upgrade/uninstall (per-user);
WebView2 absent → bootstrapper path; long paths (>260) to the download folder; non-ASCII user profile path.

## 6. Failure injection

Disk full (small VHD), read-only destination, network adapter disabled mid-transfer, sidecar missing/corrupt,
yt-dlp update with checksum mismatch (must refuse).
