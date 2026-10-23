# Test results — 2026-10-23 (Rabbit 0.6.0)

All runs on Windows 11, home broadband in India, yt-dlp 2026.10.21 (stable, bundled), FFmpeg n9.0.2 (LGPL).
Nothing here is estimated: every PASS is a completed download whose file FFprobe read back.

## 1. Platform matrix — highest quality (RABBIT_PICK=best)

`cargo test --test platform_matrix -- --ignored --nocapture` with `RABBIT_PICK=best`: for every public URL in
`src-tauri/tests/platform-urls.json`, analyze → pick the highest-resolution option (or the image / gallery
option) → download → FFprobe. Raw data: `research/platform-report.json`, log `research/final-best.txt`.

**43 passed, 15 failed** out of 58. Every failure is explained in
[EXTRACTION_FAILURES.md](EXTRACTION_FAILURES.md) (regional block, signed-API platform, expired or token-less
share links, content removed at source, one upstream yt-dlp crash).

| Platform | URL | Type | Highest offered | Chosen | Final (FFprobe) | Size | Audio | Outcome | Extractor / error |
|---|---|---|---|---|---|---|---|---|---|
| youtube | youtu.be/jNQXAC9IVRw | video | 240p | 240p (v-18) | 320×240 h264 | 0.6 MB | aac | PASS | Youtube |
| youtube | www.youtube.com/watch?v=aqz-KE-bpKQ | video | 2160p | 2160p60 (v-315+251) | 3840×2160 vp9 | 1372.5 MB | opus | PASS | Youtube |
| youtube | www.youtube.com/watch?v=jNQXAC9IVRw | video | 240p | 240p (v-18) | 320×240 h264 | 0.6 MB | aac | PASS | Youtube |
| youtube | www.youtube.com/shorts/r-rLeLMwvpE | video | 1080p | 1080p60 (v-299+140) | 1080×1920 h264 | 7.8 MB | aac | PASS | Youtube |
| bilibili | www.bilibili.com/video/BV13x41117TL | video | 480p | 480p (v-32-7+a30280) | 852×480 h264 | 68.3 MB | aac | PASS | Bilibili (Rabbit web API) |
| bilibili | www.bilibili.com/video/av8903802/ | video | 480p | 480p (v-32-7+a30280) | 852×480 h264 | 68.3 MB | aac | PASS | Bilibili (Rabbit web API) |
| bilibili | www.bilibili.com/video/BV1T44FezE7a/ | video | 480p | 480p (v-32-7+a30280) | 852×480 h264 | 15.5 MB | aac | PASS | Bilibili (Rabbit web API) |
| bilibili | b23.tv/a9P3uZO | video | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: this Bilibili short link doesn't lead anywhere — it may have expired. Copy the link a |
| bilibili | www.bilibili.tv/en/video/4794396117045760 | video | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: yt-dlp's reader for this site hit an internal error, which usually means the site cha |
| bilibili | www.bilibili.com/video/BV1efJk6aEdA?t=1.6 | video | 480p | 480p (v-32-7+a30280) | 852×480 h264 | 6.3 MB | aac | PASS | Bilibili (Rabbit web API) |
| bilibili | www.bilibili.com/video/BV1bK411W797?p=2 | video | 480p | 480p (v-32-7+a30280) | 852×480 h264 | 9.1 MB | aac | PASS | Bilibili (Rabbit web API) |
| bilibili | b23.tv/hiS7rgR | video | 480p | 480p (v-32-7+a30232) | 852×480 h264 | 10.4 MB | aac | PASS | Bilibili (Rabbit web API) |
| kuaishou | www.kuaishou.com/short-video/3x88nbkyjtga6zk | video | 720p | 720p (v-720p-1) | 720×1280 h264 | 21.8 MB | aac | PASS | Kuaishou (Rabbit share page) |
| kuaishou | v.kuaishou.com/5fkjkb | video | 1080p | 1080p (v-1080p-3) | 1080×1920 hevc | 2.8 MB | aac | PASS | Kuaishou (Rabbit share page) |
| kuaishou | www.kwai.com/@vokic313/video/5220666502875224403 | video | 720p | 720p (v-mp4) | 720×1280 h264 | 1.8 MB | aac | PASS | Kwai (Rabbit page metadata) |
| kuaishou | v.kuaishou.com/… (private share link) | video | 720p | 720p60 (v-720p-1) | 720×1280 h264 | 3.2 MB | aac | PASS | Kuaishou (Rabbit share page) |
| kuaishou | www.kuaishou.com/short-video/3xjegz725scedme | video | 720p | 720p60 (v-720p-1) | 720×1280 h264 | 3.2 MB | aac | PASS | Kuaishou (Rabbit share page) |
| kuaishou | www.kuaishou.com/f/X-9XB3YvenaCTkCE | video | 720p | 720p (v-720p-1) | 1080×720 h264 | 10.5 MB | aac | PASS | Kuaishou (Rabbit share page) |
| rednote | www.xiaohongshu.com/explore/6411cf99000000001300b6d9 | video | — | — | — | — | — | FAIL (input) | RedNote only shows notes through their share link. Use “Copy link” in the RedNote app (an xhslink.com link or  |
| rednote | www.xiaohongshu.com/discovery/item/674051740000000007027a15?xsec_token | video | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: RedNote didn't show note 674051740000000007027a15. It may have been deleted or made p |
| rednote | www.xiaohongshu.com/explore/672451ee000000001a034bc6 | any | — | — | — | — | — | FAIL (input) | RedNote only shows notes through their share link. Use “Copy link” in the RedNote app (an xhslink.com link or  |
| rednote | xhslink.com/o/… (private share link) | video | 720p | 720p (v-h264-0) | 720×1280 h264 | 4.9 MB | aac | PASS | RedNote (Rabbit mobile page) |
| rednote | xhslink.com/a/RVjdnIaGmsi2 | error | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: this RedNote share link has expired or doesn't lead to a note. Copy the link again fr |
| rednote | xhslink.cn/o/8FKGfNCXZpB | gallery | — | All 7 items (gallery-all) | 7 files: 3193×5744, 2268×4032, 2304×4096, 2304×4096, 2268×4032, 3213×5712, 1080×1920 | 6.0 MB | none | PASS | RedNote (Rabbit mobile page) |
| rednote | xhslink.cn/o/198ItN4kuCe | gallery | — | All 8 items (gallery-all) | 8 files: 3072×4096, 4096×3072, 3072×4096, 3072×4096, 1080×1440, 1440×1080, 1080×1440, 1080×1440 | 8.8 MB | none, none, none, none | PASS | RedNote (Rabbit mobile page) |
| instagram | instagram.com/p/aye83DjauH/ | video | 640p | 640p (v-dash-273381622250014v) | 640×640 h264 | 1.0 MB | none | PASS | Instagram |
| instagram | www.instagram.com/reel/DBBEGXpvwNF/ | video | 1080p | 1080p (v-dash-2019796195510917v+dash-1640403389870486a) | 1080×1920 vp9 | 6.0 MB | aac | PASS | Instagram |
| instagram | www.instagram.com/p/DEOGT-qyOwm/ | image | — | Original image (img-photo) | 1269×1269 mjpeg | 0.1 MB | none | PASS | Instagram |
| instagram | www.instagram.com/p/BqvsDleB3lV/ | image | — | Original image (img-photo) | 1080×1350 mjpeg | 0.4 MB | none | PASS | Instagram |
| instagram | www.instagram.com/p/BoHk1haB5tM/ | gallery | — | All 5 images (gallery-all) | 5 files: 1080×1080, 1080×1080, 1080×1080, 1080×1080, 1080×1080 | 0.8 MB | — | PASS | Instagram |
| instagram | www.instagram.com/p/BtOvDOfhvRr/ | gallery | — | All 2 videos (gallery-all) | 2 files: 640×640, 640×640 | 5.2 MB | none, none | PASS | Instagram |
| facebook | www.facebook.com/radiokicksfm/videos/3676516585958356/ | video | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: [facebook] 3676516585958356: Cannot parse data; please report this issue on  https:// |
| facebook | www.facebook.com/reel/1195289147628387 | video | — | Original (v-hd) | 480×848 h264 | 0.6 MB | none | PASS | Facebook |
| facebook | www.facebook.com/reel/1355164985501660 | video | 1080p | 1080p (v-757373597212310v+1092851765576675a) | 1080×1920 vp9 | 24.4 MB | aac | PASS | Facebook |
| twitter | twitter.com/starwars/status/665052190608723968 | video | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: the media is unavailable — [twitter:amplify] 93ad3568-03d2-47b9-a57b-624b88693acd: Un |
| twitter | twitter.com/captainamerica/status/719944021058060289 | video | 720p | 720p (v-mp4-2176) | 1280×720 h264 | 0.5 MB | aac | PASS | X (Rabbit embed data) |
| twitter | twitter.com/oshtru/status/1577855540407197696 | gallery | — | All 2 items (gallery-all) | 2 files: 1536×1920, 720×900 | 2.1 MB | aac | PASS | X (Rabbit embed data) |
| twitter | twitter.com/UltimaShadowX/status/1577719286659006464 | gallery | — | All 4 videos (gallery-all) | 4 files: 1280×720, 1280×720, 1280×720, 1280×720 | 1.2 MB | aac, aac, aac, aac | PASS | X (Rabbit embed data) |
| twitter | x.com/yua_mikami/status/1876940930139181424 | gallery | — | All 4 images (gallery-all) | 4 files: 2688×4032, 2688×4032, 2688×4032, 2688×4032 | 5.4 MB | — | PASS | X (Rabbit embed data) |
| twitter | twitter.com/perrypumas/status/894001459754180609 | gallery | — | All 4 images (gallery-all) | 4 files: 2048×1536, 2048×1536, 1536×2048, 1536×2048 | 1.8 MB | — | PASS | X (Rabbit embed data) |
| twitter | x.com/carrotsprout_/status/1577924293023133696 | gallery | — | All 2 items (gallery-all) | 2 files: 1800×2000, 720×800 | 0.8 MB | none | PASS | X (Rabbit embed data) |
| twitter | x.com/GrimboGrim/status/1839019491835129889 | video | — | Original (v-mp4-0) | 1280×578 h264 | 0.1 MB | none | PASS | X (Rabbit embed data) |
| twitter | x.com/gopherfootball/status/1950259395432239395 | gallery | — | All 3 items (gallery-all) | 3 files: 3724×2095, 3858×2170, 1080×1440 | 10.6 MB | aac | PASS | X (Rabbit embed data) |
| pinterest | pinterest.com/pin/dive-into-serenity-blue-lagoon-pedi-nails-for-a-tran | video | 720p | 720p (v-V_720P) | 720×1280 h264 | 1.0 MB | aac | PASS | Pinterest |
| pinterest | www.pinterest.com/pin/1477812373602736/ | video | 720p | 720p (v-V_720P) | 720×1280 h264 | 9.8 MB | aac | PASS | Pinterest |
| pinterest | pin.it/1X4o9EJSU | video | 720p | 720p (v-V_720P) | 720×1280 h264 | 9.8 MB | aac | PASS | Pinterest |
| pinterest | www.pinterest.com/pin/858146903966145189/ | image | — | Original image (img-orig) | 399×600 mjpeg | 0.1 MB | none | PASS | Pinterest (Rabbit pin resource) |
| pinterest | jp.pinterest.com/pin/858146904010573850/ | gallery | — | All 3 images (gallery-all) | 3 files: 225×400, 225×400, 675×1200 | 0.0 MB | — | PASS | Pinterest (Rabbit pin resource) |
| suno | suno.com/song/8e94d88d-912d-4bda-a5b4-42d9e04d74c4 | video | — | Original (v-video) | 824×1464 h264 | 7.8 MB | aac | PASS | Suno (Rabbit song page) |
| suno | app.suno.ai/song/b27c29f6-8ab4-47eb-81fd-efb85c848ada | video | — | Original (v-video) | 512×768 h264 | 7.7 MB | aac | PASS | Suno (Rabbit song page) |
| suno | suno.com/s/AQrKvtqiIoeTE9RI | video | — | Original (v-video) | 824×1464 h264 | 4.5 MB | aac | PASS | Suno (Rabbit song page) |
| tiktok | www.tiktok.com/@patroxofficial/video/6742501081818877190?langCountry=e | video | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: TikTok didn't return the video. TikTok is blocked in some countries (including India) |
| tiktok | vm.tiktok.com/ZTR45GpSF/ | video | — | — | — | — | — | FAIL (extractor) | Couldn't read this link: TikTok didn't return the video. TikTok is blocked in some countries (including India) |
| tiktok | www.tiktok.com/t/ZTRC5xgJp | video | — | — | — | — | — | FAIL (unsupported) | This link isn't supported: no extractor recognises this address |
| tiktok | www.tiktok.com/@evynhuynh/photo/7367130722806648106 | gallery | — | — | — | — | — | FAIL (unsupported) | This link isn't supported: no extractor recognises this address |
| douyin | www.douyin.com/video/6961737553342991651 | video | — | — | — | — | — | FAIL (loginRequired) | This content requires signing in to the platform. Rabbit doesn't sign in to platforms. |
| douyin | www.douyin.com/video/6982497745948921092 | video | — | — | — | — | — | FAIL (loginRequired) | This content requires signing in to the platform. Rabbit doesn't sign in to platforms. |
| douyin | v.douyin.com/8Tdgeu/ | video | — | — | — | — | — | FAIL (unsupported) | This link isn't supported: no extractor recognises this address |

## 2. Automated suites

| Suite | Result |
|---|---|
| Rust unit (`cargo test --lib`) | 61 / 61 pass |
| Transfer engine (`tests/http_engine.rs`, local server incl. connection cap, drops, resume, cancel) | 11 / 11 pass |
| yt-dlp updater (`tests/updater.rs`, fake GitHub) | 4 / 4 pass |
| Native extractors live (`tests/native_platforms.rs`: Bilibili video + M4A + MP3 + cover, multipart, **pause → restart → resume**, RedNote, Kuaishou short + direct, failure messages) | 7 / 7 pass |
| YouTube end-to-end live (`tests/e2e_network.rs`: video + MP3, invalid inputs, restart recovery, pause / cancel cleanup) | 4 / 4 pass |
| Full default-choice matrix (`research/regression-full.txt`) | same outcomes as §1 |
| TypeScript (`tsc --noEmit`), Vitest | pass, 4 / 4 |
| Release build (`tauri build --no-bundle`) with bundled FFmpeg | builds; UI smoke: RedNote image note (8 items), X mixed post, Suno song, Instagram photo — analyzed, downloaded, Verified |

## 3. Your links

| Link | Result |
|---|---|
| bilibili.com/video/BV1efJk6aEdA | 852×480 H.264 + AAC (guest maximum), M4A, MP3, cover |
| xhslink.com/o/… (private share link) | 720×1280 H.264 + AAC (note's maximum), MP3, cover |
| v.kuaishou.com/… (private share link) | 720×1280 60 fps H.264 + AAC (post's maximum), MP3, cover |

## 4. Transfer benchmark (`tests/bench_transfer.rs`, release build)

Same file fetched once per setting, sequentially:

```
    YouTube f299                       segments=1    257.6 MB in 352.67 s =   0.73 MB/s
    YouTube f299                       segments=2    257.6 MB in 190.27 s =   1.35 MB/s
    YouTube f299                       segments=4    257.6 MB in  83.37 s =   3.09 MB/s
    YouTube f299                       segments=8    257.6 MB in  40.81 s =   6.31 MB/s
    YouTube f299                       segments=16   257.6 MB in  20.76 s =  12.41 MB/s
    Bilibili 32-7                      segments=1     54.7 MB in  62.37 s =   0.88 MB/s
    Bilibili 32-7                      segments=2     54.7 MB in  25.36 s =   2.16 MB/s
    Bilibili 32-7                      segments=4     54.7 MB in  12.60 s =   4.34 MB/s
    Bilibili 32-7                      segments=8     54.7 MB in   7.71 s =   7.09 MB/s
    Bilibili 32-7                      segments=16    54.7 MB in   7.49 s =   7.30 MB/s
    Kuaishou 720p-1                    segments=1     21.8 MB in   8.82 s =   2.48 MB/s
    Kuaishou 720p-1                    segments=2     21.8 MB in   7.75 s =   2.82 MB/s
    Kuaishou 720p-1                    segments=4     21.8 MB in   2.54 s =   8.61 MB/s
    Kuaishou 720p-1                    segments=8     21.8 MB in   3.24 s =   6.74 MB/s
    Kuaishou 720p-1                    segments=16    21.8 MB in   6.01 s =   3.63 MB/s
```

Findings: YouTube throttles each connection to ≈ 0.75 MB/s, so throughput scales almost linearly with
connections (16 → 12.4 MB/s). Bilibili's CDN saturates at 8 (≈ 7 MB/s). The 22 MB Kuaishou file was fastest with
4 connections and slower with 16. Changes made: default connections per file 8 → 16, minimum segment 2 → 4 MiB
(a 22 MB file now uses ≈ 5 connections, a 258 MB one 16); connection back-off for servers that refuse parallel
connections.

## 5. Analysis latency (native extractors)

| Link | Run 1 | Run 2 | Run 3 |
|---|---|---|---|
| Bilibili BV1efJk6aEdA | 1 884 ms | 585 ms | 500 ms |
| RedNote xhslink.com/o/… (private share link) | 2 631 ms | 1 754 ms | 1 171 ms |
| Kuaishou v.kuaishou.com/… (private share link) | 3 543 ms | 1 660 ms | 1 596 ms |
| Kuaishou short-video/3xjegz725scedme | 3 478 ms | 1 065 ms | 1 010 ms |

yt-dlp -J on the same Bilibili link: 4 286 ms to fail. YouTube analysis: ≈ 4.7 s without a JS runtime, ≈ 5.0 s
with one.