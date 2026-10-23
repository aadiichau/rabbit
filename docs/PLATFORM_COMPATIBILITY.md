# Platform compatibility

The definitive inventory of what Rabbit supports, what was tested, and what each platform withholds.
Last updated 2026-10-23 (Rabbit 0.7.0, yt-dlp 2026.10.21). Test network: home broadband, India.

Statuses: **VERIFIED** — real public samples downloaded, FFprobe-checked, every listed media type covered ·
**PARTIALLY VERIFIED** — works, but a media type or quality tier is untested or unavailable ·
**BLOCKED BY SOURCE RESTRICTIONS** — the platform withholds it from signed-out visitors (or from this
network); Rabbit doesn't bypass it · **FAILED** — should work but doesn't · **NOT TESTED** — no sample or not
reachable from here.

Evidence: `src-tauri/tests/platform-urls.json` (58 public URLs), `research/platform-report.json` (per-URL results
with FFprobe data), `research/quality-audit-{1,2,3}.txt`, `research/regression-full.txt`,
`tests/native_platforms.rs`, `tests/e2e_network.rs`. Root causes: [EXTRACTION_FAILURES.md](EXTRACTION_FAILURES.md).
Extractor choice: [EXTRACTOR_ARCHITECTURE.md](EXTRACTOR_ARCHITECTURE.md).

## 1. Scorecard

| Platform | Video | Audio | Images | 1080p | Highest available quality (verified file) | Status | Notes |
|---|---|---|---|---|---|---|---|
| YouTube | ✅ watch, youtu.be, Shorts | ✅ M4A, Opus, MP3 | ✅ cover | ✅ 1080p60 (Short), 2160p60 | **3840×2160 VP9 + Opus** (Big Buck Bunny) | **VERIFIED** | Without a JS runtime yt-dlp lists fewer formats (muxed 360p, H.264/VP9 720p video-only, multichannel audio) but every resolution tier incl. 2160p |
| Instagram | ✅ reels, posts, silent posts | — (from video) | ✅ photos, carousels | ✅ reel 1080×1920 | 1080×1920 VP9 + AAC | **VERIFIED** | Photos: originals (1080×1350, 1269×1269); carousels: 5 photos, 2 videos. Stories / private posts: login required |
| Facebook | ✅ reels, videos | — | ❌ not implemented | ✅ reel 1080×1920 | 1080×1920 VP9 + AAC | **PARTIALLY VERIFIED** | One video page sends no video data to signed-out visitors. Photos not supported (no public extractor) |
| X / Twitter | ✅ video, multi-video, GIF | — | ✅ photos (originals), mixed posts | ✅ 1080×1440 | 1080×1440 H.264 + AAC; photos 3858×2170 | **VERIFIED** | Native embed-data extractor; yt-dlp fallback. Amplify card from 2015: removed at source |
| Pinterest | ✅ video pins (HLS / MP4) | — | ✅ image pins, story pins | — not offered by samples | 720×1280 H.264 + AAC | **VERIFIED** | Rendition sizes corrected (yt-dlp reported 1080×1920 for 720p files). Story-pin originals answer 403; 736-wide copies used |
| Bilibili | ✅ BV / av, `?p=` parts, b23.tv | ✅ M4A, MP3 | ✅ cover | ✅ with your session (opt-in) | **1920×1080 H.264 + AAC** with your session; 852×480 as a guest | **VERIFIED** (guest + signed-in) | Guests: ≤ 480p (source restriction). **With your own session (opt-in, 0.7.0): 1080p VERIFIED** — a non-premium account, 1920×1080 H.264 + AAC, delivered by the bilivideo.com mirror after Akamai cut the HD stream (mirror failover). 720p progressive exists but its CDN failed half the downloads — not offered. bilibili.tv: FAILED (yt-dlp crash). Live / bangumi: NOT TESTED |
| Kuaishou | ✅ v.kuaishou.com, short-video, /f/ | ✅ MP3 | ✅ cover | ✅ 1080×1920 HEVC | 1080×1920 HEVC + AAC | **PARTIALLY VERIFIED** | Recommended option is H.264 (720p) for playback compatibility; 1080p HEVC selectable. Photo albums: NOT TESTED (no sample) |
| Kwai | ✅ video pages | ✅ MP3 | ✅ cover | — not offered by sample | 720×1280 H.264 + AAC | **VERIFIED** (1 sample) | schema.org page metadata |
| RedNote | ✅ video notes | ✅ MP3 | ✅ image notes, Live Photos | — not offered by sample | 720×1280 H.264 + AAC; images 3193×5744 JPEG | **VERIFIED** | Needs share links (xhslink.com / xhslink.cn / `xsec_token`); token-less links are refused by RedNote |
| Suno | ✅ song video (MP4 with the full song) | 🔒 audio file withheld | ✅ cover | — | 824×1464 H.264 + AAC | **PARTIALLY VERIFIED** | Suno marks the audio file forbidden for signed-out visitors (MP3 → 403); Rabbit offers the public song video and doesn't convert it to MP3 |
| TikTok | — | — | — | — | — | **BLOCKED BY SOURCE RESTRICTIONS** (regional) / NOT TESTED | Blocked by the Government of India on this network; Rabbit doesn't route around regional blocks. `/t/` short links: no extractor |
| Douyin | — | — | — | — | — | **BLOCKED BY SOURCE RESTRICTIONS** | Signed out: post data only via signed web APIs (a_bogus / Argus) or app protocol; share pages are empty shells. yt-dlp: "Fresh cookies are needed". **With your own session (opt-in, 0.7.0): NOT TESTED** — Douyin is banned in India, so no account can be signed in from the test location; **BLOCKED (regional)** like TikTok |
| Other sites | yt-dlp generic | | | | | No promise | Shown as "Other" |

## 2. Inventory: platforms and media types

Advertised by DLBunny (REA capture `research/rea/s04`, `s08`): Douyin, Kuaishou (incl. Kwai links), RedNote,
Bilibili, YouTube, Instagram, Facebook, Twitter, Pinterest, Suno, TikTok. Rabbit detects all of them
(`engine/platform.rs`), plus `gifshow.com`, `chenzhongtech.com`, `xhslink.cn`, `rednote.com`, `bili2233.cn`,
`bili.im`, `pin.it`, `suno.ai`.

| Platform | Media types | Tested samples | Result |
|---|---|---|---|
| YouTube | standard video, Shorts, short link, audio, cover | 4 URLs + 4 e2e tests | all pass |
| TikTok | video, short links (vm./`/t/`), photo posts | 4 | not reachable (regional block) |
| Douyin | video, short link, image posts | 3 | blocked (signed APIs) |
| Bilibili | video, multipart, short link, audio, cover, live, bangumi, bilibili.tv | 8 + 7 live tests | video/multipart/short/audio pass; bilibili.tv fails; live/bangumi untested |
| Kuaishou / Kwai | video (short, desktop, /f/), Kwai, photo albums | 7 | video + Kwai pass; albums untested |
| RedNote | video note, image note, Live Photo, gallery | 7 | pass with share links; token-less links refused by source |
| Instagram | reel, video post, silent post, photo, carousel (photos / videos) | 6 | all pass |
| Facebook | reel, video page, photo | 3 | reels pass; one page without public data; photos not implemented |
| X | video, multi-video, GIF, photo, mixed | 9 | all pass (except a 2015 card removed at source and a link-only post) |
| Pinterest | video pin, short link, image pin, story pin | 5 | all pass |
| Suno | song page, share link | 3 | song video + cover pass; audio withheld by source |

## 3. Not done, and why

* **TikTok, Suno audio** — withheld by the source or the network; see EXTRACTION_FAILURES for the evidence.
* **Bilibili HD and Douyin** — withheld from signed-out visitors. Since 0.7.0 you can opt in to your own browser
  session (Settings → Sessions; docs/DESIGN_SIGNED_IN_SESSIONS.md). Verified: Bilibili 1080p with a real signed-in session, the off-by-default
  behaviour, the guest fallback and the host allow-list. Douyin with a session: not testable from India (banned).
* **Facebook photos, Kuaishou photo albums** — no maintained public extractor (Facebook) / no public sample
  found (Kuaishou).
* **Bilibili live, bangumi; bilibili.tv** — routed to yt-dlp; bilibili.tv crashes there (upstream).
