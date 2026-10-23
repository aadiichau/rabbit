use std::collections::BTreeMap;

use serde_json::Value;

use super::model::*;
use crate::error::{AppError, AppResult};

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "heic", "avif"];
const AUDIO_EXTS: &[&str] = &["m4a", "mp3", "opus", "ogg", "aac", "wav", "flac", "weba"];
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mov", "mkv", "flv", "3gp", "ts", "m4v"];

pub struct Normalized {
    pub kind: MediaKind,
    pub item_count: Option<usize>,
    pub options: Vec<(DownloadOption, Plan)>,
}

#[derive(Debug, Clone)]
struct Fmt {
    id: String,
    ext: String,
    vcodec: Option<String>,
    acodec: Option<String>,
    has_v: bool,
    has_a: bool,
    /// False when has_a is a guess (no codec info from the extractor).
    audio_known: bool,
    image: bool,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<f64>,
    tbr: f64,
    abr: f64,
    size: Option<u64>,
    size_approx: bool,
    protocol: String,
    url: Option<String>,
    headers: BTreeMap<String, String>,
    /// Backup URLs for the same file (Rabbit's extractors set `backup_urls`).
    mirrors: Vec<String>,
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string).filter(|x| !x.is_empty())
}
fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}
fn u(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64)))
}

fn id_is_drc(v: &Value) -> bool {
    v.get("format_id").and_then(Value::as_str).is_some_and(|id| id.ends_with("-drc"))
}

fn parse_fmt(v: &Value) -> Option<Fmt> {
    let id = s(v, "format_id")?;
    let protocol = s(v, "protocol").unwrap_or_else(|| "https".into());
    let note = s(v, "format_note").unwrap_or_default().to_ascii_lowercase();
    if protocol == "mhtml" || note.contains("storyboard") || v.get("has_drm").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let ext = s(v, "ext").unwrap_or_else(|| "bin".into()).to_ascii_lowercase();
    let codec = |k: &str| s(v, k).filter(|c| c != "none");
    let vcodec = codec("vcodec");
    let acodec = codec("acodec");
    let explicit_none = |k: &str| s(v, k).as_deref() == Some("none");

    // yt-dlp's generic extractor reports the page itself as "unknown_video" (with video_ext
    // "unknown_video") when it finds nothing. That's an HTML page, not media.
    if ext == "unknown_video" && vcodec.is_none() && acodec.is_none() {
        return None;
    }
    let height = u(v, "height").map(|h| h as u32).filter(|h| *h > 0);
    let image = IMAGE_EXTS.contains(&ext.as_str()) && acodec.is_none();
    // Codecs are authoritative. yt-dlp's video_ext/audio_ext only help when codecs are missing:
    // muxed formats carry audio_ext "none" even though they contain audio.
    let has_v = !image
        && (vcodec.is_some()
            || (!explicit_none("vcodec")
                && match v.get("video_ext").and_then(Value::as_str) {
                    Some(x) => x != "none",
                    None => height.is_some() || VIDEO_EXTS.contains(&ext.as_str()),
                }));
    let has_a = !image
        && (acodec.is_some()
            || (!explicit_none("acodec")
                && match v.get("audio_ext").and_then(Value::as_str) {
                    Some(x) if x != "none" => true,
                    _ => AUDIO_EXTS.contains(&ext.as_str()) || (vcodec.is_none() && !explicit_none("vcodec") && VIDEO_EXTS.contains(&ext.as_str())),
                }));

    let audio_known = acodec.is_some() || explicit_none("acodec") || v.get("audio_ext").and_then(Value::as_str).is_some_and(|x| x != "none");

    let (size, size_approx) = match (u(v, "filesize"), u(v, "filesize_approx")) {
        (Some(x), _) => (Some(x), false),
        (None, Some(x)) => (Some(x), true),
        _ => (None, false),
    };
    let headers = v
        .get("http_headers")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, val)| val.as_str().map(|x| (k.clone(), x.to_string()))).collect())
        .unwrap_or_default();

    Some(Fmt {
        id,
        ext,
        vcodec,
        acodec,
        has_v,
        has_a,
        audio_known,
        image,
        width: u(v, "width").map(|w| w as u32).filter(|w| *w > 0),
        height,
        fps: f(v, "fps").filter(|x| *x > 0.0),
        tbr: f(v, "tbr").unwrap_or(0.0),
        // Dynamic-range-compressed variants rank below the original mix at any bitrate.
        abr: f(v, "abr").or_else(|| f(v, "tbr")).unwrap_or(0.0) - if note.contains("drc") || id_is_drc(v) { 10_000.0 } else { 0.0 },
        size,
        size_approx,
        protocol,
        url: s(v, "url"),
        headers,
        mirrors: v
            .get("backup_urls")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).filter(|u| u.starts_with("https://") || u.starts_with("http://")).map(str::to_string).collect())
            .unwrap_or_default(),
    })
}

fn transfer(fm: &Fmt) -> Transfer {
    match (&fm.url, fm.protocol.as_str()) {
        (Some(url), "https" | "http") => Transfer::Native { url: url.clone(), headers: fm.headers.clone(), mirrors: fm.mirrors.clone() },
        _ => Transfer::YtDlp { format_spec: fm.id.clone() },
    }
}

fn is_fragmented(fm: &Fmt) -> bool {
    !matches!(fm.protocol.as_str(), "https" | "http") || fm.url.is_none()
}

pub fn short_codec(c: &str) -> &'static str {
    let c = c.to_ascii_lowercase();
    match () {
        _ if c.starts_with("avc") || c.starts_with("h264") => "H.264",
        _ if c.starts_with("hev") || c.starts_with("hvc") || c.starts_with("h265") => "HEVC",
        _ if c.starts_with("vp9") || c.starts_with("vp09") => "VP9",
        _ if c.starts_with("vp8") => "VP8",
        _ if c.starts_with("av01") || c == "av1" => "AV1",
        _ if c.starts_with("mp4a") || c == "aac" => "AAC",
        _ if c.starts_with("opus") => "Opus",
        _ if c.starts_with("mp3") => "MP3",
        _ if c.starts_with("vorbis") => "Vorbis",
        _ if c.starts_with("flac") => "FLAC",
        _ if c.starts_with("ac-3") || c.starts_with("ac3") => "AC-3",
        _ if c.starts_with("ec-3") || c.starts_with("eac3") => "E-AC-3",
        _ => "",
    }
}

fn codec_name(c: &Option<String>) -> String {
    match c {
        Some(c) => {
            let short = short_codec(c);
            if short.is_empty() { c.split('.').next().unwrap_or(c).to_string() } else { short.to_string() }
        }
        None => "not reported".into(),
    }
}

fn sum_size(parts: &[&Fmt]) -> (Option<u64>, bool) {
    let mut total = 0u64;
    let mut known = 0;
    let mut approx = false;
    for p in parts {
        if let Some(x) = p.size {
            total += x;
            known += 1;
            approx |= p.size_approx;
        }
    }
    match known {
        0 => (None, false),
        n if n == parts.len() => (Some(total), approx),
        _ => (Some(total), true),
    }
}

fn best_by<'a>(items: impl Iterator<Item = &'a Fmt>, key: impl Fn(&Fmt) -> f64) -> Option<&'a Fmt> {
    items.max_by(|a, b| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal))
}

fn merged_ext(video: &Fmt, audio: &Fmt) -> &'static str {
    match (video.ext.as_str(), audio.ext.as_str()) {
        ("mp4", "m4a" | "mp4") => "mp4",
        ("webm", "webm" | "weba" | "opus") => "webm",
        _ => "mkv",
    }
}

/// The resolution a quality label names: the shorter side, so a 720×1280 portrait video is "720p".
/// The quality tier a size is known by: the larger of its short side and the standard tier its long
/// side belongs to. Cinema-wide 1920×800 is "1080p" (as YouTube labels it), portrait 720×1280 is "720p",
/// 3:4 1080×1440 is "1080p".
fn quality_class(width: Option<u32>, height: Option<u32>) -> Option<u32> {
    let short = short_side(width, height)?;
    let (Some(w), Some(h)) = (width, height) else { return Some(short) };
    const TIERS: [(u32, u32); 8] = [(7680, 4320), (3840, 2160), (2560, 1440), (1920, 1080), (1280, 720), (854, 480), (640, 360), (426, 240)];
    // 1-2 % slack: encoders round (1916 wide is still the 1920 tier).
    let long_tier = TIERS.iter().find(|(long, _)| w.max(h) * 100 >= long * 98).map(|(_, p)| *p).unwrap_or(0);
    Some(short.max(long_tier))
}

fn short_side(width: Option<u32>, height: Option<u32>) -> Option<u32> {
    match (width, height) {
        (Some(w), Some(h)) => Some(w.min(h)),
        (_, h) => h,
    }
}

fn video_options(fmts: &[Fmt], duration: Option<f64>, has_ffmpeg: bool, out: &mut Vec<(DownloadOption, Plan)>) {
    let videos: Vec<&Fmt> = fmts.iter().filter(|x| x.has_v).collect();
    let audios: Vec<&Fmt> = fmts.iter().filter(|x| x.has_a && !x.has_v).collect();
    if videos.is_empty() {
        return;
    }
    let best_audio = |mp4: bool| -> Option<&Fmt> {
        let pref = best_by(audios.iter().copied().filter(|a| (a.ext == "m4a") == mp4), |a| a.abr);
        pref.or_else(|| best_by(audios.iter().copied(), |a| a.abr))
    };

    let mut heights: Vec<Option<u32>> = videos.iter().map(|v| v.height).collect();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();

    // Some sources (old Instagram posts, silent reels) carry no audio at all: their video-only
    // streams are the complete media.
    let silent_source = audios.is_empty() && !videos.iter().any(|v| v.has_a);

    let push = |v: &Fmt, out: &mut Vec<(DownloadOption, Plan)>| {
        let pair = if v.has_a { None } else { best_audio(v.ext == "mp4") };
        if !v.has_a && !silent_source && (pair.is_none() || !has_ffmpeg) {
            return; // video-only stream with no way to add sound
        }
        let expect_audio = match (pair, v.has_a, v.audio_known) {
            (Some(_), _, _) | (None, true, true) => Some(true),
            (None, true, false) => None,
            (None, false, _) => Some(false),
        };
        let id = match pair {
            Some(a) => format!("v-{}+{}", v.id, a.id),
            None => format!("v-{}", v.id),
        };
        if out.iter().any(|(o, _)| o.id == id) {
            return;
        }
        let parts: Vec<&Fmt> = std::iter::once(v).chain(pair).collect();
        let (size, est) = sum_size(&parts);
        let fragmented = parts.iter().any(|p| is_fragmented(p));
        let out_ext = match pair {
            Some(a) => merged_ext(v, a).to_string(),
            None => v.ext.clone(),
        };
        let codec = match (pair, expect_audio) {
            (Some(a), _) => format!("{} + {}", codec_name(&v.vcodec), codec_name(&a.acodec)),
            (None, Some(false)) => codec_name(&v.vcodec),
            (None, _) if v.vcodec.is_none() && v.acodec.is_none() => "not reported".to_string(),
            (None, _) => format!("{} + {}", codec_name(&v.vcodec), codec_name(&v.acodec)),
        };
        let fps_suffix = v.fps.filter(|f| *f > 31.0).map(|f| format!("{}", f.round() as u32)).unwrap_or_default();
        let label = match quality_class(v.width, v.height) {
            Some(h) => format!("{h}p{fps_suffix}"),
            None => "Original".to_string(),
        };
        let mut note = match (pair, expect_audio) {
            (Some(a), _) => format!("merges with {} audio", codec_name(&a.acodec)),
            (None, Some(false)) => "no audio track in the source".to_string(),
            (None, None) => "single file · audio not reported by the source".to_string(),
            (None, Some(true)) => "single file".to_string(),
        };
        if fragmented {
            note.push_str(" · segmented stream");
        }
        let plan = if fragmented {
            let spec = parts.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join("+");
            Plan {
                streams: vec![StreamPlan { transfer: Transfer::YtDlp { format_spec: spec }, ext: out_ext.clone(), size, item_name: None }],
                post: Post::None,
                out_ext: out_ext.clone(),
                expected_duration: duration,
                gallery: false,
                expect_audio,
                // Only exact sizes are enforced; a height-only quality class (e.g. Bilibili qn) isn't.
                min_short_side: v.width.and(short_side(v.width, v.height)),
            }
        } else {
            Plan {
                streams: parts.iter().map(|p| StreamPlan { transfer: transfer(p), ext: p.ext.clone(), size: p.size, item_name: None }).collect(),
                post: if pair.is_some() { Post::Merge } else { Post::None },
                out_ext: out_ext.clone(),
                expected_duration: duration,
                gallery: false,
                expect_audio,
                // Only exact sizes are enforced; a height-only quality class (e.g. Bilibili qn) isn't.
                min_short_side: v.width.and(short_side(v.width, v.height)),
            }
        };
        out.push((
            DownloadOption {
                id,
                group: OptionGroup::Video,
                label,
                container: out_ext,
                codec,
                fps: v.fps,
                width: v.width,
                height: v.height,
                size,
                size_estimated: est,
                note,
                conversion: false,
                recommended: false,
            },
            plan,
        ));
    };

    for h in heights {
        let at: Vec<&Fmt> = videos.iter().copied().filter(|v| v.height == h).collect();
        let compat = best_by(
            at.iter().copied().filter(|v| v.ext == "mp4" && v.vcodec.as_deref().is_some_and(|c| short_codec(c) == "H.264")),
            // Direct HTTPS beats HLS/DASH fragments: Rabbit can segment and resume it natively.
            |v| v.tbr + if v.has_a { 0.5 } else { 0.0 } - if is_fragmented(v) { 1e6 } else { 0.0 },
        );
        // Prefer non-fragmented, then bitrate.
        let best = best_by(at.iter().copied(), |v| v.tbr - if is_fragmented(v) { 1e6 } else { 0.0 });
        for v in compat.into_iter().chain(best) {
            push(v, out);
        }
    }

    // Recommend the best H.264/MP4 option up to 1080p, else the first option.
    let first_video = out.iter().position(|(o, _)| o.group == OptionGroup::Video);
    let rec = out
        .iter()
        .position(|(o, _)| o.group == OptionGroup::Video && o.container == "mp4" && o.codec.starts_with("H.264") && quality_class(o.width, o.height).is_none_or(|h| h <= 1080))
        .or(first_video);
    if let Some(i) = rec {
        out[i].0.recommended = true;
    }
}

fn audio_options(fmts: &[Fmt], duration: Option<f64>, has_ffmpeg: bool, video_present: bool, out: &mut Vec<(DownloadOption, Plan)>) {
    let audios: Vec<&Fmt> = fmts.iter().filter(|x| x.has_a && !x.has_v).collect();
    let add = |a: &Fmt, post: Post, out_ext: &str, label: String, note: String, conversion: bool, out: &mut Vec<(DownloadOption, Plan)>| {
        let id = format!("a-{}-{}", out_ext, a.id);
        if out.iter().any(|(o, _)| o.id == id) {
            return;
        }
        let kbps = if a.abr > 0.0 { format!(" · {} kbps", a.abr.round() as u32) } else { String::new() };
        let plan = Plan {
            streams: vec![StreamPlan {
                transfer: if is_fragmented(a) { Transfer::YtDlp { format_spec: a.id.clone() } } else { transfer(a) },
                ext: a.ext.clone(),
                size: a.size,
                item_name: None,
            }],
            post,
            out_ext: out_ext.to_string(),
            expected_duration: duration,
            gallery: false,
            expect_audio: None,
                min_short_side: None,
        };
        out.push((
            DownloadOption {
                id,
                group: OptionGroup::Audio,
                label,
                container: out_ext.to_string(),
                codec: format!("{}{}", if conversion { "MP3".into() } else { codec_name(&a.acodec) }, kbps),
                fps: None,
                width: None,
                height: None,
                size: if conversion { None } else { a.size },
                size_estimated: a.size_approx,
                note,
                conversion,
                recommended: false,
            },
            plan,
        ));
    };

    let best_m4a = best_by(audios.iter().copied().filter(|a| a.ext == "m4a"), |a| a.abr);
    let best_other = best_by(audios.iter().copied().filter(|a| a.ext != "m4a"), |a| a.abr);
    let best_any = best_by(audios.iter().copied(), |a| a.abr);
    for a in best_m4a.into_iter().chain(best_other) {
        add(a, Post::None, &a.ext.clone(), a.ext.to_uppercase(), "original stream, no conversion".into(), false, out);
    }
    if has_ffmpeg {
        if let Some(a) = best_any {
            add(a, Post::ToMp3, "mp3", "MP3".into(), "conversion (FFmpeg)".into(), true, out);
        } else if video_present {
            // No separate audio stream: extract from the best single-file video.
            // Only when the extractor confirms the video carries audio — never on a guess.
            if let Some(v) = best_by(fmts.iter().filter(|x| x.has_v && x.has_a && x.audio_known), |x| x.tbr) {
                add(v, Post::ToMp3, "mp3", "MP3".into(), "extracted from video (FFmpeg)".into(), true, out);
            }
        }
    }
    if let Some(first) = out.iter_mut().find(|(o, _)| o.group == OptionGroup::Audio) {
        if !video_present {
            first.0.recommended = true;
        }
    }
}

fn image_ext_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or("");
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if IMAGE_EXTS.contains(&ext.as_str()) { ext } else { "jpg".into() }
}

fn image_options(v: &Value, fmts: &[Fmt], kind: MediaKind, out: &mut Vec<(DownloadOption, Plan)>) {
    if let Some(img) = best_by(fmts.iter().filter(|x| x.image), |x| f64::from(x.width.unwrap_or(0)) * f64::from(x.height.unwrap_or(0))) {
        out.push((
            DownloadOption {
                id: format!("img-{}", img.id),
                group: OptionGroup::Image,
                label: "Original image".into(),
                container: img.ext.clone(),
                codec: String::new(),
                fps: None,
                width: img.width,
                height: img.height,
                size: img.size,
                size_estimated: img.size_approx,
                note: String::new(),
                conversion: false,
                recommended: kind == MediaKind::Image,
            },
            Plan {
                streams: vec![StreamPlan { transfer: transfer(img), ext: img.ext.clone(), size: img.size, item_name: None }],
                post: Post::None,
                out_ext: img.ext.clone(),
                expected_duration: None,
                gallery: false,
                expect_audio: None,
                min_short_side: None,
            },
        ));
    }
    if kind != MediaKind::Image {
        if let Some(thumb) = s(v, "thumbnail").filter(|t| t.starts_with("https://") || t.starts_with("http://")) {
            let ext = image_ext_from_url(&thumb);
            out.push((
                DownloadOption {
                    id: "cover".into(),
                    group: OptionGroup::Image,
                    label: "Cover image".into(),
                    container: ext.clone(),
                    codec: String::new(),
                    fps: None,
                    width: None,
                    height: None,
                    size: None,
                    size_estimated: false,
                    note: "as published by the platform".into(),
                    conversion: false,
                    recommended: false,
                },
                Plan {
                    streams: vec![StreamPlan { transfer: Transfer::Native { url: thumb, headers: BTreeMap::new(), mirrors: Vec::new() }, ext: ext.clone(), size: None, item_name: None }],
                    post: Post::None,
                    out_ext: ext,
                    expected_duration: None,
                    gallery: false,
                    expect_audio: None,
                min_short_side: None,
                },
            ));
        }
    }
}

fn subtitle_options(v: &Value, out: &mut Vec<(DownloadOption, Plan)>) {
    let Some(subs) = v.get("subtitles").and_then(Value::as_object) else { return };
    for (lang, list) in subs {
        if lang == "live_chat" {
            continue;
        }
        let Some(list) = list.as_array() else { continue };
        let pick = ["srt", "vtt"].iter().find_map(|want| list.iter().find(|x| s(x, "ext").as_deref() == Some(want)));
        let Some(sub) = pick else { continue };
        let (Some(url), Some(ext)) = (s(sub, "url"), s(sub, "ext")) else { continue };
        let name = s(sub, "name").unwrap_or_else(|| lang.clone());
        out.push((
            DownloadOption {
                id: format!("sub-{lang}"),
                group: OptionGroup::Subtitle,
                label: name,
                container: ext.clone(),
                codec: lang.clone(),
                fps: None,
                width: None,
                height: None,
                size: None,
                size_estimated: false,
                note: "uploaded subtitles".into(),
                conversion: false,
                recommended: false,
            },
            Plan {
                streams: vec![StreamPlan { transfer: Transfer::Native { url, headers: BTreeMap::new(), mirrors: Vec::new() }, ext: ext.clone(), size: None, item_name: None }],
                post: Post::None,
                out_ext: ext,
                expected_duration: None,
                gallery: false,
                expect_audio: None,
                min_short_side: None,
            },
        ));
    }
}

fn formats_of(v: &Value) -> Vec<Fmt> {
    let mut fmts: Vec<Fmt> = v.get("formats").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_fmt).collect()).unwrap_or_default();
    if fmts.is_empty() {
        // Single-format results put the format fields on the root object.
        if v.get("url").is_some() {
            let mut root = v.clone();
            if root.get("format_id").is_none() {
                root["format_id"] = Value::String("0".into());
            }
            fmts.extend(parse_fmt(&root));
        }
    }
    fmts
}

fn gallery(v: &Value, entries: &[Value]) -> AppResult<Normalized> {
    let mut streams = Vec::new();
    let mut images = 0;
    let mut videos = 0;
    let mut total = 0u64;
    let mut est = false;
    // An item of unknown size makes any total a guess too low to be useful, so none is shown.
    let mut unknown = false;
    for (i, e) in entries.iter().enumerate() {
        let fmts = formats_of(e);
        let size_rank = |x: &Fmt| f64::from(x.height.unwrap_or(0)) * 1e6 + x.tbr;
        // Items aren't merged, so split video + audio streams can't be used. A video-only file is
        // the whole item when the item has no audio at all (silent Instagram carousel videos) or
        // when the extractor didn't report its audio.
        let silent = !fmts.iter().any(|x| x.has_a);
        let pick = best_by(fmts.iter().filter(|x| x.has_v && x.has_a && !is_fragmented(x)), size_rank)
            .or_else(|| best_by(fmts.iter().filter(|x| x.has_v && (silent || !x.audio_known) && !is_fragmented(x)), size_rank))
            .or_else(|| best_by(fmts.iter().filter(|x| x.image && !is_fragmented(x)), |x| f64::from(x.width.unwrap_or(0)) * f64::from(x.height.unwrap_or(0))));
        let Some(p) = pick else { continue };
        if p.image { images += 1 } else { videos += 1 }
        match p.size {
            Some(sz) => {
                total += sz;
                est |= p.size_approx;
            }
            None => unknown = true,
        }
        streams.push(StreamPlan { transfer: transfer(p), ext: p.ext.clone(), size: p.size, item_name: Some(format!("{:02}.{}", i + 1, p.ext)) });
    }
    if streams.is_empty() {
        return Err(AppError::Unsupported("none of the items in this post have a directly downloadable file".into()));
    }
    let n = streams.len();
    let label = match (images, videos) {
        (i, 0) => format!("All {i} images"),
        (0, v) => format!("All {v} videos"),
        _ => format!("All {n} items"),
    };
    let option = DownloadOption {
        id: "gallery-all".into(),
        group: if videos > images { OptionGroup::Video } else { OptionGroup::Image },
        label,
        container: "folder".into(),
        codec: String::new(),
        fps: None,
        width: None,
        height: None,
        size: (total > 0 && !unknown).then_some(total),
        size_estimated: est,
        note: format!("saved into a folder named after the post · {images} image(s), {videos} video(s)"),
        conversion: false,
        recommended: true,
    };
    let _ = v;
    Ok(Normalized {
        kind: MediaKind::Gallery,
        item_count: Some(n),
        options: vec![(option, Plan { streams, post: Post::None, out_ext: String::new(), expected_duration: None, gallery: true, expect_audio: None, min_short_side: None })],
    })
}

pub fn normalize(v: &Value, has_ffmpeg: bool) -> AppResult<Normalized> {
    if s(v, "_type").as_deref() == Some("playlist") {
        let entries = v.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
        if entries.is_empty() {
            return Err(AppError::Unsupported("the link points to an empty collection".into()));
        }
        return gallery(v, &entries);
    }
    let fmts = formats_of(v);
    let duration = f(v, "duration");
    let has_video = fmts.iter().any(|x| x.has_v);
    let has_audio = fmts.iter().any(|x| x.has_a);
    let kind = if has_video {
        MediaKind::Video
    } else if has_audio {
        MediaKind::Audio
    } else if fmts.iter().any(|x| x.image) {
        MediaKind::Image
    } else {
        return Err(AppError::Unsupported("no downloadable media was found on this page".into()));
    };

    let mut options = Vec::new();
    video_options(&fmts, duration, has_ffmpeg, &mut options);
    // An extractor can rule out audio conversions (Suno withholds song audio from signed-out visitors).
    let convert_audio = has_ffmpeg && v.get("_rabbit_no_audio_conversion").and_then(Value::as_bool) != Some(true);
    audio_options(&fmts, duration, convert_audio, has_video, &mut options);
    image_options(v, &fmts, kind, &mut options);
    subtitle_options(v, &mut options);
    if options.is_empty() {
        let why = if has_ffmpeg { "no downloadable media was found on this page" } else { "formats exist but none can be saved without FFmpeg" };
        return Err(AppError::Unsupported(why.into()));
    }
    Ok(Normalized { kind, item_count: None, options })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn youtube_me_at_the_zoo() {
        let v = fixture("youtube-jNQXAC9IVRw.json");
        let n = normalize(&v, true).unwrap();
        assert_eq!(n.kind, MediaKind::Video);
        let video: Vec<_> = n.options.iter().filter(|(o, _)| o.group == OptionGroup::Video).collect();
        assert!(!video.is_empty());
        // Every option must come from a real format id present in the fixture.
        let ids: Vec<String> = v["formats"].as_array().unwrap().iter().map(|f| f["format_id"].as_str().unwrap().to_string()).collect();
        for (o, _) in &n.options {
            if let Some(rest) = o.id.strip_prefix("v-") {
                for part in rest.split('+') {
                    assert!(ids.contains(&part.to_string()), "unknown format {part}");
                }
            }
        }
        assert_eq!(n.options.iter().filter(|(o, _)| o.recommended && o.group == OptionGroup::Video).count(), 1);
        assert!(n.options.iter().any(|(o, p)| o.container == "mp3" && p.post == Post::ToMp3 && o.conversion));
        assert!(n.options.iter().any(|(o, _)| o.id == "cover"));
        // Heights are listed highest first.
        let hs: Vec<u32> = video.iter().filter_map(|(o, _)| o.height).collect();
        assert!(hs.windows(2).all(|w| w[0] >= w[1]));
        // Format 18 is muxed H.264 + AAC: one file, never re-merged with a second audio track.
        let f18 = n.options.iter().find(|(o, _)| o.id == "v-18").expect("muxed format 18 offered as a single file");
        assert_eq!(f18.0.note, "single file");
        assert_eq!(f18.1.post, Post::None);
        assert!(n.options.iter().all(|(o, _)| !o.id.starts_with("v-18+")));
        // The DRC audio variant must not be chosen over the original mix.
        assert!(n.options.iter().all(|(o, _)| !o.id.ends_with("-drc")), "{:?}", n.options.iter().map(|(o, _)| &o.id).collect::<Vec<_>>());
    }

    #[test]
    fn no_ffmpeg_hides_merges_and_conversions() {
        let v = fixture("youtube-jNQXAC9IVRw.json");
        let n = normalize(&v, false).unwrap();
        assert!(n.options.iter().all(|(o, p)| !o.conversion && p.post != Post::Merge));
    }

    #[test]
    fn video_only_and_audio_only_detection() {
        let v = serde_json::json!({
            "title": "t", "duration": 10.0,
            "formats": [
                {"format_id": "a1", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "abr": 128.0, "protocol": "https", "url": "https://cdn.example/a", "filesize": 100},
                {"format_id": "v1", "ext": "mp4", "vcodec": "avc1.640028", "acodec": "none", "height": 1080, "width": 1920, "tbr": 3000.0, "protocol": "https", "url": "https://cdn.example/v", "filesize": 1000},
                {"format_id": "h1", "ext": "mp4", "vcodec": "avc1.4d401f", "acodec": "mp4a.40.2", "height": 720, "tbr": 1500.0, "protocol": "m3u8_native", "url": "https://cdn.example/h.m3u8"}
            ]
        });
        let n = normalize(&v, true).unwrap();
        let o = n.options.iter().find(|(o, _)| o.id == "v-v1+a1").expect("merged 1080p");
        assert_eq!(o.0.container, "mp4");
        assert_eq!(o.0.size, Some(1100));
        assert_eq!(o.1.post, Post::Merge);
        assert!(o.0.recommended);
        let h = n.options.iter().find(|(o, _)| o.id == "v-h1").expect("hls 720p");
        assert!(matches!(h.1.streams[0].transfer, Transfer::YtDlp { .. }));
        assert!(h.0.note.contains("segmented"));
    }

    #[test]
    fn silent_source_offers_video_only_formats() {
        // Field shapes observed from yt-dlp for instagram.com/p/aye83DjauH (2013, no audio track).
        let v = serde_json::json!({
            "title": "Video by naomipq",
            "formats": [
                {"format_id": "0", "ext": "mp4", "acodec": "none", "video_ext": "mp4", "audio_ext": "none", "protocol": "https", "url": "https://cdn.example/0.mp4"},
                {"format_id": "dash-273381622250014v", "ext": "mp4", "vcodec": "avc1.42c01f", "acodec": "none", "video_ext": "mp4", "audio_ext": "none", "width": 640, "height": 640, "protocol": "https", "url": "https://cdn.example/d.mp4"}
            ]
        });
        let n = normalize(&v, true).unwrap();
        let videos: Vec<_> = n.options.iter().filter(|(o, _)| o.group == OptionGroup::Video).collect();
        assert!(!videos.is_empty(), "silent videos must still be downloadable");
        assert!(videos.iter().all(|(o, p)| o.note.contains("no audio") && p.expect_audio == Some(false)));
        assert!(n.options.iter().all(|(o, _)| o.container != "mp3"), "no MP3 from a silent source");
    }

    #[test]
    fn unknown_audio_is_not_assumed() {
        // Field shapes observed for facebook.com/reel/1195289147628387: progressive sd/hd without
        // codec info, DASH video-only, and no audio streams.
        let v = serde_json::json!({
            "title": "reel", "duration": 9.579,
            "formats": [
                {"format_id": "sd", "ext": "mp4", "video_ext": "mp4", "audio_ext": "none", "protocol": "https", "url": "https://cdn.example/sd.mp4"},
                {"format_id": "hd", "ext": "mp4", "video_ext": "mp4", "audio_ext": "none", "protocol": "https", "url": "https://cdn.example/hd.mp4"},
                {"format_id": "1417507216490308v", "ext": "mp4", "vcodec": "vp09.00.21.08", "acodec": "none", "width": 360, "height": 636, "protocol": "https", "url": "https://cdn.example/v.mp4"}
            ]
        });
        let n = normalize(&v, true).unwrap();
        let hd = n.options.iter().find(|(o, _)| o.id == "v-hd").expect("progressive hd offered");
        assert_eq!(hd.1.expect_audio, None, "verification must not demand audio the source never declared");
        assert!(hd.0.note.contains("audio not reported"));
        assert!(n.options.iter().all(|(o, _)| o.container != "mp3"), "no MP3 conversion from a guessed audio track");
    }

    #[test]
    fn page_without_media_has_a_clear_error() {
        // Shape observed from yt-dlp's generic extractor for b23.tv/a9P3uZO (the page itself).
        let v = serde_json::json!({"title": "a9P3uZO", "extractor_key": "Generic", "url": "https://b23.tv/a9P3uZO", "ext": "unknown_video", "video_ext": "unknown_video", "audio_ext": "none", "protocol": "https"});
        let Err(e) = normalize(&v, true) else { panic!("a page with no media must not produce options") };
        assert!(e.to_string().contains("no downloadable media"), "{e}");
    }

    #[test]
    fn quality_classes() {
        let q = |w, h| quality_class(Some(w), Some(h));
        assert_eq!(q(1920, 800), Some(1080), "cinema-wide film");
        assert_eq!(q(1916, 798), Some(1080));
        assert_eq!(q(720, 1280), Some(720));
        assert_eq!(q(1080, 1440), Some(1080));
        assert_eq!(q(852, 480), Some(480));
        assert_eq!(q(1080, 608), Some(608));
        assert_eq!(q(3840, 1600), Some(2160));
        assert_eq!(quality_class(None, Some(720)), Some(720));
    }

    #[test]
    fn portrait_videos_are_labelled_by_short_side() {
        let v = serde_json::json!({"title": "t", "formats": [
            {"format_id": "p", "ext": "mp4", "vcodec": "avc1.64001f", "acodec": "mp4a.40.2", "width": 1080, "height": 1920, "protocol": "https", "url": "https://cdn.example/p.mp4"}
        ]});
        let n = normalize(&v, true).unwrap();
        let (o, _) = n.options.iter().find(|(o, _)| o.id == "v-p").unwrap();
        assert_eq!(o.label, "1080p");
        assert!(o.recommended, "1080×1920 counts as 1080p for the recommendation");
    }

    #[test]
    fn gallery_of_images() {
        let v = serde_json::json!({
            "_type": "playlist", "title": "post",
            "entries": [
                {"formats": [{"format_id": "0", "ext": "jpg", "width": 1080, "height": 1350, "url": "https://img.example/1.jpg", "protocol": "https"}]},
                {"formats": [{"format_id": "0", "ext": "jpg", "width": 1080, "height": 1350, "url": "https://img.example/2.jpg", "protocol": "https"}]}
            ]
        });
        let n = normalize(&v, true).unwrap();
        assert_eq!(n.kind, MediaKind::Gallery);
        assert_eq!(n.item_count, Some(2));
        assert_eq!(n.options[0].0.label, "All 2 images");
        assert_eq!(n.options[0].1.streams[1].item_name.as_deref(), Some("02.jpg"));
    }
}
