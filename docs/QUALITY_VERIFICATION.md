# Quality verification

How Rabbit makes sure the quality it offers is the quality it delivers, and what the highest-quality audit
found per platform (2026-10-23).

## 1. How options are labelled

* **Label = short side** of the stream (`720p` for 720×1280 and 1280×720), with the frame rate when above 31
  (`2160p60`). A stream without an exact size is labelled "Original".
* Sizes come only from the source or from the bytes: yt-dlp format data, native extractor data that states the
  rendition size (Bilibili DASH, Kuaishou manifest, RedNote stream list, X rendition path, Kwai JSON-LD), or a
  read of the file header (Bilibili progressive MP4 `tkhd`). Known mislabels are corrected at extraction:
  Pinterest renditions (`V_720P` reported the upload size 1080×1920), X GIFs (display size ≠ file size).
* Codecs are shown only when declared ("not reported" otherwise). No option is invented; MP3 is the only
  synthesized output and is marked "conversion".

## 2. How downloads are checked (FFprobe)

`engine/post.rs::verify`, run on every finished file before it is moved into place:

| Check | Fails when |
|---|---|
| Non-empty | 0 bytes |
| Video present | a video option produced no video stream (cover art doesn't count) |
| Audio present | the option declared audio (source said so) and the file has none |
| Duration | differs from the source by more than max(2 s, 2 %) |
| **Resolution** (new) | the video's short side is below 90 % of the short side the option promised (exact sizes only; tolerates 1080 vs 1088 padding) |

Merges are FFmpeg stream copies (`-map 0:v:0 -map 1:a:0 -c copy`); nothing is re-encoded except an explicit MP3.

## 3. Highest-quality audit

`RABBIT_PICK=best cargo test --test platform_matrix -- --ignored --nocapture` picks the highest-resolution option
of every video and records what FFprobe reads from the result (`research/quality-audit-*.txt`,
`research/platform-report.json`).

| Platform | Sample | Highest offered | File (FFprobe) | Codecs | Verdict |
|---|---|---|---|---|---|
| YouTube | Big Buck Bunny `aqz-KE-bpKQ` | 2160p60 | 3840×2160 | VP9 + Opus | **4K verified** (1.37 GB) |
| YouTube | Me at the zoo `jNQXAC9IVRw` (+ youtu.be) | 240p | 320×240 | H.264 + AAC | source maximum |
| Instagram | reel `DBBEGXpvwNF` | 1080p | 1080×1920 | VP9 + AAC | **1080p verified** |
| Facebook | reel `1355164985501660` | 1080p | 1080×1920 | VP9 + AAC | **1080p verified** |
| Kuaishou | `v.kuaishou.com/5fkjkb` | 1080p | 1080×1920 | HEVC + AAC | **1080p verified** |
| Kuaishou | a share link (v.kuaishou.com) | 720p60 | 720×1280 | H.264 + AAC | source maximum |
| Kwai | `@vokic313/video/5220…` | 720p | 720×1280 | H.264 + AAC | source maximum |
| Bilibili | 5 videos incl. your link, multipart, b23.tv | 480p | 852×480 | H.264 + AAC | **guest cap** (1080p needs an account) |
| Bilibili + your session (0.7.0) | your link `BV1efJk6aEdA`, non-premium account | 1080p | 1920×1080 | H.264 + AAC | **1080p verified** (`session_bilibili_hd`) |
| RedNote | a video share link (xhslink.com) | 720p | 720×1280 | H.264 + AAC | source maximum for this note |
| X | 4-video post, mixed post | 720p | 1280×720, 720×900 | H.264 + AAC | source maximum of these samples; 1080p **not tested** (no sample) |
| Pinterest | 3 video pins | 720p | 720×1280 | H.264 + AAC | source maximum (yt-dlp's "1080×1920" was the mislabel above) |
| Suno | 3 songs | song video | 824×1464, 512×768 | H.264 + AAC | source maximum (no audio-only file offered by Suno) |

Images (full resolution, FFprobe sizes of the saved files):

| Platform | Sample | Result |
|---|---|---|
| RedNote | `xhslink.cn/o/8FKGfNCXZpB` (6 photos + Live Photo) | 3193×5744, 2268×4032, 2304×4096 ×2, 2268×4032, 3213×5712 JPEG + 1080×1920 clip |
| RedNote | `xhslink.cn/o/198ItN4kuCe` (4 Live Photos) | 3072×4096 / 4096×3072 JPEG + 1080×1440 clips |
| X | `perrypumas/status/894001459754180609` (4 photos) | 2048×1536 ×2, 1536×2048 ×2 (`name=orig`) |
| X | `yua_mikami/status/1876940930139181424` | 4 × 2688×4032 |
| Instagram | `BoHk1haB5tM` (carousel) | 5 × 1080×1080 |
| Instagram | `BqvsDleB3lV`, `DEOGT-qyOwm` | 1080×1350, 1269×1269 |
| Pinterest | image pin `858146903966145189` | 399×600 (`orig`) |
| Pinterest | story pin `858146904010573850` | 225×400 ×2, 675×1200 (736-wide copies; `originals/` answers 403) |

## 4. What "1080p where available" means per platform

* **Available and verified:** YouTube (to 2160p60), Instagram, Facebook, Kuaishou.
* **Withheld by the source for signed-out visitors:** Bilibili (480p for guests; **1080p verified with your own
  session**, opt-in since 0.7.0), Suno (no audio file).
* **Not seen in public samples:** X and RedNote videos (the tested posts topped out at 720p; both extractors
  list every rendition the source offers, so a 1080p post would be offered).
* **Not testable from this network:** TikTok. **Not readable without signed requests:** Douyin.
