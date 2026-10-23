//! Corrections applied to yt-dlp's output for specific platforms, from observed failures.
//!
//! * Instagram: yt-dlp reads posts (single photos and carousels) through Instagram's logged-out
//!   web query but only turns videos into formats; a photo arrives as its `thumbnails` — the
//!   post's `image_versions2` candidates, smallest first, the last being the original upload (its
//!   URL carries no size cap). With `--ignore-no-formats-error`, those become photo formats.
//! * Pinterest: video renditions (`V_720P`, `V_EXP4`…) carry the original upload's size, not the
//!   file's — a "1080×1920" V_720P file was 720×1280 — so sizes are rescaled to the rendition.

use serde_json::{Value, json};

use crate::engine::platform::Platform;

/// Extra yt-dlp options for a platform.
pub fn ytdlp_args(p: Platform) -> &'static [&'static str] {
    match p {
        // Keep photo-only posts instead of failing on "no video".
        Platform::Instagram => &["--ignore-no-formats-error"],
        _ => &[],
    }
}

pub fn apply(p: Platform, v: &mut Value) {
    match p {
        Platform::Instagram => {
            for_each_item(v, add_photo);
            // yt-dlp titles every post "Video by <user>"; say what a photo post is.
            let photo_only = v.get("entries").is_none() && v["formats"].as_array().is_some_and(|f| f.iter().all(|f| f["format_id"] == "photo"));
            if photo_only && let Some(t) = v["title"].as_str().and_then(|t| t.strip_prefix("Video by ")).map(|u| format!("Photo by {u}")) {
                v["title"] = json!(t);
            }
        }
        Platform::Pinterest => for_each_item(v, fix_pinterest_sizes),
        _ => {}
    }
}

fn for_each_item(v: &mut Value, f: fn(&mut Value)) {
    if let Some(entries) = v.get_mut("entries").and_then(Value::as_array_mut) {
        entries.iter_mut().for_each(f);
    } else {
        f(v);
    }
}

fn add_photo(item: &mut Value) {
    if item.get("formats").and_then(Value::as_array).is_some_and(|f| !f.is_empty()) {
        return;
    }
    let Some(best) = item.get("thumbnails").and_then(Value::as_array).and_then(|t| t.last()) else { return };
    let Some(url) = best.get("url").and_then(Value::as_str).filter(|u| u.starts_with("https://")) else { return };
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    let ext = ["jpg", "jpeg", "webp", "heic", "png"].into_iter().find(|e| path.ends_with(&format!(".{e}"))).unwrap_or("jpg");
    item["formats"] = json!([{
        "format_id": "photo",
        "ext": ext,
        "width": best.get("width"),
        "height": best.get("height"),
        "protocol": "https",
        "url": url,
        "http_headers": {"Referer": "https://www.instagram.com/"},
    }]);
}

/// `V_720P` → 720 on the short side, keeping the reported aspect ratio. Other ids (HLS variants
/// measured from the playlist) are left alone.
fn fix_pinterest_sizes(item: &mut Value) {
    let Some(formats) = item.get_mut("formats").and_then(Value::as_array_mut) else { return };
    for f in formats {
        let Some(class) = f["format_id"].as_str().and_then(|id| id.strip_prefix("V_")).and_then(|r| r.strip_suffix('P')).and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let (Some(w), Some(h)) = (f["width"].as_u64(), f["height"].as_u64()) else { continue };
        let short = w.min(h) as f64;
        if short as u32 <= class {
            continue;
        }
        let scale = f64::from(class) / short;
        // Encoders round to even sizes.
        let even = |x: u64| ((x as f64 * scale / 2.0).round() * 2.0) as u64;
        f["width"] = json!(even(w));
        f["height"] = json!(even(h));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::model::MediaKind;
    use crate::extract::normalize::normalize;

    const ORIGINAL: &str = "https://instagram.fpnq2-1.fna.fbcdn.net/v/t51.75761-15/472105053_n.jpg?stp=dst-jpg_e35_tt6&_nc_cat=111";

    #[test]
    fn instagram_single_photo_post() {
        // Shape observed from yt-dlp 2026.10.21 for instagram.com/p/DEOGT-qyOwm (photo only).
        let mut v = json!({"title": "Video by khloekardashian", "formats": [], "thumbnails": [
            {"id": "0", "url": "https://instagram.fpnq2-1.fna.fbcdn.net/v/t51.75761-15/472105053_n.jpg?stp=dst-jpg_e35_s150x150_tt6"},
            {"id": "13", "url": ORIGINAL}
        ]});
        apply(Platform::Instagram, &mut v);
        assert_eq!(v["title"], "Photo by khloekardashian");
        let n = normalize(&v, true).unwrap();
        assert_eq!(n.kind, MediaKind::Image);
        let (o, _) = n.options.iter().find(|(o, _)| o.recommended).unwrap();
        assert_eq!((o.id.as_str(), o.container.as_str()), ("img-photo", "jpg"));
    }

    #[test]
    fn instagram_carousel_keeps_videos_and_adds_photos() {
        let mut v = json!({"_type": "playlist", "title": "Post by x", "entries": [
            {"formats": [{"format_id": "1", "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a.40.2", "width": 720, "height": 1280, "protocol": "https", "url": "https://cdn.example/v.mp4"}],
             "thumbnails": [{"url": ORIGINAL}]},
            {"formats": [], "thumbnails": [{"url": ORIGINAL}]},
            {"thumbnails": []}
        ]});
        apply(Platform::Instagram, &mut v);
        assert_eq!(v["entries"][0]["formats"][0]["format_id"], "1", "videos untouched");
        assert_eq!(v["entries"][1]["formats"][0]["url"], ORIGINAL);
        let n = normalize(&v, true).unwrap();
        assert_eq!(n.item_count, Some(2), "the empty item is skipped");
        assert_eq!(n.options[0].0.label, "All 2 items");
    }

    #[test]
    fn pinterest_rendition_sizes() {
        // Observed for pinterest.com/pin/2885187256207927: V_720P reported 1080×1920, file was 720×1280.
        let mut v = json!({"formats": [
            {"format_id": "V_720P", "width": 1080, "height": 1920, "url": "https://v1.pinimg.com/videos/mc/720p/x.mp4"},
            {"format_id": "V_HLSV4-1576", "width": 720, "height": 1280, "url": "https://v1.pinimg.com/x_720w.m3u8"},
            {"format_id": "V_EXP7", "width": 1080, "height": 1920}
        ]});
        apply(Platform::Pinterest, &mut v);
        assert_eq!((v["formats"][0]["width"].as_u64(), v["formats"][0]["height"].as_u64()), (Some(720), Some(1280)));
        assert_eq!(v["formats"][1]["height"], 1280);
        assert_eq!(v["formats"][2]["height"], 1920, "unknown rendition names are left alone");
    }
}
