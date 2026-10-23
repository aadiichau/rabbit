//! X / Twitter posts from the public embed data (`cdn.syndication.twimg.com/tweet-result`), the
//! source X's own embed widget (publish.twitter.com) reads. It lists every photo at its original
//! size and every progressive MP4 rendition of every video, so photo posts, mixed posts and
//! multi-video posts work without yt-dlp. Posts it doesn't describe (sensitive, deleted, text-only
//! or link cards) go to yt-dlp, which explains them.

use serde_json::{Value, json};
use url::Url;

use super::web::{self, DESKTOP_UA, Web};
use crate::error::{AppError, AppResult};

pub fn handles(url: &Url) -> bool {
    status_id(url).is_some()
}

/// `/<user>/status/<id>`, `/i/web/status/<id>`, `/i/status/<id>` on x.com / twitter.com.
fn status_id(url: &Url) -> Option<u64> {
    if !(web::host_is(url, "x.com") || web::host_is(url, "twitter.com")) {
        return None;
    }
    let segs: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let pos = segs.iter().position(|s| *s == "status" || *s == "statuses")?;
    segs.get(pos + 1)?.parse().ok()
}

/// The token the embed widget sends with each request (react-tweet's `getToken`):
/// `(id / 1e15 * π).toString(36)` without zeros and the point. (The endpoint only checks that a
/// token is present, but Rabbit sends what the widget sends.)
fn token(id: u64) -> String {
    let x = id as f64 / 1e15 * std::f64::consts::PI;
    let (int, frac) = js_radix36(x);
    format!("{int}{frac}").replace('0', "")
}

/// JavaScript's `Number.prototype.toString(36)` for a positive double, as V8 implements it
/// (`DoubleToRadixCString`): fraction digits until the value is uniquely identified, then rounding.
fn js_radix36(x: f64) -> (String, String) {
    let mut int = x.trunc();
    let mut fraction = x - int;
    let mut delta = (0.5 * (x.next_up() - x)).max(0.0f64.next_up());
    let mut digits: Vec<u32> = Vec::new();
    if fraction >= delta {
        loop {
            fraction *= 36.0;
            delta *= 36.0;
            let digit = fraction.trunc() as u32;
            digits.push(digit);
            fraction -= f64::from(digit);
            if (fraction > 0.5 || (fraction == 0.5 && digit & 1 == 1)) && fraction + delta > 1.0 {
                // Round up, carrying into earlier digits (and the integer part) as needed.
                loop {
                    match digits.pop() {
                        None => {
                            int += 1.0;
                            break;
                        }
                        Some(d) if d + 1 < 36 => {
                            digits.push(d + 1);
                            break;
                        }
                        Some(_) => {}
                    }
                }
                break;
            }
            if fraction < delta {
                break;
            }
        }
    }
    let frac: String = digits.iter().map(|d| std::char::from_digit(*d, 36).unwrap()).collect();
    (radix36(int as u64), frac)
}

fn radix36(mut n: u64) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut digits = Vec::new();
    while n > 0 {
        digits.push(std::char::from_digit((n % 36) as u32, 36).unwrap());
        n /= 36;
    }
    digits.iter().rev().collect()
}

pub async fn extract(w: &Web, url: &Url) -> AppResult<Option<Value>> {
    let Some(id) = status_id(url) else { return Ok(None) };
    let api = format!("https://cdn.syndication.twimg.com/tweet-result?id={id}&lang=en&token={}", token(id));
    let page = w.get(&api, DESKTOP_UA, &[]).await?;
    // 404: deleted, protected, or not served to embeds (e.g. age-restricted). yt-dlp says which.
    if page.status == 404 || !(200..300).contains(&page.status) {
        return Ok(None);
    }
    let t: Value = serde_json::from_str(&page.body).map_err(|_| AppError::Extractor("X's embed data has changed and Rabbit can't read it".into()))?;
    if t.get("tombstone").is_some() || t["__typename"].as_str().is_some_and(|k| k != "Tweet") {
        return Ok(None);
    }
    Ok(build(&t, id))
}

fn build(t: &Value, id: u64) -> Option<Value> {
    let media = t["mediaDetails"].as_array().filter(|m| !m.is_empty())?;
    let items: Vec<Value> = media.iter().filter_map(item).collect();
    if items.is_empty() {
        return None;
    }
    let user = t["user"]["screen_name"].as_str().unwrap_or("x");
    // Drop the trailing t.co link that points at the media itself.
    let text: String = t["text"].as_str().unwrap_or("").split_whitespace().filter(|w| !w.starts_with("https://t.co/")).collect::<Vec<_>>().join(" ");
    let title = if text.is_empty() { format!("Post by @{user}") } else { text.chars().take(120).collect() };
    let mut base = json!({
        "id": id.to_string(),
        "title": title,
        "description": t["text"],
        "uploader": t["user"]["name"],
        "uploader_id": user,
        "upload_date": t["created_at"].as_str().and_then(|d| d.get(..10)).map(|d| d.replace('-', "")),
        "thumbnail": media[0]["media_url_https"],
        "webpage_url": format!("https://x.com/{user}/status/{id}"),
        "extractor_key": "X (Rabbit embed data)",
    });
    if let [only] = items.as_slice() {
        for (k, v) in only.as_object().into_iter().flatten() {
            base[k] = v.clone();
        }
        return Some(base);
    }
    base["_type"] = json!("playlist");
    base["entries"] = Value::Array(items);
    Some(base)
}

/// One photo or video as a yt-dlp-style item with its formats.
fn item(m: &Value) -> Option<Value> {
    let headers = json!({"Referer": "https://x.com/"});
    match m["type"].as_str()? {
        "photo" => {
            let src = m["media_url_https"].as_str()?;
            let (base, ext) = src.rsplit_once('.')?;
            // `name=orig` serves the upload as stored (up to 4096 px), not a resized copy.
            Some(json!({"formats": [{
                "format_id": "orig",
                "ext": if ext == "jpeg" { "jpg" } else { ext },
                "width": m["original_info"]["width"],
                "height": m["original_info"]["height"],
                "protocol": "https",
                "url": format!("{base}?format={ext}&name=orig"),
                "http_headers": headers,
            }]}))
        }
        kind @ ("video" | "animated_gif") => {
            let gif = kind == "animated_gif";
            let formats: Vec<Value> = m["video_info"]["variants"]
                .as_array()?
                .iter()
                .filter(|v| v["content_type"] == "video/mp4")
                .filter_map(|v| {
                    let url = v["url"].as_str()?;
                    // Only the path tells a rendition's real size. `original_info` is the display
                    // size (a GIF post said 768 wide; its MP4 was 578p), so it isn't used here.
                    let (w, h) = dims_from_path(url).map_or((None, None), |(w, h)| (Some(w), Some(h)));
                    let bitrate = v["bitrate"].as_f64().unwrap_or(0.0);
                    // X serves MP4 renditions as H.264 + AAC; GIF posts are silent loops.
                    Some(json!({
                        "format_id": format!("mp4-{}", (bitrate / 1000.0).round()),
                        "ext": "mp4",
                        "vcodec": "avc1",
                        "acodec": if gif { "none" } else { "mp4a.40.2" },
                        "width": w,
                        "height": h,
                        "tbr": bitrate / 1000.0,
                        "protocol": "https",
                        "url": url,
                        "http_headers": headers,
                    }))
                })
                .collect();
            let duration = m["video_info"]["duration_millis"].as_f64().map(|ms| ms / 1000.0);
            (!formats.is_empty()).then(|| json!({"formats": formats, "duration": duration}))
        }
        _ => None,
    }
}

/// Renditions carry their size in the path: `/vid/1280x720/…mp4` (or `/vid/avc1/1280x720/`).
fn dims_from_path(url: &str) -> Option<(u32, u32)> {
    let path = url.split('?').next()?;
    path.split('/').find_map(|seg| {
        let (w, h) = seg.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::model::MediaKind;
    use crate::extract::normalize::normalize;

    #[test]
    fn recognises_post_addresses() {
        let id = |s: &str| status_id(&Url::parse(s).unwrap());
        assert_eq!(id("https://x.com/yua_mikami/status/1876940930139181424"), Some(1876940930139181424));
        assert_eq!(id("https://twitter.com/i/web/status/1170041925560258560"), Some(1170041925560258560));
        assert_eq!(id("https://mobile.twitter.com/a/status/12?s=20"), Some(12));
        assert_eq!(id("https://x.com/yua_mikami"), None);
        assert_eq!(id("https://x.com.example.org/a/status/1"), None);
    }

    #[test]
    fn token_matches_the_embed_widget() {
        // Values computed with react-tweet's getToken in Node 24.
        assert_eq!(token(894001459754180609), "26l6l8q7is");
        assert_eq!(token(1577924293023133696), "3tp7171caj");
        assert_eq!(token(1876940930139181424), "4jslnijskw");
    }

    #[test]
    fn photo_post_becomes_a_gallery_of_originals() {
        // Shape observed for twitter.com/perrypumas/status/894001459754180609 (4 photos).
        let photo = |n: &str, w: u32, h: u32| json!({"type": "photo", "media_url_https": format!("https://pbs.twimg.com/media/{n}.jpg"), "original_info": {"width": w, "height": h}});
        let t = json!({"__typename": "Tweet", "text": "Thank you https://t.co/abc", "created_at": "2017-08-06T01:05:32.000Z",
            "user": {"screen_name": "perrypumas", "name": "Perry"},
            "mediaDetails": [photo("A", 2048, 1536), photo("B", 2048, 1536), photo("C", 1536, 2048), photo("D", 1536, 2048)]});
        let v = build(&t, 894001459754180609).unwrap();
        assert_eq!(v["title"], "Thank you");
        assert_eq!(v["upload_date"], "20170806");
        assert_eq!(v["entries"][0]["formats"][0]["url"], "https://pbs.twimg.com/media/A?format=jpg&name=orig");
        let n = normalize(&v, true).unwrap();
        assert_eq!((n.kind, n.item_count), (MediaKind::Gallery, Some(4)));
        assert_eq!(n.options[0].0.label, "All 4 images");
    }

    #[test]
    fn single_video_offers_every_rendition() {
        // Shape observed for x.com/carrotsprout_/status/1577924293023133696 (video part).
        let v = |b: u32, w: u32, h: u32| json!({"content_type": "video/mp4", "bitrate": b, "url": format!("https://video.twimg.com/ext_tw_video/1/pu/vid/{w}x{h}/x.mp4?tag=12")});
        let t = json!({"__typename": "Tweet", "text": "", "user": {"screen_name": "a"},
            "mediaDetails": [{"type": "video", "original_info": {"width": 1152, "height": 1280}, "media_url_https": "https://pbs.twimg.com/t.jpg",
                "video_info": {"duration_millis": 2466, "variants": [{"content_type": "application/x-mpegURL", "url": "https://video.twimg.com/p.m3u8"}, v(632000, 320, 354), v(2176000, 720, 800)]}}]});
        let v = build(&t, 1).unwrap();
        assert_eq!(v["title"], "Post by @a");
        let n = normalize(&v, true).unwrap();
        assert_eq!(n.kind, MediaKind::Video);
        let labels: Vec<_> = n.options.iter().filter(|(o, _)| o.container == "mp4").map(|(o, _)| o.label.clone()).collect();
        assert_eq!(labels, ["720p", "320p"]);
        assert!(n.options.iter().any(|(o, _)| o.container == "mp3"), "audio is declared, so MP3 is offered");
    }

    #[test]
    fn text_only_posts_go_to_yt_dlp() {
        assert!(build(&json!({"__typename": "Tweet", "text": "hi", "mediaDetails": []}), 1).is_none());
    }
}
