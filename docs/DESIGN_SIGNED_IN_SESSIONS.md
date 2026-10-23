# Design: signed-in sessions (opt-in) for Bilibili HD and Douyin

Status: **built in 0.7.0** (approved design; deviations noted in §9). Off by default. Rabbit never asks for a password and never logs in; it can
reuse a session *you* created in your own browser, for the sites you switch on, and only for those sites.

## 1. Why

| Site | Signed out (today) | With your session |
|---|---|---|
| Bilibili | ≤ 480p (server cap for guests) | what your account is entitled to: 720p/1080p for any account, 1080P+/4K/HDR for 大会员 (premium). Paid or DRM content stays unavailable unless your account already has it |
| Douyin | not readable: post data needs session cookies + signed requests | yt-dlp's Douyin extractor works when it has fresh cookies from a real browser session |

## 2. Where the session comes from

| Source | How | Notes |
|---|---|---|
| **Firefox profile** (recommended) | Rabbit reads `cookies.sqlite` from a profile you pick (auto-listed from `%APPDATA%\Mozilla\Firefox\profiles.ini`), copying the database + WAL to a temp folder first so a running Firefox isn't disturbed | Firefox stores cookie values unencrypted for the signed-in Windows user, so no decryption is involved |
| **cookies.txt file** | A Netscape-format file you export yourself (e.g. a browser extension) and pick in Settings | Works with any browser. You re-export when the session expires |
| Chrome / Edge directly | **Not supported** | Since Chrome 127, Windows cookies use App-Bound Encryption; yt-dlp can't read them (yt-dlp #10927). Defeating a browser's own anti-theft protection is out of bounds for Rabbit. Use Firefox or a cookies.txt export instead |

Settings store only **which source** (profile path or file path) and **which sites** are on — never cookie
values. Values are read fresh for each analysis, held in memory for that request, and dropped.

## 3. Scope rules (the safety core)

1. **Per-site switches**, both off by default: Bilibili, Douyin. Nothing else uses a session.
2. **Domain allow-list**: only cookies whose domain is `bilibili.com` / `douyin.com` (and their subdomains) are
   ever read into memory; everything else in the source is skipped while reading.
3. **Destination allow-list**:
   * Bilibili (Rabbit's own extractor): the cookie header goes only to `www.bilibili.com` and
     `api.bilibili.com`. Media downloads from the CDN carry no cookies (they don't need them — verified in the
     0.5.0 work).
   * Douyin (yt-dlp): Rabbit writes a temporary Netscape file containing **only** the Douyin cookies, passes it
     with `--cookies`, and deletes it when yt-dlp exits (success, failure, cancel). The temp file lives in
     Rabbit's local app-data folder (user-only by default).
4. **Never logged, never persisted, never sent anywhere else**: request logging redacts `Cookie` headers;
   errors never include cookie values; the task database stores only the source URL, as today.
5. **Transparent**: when a session is used, the analysis line says so ("· your Bilibili session"), and options
   that need it are marked.

## 4. Behaviour

**Bilibili** (`extract/bilibili.rs`, unchanged route — WBI-signed web API):
* Switch on + session present → `x/web-interface/nav` with your cookies tells Rabbit whether the session is
  valid, the account name and premium status. Shown in Settings.
* Valid → `playurl` with your cookies (`fnval=4048`, `fourk=1`); every quality the server returns is offered
  (normal DASH merge, same verification incl. the resolution check).
* Expired / not logged in → falls back to the guest route with a notice: "Your Bilibili session has expired —
  showing guest qualities. Sign in again in Firefox."
* Switch off → exactly today's behaviour.

**Douyin** (yt-dlp `Douyin` extractor + `--cookies <temp file>`):
* Switch on → analysis runs through yt-dlp with the filtered cookies; formats as yt-dlp reports them.
* Switch off → today's error, plus a pointer: "Douyin needs your own signed-in session — Settings → Signed-in
  sites."
* yt-dlp says cookies are stale → "Your Douyin session has expired. Open douyin.com in Firefox, then try again."

Resume after restart and expired-link re-analysis use the same rules (the session is re-read; nothing is stored
with the task).

## 5. UI (one new Settings card, no redesign)

```
Signed-in sites                                                     [ off ]
Use your own browser session for sites that hide videos or HD from signed-out
visitors. Rabbit never sees your password; cookies stay on this PC and are only
sent to the site they belong to.

Session source   (•) Firefox profile  [ default-release  ▾ ]
                 ( ) cookies.txt file [ Choose… ]

  Bilibili   [on]   Signed in as <name> · 大会员 · up to 1080P+        [Check]
  Douyin     [off]  Not checked

Platforms may limit accounts used for heavy automated downloading.
```

* The card is collapsed and off until you switch it on.
* "Check" runs the session test (Bilibili: `nav`; Douyin: a lightweight yt-dlp probe) and shows the result.
* Turning the master switch off clears the stored source.

## 6. Code changes (when approved)

| Area | Change |
|---|---|
| `src-tauri/src/sys/cookies.rs` (new) | `CookieSource` (FirefoxProfile / NetscapeFile), Firefox profile discovery, read-only SQLite read of a temp copy (`rusqlite`, already a dependency), Netscape parser + writer, domain filtering |
| `store.rs` / `Settings` | `sessions: { source, bilibili: bool, douyin: bool }` with serde defaults (off); no values |
| `extract/bilibili.rs` | Session type gets an optional user cookie set; `nav` result exposed; guest fallback on expiry |
| `extract/ytdlp.rs`, `engine/manager.rs` | `--cookies <temp>` for Douyin when on; temp-file guard that deletes on drop; Douyin error hints |
| `commands.rs` | `sessions_status`, `sessions_check(site)`, `list_firefox_profiles` |
| Frontend | `SettingsView` card as above; analysis line badge |
| No new dependencies | |

## 7. Testing

* Unit: Netscape parse/write round-trip, domain filtering (a `.google.com` cookie in the source never reaches
  memory), Firefox reader against a fixture database, temp-file deletion on success / error / cancel, `Cookie`
  header redaction in logs.
* Engine/integration: a local server records which hosts received a `Cookie` header — only the allow-listed
  Bilibili API hosts may.
* Live (needs **your** session; Rabbit's tests never handle credentials): gated by `RABBIT_SESSION_SOURCE`.
  You sign in to Bilibili/Douyin in Firefox yourself; the tests then verify Bilibili 720p/1080p downloads with
  FFprobe and a Douyin video download. Until you run them, the statuses stay "NOT TESTED".

## 8. Out of scope

Login forms inside Rabbit, password storage, Chrome/Edge cookie decryption, other sites (Instagram stories,
age-restricted YouTube — could reuse the same mechanism later, each as its own switch).

## 9. As built (0.7.0)

| Piece | Where |
|---|---|
| Cookie sources, domain filter, Firefox profile discovery, Netscape read/write, temp file | `src-tauri/src/sys/cookies.rs` |
| Host allow-list (`SessionCookies`, `get_json_with_session`) | `src-tauri/src/extract/web.rs` |
| Bilibili guest/user caller, `nav` account check, guest fallback note | `src-tauri/src/extract/bilibili.rs` (`SESSION_HOSTS` = api / www.bilibili.com) |
| Douyin `--cookies <temp>` and error hints; Settings check | `src-tauri/src/engine/manager.rs` |
| Commands | `list_firefox_profiles`, `choose_cookie_file`, `session_check` |
| UI | `src/features/settings/SignedInSites.tsx` (Settings → Sessions), session note on the analysis line |

Deviations from the proposal:

* **Douyin "Check"** looks for a login cookie (`sessionid`, `sessionid_ss`, `sid_tt`, `sid_guard`) instead of a
  yt-dlp probe: Douyin has no unsigned account endpoint, and a probe needs a video URL. yt-dlp's own message is
  "Fresh cookies (not necessarily logged in) are needed", so a recent visit without signing in may also work; the
  check says so.
* **Douyin downloads**: the session is used for analysis. Media downloads use the direct links yt-dlp returns,
  without cookies. If a Douyin format ever needs yt-dlp to download it (segmented), that step runs without the
  session — to be confirmed in your live test.

Tests: `sys::cookies` unit tests (6), `tests/session_cookies.rs` (host allow-list against a recording server,
redaction), `tests/native_platforms.rs` — `sessions_off_by_default_change_nothing`,
`session_fallbacks_with_a_signed_out_cookie_file` (made-up cookies: guest fallback + note, Douyin hint, temp file
deleted), and the two tests that need your session: `session_bilibili_hd`, `session_douyin_video`.

### Running the live tests with your session

1. In Firefox, sign in to bilibili.com and douyin.com (and open one video on each).
2. Find the profile folder: Firefox → `about:profiles` → "Root Directory" of the profile in use.
3. In a terminal in `src-tauri`:

```powershell
$env:RABBIT_SESSION_SOURCE = "C:\Users\<you>\AppData\Roaming\Mozilla\Firefox\Profiles\<profile>"
$env:RABBIT_DOUYIN_URL = "https://www.douyin.com/video/<a public video id>"
cargo test --test native_platforms session_ -- --ignored --nocapture --test-threads=1
```

The tests print the account name and the qualities found, never cookie values.
### Live results (2026-10-23)

| Test | Result |
|---|---|
| `session_bilibili_hd` with the user's Firefox profile (non-premium account) | **Pass** — Check: signed in, "up to 1080p"; analysis offered 1080p; file 1920×1080 H.264 + AAC, verified |
| First runs of the same test | Failed: Akamai (`upos-hz-mirrorakam`) cut the 1080p stream after 277 908 bytes on every connection, then refused connections; the 480p stream on the same host and the 1080p stream on the `bilivideo.com` backup were served in full. Fixed with mirror failover (`engine::http::fetch_mirrored`) |
| `session_douyin_video` | **Not run** — Douyin is banned in India, so no account could be signed in |