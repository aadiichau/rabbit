//! Kuaishou videos from the public mobile share page (`/fw/photo/<id>`), which is where
//! v.kuaishou.com share links and www.kuaishou.com video links lead a phone browser.
//! The page embeds the video's public CDN links; Rabbit never signs in.

use serde_json::{Value, json};
use url::Url;

use super::web::{self, MOBILE_UA, Web};
use crate::error::{AppError, AppResult};

/// Hosts that serve Kuaishou's share pages (gifshow and chenzhongtech are Kuaishou's own domains).
const HOSTS: &[&str] = &["kuaishou.com", "gifshow.com", "chenzhongtech.com"];
/// Hosts observed serving the same `/fw/photo/<id>` share page (research/tools/kuaishou-routes.mjs).
const SHARE_HOSTS: &[&str] = &["v.m.chenzhongtech.com", "m.gifshow.com", "v.kuaishou.com"];

pub fn handles(url: &Url) -> bool {
    HOSTS.iter().any(|h| web::host_is(url, h)) || web::host_is(url, "kwai.com")
}

fn photo_id(url: &Url) -> Option<String> {
    let segs: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let id = match segs.as_slice() {
        ["fw", "photo" | "long-video", id, ..] | ["short-video", id, ..] => *id,
        _ => return None,
    };
    (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')).then(|| id.to_string())
}

pub async fn extract(w: &Web, input: &Url) -> AppResult<Value> {
    if web::host_is(input, "kwai.com") {
        return kwai(w, input).await;
    }
    // A desktop video link already names the post; share links redirect a phone browser to the share page.
    let not_video = || AppError::Unsupported("this Kuaishou link doesn't lead to a video (profiles, live streams and searches can't be downloaded)".into());
    let is_share_link = web::host_is(input, "v.kuaishou.com") || input.path().starts_with("/f/");
    let resolved = match photo_id(input) {
        Some(_) => input.clone(),
        None if is_share_link => w.resolve(input, MOBILE_UA, |u| photo_id(u).is_some()).await?,
        None => return Err(not_video()),
    };
    let id = photo_id(&resolved).ok_or_else(not_video)?;
    // Desktop addresses (`/short-video/<id>`) go straight to the share page instead of through
    // www.kuaishou.com, which often times out from outside mainland China.
    let page_url = if resolved.path().starts_with("/fw/") {
        resolved
    } else {
        let mut u = Url::parse(&format!("https://{}/fw/photo/{id}", SHARE_HOSTS[0])).expect("valid share url");
        u.set_query(resolved.query());
        u
    };
    let page = share_page(w, &page_url).await?;
    if !(200..300).contains(&page.status) {
        return Err(AppError::Http(page.status));
    }
    let state: Value = web::object_after(&page.body, "INIT_STATE")
        .and_then(|s| serde_json::from_str(s).ok())
        .ok_or_else(|| AppError::Extractor("Kuaishou's share page has changed and Rabbit can't read it".into()))?;
    let Some(photo) = find_photo(&state) else {
        if page.body.to_ascii_lowercase().contains("captcha") {
            return Err(AppError::Extractor("Kuaishou asked for a verification check, which Rabbit doesn't do. Try again later".into()));
        }
        return Err(AppError::Extractor("Kuaishou's share page doesn't include this video. It may be private, deleted, or only viewable in the app".into()));
    };
    build(photo, &id)
}

/// Fetches the share page, trying Kuaishou's other share hosts (same path and query) when one
/// can't be reached: individual hosts time out from some networks.
async fn share_page(w: &Web, url: &Url) -> AppResult<web::Page> {
    let first = url.host_str().unwrap_or_default().to_string();
    let hosts = std::iter::once(first.as_str()).chain(SHARE_HOSTS.iter().copied().filter(|h| *h != first));
    let mut last = None;
    for host in hosts {
        let mut u = url.clone();
        if u.set_host(Some(host)).is_err() {
            continue;
        }
        match w.get(u.as_str(), MOBILE_UA, &[]).await {
            Err(e @ AppError::Network(_)) => {
                tracing::info!(host, "Kuaishou share host unreachable, trying the next one: {e}");
                last = Some(e);
            }
            other => return other,
        }
    }
    Err(last.unwrap_or_else(|| AppError::Network("no Kuaishou share host could be reached".into())))
}

/// Kwai (Kuaishou's international app): its public video page describes the video as a schema.org
/// `VideoObject` (JSON-LD for search engines), including the MP4 `contentUrl`, size and dates.
async fn kwai(w: &Web, url: &Url) -> AppResult<Value> {
    let segs: Vec<&str> = url.path_segments().map(|s| s.filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let Some(id) = segs.iter().position(|s| *s == "video").and_then(|i| segs.get(i + 1)).filter(|id| id.chars().all(|c| c.is_ascii_digit())) else {
        return Err(AppError::Unsupported("this Kwai link doesn't lead to a video (profiles and searches can't be downloaded)".into()));
    };
    let page = w.get(url.as_str(), MOBILE_UA, &[]).await?;
    if !(200..300).contains(&page.status) {
        return Err(AppError::Http(page.status));
    }
    let video = ld_json_video(&page.body).ok_or_else(|| AppError::Extractor("Kwai's page doesn't describe this video (it may be private or removed)".into()))?;
    kwai_build(&video, id)
}

fn ld_json_video(html: &str) -> Option<Value> {
    fn find(v: &Value) -> Option<&Value> {
        match v {
            Value::Object(m) if m.get("@type").and_then(Value::as_str) == Some("VideoObject") => Some(v),
            Value::Object(m) => m.values().find_map(find),
            Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    let mut rest = html;
    while let Some(at) = rest.find("application/ld+json") {
        rest = &rest[at..];
        let start = rest.find('>')? + 1;
        let end = rest.find("</script>")?;
        if let Ok(v) = serde_json::from_str::<Value>(&rest[start..end])
            && let Some(video) = find(&v)
        {
            return Some(video.clone());
        }
        rest = &rest[end..];
    }
    None
}

/// ISO 8601 durations as used by schema.org (`PT1M7S`).
fn iso_duration(s: &str) -> Option<f64> {
    let t = s.strip_prefix("PT")?;
    let (mut total, mut num) = (0.0, String::new());
    for c in t.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'H' | 'M' | 'S' => {
                let n: f64 = num.parse().ok()?;
                total += n * match c { 'H' => 3600.0, 'M' => 60.0, _ => 1.0 };
                num.clear();
            }
            _ => return None,
        }
    }
    Some(total)
}

fn kwai_build(v: &Value, id: &str) -> AppResult<Value> {
    let url = v["contentUrl"].as_str().filter(|u| u.starts_with("https://")).ok_or_else(|| AppError::Extractor("Kwai's page lists this video without a playable link".into()))?;
    let title = [v["description"].as_str(), v["name"].as_str()].into_iter().flatten().map(str::trim).find(|t| !t.is_empty()).unwrap_or("Kwai video");
    let creator = &v["creator"]["mainEntity"];
    Ok(json!({
        "id": id,
        "title": title.chars().take(120).collect::<String>(),
        "uploader": creator["name"],
        "uploader_id": creator["alternateName"],
        "duration": v["duration"].as_str().and_then(iso_duration),
        "upload_date": v["uploadDate"].as_str().and_then(|d| d.get(..10)).map(|d| d.replace('-', "")),
        "thumbnail": v["thumbnailUrl"].get(0).or(Some(&v["thumbnailUrl"])).and_then(Value::as_str),
        "webpage_url": v["url"].as_str().unwrap_or_default(),
        "extractor_key": "Kwai (Rabbit page metadata)",
        // The page gives the size but not the codecs; the file is checked after download.
        "formats": [{"format_id": "mp4", "ext": "mp4", "width": v["width"], "height": v["height"], "protocol": "https", "url": url, "http_headers": {"User-Agent": MOBILE_UA}}],
    }))
}

/// The page state's keys are obfuscated, so the post is found by its fields instead of its path.
fn find_photo(v: &Value) -> Option<&Value> {
    match v {
        Value::Object(m) if m.get("mainMvUrls").is_some_and(Value::is_array) && (m.contains_key("photoId") || m.contains_key("caption")) => Some(v),
        Value::Object(m) => m.values().find_map(find_photo),
        Value::Array(a) => a.iter().find_map(find_photo),
        _ => None,
    }
}

fn build(photo: &Value, id: &str) -> AppResult<Value> {
    let kind = photo["type"].as_str().unwrap_or("VIDEO");
    if kind != "VIDEO" {
        return Err(AppError::Unsupported(format!("this Kuaishou post is a photo album ({kind}); Rabbit only downloads Kuaishou videos so far")));
    }
    let headers = json!({"User-Agent": MOBILE_UA});
    let muted = photo["mute"].as_bool() == Some(true);
    let mut formats = Vec::new();
    let reps = photo["manifest"]["adaptationSet"].as_array().into_iter().flatten().flat_map(|s| s["representation"].as_array().into_iter().flatten());
    for r in reps {
        let Some(url) = r["url"].as_str() else { continue };
        let silent = muted || r["mute"].as_bool() == Some(true);
        formats.push(json!({
            "format_id": format!("{}-{}", r["qualityType"].as_str().unwrap_or("q"), r["id"].as_u64().unwrap_or(formats.len() as u64)),
            "ext": "mp4",
            "vcodec": r["videoCodec"],
            // The page declares whether the video has sound, but not the audio codec.
            "acodec": if silent { Some("none") } else { None },
            "audio_ext": if silent { "none" } else { "m4a" },
            "width": r["width"],
            "height": r["height"],
            "fps": r["frameRate"],
            "tbr": r["avgBitrate"],
            "filesize": r["fileSize"],
            "protocol": "https",
            "url": web::https(url),
            "backup_urls": r["backupUrl"].as_array().into_iter().flatten().filter_map(Value::as_str).map(web::https).collect::<Vec<_>>(),
            "http_headers": headers,
        }));
    }
    if formats.is_empty()
        && let Some(url) = photo["mainMvUrls"][0]["url"].as_str()
    {
        formats.push(json!({
            "format_id": "main", "ext": "mp4", "width": photo["width"], "height": photo["height"],
            "audio_ext": if muted { "none" } else { "m4a" }, "protocol": "https", "url": web::https(url), "http_headers": headers,
        }));
    }
    if formats.is_empty() {
        return Err(AppError::Extractor("Kuaishou's share page lists this video without a playable link".into()));
    }
    let caption = photo["caption"].as_str().map(str::trim).filter(|c| !c.is_empty());
    Ok(json!({
        "id": id,
        "title": caption.map(str::to_string).unwrap_or_else(|| format!("Kuaishou video {id}")),
        "uploader": photo["userName"],
        "uploader_id": photo["userEid"].as_str().or_else(|| photo["userId"].as_str()),
        "duration": photo["duration"].as_f64().map(|ms| ms / 1000.0),
        "thumbnail": photo["coverUrls"][0]["url"].as_str().map(web::https),
        "upload_date": photo["timestamp"].as_i64().and_then(|t| web::upload_date(t / 1000)),
        "view_count": photo["viewCount"],
        "webpage_url": format!("https://www.kuaishou.com/short-video/{id}"),
        "extractor_key": "Kuaishou (Rabbit share page)",
        "formats": formats,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn recognises_video_addresses() {
        assert_eq!(photo_id(&u("https://v.m.chenzhongtech.com/fw/photo/3xjegz725scedme?fid=1&cc=share_copylink")).as_deref(), Some("3xjegz725scedme"));
        assert_eq!(photo_id(&u("https://www.kuaishou.com/short-video/3x88nbkyjtga6zk")).as_deref(), Some("3x88nbkyjtga6zk"));
        assert_eq!(photo_id(&u("https://m.gifshow.com/fw/photo/3xjegz725scedme")).as_deref(), Some("3xjegz725scedme"));
        assert_eq!(photo_id(&u("https://www.kuaishou.com/profile/3xabc")), None);
        assert!(handles(&u("https://v.kuaishou.com/5fkjkb")));
        assert!(handles(&u("https://www.kwai.com/@vokic313/video/5220666502875224403")));
    }

    #[test]
    fn finds_the_post_in_obfuscated_state() {
        // Shape observed on the share page for 3xjegz725scedme (keys shortened).
        let state = json!({"tusjoh.0sftu": {"fsc": {"photo": {
            "photoId": "5259078675536249939", "caption": "#喜爱度激励计划", "userName": "雪顶", "duration": 15950, "type": "VIDEO",
            "timestamp": 1791863687033_i64, "mute": false, "mainMvUrls": [{"cdn": "txmov2.a.kwimgs.com", "url": "https://txmov2.a.kwimgs.com/upic/a.mp4"}],
            "coverUrls": [{"url": "https://p23.a.yximgs.com/upic/c.jpg"}],
            "manifest": {"adaptationSet": [{"id": 1, "representation": [
                {"id": 1, "url": "https://v23-3.kwaicdn.com/upic/hi.mp4", "videoCodec": "avc", "width": 720, "height": 1280, "avgBitrate": 1577, "frameRate": 60, "fileSize": 3150008, "qualityType": "720p", "mute": false},
                {"id": 2, "url": "https://v23-3.kwaicdn.com/upic/lo.mp4", "videoCodec": "avc", "width": 720, "height": 1280, "avgBitrate": 675, "frameRate": 60, "fileSize": 1350000, "qualityType": "720p", "mute": false}
            ]}]}
        }}}});
        let photo = find_photo(&state).unwrap();
        let v = build(photo, "3xjegz725scedme").unwrap();
        assert_eq!(v["title"], "#喜爱度激励计划");
        assert_eq!(v["duration"], 15.95);
        assert_eq!(v["upload_date"], "20261013");
        let n = crate::extract::normalize::normalize(&v, true).unwrap();
        let rec = n.options.iter().find(|(o, _)| o.recommended).unwrap();
        assert_eq!(rec.0.id, "v-720p-1", "highest bitrate H.264 at 720p");
        assert_eq!(rec.1.expect_audio, Some(true));
        assert_eq!(rec.0.codec, "H.264 + not reported");
    }

    #[test]
    fn photo_albums_are_declined_clearly() {
        let e = build(&json!({"type": "ATLAS", "mainMvUrls": []}), "x").unwrap_err();
        assert!(e.to_string().contains("photo album"), "{e}");
    }
}

#[cfg(test)]
mod kwai_tests {
    use super::*;

    #[test]
    fn kwai_video_object() {
        // Shape observed on kwai.com/@vokic313/video/5220666502875224403 (schema.org JSON-LD).
        let html = r#"<script type="application/ld+json" id="Page">{"@type":"ItemPage"}</script>
            <script type="application/ld+json">{"@context":"https://schema.org/","@type":"VideoObject","url":"https://www.kwai.com/@vokic313/video/5220666502875224403",
            "name":"Name","description":"ACEITA UM BEIJO","thumbnailUrl":["https://aws-br-pic.kwai.net/t.jpg"],"uploadDate":"2025-08-25T02:24:33Z",
            "contentUrl":"https://aws-br-cdn.kwai.net/upic/v.mp4?tag=1","duration":"PT1M7S","width":720,"height":1280,
            "creator":{"mainEntity":{"name":"Rev","alternateName":"vokic313"}}}</script>"#;
        let v = ld_json_video(html).unwrap();
        let out = kwai_build(&v, "5220666502875224403").unwrap();
        assert_eq!((out["duration"].as_f64(), out["upload_date"].as_str()), (Some(67.0), Some("20250825")));
        let n = crate::extract::normalize::normalize(&out, true).unwrap();
        let (o, _) = n.options.iter().find(|(o, _)| o.recommended).unwrap();
        assert_eq!(o.label, "720p");
        assert_eq!(iso_duration("PT7S"), Some(7.0));
    }
}