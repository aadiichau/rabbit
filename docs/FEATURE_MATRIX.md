# Feature Matrix

Status vocabulary — used everywhere in Rabbit's docs and UI:

| Status | Meaning |
|---|---|
| **Verified** | Exercised end-to-end on a real public URL in a recorded test run (test ID cited) |
| **Partial** | Works for some content types; limits listed |
| **Planned** | Designed, not yet implemented or not yet tested |
| **Unsupported** | Not offered; reason listed. The UI says so instead of failing silently |


Last updated: 2026-10-23 (0.6.0).

## 1. Platforms

Moved to [PLATFORM_COMPATIBILITY.md](PLATFORM_COMPATIBILITY.md) — scorecard, media types and tested samples.
It is the single source of truth for platform status since 0.6.0.

## 2. Application features vs reference

| Feature | DLBunny (evidence class) | Rabbit target | Status |
|---|---|---|---|
| Paste button / clipboard read | ✔ [S] | ✔ + Ctrl+V anywhere | Implemented (manual use observed) |
| Share-text URL extraction | ✔ claimed [O] | ✔ local parser | **Verified** (unit tests incl. Douyin/RedNote share text) |
| Multiple URLs at once | 500-char box [O] | ✔ batch → queue | Partial — detected and counted; only the first is analyzed |
| Platform detection before submit | ✘ (page-per-platform) | ✔ instant, local | **Verified** (unit tests, 42 reference URLs + look-alike hosts) |
| Media preview (thumb, title, creator, duration) | ✔ after extraction [S] | ✔ | **Verified** (e2e + live UI) |
| Format choice mp4 / mp3 / image | ✔ [S] | ✔ + container/codec where real | **Verified** on YouTube |
| Resolution list with sizes | ✔ variant buttons [S] | ✔ from real formats only | **Verified** on YouTube (fixture test asserts every option maps to a real format id) |
| Cover image download | ✔ [S] | ✔ | **Verified** (e2e) |
| Subtitles download | ✔ [S] | ✔ where extractor provides | Implemented, not yet tested |
| Gallery / multi-image | ✔ masonry [S] | ✔ folder of items | **Verified** for a multi-video X post; image galleries not available through yt-dlp on the tested platforms |
| Live Photo indication | ✔ [S] | ✔ where data exists | Planned |
| Real progress / speed / ETA | ✘ estimate only [S] | ✔ | **Verified** in the live app (REA screenshots) |
| Queue + concurrency | ✘ | ✔ | Implemented (4 concurrent in e2e) |
| Pause / resume | ✘ | ✔ when source supports ranges | **Verified** for native transfers (engine tests, live e2e, real app). HLS/yt-dlp-managed pause untested |
| Restart / crash recovery | ✘ | ✔ | **Verified** — force-killed app, relaunched, resumed from saved bytes |
| Retry with backoff | ✘ (manual) | ✔ | Automatic per-segment retry **Verified**; expired links re-analyzed automatically (implemented; observed only via resume path) |
| Persistent history / library | 30-min localStorage [O/S] | ✔ SQLite + filters/sort | **Verified** (unit tests + live UI) |
| Notifications | ✘ | ✔ when unfocused | Implemented, not yet observed |
| Extractor updates without reinstall | n/a (server-side) | ✔ yt-dlp Stable/Nightly, SHA-256 verified, user-triggered, revertible | **Verified** (fake-release tests incl. tampering; live GitHub nightly install; real app via REA) |
| Privacy mode | hides from public ticker [S] | N/A — nothing is ever shared | — |
| CAPTCHA | required [O] | none (local) | — |
| Ads / tracking | many [O] | none | — |
| Languages | 18 [O] | English (strings centralised) | Planned |
| Notifications | ✘ | ✔ Windows toasts | Planned |
