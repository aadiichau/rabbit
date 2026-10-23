# Extraction failures — root causes and decisions

Every failure seen in the 2026-10-23 compatibility work, with evidence. **[V]** = verified by a recorded run or
probe (script cited), **[H]** = hypothesis. Test network: home broadband in India; yt-dlp 2026.10.21 (stable)
unless noted.

## Fixed

| Platform / case | Stage | Root cause [V] | Fix |
|---|---|---|---|
| Bilibili, every video | Metadata | yt-dlp: "Unable to extract initial state" — pages no longer embed it (yt-dlp #17894; nightly 2026.10.21.233146 also fails). Plain `x/web-interface/view` → HTTP 412 | `extract/bilibili.rs`: WBI-signed API (`bilibili-wbi.mjs`) |
| Bilibili, expired b23.tv code | Redirect | b23.tv answers `{"code":-404}` with HTTP 200 instead of redirecting | Reported as an expired short link |
| RedNote, every link | Short-link resolution | www.xiaohongshu.com resets TLS on this network (SNI); yt-dlp's client is reset at xhslink | `extract/rednote.rs` via www.rednote.com mobile page |
| RedNote image notes | Format discovery | yt-dlp reads videos only. The parameter-free image URL is a tiled **HEIC** for recent notes | Full-resolution JPEG via `?imageView2/2/format/jpg` (3193×5744 verified, `rednote-media.mjs` + ffprobe) |
| RedNote xhslink.cn links | URL recognition | Domain unknown to Rabbit | Added to detection and the short-link resolver |
| Kuaishou, every link | Missing extractor | yt-dlp has none | `extract/kuaishou.rs` share page |
| Kuaishou desktop links | Network | www.kuaishou.com times out intermittently from this network | Desktop paths go straight to the share host; next share host on timeout; one retry on connection errors |
| Kwai | Missing extractor | yt-dlp has none | schema.org `VideoObject` on the page (`kwai-ldjson.mjs`) |
| Instagram photo posts / carousels | Format discovery | yt-dlp gets the post (logged-out GraphQL) but raises "There is no video in this post"; photos are only in `thumbnails` | `--ignore-no-formats-error` + `fixups`: last candidate (no size cap) becomes the photo |
| Instagram silent carousel videos | Gallery selection | Items with `acodec: none` were rejected (gallery wanted muxed audio) | Silent items accepted |
| X photo posts | Format discovery | yt-dlp: "No video could be found in this tweet"; photo-only posts were reported as "links to another site" | `extract/x.rs`: embed data, `name=orig` originals (2688×4032 verified) |
| X GIF posts | Labelling | `original_info` is display size (768 wide), the MP4 is 578p | Sizes only from the rendition path; else "Original" |
| Pinterest image pins | Format discovery | yt-dlp: "No video formats found" | `extract/pinterest.rs` (`orig` image) |
| Pinterest story pins | Format discovery / HTTP | yt-dlp returns the cover only; `originals/` URLs answer **403**, 736-wide copies 200 | Per-page candidates, first one the CDN serves |
| Pinterest video pins | Labelling | `V_720P` reports the upload's 1080×1920; the file is 720×1280 (caught by the new resolution check) | Rendition size rescaled from the name |
| Suno | Policy | yt-dlp `KnownLiabilityIE` (project decision, alongside udio.com) | `extract/suno.rs`, public song page; see restriction below |
| Kuaishou share page | Network | One host returned "error sending request" | Same as desktop links |
| Bilibili HD with a signed-in session (0.7.0) | Transfer | The Akamai mirror (`upos-hz-mirrorakam`) cut 720p/1080p streams after 277 908 bytes per connection, then refused connections ("error sending request"); the same file on the `bilivideo.com` backup URL streamed in full | Mirror failover: backup URLs are carried with each stream and used when a host keeps failing; small partial progress no longer resets the retry budget |
| Portrait videos | Labelling | Labels used height: 720×1280 showed "1280p" | Label = short side |

## Source restrictions (not bypassed)

| Platform | What is withheld | Evidence | Why Rabbit stops there |
|---|---|---|---|
| **Bilibili above 480p** | `accept_quality` lists 1080P+…360P, but signed-out requests return ≤ 480p DASH | `bilibili-wbi.mjs`, also with `try_look=1` | Higher tiers require an account |
| Bilibili progressive 720p | `fnval=1` offers a 720p single file to guests, but the CDN cut transfers short (10.4 of 27.8 MB over one connection), answered 503 to parallel requests and 404 to some links | `range-test.mjs`, `bilibili-mp4-url.mjs`; 3 of 6 matrix downloads failed even with connection back-off | Not reliable enough to offer; documented, not used |
| **Douyin** (also banned in India, so the opt-in session can't be tested from here) | Share pages (`iesdouyin.com`, `m.douyin.com`) now render an empty shell; post data only comes from web APIs that require JS-generated `a_bogus` / Argus signatures and session cookies, or from app-protocol APIs. yt-dlp: "Fresh cookies are needed" | `douyin-share.mjs`, `douyin-mshare.mjs`; community reports in SuInk/Diana#612, Ayuan97/douyin-no-watermark#1 (October 2026) | Forging anti-bot signatures or impersonating the mobile app is detection evasion. Possible legitimate route: importing the user's own browser cookies into yt-dlp — a product decision, not implemented |
| **Suno audio file** | Page data sets `audio_url` to `…/api/forbidden`; the MP3 on cdn1 answers 403. The player streams an m4a | `suno-clip.mjs`, `suno-probe.mjs` | Rabbit offers the public song **video** (H.264 + AAC, full song) and cover, not the audio stream, and doesn't convert the video to MP3 |
| RedNote notes without `xsec_token` | 302 to `/404/sec_…`, even for a public note | `hops.mjs` | Only share links work; Rabbit says so |
| Instagram stories, private posts | Login required (yt-dlp) | — | No sign-in |
| Facebook `radiokicksfm/videos/3676516585958356` | Page sent to a signed-out visitor contains no video data (no playable/progressive URLs); yt-dlp: "Cannot parse data" | probe in session log | Treated as not publicly available |

## Network-bound (cannot be tested from here)

| Platform | Observation [V] | Status |
|---|---|---|
| TikTok | Pages return the notice that the Government of India blocked the app | NOT TESTED; Rabbit doesn't route around regional blocks. `/t/` short links have no yt-dlp extractor |

## Still failing (upstream / out of scope)

| Case | Evidence | Status |
|---|---|---|
| bilibili.tv (international site) | yt-dlp: `'NoneType' object is not subscriptable` (now explained as an extractor crash) | FAILED, waits for yt-dlp |
| X Amplify card `twitter.com/starwars/status/665052190608723968` | yt-dlp: media unavailable | Source removed |
| X post linking only to a shop page | "links to lily-brw.com" | Correct: no media of its own |
| Kuaishou photo albums | No public sample found | NOT TESTED; declined with a message |
| Facebook photos | yt-dlp has no Facebook photo support | NOT IMPLEMENTED |
