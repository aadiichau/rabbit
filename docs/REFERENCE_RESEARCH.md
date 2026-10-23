# Reference Research — DLBunny (dlbunny.com/en)

Research date: 2026-10-22 · Researcher: Claude (agent) · Host: Windows 11 x64, Edge 152 (headless, via REA launch mode)

This document records what was **actually observed** about DLBunny, how it was observed, and what remains
unknown. It is the factual basis for Rabbit's feature scope. Rabbit reproduces *functionality*, not DLBunny's
code, assets, branding, servers or private APIs.

---

## 1. Method and tooling

### 1.1 REA status in this environment

| Check | Result |
|---|---|
| REA package | `rea-agents@6.7.0` (npm), Node 24.22.1 |
| Skill | `~/.claude/skills/reverse-engineer-anything` v42, 207 tools — read before use |
| MCP registration | `~/.claude.json` → `cmd.exe /d /c npx -y rea-agents@6.7.0 mcp`; `rea doctor` = aligned |
| MCP tools in *this* agent session | **Not loaded.** The session started before registration; MCP tools only appear after a restart. |
| Route used | **REA CLI** (`npx -y rea-agents@6.7.0 <command>`), which the REA skill documents as the supported fallback. Same engine, same Evidence records. |

### 1.2 REA tools actually invoked

| # | REA command | Purpose | Evidence ID | Outcome |
|---|---|---|---|---|
| s01 | `capture-browser-scenario` (launch, Edge headless) | Homepage: accessibility tree, screenshot, console, full network event log | `ev_2b2e96cf0031172c02ccb8f235ee2395c6af05a7eb63f134329524c2631f351a` | complete; 704 events, 163 unique request paths |
| s02 | `capture-browser-scenario` | Drive menus: Advanced settings, platform menu, language menu, Paste, FAQ | `ev_05cc6f2988147b952ba0c24e93507b105d45ae2477e6ab2f54bb9a1b99360ece` | **incomplete** — first click (`Advanced settings`) timed out after 8 s; remaining steps did not run. Superseded by s03 DOM reading. |
| s03 | `capture-browser-scenario` (DOM at end) | Full rendered DOM incl. inline scripts of homepage | `ev_8a385ce2f1f507151de92e364f2270d678c54b1fb31778802f709d22d0a7b889` | complete; 279 KB DOM |
| s04 | `capture-browser-scenario` (23 steps, `goto` × 11) | Accessibility tree of every platform page + `/en/history` | `ev_dec2ab8257640c23026252914a7f9a46bfc9147f15cec7bc889403130c72116e` | complete |
| s05 | `capture-browser-scenario` (DOM after each step) | DOM of YouTube, Bilibili, Instagram pages — check for per-platform quality controls | `ev_b573135f9ee7d9f2fb25f8d090ee00f8a885f1fa6f66acfa835b2b2df68b12fb` | complete |

Raw inputs and outputs are kept in `research/rea/` (`sNN-*.json` = scenario input, `*.out.json` = REA
Evidence, `*.txt`/`*.png` = extracted artifacts). Extraction helpers: `research/tools/*.mjs`.

### 1.3 Boundaries respected

* **CAPTCHA not solved or bypassed.** Every DLBunny submission requires a 4-character image CAPTCHA
  (`/api/get_captcha`). Consequently **no extraction request was ever submitted**, and the live result
  screens were never observed. Result-screen behaviour below comes from reading the publicly served inline
  JavaScript captured in the DOM (static reading — REA did not execute or observe those code paths).
* Cookie-consent banner was **not** accepted. Login was not attempted. No DLBunny endpoint is used by Rabbit.
* No DLBunny assets (logos, icons, CSS, copy) are reused.

### 1.4 Evidence classes used below

* **[O] Observed** — present in a REA capture (accessibility tree, screenshot, DOM, network log).
* **[S] Static** — read from public inline JS in a REA-captured DOM. Describes code, not observed execution.
* **[A] Assumption / inference** — not directly evidenced; flagged.
* **[U] Unknown** — could not be determined without solving the CAPTCHA or logging in.

---

## 2. Product overview

| Aspect | Finding | Class |
|---|---|---|
| Delivery | Server-rendered web page (Bootstrap 5 + jQuery 3.7.1). Not a desktop app. | O |
| Extraction location | **Server-side.** Browser posts link text to `/api/create_oxy_order`; server returns finished media URLs. | S |
| Anti-abuse | Image CAPTCHA per submission + Cloudflare JS challenge (`/cdn-cgi/challenge-platform/...`). | O |
| Monetisation | MGID native ads, ShareThis, header-bidding sync pixels; "Buy a coffee" Ko-fi button; some variants carry `donate_flag` ("Donation unlocks premium downloads"). | O/S |
| Tracking | GA4, Microsoft Clarity, Mixpanel, Cloudflare RUM, ~60 third-party ad/ID-sync hosts on a single page load (s01 network log). | O |
| Accounts | "Login" menu present; usage advertised as no-registration-needed. | O |
| Languages | 18 + x-default: ar, de, en, es, fr, hi, id, it, ja, ko, ms, pt, ru, th, tr, vi, zh-CN, zh-HK (`hreflang`). | O |
| Media retention | "This media will be automatically deleted within 30 minutes" — server-side proxy copies. | S |

## 3. Core workflow (single page per platform)

1. **Input** — `<textarea>` with a 500-character limit and live "N remaining" counter; placeholder
   "Paste <Platform> link here". Autosaves to `localStorage` (`tiktok_autosave_text`) 500 ms after typing. [O/S]
2. **Paste** — "Paste" link calls `navigator.clipboard.readText()` and replaces the textarea value; failure →
   toast "Failed to read clipboard contents". "Clear Text" appears when non-empty. [S]
3. **Share-text tolerant** — Tutorial: "paste the copied content directly… you don't need to manually clear the
   text inside", i.e. the server extracts the URL from app share text. [O] (extraction itself [U])
4. **Advanced settings** (collapsible panel) [S]
   * 🔏 Privacy mode switch — hides the item from the public "barrage" feed.
   * File format radio: **mp4** (default) · **mp3** · **jpeg**. Identical on every platform page (s05).
5. **Submit** — requires CAPTCHA text; client validation messages for empty input / empty CAPTCHA / length over
   the per-platform limit. Request: `POST /api/create_oxy_order` JSON
   `{user_uuid_text, user_input_text, user_select_media_format, user_select_appid, user_select_app,
   user_input_captcha_text, user_anonymous}`; timeout 200 s (400 s for >20 000 chars). [S]
6. **Progress** — "Uploading to the cloud and queuing for analysis… N% … Estimated analysis time: 10 second".
   The bar is driven by an **estimated time**, not by server progress. [S]
7. **Result** (`status 200`) [S]
   * Author card: avatar (`author_head`), nickname (`author_name`), caption (`media_desc`).
   * **Video** (`media_format=mp4`): Fluid Player preview (`video_base_url`, poster `video_cover`); one button per
     entry of `video_play_addr[]` labelled `Download [W×H] <size>M`, entries with `audio_quality` become
     `「🎵mp3」` buttons; **"Video cover"** button (webp); **Subtitles** buttons `@<ext> - <name>` from `subs[]`.
   * **Images** (`media_type=2` or format jpeg/webp): masonry gallery from `image_url_list[]`, Live Photo badge
     per `image_live_flags[]`, lightbox; items whose URL contains `.mp4` render as inline video.
   * **Audio** (`mp3`): WaveSurfer waveform + `<audio>` (`auto_stand_url`, `audio_format`); disabled with a notice
     (`auto_offline_desc`) when unavailable.
   * Auto-switch: requesting mp4 on an image post switches to the album (toast); requesting images on a video
     switches to video.
8. **Download** — desktop: `window.open(url)` in a new tab (browser handles the save); mobile/backup: fetch-based
   download with a progress bar. No queue, no concurrency control, no resume. [S]
9. **Errors** — status codes mapped to toasts: `403`, `404`, `407` (input highlighted red), `409` (CAPTCHA
   wrong, field highlighted), `500` (30 s error toast); network failure → "Network connection failed. Please
   refresh the page and try again". Every outcome refreshes the CAPTCHA. Message texts come from the server [U]. [S]
10. **History** (`/en/history`) — client-only `localStorage.downloadHistory` entries
    `{mediaUrl, text, expiration, appName, mediaFormat}`, **expire after 30 minutes**, de-duplicated by URL or
    text; page states "valid for 30 minutes, … only in the current browser". [O/S]
11. **Social proof** — "barrage" ticker from `/api/batch_get_barrages` (recent downloads by other users with
    city/country and titles); "converted N media files for you" counter = local history count. [O/S]

## 4. Platform research

`appid` values come from the barrage renderer and per-page hard-coding [S]. URL formats are the examples each
page's tutorial lists [O]. "yt-dlp extractor" = whether an upstream extractor is known to exist (author knowledge,
**not yet verified in this environment** — yt-dlp is not installed yet). Rabbit statuses are maintained in
`FEATURE_MATRIX.md`.

### 4.1 TikTok & Douyin (served at `/en`, appid 1)
1. **URLs [O]:** `vm.tiktok.com/<code>/`, `www.tiktok.com/@user/video/<id>`, `m.tiktok.com/v/<id>.html`,
   `www.tiktok.com/@user/photo/<id>`, `v.douyin.com/<code>/`, `www.douyin.com/video/<id>`,
   `www.douyin.com/shipin/<id>`, `www.douyin.com/discover?modal_id=<id>`.
2. **Media:** video, photo carousels (Live Photo flags) [S]. 3. **Formats:** mp4, mp3, jpeg/webp [S].
4. **Quality:** per-variant W×H list after extraction [S]; which variants TikTok exposes [U].
5. **Metadata:** author name/avatar, caption, cover [S]. 6. **Limits:** 500 chars [O]; "No watermark" claimed [O] — mechanism [U].
7. **Auth:** none advertised; private accounts not supported (FAQ title) [O].
8. **Independent implementation:** TikTok — yes via yt-dlp `TikTok` extractor (video); photo posts need
   verification [A]. Douyin — yt-dlp has a `Douyin` extractor that historically needs fresh cookies [A]; high risk.
9. **Verified:** URL list, page structure. 10. **Uncertain:** watermark-free source availability, photo-post support in yt-dlp, Douyin viability.

### 4.2 Kuaishou / Kwai (`/en/kuaishou`, appid 5)
URLs [O]: `v.kuaishou.com/<code>`, `www.kuaishou.com/f/<code>`, `www.kuaishou.com/short-video/<id>`,
`www.kwai.com/@user/video/<id>`. Media: video/photo [O heading]. Independent implementation: **no known
maintained yt-dlp extractor** [A] → candidate for a custom adapter or "unsupported". Uncertain: everything about extraction.

### 4.3 RedNote / Xiaohongshu (`/en/xhs`, appid 7)
URLs [O]: `www.xiaohongshu.com/explore/<id>`, `www.xiaohongshu.com/discovery/item/<id>`, `xhslink.com/a/<code>`;
barrage also shows `www.rednote.com/discovery/item/<id>?xsec_token=…` [O]. Media: image notes and video [O heading].
yt-dlp has a `XiaoHongShu` extractor (video) [A]; image notes may need gallery-dl [A]. `xsec_token` appears
required on links [A].

### 4.4 Bilibili (`/en/bilibili`, appid 3)
URLs [O]: `www.bilibili.com/video/BV…`, short links `b23.tv`, `bili2233.cn`, `bili.im`, `live.bilibili.com/<room>`,
international `www.bilibili.tv/en/video/<id>`, `www.bilibili.tv/en/play/<id>`. Tagline "Film quality is optional"
[O] but the page has the same three format radios (s05) — quality choice is presumably the post-extraction
variant list [A]. FAQ: "Why is the downloaded video silent?" [O] → separate DASH audio/video streams [A]; Rabbit
must merge with FFmpeg. yt-dlp `BiliBili`, `BiliIntl`, `BilibiliLive` extractors exist [A]; >480p often needs login [A].

### 4.5 YouTube (`/en/youtube`, appid 2)
URLs [O]: `youtube.com/watch?v=`, `youtube.com/shorts/<id>`, `youtu.be/<id>?si=`. Formats: mp4, mp3, subtitles,
cover [S]. FAQ "Why does the download show error 403?" [O] → expiring/IP-bound stream URLs [A]. yt-dlp is the
reference implementation; recent yt-dlp releases need an external JavaScript runtime for full YouTube format
access [A — must verify]. Some formats donation-gated on DLBunny [S] — Rabbit has no such gating.

### 4.6 Instagram (`/en/instagram`, appid 6)
URLs [O]: `/p/<code>`, `/reel/<code>`, `/tv/<code>`, `/stories/<user>/<id>`, `/s/<highlight>`. Media: photos,
videos, reels [O heading]. Stories/highlights normally require login [A] — Rabbit will mark them unsupported
without user-provided cookies. yt-dlp `Instagram` extractor exists but frequently requires login [A].

### 4.7 Facebook (`/en/facebook`, appid 8)
URLs [O]: `/share/v/<code>/`, `/share/r/<code>/`, `/reel/<id>`, `/<page>/videos/<id>/`,
`m.facebook.com/groups/<g>/permalink/<id>/`, `m.facebook.com/story.php?story_fbid=…`. Video/audio [O heading].
yt-dlp `Facebook` extractor (public videos) [A].

### 4.8 X / Twitter (`/en/twitter`, appid 9)
URLs [O]: `x.com/<user>/status/<id>` (twitter.com equivalents [A]). Video/photo [O heading]. yt-dlp `Twitter`
(video) [A]; image-only posts → gallery-dl [A].

### 4.9 Pinterest (`/en/pinterest`, appid 10)
URLs [O]: `www.pinterest.com/pin/<id>/`, `pin.it/<code>`. Video/photo [O heading]. yt-dlp `Pinterest` (video pins) [A];
image pins via gallery-dl or page metadata [A].

### 4.10 Suno (`/en/suno`, appid 4)
URLs [O]: `suno.com/s/<code>`, `suno.com/song/<uuid>`, `app.suno.ai/song/<uuid>`. Music/song download [O heading].
No yt-dlp extractor known [A]; public song pages are expected to expose audio/cover in page metadata [A] → small
custom adapter candidate.

### 4.11 Not supported by DLBunny's UI
`/en/tiktok` returns DLBunny's 404 page [O] — TikTok shares the `/en` page with Douyin.

## 5. What Rabbit should match, and where it should differ

| Area | DLBunny (observed) | Rabbit decision |
|---|---|---|
| Extraction | Remote server, CAPTCHA-gated | **Local** (yt-dlp + adapters), no CAPTCHA, no account |
| Progress | Time-based estimate | **Real** bytes/speed/ETA from the transfer engine |
| Quality choice | Variant buttons after extraction | Format/resolution picker built only from real extracted formats |
| Download | Opens URL in new tab | Managed queue, concurrency, segmented HTTP, resume, retry, verification |
| History | 30-min localStorage | Persistent SQLite library with search/filter, user-controlled deletion |
| Privacy | ~60 third-party tracking hosts | Zero telemetry; network traffic only to the media source and (opt-in) update checks |
| Gating | Donation-gated variants | None |
| Galleries | Masonry + lightbox | Grid preview, select-all/individual, Live-Photo video parts where present |
| Multi-link | 500-char box, share text tolerated | URL extraction from pasted share text, multiple URLs → batch |
| Languages | 18 | English first; strings centralised for later localisation |

## 6. Technical risks identified

1. **Platform churn** — TikTok/Douyin/Instagram/Kuaishou/RedNote change frequently; yt-dlp updates must be
   user-applicable without reinstalling Rabbit.
2. **Login walls** — Instagram stories, high-quality Bilibili, some Facebook content. Rabbit will not bypass
   these; optional user-supplied browser cookies may be considered later (stored encrypted, never logged).
3. **YouTube** — format access needs a JS runtime in recent yt-dlp [A]; must verify and budget the size.
4. **Expiring media URLs** — direct URLs from extraction expire (DLBunny's 403 FAQ). Rabbit must re-extract on
   403/410 and on resume after restart.
5. **Watermark-free claims** — Rabbit will only report "no watermark" when the extracted format is documented as
   such by the extractor; never as a blanket claim.
6. **Kuaishou, Douyin, Suno** — no confirmed maintained extractor; may ship as "unsupported" until a tested
   adapter exists.

## 7. Open questions (cannot be resolved without CAPTCHA or login)

* Exact server error texts and which situations map to 403/404/407/409/500.
* Whether DLBunny returns multiple resolutions for TikTok, and what the "premium" (donate) variants are.
* How DLBunny handles multiple links in one submission (the UI suggests one link per submission).
