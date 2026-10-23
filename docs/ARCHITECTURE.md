# Rabbit — Architecture

Status: proposed (Phase 2). Decisions marked **[bench]** are provisional until `BENCHMARKS.md` has measurements.

## 1. Stack evaluation

| Criterion | Tauri 2 + Rust + React/Vite | Electron | Native WinUI 3 (C#) |
|---|---|---|---|
| Startup | WebView2 is shared with Edge and usually warm; Rust core starts in ms | Ships Chromium; typically slower cold start | Fast |
| Idle RAM | WebView2 processes (~80–130 MB total typical) + small Rust core | 150–300 MB typical | Lowest |
| Install size | ~5–10 MB app + sidecars | 80 MB+ before sidecars | Small app, large runtime deps |
| Download engine | Rust async (tokio/reqwest) — ideal | Node streams — adequate | .NET — adequate |
| Design freedom | Full CSS — needed for a bespoke look | Full CSS | XAML — strong but slower to iterate on a bespoke look |
| Security model | Capability-scoped IPC, no Node in renderer | Needs careful hardening | Native |
| Packaging | NSIS/MSI built in, sidecar support | electron-builder | MSIX |

**Decision: Tauri 2 + Rust + React 19 + TypeScript + Vite + Tailwind CSS 4.** No evidence justifies Electron.
RAM figures above are expectations, not measurements; Phase 8 measures them.

### Frontend dependencies (kept deliberately small)

| Package | Why | Alternative rejected |
|---|---|---|
| react, react-dom | UI | — |
| zustand | ~1 KB store, selector subscriptions avoid re-renders | Redux (heavier), Context (re-render fan-out) |
| @tanstack/react-virtual | Virtualised library/queue lists | react-window (less flexible) |
| @tauri-apps/api + plugin bindings | IPC | — |
| tailwindcss 4 | Token-driven styling, no runtime | CSS-in-JS (runtime cost) |
| Fonts via @fontsource (bundled, latin subsets) | Offline, no font CDN | Google Fonts (network, privacy) |

No router (5 top-level views, switched by store state), no animation library (CSS transitions + WAAPI), no
component library, custom 16-px SVG icon set (no icon package).

### Rust crates

`tauri 2`, `tokio` (rt-multi-thread, process, fs, time, sync), `reqwest` (rustls, http2, stream, gzip off for
media), `serde`/`serde_json`, `rusqlite` (bundled SQLite, WAL), `thiserror`, `tracing` + `tracing-appender`,
`url`, `regex` (URL detection), `sha2` (update verification), `md-5` (Bilibili's request signature — the
web API requires an MD5 `w_rid`; same RustCrypto family as `sha2`), `ts-rs` (dev-time TS type generation),
Tauri plugins: `dialog`, `opener`, `notification`, `clipboard-manager`, `window-state`, `single-instance`.
**Not** used: `tauri-plugin-shell` (renderer must never spawn processes).

## 2. Process model

```
┌──────────────────────────── Rabbit.exe (Rust) ─────────────────────────────┐
│  Tauri core ── IPC commands / Channels ──► WebView2 renderer (React UI)    │
│      │                                                                       │
│  AppState                                                                    │
│   ├─ Ingest  (URL extraction + validation + platform detection)  sync, µs   │
│   ├─ Extract (adapter registry) ──spawn──► yt-dlp.exe  (-J, argv only)      │
│   ├─ Queue / Scheduler (tokio tasks, Semaphore = concurrent limit)          │
│   │     └─ Transfer strategies                                              │
│   │          ├─ NativeHttp  (reqwest, segmented ranges, resume)             │
│   │          └─ YtDlpManaged (yt-dlp download, -N fragments)  [bench]       │
│   ├─ PostProcess ──spawn──► ffmpeg.exe (merge / remux / extract audio)      │
│   ├─ Verify      ──spawn──► ffprobe.exe                                     │
│   ├─ Store (SQLite WAL: history, queue snapshot, settings)                   │
│   └─ Events (throttled progress → Channel, ≤10 Hz per task)                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

All child processes are spawned by Rust with an explicit argv (never a shell), `CREATE_NO_WINDOW`, and are
attached to a Windows **Job Object** with `KILL_ON_JOB_CLOSE`, so a crash or exit of Rabbit kills them.

## 3. Download pipeline

| # | Stage | Module | Notes |
|---|---|---|---|
| 1 | URL ingestion | `engine::ingest` | Pull every `http(s)://` URL out of pasted share text; trim trailing punctuation; dedupe; max 2 048 chars per URL |
| 2 | Source detection | `engine::platform` | Host + path regex table → `Platform` enum (or `Other`). Pure function, unit-tested with the URL lists in REFERENCE_RESEARCH |
| 3 | Metadata extraction | `extract::{bilibili, rednote, kuaishou, ytdlp}` | Extraction chain (below). yt-dlp runs `-J --no-playlist --ignore-config --no-plugin-dirs -- <url>`; timeout 90 s; cancellable |
| 4 | Format discovery | `extract::normalize` | Map raw formats → `MediaFormat {id, kind, container, vcodec, acodec, width, height, fps, tbr, filesize/approx, protocol, needs_merge}`; dedupe; never synthesize formats except "extract audio (FFmpeg)" which is labelled as conversion |
| 5 | Queue management | `engine::queue` | `Task` state machine (below); persisted on every state change |
| 6 | Scheduling | `engine::scheduler` | Global Semaphore (default 3); per-host limit (default 2); FIFO with manual reorder |
| 7 | Transfer | `engine::transfer` | Strategy chosen per format (§4) |
| 8 | Post-processing | `engine::post` | FFmpeg `-c copy` merge/remux; audio extraction transcodes only when the target codec differs |
| 9 | Verification | `engine::verify` | Size check vs Content-Length; ffprobe stream count + duration (±2 s tolerance); image decode header check |
| 10 | Finalise | `engine::fs` | Atomic rename `.rabbit-part` → final name; collision → ` (2)`, ` (3)`…; never overwrite |
| 11 | History | `store::history` | Insert library row only after finalise succeeds |
| 12 | UI events | `ipc::events` | Per-task Channel; coalesced progress |

### Extraction chain

`Manager::extract` (engine/manager.rs) runs these steps for every analysis, including the re-analysis a
resumed or expired task does to get fresh stream links:

1. **Normalize + detect**: `ingest` → `platform::detect` (stages 1–2).
2. **Primary**: if one of Rabbit's own extractors claims the address (`extract::Native::for_url`), it runs
   first. Each reads what a signed-out browser gets and returns yt-dlp-shaped JSON (`formats` with
   `http_headers`, or a `playlist` of entries for galleries), so everything after this is shared:
   | Extractor | Claims | Source |
   |---|---|---|
   | `bilibili` | `bilibili.com/video/BV…` or `/av…` (`?p=` part), `b23.tv`, `bili2233.cn`, `bili.im` | Public web API the bilibili.com player calls: anonymous visitor cookies, WBI-signed `x/web-interface/wbi/view/detail` + `x/player/wbi/playurl` (DASH). Session + signing key cached 1 h |
   | `rednote` | `xhslink.com`, `xiaohongshu.com` / `rednote.com` note paths | Mobile note page on www.rednote.com (`window.__INITIAL_STATE__`), share link resolved hop by hop and stopped at the note address |
   | `kuaishou` | `kuaishou.com`, `gifshow.com`, `chenzhongtech.com` (`kwai.com` declined) | Mobile share page `/fw/photo/<id>` (`window.INIT_STATE`, obfuscated keys → post found by its fields); next share host on timeout |
3. **Validation**: the result only counts when `normalize` finds downloadable media in it. No metadata is
   ever synthesized.
4. **Fallback**: on failure, yt-dlp runs — except for definitive answers (sign-in only, bad link) and for
   Kuaishou, which yt-dlp has no extractor for. If both fail, the native extractor's error is shown: it
   knows the platform's actual answer (expired share code, missing token, region lock, removed video).
   `Ok(None)` from a native extractor (a short link that led to a page it doesn't read, e.g. a Bilibili
   live room) hands the address to yt-dlp without an error.
5. **Format selection → download → verification**: unchanged (stages 4–9). Media requests carry the headers
   the extractor attached (Bilibili's CDN needs its Referer).

Rabbit's extractors never sign in, never send tokens of their own, never solve CAPTCHAs; what a platform
reserves for accounts (Bilibili above 480p, RedNote notes without a share token) stays unavailable.

### Task state machine

```
Queued → Extracting → Ready → Downloading ⇄ Paused
                                   │
                                   ├→ PostProcessing → Verifying → Completed
                                   └→ Failed(kind, retryable) → (Retry) → Queued
any non-terminal ──Cancel──► Cancelled (temp files removed)
```

`Failed.kind ∈ {Network, Http(status), Expired, DiskFull, Permission, InvalidPath, Unsupported, LoginRequired,
Extractor(msg), PostProcess(msg), Verification(msg), Cancelled}` — each has a user-facing message and a
`retryable` flag. A failed task never affects other tasks.

## 4. Transfer strategies

| Case | Strategy | Detail |
|---|---|---|
| Progressive HTTP file (TikTok mp4, images, Twitter mp4, single-file formats) | **NativeHttp** | HEAD/GET probe for `Accept-Ranges` + length. If ranged and ≥ 8 MiB: N segments (default 4, max 16) with adaptive count; else single stream. Headers from extractor (`http_headers`) are passed through; cookies never logged |
| Separate video + audio (YouTube DASH, Bilibili) | NativeHttp for each stream in parallel → FFmpeg merge **[bench]** vs yt-dlp managed | YouTube throttles large ranges; may require chunked requests (≤10 MiB). Benchmark decides |
| HLS / DASH manifests (`m3u8`, `mpd`, fragmented) | **YtDlpManaged** with `-N <fragments>` | Mature fragment handling; progress parsed from `--progress-template` JSON lines |
| aria2 | Only if benchmarks show a measurable win over NativeHttp | Would be an extra GPL-2.0 sidecar |

### NativeHttp engine details
* One shared `reqwest::Client` (connection pool, HTTP/2 when offered, rustls, keep-alive, 30 s connect timeout,
  per-read stall timeout 20 s).
* Pre-allocate the `.rabbit-part` file to its full length (`set_len`) when the size is known; each segment writes at
  its own offset with positioned writes (`seek_write` on Windows) through a 256 KiB buffer — no whole-file buffering.
* Sidecar `<file>.rabbit-state` (JSON: url fingerprint, total length, ETag/Last-Modified, segment ranges + bytes
  done) flushed every 2 s → resume after pause, crash or restart. Validator mismatch (ETag/length change) →
  restart from zero; expired URL (403/410) → re-extract then resume if validators match.
* Adaptive segments: start with 2, add segments while aggregate throughput improves ≥10 % per added segment, stop at
  the configured max; drop back on 429/503.
* Retries: exponential backoff with full jitter, base 500 ms, cap 30 s, max 6 attempts per segment; honour
  `Retry-After`.
* Speed: EWMA over 1 s samples (α = 0.3); ETA = remaining / EWMA; shown as "—" until 2 s of samples exist.
* Speed limit: token bucket shared across the task's segments (global limit split by active tasks).
* Cancellation: `CancellationToken` per task, checked between buffer writes; segment tasks aborted; temp + state
  files deleted on Cancel, kept on Pause.

## 5. Data model (SQLite, `%APPDATA%\com.rabbit.app\rabbit.db`, WAL)

```sql
CREATE TABLE library (
  id INTEGER PRIMARY KEY, source_url TEXT NOT NULL, platform TEXT NOT NULL, title TEXT, uploader TEXT,
  duration_s REAL, kind TEXT NOT NULL, format_label TEXT, container TEXT, width INT, height INT,
  file_path TEXT NOT NULL, file_size INT, thumbnail_path TEXT, completed_at INTEGER NOT NULL
);
CREATE INDEX library_completed ON library(completed_at DESC);
CREATE VIRTUAL TABLE library_fts USING fts5(title, uploader, content='library', content_rowid='id');
CREATE TABLE tasks (id TEXT PRIMARY KEY, state TEXT NOT NULL, payload TEXT NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```
Thumbnails are cached as small JPEG/WebP files under `%LOCALAPPDATA%\com.rabbit.app\thumbs` and served through
Tauri's asset protocol scope limited to that directory.

## 6. IPC contract

Commands (request/response, all `async`, never block the main thread):
`analyze(input: String) -> AnalyzeResult` · `enqueue(plan: DownloadPlan) -> TaskId` · `pause(id)` · `resume(id)` ·
`cancel(id)` · `retry(id)` · `remove(id)` · `list_tasks()` · `library_query(q: LibraryQuery) -> Page<LibraryItem>` ·
`library_remove(id, delete_file: bool)` · `open_path(id)` · `reveal_path(id)` · `get_settings()` ·
`set_settings(patch)` · `tool_status() -> ToolStatus` · `update_ytdlp() -> UpdateResult`.

Events: one `Channel<TaskEvent>` subscribed at startup; `TaskEvent = Progress{id, done, total?, speed, eta} |
State{id, state} | Added{task} | Removed{id}`. Progress is coalesced to ≤10 Hz per task and ≤30 Hz total.

Types are defined once in Rust and exported to `src/ipc/types.gen.ts` with `ts-rs`; a CI test fails if the
generated file is stale.

## 7. Security

* **Capabilities:** main window only; allow-list of Rabbit's own commands plus `dialog:open` (folder pick),
  `notification`, `clipboard-manager:read-text`, `window-state`, core window controls. No `fs`, no `shell`.
* **CSP:** `default-src 'self'; img-src 'self' asset: http://asset.localhost https: data:; style-src 'self'
  'unsafe-inline'; connect-src ipc: http://ipc.localhost; script-src 'self'`. Remote thumbnails are fetched by
  Rust and cached; the renderer does not load arbitrary remote scripts.
* **URL validation:** only `http`/`https`, no credentials in URL, length ≤ 2 048, no private-IP/loopback hosts.
* **Subprocesses:** fixed executable paths resolved by Rust; argv arrays; `--` before user URLs; yt-dlp always gets
  `--ignore-config --no-plugin-dirs` and never `--exec`, `--batch-file`, or output templates from metadata.
* **Paths:** output = settings dir ⨝ sanitized filename (Windows-reserved names, `<>:"/\|?*`, control chars,
  trailing dots/spaces removed; ≤ 180 chars; path ≤ 32 767 via `\\?\`); result canonicalised and verified to be
  inside the chosen folder.
* **Logging:** `tracing` to rolling file; query strings and `Cookie`/`Authorization` headers redacted.
* **yt-dlp updates** (`src-tauri/src/sys/updater.rs`, implemented 2026-10-23):
  - Only on user action (Settings → Tools → *Check for updates* / *Update*). Rabbit makes no update requests on its own.
  - Channels: Stable (`yt-dlp/yt-dlp`) or Nightly (`yt-dlp/yt-dlp-nightly-builds`). The release tag must match
    `YYYY.MM.DD[.N]`; asset URLs are built from fixed parts and never taken from the API response.
  - Order of operations: fetch `SHA2-256SUMS` → stream `yt-dlp.exe` (≤ 150 MB) while hashing → **compare SHA-256** →
    only then rename into place and run `--version` (must equal the tag) → write `active.json` atomically.
    Any failure deletes the download; nothing partial is ever activated.
  - Copies are stored as `%LOCALAPPDATA%\com.rabbit.app\bin\yt-dlp-<version>.exe` (versioned, so a running copy never
    blocks an update). The manager swaps its tool snapshot; operations already running keep their binary.
  - *Use bundled version* deletes the manifest and updated copies; the bundled sidecar is always the fallback.
  - **Trust limit:** the checksum file comes from the same GitHub release over HTTPS, so this protects against
    corruption, truncation and swapped assets, not against a compromised GitHub release. yt-dlp also publishes a
    GPG signature (`SHA2-256SUMS.sig`); verifying it is a planned hardening step.
* **Privacy:** no telemetry, no analytics, no account.

## 8. Packaging

* Tauri bundler → **NSIS** installer (per-user install by default, no admin), WebView2 `downloadBootstrapper` (Win 11
  and updated Win 10 already ship WebView2).
* Sidecars in `src-tauri/binaries/` with target-triple suffix: `yt-dlp-x86_64-pc-windows-msvc.exe`,
  `ffmpeg-…`, `ffprobe-…`, fetched and SHA-256-verified by `scripts/fetch-sidecars.ps1` (not committed).
* **FFmpeg (bundled since 0.5.0):** BtbN's **LGPLv3 shared** build, pinned to release
  `autobuild-2026-10-22-13-06` (FFmpeg n9.0.2-33-g4cf298e66a, includes libmp3lame). Only `ffmpeg.exe`,
  `ffprobe.exe` and the 7 DLLs they load ship (≈ 149 MB unpacked; no ffplay). The DLLs and the licence are bundle
  `resources` mapped to the install folder, so they sit next to the sidecars; `licenses/FFmpeg-LICENSE.txt` and
  `licenses/FFmpeg-SOURCE.txt` (build, source link, hash) are installed with the app. Release builds use only
  these bundled tools (never PATH), so a fresh Windows install needs nothing else for merges and MP3.
* `THIRD_PARTY_NOTICES.md` lists every bundled component and licence (yt-dlp: Unlicense; FFmpeg: LGPL-2.1+ with
  source pointer; fonts: OFL-1.1; Rust/npm deps: generated via `cargo about` / `license-checker`).

## 9. Performance risks and mitigations

| Risk | Mitigation |
|---|---|
| yt-dlp (PyInstaller exe) cold start ~1–2 s per call **[bench]** | Run analysis immediately on paste; show platform-detected skeleton instantly; cache extraction results for 10 min |
| Progress events flooding IPC | Coalescing (§6); renderer updates via zustand selectors per row |
| Large library lists | Virtualised list, paged queries (100 rows), FTS search |
| WebView2 RAM | No heavy libs, lazy-load Library/Settings views, thumbnails downscaled to 320 px |
| Disk contention with many segments | Positioned writes, 256 KiB buffers, cap segments per task |
| Sidecar update bricking downloads | Verified swap + fallback to bundled copy |

## 10. Project layout

```
rabbit/
  docs/                 ARCHITECTURE, DESIGN_SYSTEM, FEATURE_MATRIX, REFERENCE_RESEARCH, IMPLEMENTATION_PLAN, BENCHMARKS, TEST_PLAN
  research/             REA scenarios + evidence (not shipped)
  src/                  React app
    app/                shell, title bar, navigation
    design/             tokens.css, primitives (Button, Input, Field, Menu, Tooltip, Dialog, Toast, Progress)
    icons/              16-px SVG icon set
    features/{home,preview,queue,library,settings}/
    state/              zustand stores
    ipc/                typed command wrappers + types.gen.ts
  src-tauri/
    src/
      main.rs, lib.rs, commands.rs, state.rs, error.rs
      engine/{ingest,platform,queue,scheduler,transfer/{native,ytdlp},post,verify,fs}.rs
      extract/{mod,model,ytdlp,normalize,web,bilibili,rednote,kuaishou}.rs
      store/{db,history,settings,tasks}.rs
      sys/{jobobject,paths,sidecar}.rs
    binaries/           sidecars (git-ignored, fetched by script)
    capabilities/default.json
  scripts/              fetch-sidecars.ps1 (pinned URLs + SHA-256), bench/
  tests/                e2e (WebDriver via tauri-driver)
```
