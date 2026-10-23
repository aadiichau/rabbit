# Extractor architecture

How Rabbit turns a pasted link into a verified file, and why each platform uses the extractor it uses.
Status of each platform: [PLATFORM_COMPATIBILITY.md](PLATFORM_COMPATIBILITY.md). Failures and their root causes:
[EXTRACTION_FAILURES.md](EXTRACTION_FAILURES.md).

## 1. Pipeline

```
input text
 → ingest::extract_urls / validate          (share text → URL; http(s) only; length limits)
 → platform::detect                         (host table → Platform)
 → Native::for_url                          (does one of Rabbit's own extractors claim this address?)
     ├─ yes → native extractor              (public page / API a signed-out browser uses)
     │         ├─ Ok(Some(json)) → normalize (must yield downloadable media, else counts as failure)
     │         ├─ Ok(None)       → yt-dlp    (address isn't one it reads: e.g. a short link to a live room)
     │         ├─ Err(LoginRequired | Input) → shown as is (yt-dlp can't change the platform's answer)
     │         └─ Err(other)     → yt-dlp fallback where yt-dlp has an extractor; native error shown if both fail
     └─ no  → yt-dlp -J  (+ fixups::ytdlp_args)  → fixups::apply → normalize
 → options + plans                          (stream URLs and headers stay in Rust; never persisted)
 → download                                 (native segmented HTTP, or yt-dlp for HLS/DASH fragments)
 → FFmpeg stream-copy merge / MP3           (only when needed)
 → verify (FFprobe)                         (streams present, duration, promised resolution)
 → atomic rename → library
```

All extractors return **yt-dlp-shaped JSON** (`formats` with `url`, `protocol`, `http_headers`, `width`,
`height`, `vcodec`, `acodec`, `filesize`; galleries as `_type: playlist` + `entries`). One normalizer
(`extract/normalize.rs`) turns that into options, so format selection, labels, recommendations and verification
behave the same everywhere.

Re-analysis (resume after restart, expired links mid-download) re-runs the same chain on the stored
`webpage_url`; every native extractor writes a `webpage_url` it can read again (RedNote keeps the share
token, Bilibili keeps `?p=`).

## 2. Extractor per platform

| Platform | Primary | Fallback | Why this choice |
|---|---|---|---|
| YouTube | yt-dlp | — | Best maintained YouTube extractor anywhere; 2160p60 verified |
| Bilibili (video pages, short links) | `extract/bilibili.rs` — WBI-signed web API with anonymous visitor cookies | yt-dlp | yt-dlp's extractor broken upstream (issue #17894); this is the route of its pending fix PR #17898 |
| Bilibili (live, bangumi, bilibili.tv) | yt-dlp | — | Not covered by the native reader |
| Kuaishou | `extract/kuaishou.rs` — mobile share page `/fw/photo/<id>`, host rotation | — | yt-dlp has no Kuaishou extractor |
| Kwai | `extract/kuaishou.rs` — schema.org `VideoObject` on the public page | — | yt-dlp has no Kwai extractor |
| RedNote | `extract/rednote.rs` — mobile note page on www.rednote.com | yt-dlp | xiaohongshu.com resets connections on some networks; yt-dlp only reads xiaohongshu.com and only videos |
| X / Twitter | `extract/x.rs` — public embed data (`cdn.syndication.twimg.com/tweet-result`) | yt-dlp | yt-dlp reads videos only; the embed data lists original photos and every MP4 rendition |
| Pinterest (image / story pins) | `extract/pinterest.rs` — public `PinResource` | — (video pins → yt-dlp) | yt-dlp fails on image pins and returns only the cover of story pins |
| Pinterest (video pins) | yt-dlp + `fixups` (rendition sizes) | — | yt-dlp handles the HLS streams |
| Instagram | yt-dlp `--ignore-no-formats-error` + `fixups` (photos) | — | yt-dlp already reads photo posts and carousels but discards photos; Rabbit keeps them |
| Facebook | yt-dlp | — | Best available public extractor |
| Suno | `extract/suno.rs` — public song page (Next.js flight data) | — | yt-dlp lists suno.com as a "liability" (project decision, not an access restriction) |
| TikTok, Douyin, others | yt-dlp | — | See EXTRACTION_FAILURES for why these don't work from this network / without sign-in |

Shared helpers (`extract/web.rs`): a client that never follows redirects on its own (short links are resolved hop
by hop and stopped at the target, so an unreachable mainland host is never contacted), one retry on
connection-level errors, a brace-matching reader for `window.X = {…}` page state, a JS-object parser
(`undefined`, `new Set`), image sniffing, an MP4 `tkhd` size reader, and the base-36 conversion X's embed token
uses.

## 3. Rules every extractor follows

* Reads only what a signed-out browser receives. No accounts, no cookies imported from browsers, no tokens of
  Rabbit's own, no CAPTCHA solving, no DRM circumvention, no forged anti-bot signatures, no app-protocol
  impersonation. Where a platform withholds something (Bilibili above 480p, Suno's audio file, RedNote notes
  without a share token, Douyin's signed APIs), Rabbit says so instead of working around it.
* Never synthesizes media. Sizes and codecs are reported only when the source states them or a probe of the
  actual bytes shows them; otherwise the UI says "not reported" and verification doesn't demand them.
* Never hardcodes media URLs. Every URL is extracted fresh; temporary URLs are never written to disk.

## 4. Verification

`engine/post.rs::verify` (FFprobe) rejects a finished file when: it is empty; a video option has no video stream;
an option that declared audio has none; the duration differs from the source by more than max(2 s, 2 %); or
— since this release — **the video is clearly smaller than the option promised** (short side below 90 % of the
promised short side, exact sizes only). The last rule caught a real mislabel during testing (Pinterest
"V_720P" reported as 1080×1920). Details: [QUALITY_VERIFICATION.md](QUALITY_VERIFICATION.md).

## 5. Transfer engine changes in this release

* **Connection back-off** (`engine/http.rs::Gate`): segments share a gate whose width starts at the segment count
  and shrinks by one each time the server pushes back (503/429, or refusing/resetting a connection before any
  data). Waiting segments don't burn retry attempts. Tested against a local server that caps parallel
  connections (`tests/http_engine.rs::backs_off_when_the_server_caps_connections`).
* Progress resets a segment's retry budget, so a long transfer over a flaky link isn't abandoned after six
  unrelated drops.
