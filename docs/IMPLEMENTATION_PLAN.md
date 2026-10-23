# Implementation Plan & Checklist

Each milestone is a vertical slice that must build, pass its tests, and be recorded here before the next starts.
`[x]` = done and verified · `[~]` = done, verification pending · `[ ]` = not started.

## Phase 0 — Environment (2026-10-22)

| Item | State | Detail |
|---|---|---|
| Node.js / npm | [x] | 24.22.1 / 11.19.1 |
| Git | [x] | 2.55.0 |
| WebView2 | [x] | 152.0.4191.66 |
| FFmpeg / ffprobe (dev) | [x] | 9.0.2 full build (winget, user scope) — dev only; the installer bundles its own |
| Rust toolchain | [x] | rustc 1.99.1, `stable-x86_64-pc-windows-msvc` (installed with approval) |
| MSVC Build Tools + Windows SDK | [x] | VS 2026 Build Tools (C++ workload) at `…\18\BuildToolsRabbit`, SDK 10.0.26100. The VS 2022 installer was blocked by a stale VS 2026 instance left from an earlier uninstall; installed to a fresh path instead (approved) |
| yt-dlp (dev + sidecar) | [x] | 2026.10.21, SHA-256 verified against the release's SHA2-256SUMS by `scripts/fetch-sidecars.ps1` |
| Cargo target dir | [x] | `%LOCALAPPDATA%\rabbit-build\target` via `src-tauri/.cargo/config.toml` (project path is 181 chars) |
| aria2 | [ ] | Deferred until benchmarks show a need |
| REA skill + MCP registration | [x] | Skill read; MCP needs a session restart; CLI used |

## Phase 1–3 — Research, architecture, design

- [x] REA investigation (5 scenarios, evidence in `research/rea/`) → `REFERENCE_RESEARCH.md`
- [x] `FEATURE_MATRIX.md`
- [x] `ARCHITECTURE.md`
- [x] `DESIGN_SYSTEM.md` + static specimen (`docs/design/specimen.html`)

## Phase 4 — Functional MVP ✅ (2026-10-22)

M4.1 Scaffold
- [x] Tauri 2.12 + Vite 8 + React 19 + TS 7 + Tailwind 4; Plex fonts bundled (@fontsource); tokens wired; app icon
- [x] Custom title bar (drag region, min/max/restore/close)
- [ ] `scripts/check.ps1` wrapper for all suites (commands documented in TEST_PLAN §0 meanwhile)

M4.2 Ingest & detection
- [x] `engine::ingest` share-text URL extraction + validation (unit tests)
- [x] `engine::platform` detection table (42 reference URLs + look-alikes)

M4.3 Extraction
- [x] Sidecar resolution + per-child Job Object (kills PyInstaller grandchild) + `CREATE_NO_WINDOW`
- [x] yt-dlp `-J` adapter, normaliser (real-fixture tests), 90 s timeout, cancel, error classification
- [x] `analyze` command; preview UI with real data and inline thumbnail

M4.4 First download
- [x] Native transfer: single stream and parallel ranges with real progress, speed (EWMA), ETA
- [x] Safe filename + collision handling (` (2)`) + temp files cleaned by prefix + atomic rename
- [x] Open file / reveal in Explorer
- [x] Real public URL end-to-end (TEST_PLAN §0: automated e2e + user-performed 1080p)

## Phase 5 — Download manager ✅ core (2026-10-22)
- [x] Queue + scheduler (global semaphore, live resize)
- [x] Queue persistence (SQLite `tasks`, migration 2) + restart recovery: running tasks return as Paused/"Interrupted" with saved bytes; stream URLs are never stored — resume re-analyzes
- [x] Segmented ranges with in-transfer resume, retry/backoff, stall detection, Retry-After (engine tests)
- [x] Resume state files (`.rabbit-<id>-N.state.json`): only bytes handed to the OS are recorded; validated by total size + ETag; completed streams are skipped on resume
- [x] Pause / resume / cancel (discards partial data) / retry / remove, Pause all / Resume all, Space/Delete/Enter on rows
- [x] Expired links mid-transfer: automatic re-analysis and resume (once)
- [x] FFmpeg merge (stream copy) / MP3 conversion; ffprobe verification
- [x] Library: SQLite, search, platform/kind filter chips with counts, sort (newest/oldest/largest/title), virtualised list, open/reveal/copy URL/remove/delete-with-confirm
- [x] Settings persistence (folder, concurrency, segments, notifications) with defaults for older saved settings
- [x] Windows notifications on finish/failure when the window isn't focused (setting) — implemented; **not yet observed on screen**
- [x] App manifest embedded in every binary (Common Controls v6, PerMonitorV2 DPI, longPathAware)
- [ ] Library: FTS5 index, thumbnails
- [ ] Adaptive segment count; per-host connection limit; global speed limit
- [ ] yt-dlp-managed (HLS) tasks: pause relies on yt-dlp's own `.part` resume — not yet tested

## Phase 6 — Platforms ✅ (2026-10-23)
- [x] Test URL manifest (yt-dlp `_TESTS` + DLBunny examples) and `platform_matrix` probe through the real pipeline
- [x] Per-platform status with evidence in FEATURE_MATRIX
- [x] Normaliser: silent sources, unknown-audio handling, generic-page rejection; clearer platform errors
- [x] Suno: declined (yt-dlp "Liability" exclusion). Kuaishou: researched, not shipped (unreachable during testing)
- [x] RedNote: blocked by CAPTCHA for automated visitors — not pursued
- [ ] **Image posts** (Instagram/X photos, Pinterest image pins, TikTok photos, RedNote): yt-dlp doesn't extract them. Candidate: gallery-dl as a second sidecar — needs approval to download, a licence check (GPL-2.0) and its own test pass
- [x] **yt-dlp in-app update** — Stable/Nightly, SHA-256 verified, version-checked, user-triggered, revertible (2026-10-23)
- [ ] Verify yt-dlp's GPG signature on `SHA2-256SUMS` (stronger than same-origin checksums)
- [ ] Re-test TikTok, RedNote and Kuaishou from a network where they are reachable

## Phase 7 — UI polish
- [ ] Motion, empty/error states, shortcuts, focus order, high-contrast check, 100/125/150/200 % DPI screenshots

## Phase 8 — Performance
- [ ] Startup (cold/warm), idle RAM, CPU, analysis latency, throughput matrix, library 10 k rows → `BENCHMARKS.md`

## Phase 9 — Release
- [ ] Sidecar fetch script with pinned SHA-256
- [ ] NSIS installer, install/uninstall test, size measurement
- [ ] THIRD_PARTY_NOTICES, README (build, troubleshooting), CHANGELOG
