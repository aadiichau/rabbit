# Bilibili, Kuaishou and RedNote — investigation and fix (2026-10-23)

Labels: **[V]** verified by a recorded run or probe (file cited) · **[H]** hypothesis, not verifiable from here.

## 1. Reproduction (Rabbit 0.4.0, yt-dlp 2026.10.21 stable and 2026.10.21.233146 nightly)

| Link | Failing stage | Observed error [V] |
|---|---|---|
| `bilibili.com/video/BV1efJk6aEdA?t=1.6` | Metadata (yt-dlp) | `Unable to extract initial state` — the video page no longer embeds `__INITIAL_STATE__`; yt-dlp issue #17894, fix PR #17898 not merged. The plain `x/web-interface/view` API answers HTTP 412 (risk control) |
| a RedNote video share link (xhslink.com) | Short-link resolution (yt-dlp) | Connection reset. `www.xiaohongshu.com` resets TLS for every client on this network (SNI-level) while `www.rednote.com` on the same IPs answers |
| a Kuaishou share link (v.kuaishou.com) | Detection (Rabbit) | Declined up front: yt-dlp has no Kuaishou extractor, and in Phase 6 www.kuaishou.com timed out |

## 2. DLBunny — what can and can't be known

Observed with REA (`capture-browser-scenario`, evidence `ev_3e8e062b…`, `research/rea/s08.*`) [V]:
- `/en/bilibili`, `/en/kuaishou`, `/en/xhs` each show a CAPTCHA image next to the Download button and say to
  "wait for our servers to complete the respective work": extraction happens on DLBunny's servers.
- Listed link formats — Bilibili: bili2233.cn, b23.tv, bili.im, live.bilibili.com, bilibili.tv, bilibili.com/video;
  Kuaishou: v.kuaishou.com, kuaishou.com/f/, kuaishou.com/short-video/, kwai.com; RedNote: xiaohongshu.com
  /explore and /discovery/item, xhslink.com/a/.
- Their examples today: `b23.tv/a9P3uZO` answers Bilibili code -404 (expired); `xhslink.com/a/RVjdnIaGmsi2`
  redirects to the RedNote home page (expired); `kuaishou.com/f/X-9XB3YvenaCTkCE` still resolves to a video.

Not knowable from the outside [H]: which extractor DLBunny runs, where its servers are (a mainland-China network
would explain why xiaohongshu.com and www.kuaishou.com work for it), and whether it uses accounts or cookies for
higher Bilibili qualities. Rabbit doesn't depend on any of it: DLBunny's private API sits behind a CAPTCHA and
isn't used.

## 3. Routes chosen (all public, signed-out)

| Platform | Route | Probe [V] |
|---|---|---|
| Bilibili | Anonymous visitor cookies (homepage + `x/frontend/finger/spi`), WBI signing key from `x/web-interface/nav`, signed `x/web-interface/wbi/view/detail` and `x/player/wbi/playurl` (fnval 4048) — the route of yt-dlp PR #17898 and of bilibili.com's player | `research/tools/bilibili-wbi.mjs`: code 0, DASH AVC/HEVC/AV1 at 480p/360p + AAC, CDN range requests 206 with Referer |
| RedNote | Resolve xhslink hop by hop, stop at the note address, fetch `www.rednote.com/discovery/item/<id>?<share query>` with a phone browser's User-Agent; read `window.__INITIAL_STATE__.noteData.data.noteData` | `research/tools/rednote-mobile.mjs`, `rednote-dump.mjs`, `rednote-media.mjs`: full note, h264 stream, originals without resize parameters, no CAPTCHA |
| Kuaishou | Phone-browser redirect to the share page `/fw/photo/<id>` (v.m.chenzhongtech.com, m.gifshow.com, v.kuaishou.com all serve it); read `window.INIT_STATE` (keys obfuscated, post found by its fields) | `research/tools/kuaishou-photo.mjs`, `kuaishou-routes.mjs`: manifest representations with sizes, codecs, CDN 206 |

Rejected: `try_look=1` on Bilibili (sent by yt-dlp for signed-out users) — tested, no change to the 480p cap.
Rejected after testing: Bilibili's progressive MP4 (`playurl` with `fnval=1`). It is offered to signed-out
visitors at **720p** (H.264 + AAC in one file, verified with ffprobe on 4 videos, `research/tools/bilibili-mp4-url.mjs`),
but its CDN reset or answered 503 to parallel range requests even at 4 connections (`research/tools/range-test.mjs`)
and a single connection stalled; in the matrix 3 of 4 downloads failed. Rabbit keeps the reliable DASH route
(480p); the progressive form is used only when a video has no DASH streams at all.
Accounts, cookies from a browser, CAPTCHA solving and DLBunny's API — out of bounds by design.

## 4. Platform limits found (not Rabbit bugs) [V]

- Bilibili gives signed-out visitors 480p at most (`accept_quality` lists 1080P+…360P; only ≤480p streams are
  returned).
- RedNote answers a note link without its `xsec_token` with a 302 to `/404/sec_…`, even for a note that opens
  fine through its share link. Rabbit says "use the share link".
- Kuaishou's www.kuaishou.com host intermittently times out from this network; the share hosts are reachable.

## 5. Results

See FEATURE_MATRIX §1 (per-platform status), TEST_PLAN §0d (suites) and BENCHMARKS §1b (latency).
