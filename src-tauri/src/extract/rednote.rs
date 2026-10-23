//! RedNote (Xiaohongshu) notes from the public mobile note page on www.rednote.com, which serves
//! the same notes as www.xiaohongshu.com. Share links carry an `xsec_token` that the page needs;
//! Rabbit passes it through unchanged and never signs in.

use serde_json::{Value, json};
use url::Url;

use super::web::{self, MOBILE_UA, Web};
use crate::error::{AppError, AppResult};

const PAGE_HOST: &str = "https://www.rednote.com";

pub fn handles(url: &Url) -> bool {
    is_short(url) || note_id(url).is_some()
}

/// Share links: xhslink.com and xhslink.cn.
fn is_short(url: &Url) -> bool {
    web::host_is(url, "xhslink.com") || web::host_is(url, "xhslink.cn")
}

fn note_id(url: &Url) -> Option<String> {
    if !(web::host_is(url, "xiaohongshu.com") || web::host_is(url, "rednote.com")) {
        return None;
    }
    let segs: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let id = match segs.as_slice() {
        ["explore", id] | ["discovery", "item", id] | ["user", "profile", _, id] => *id,
        _ => return None,
    };
    (id.len() == 24 && id.chars().all(|c| c.is_ascii_hexdigit())).then(|| id.to_ascii_lowercase())
}

/// `Ok(None)` when a short link leads somewhere other than a note.
pub async fn extract(w: &Web, input: &Url) -> AppResult<Option<Value>> {
    // Resolve share links hop by hop and stop at the note address: www.xiaohongshu.com itself
    // isn't reachable from every network, while www.rednote.com serves the same page.
    let url = if is_short(input) {
        w.resolve(input, MOBILE_UA, |u| web::host_is(u, "xiaohongshu.com") || web::host_is(u, "rednote.com")).await?
    } else {
        input.clone()
    };
    let Some(id) = note_id(&url) else {
        if is_short(input) {
            // Expired share codes redirect to the RedNote home page.
            return Err(AppError::Extractor("this RedNote share link has expired or doesn't lead to a note. Copy the link again from RedNote".into()));
        }
        return Ok(None);
    };
    let query = url.query().map(|q| format!("?{q}")).unwrap_or_default();
    let page_url = format!("{PAGE_HOST}/discovery/item/{id}{query}");

    let page = w.get(&page_url, MOBILE_UA, &[]).await?;
    if (300..400).contains(&page.status) || page.status == 404 {
        return Err(not_shown(&id, &query));
    }
    if !(200..300).contains(&page.status) {
        return Err(AppError::Http(page.status));
    }
    if page.body.contains("/website-login/captcha") || page.body.contains("verifyType") && !page.body.contains("noteData") {
        return Err(AppError::Extractor("RedNote asked for a verification check, which Rabbit doesn't do. Try again later".into()));
    }
    let state = web::object_after(&page.body, "window.__INITIAL_STATE__")
        .and_then(web::parse_js_object)
        .ok_or_else(|| AppError::Extractor("RedNote's note page has changed and Rabbit can't read it".into()))?;
    let note = &state["noteData"]["data"]["noteData"];
    if !note.is_object() {
        return Err(not_shown(&id, &query));
    }
    build(note, &id, &format!("{PAGE_HOST}/discovery/item/{id}{query}")).map(Some)
}

fn not_shown(id: &str, query: &str) -> AppError {
    if query.contains("xsec_token") {
        AppError::Extractor(format!("RedNote didn't show note {id}. It may have been deleted or made private, or the share link expired — copy a fresh share link"))
    } else {
        AppError::Input("RedNote only shows notes through their share link. Use “Copy link” in the RedNote app (an xhslink.com link or one with xsec_token)".into())
    }
}

fn build(note: &Value, id: &str, webpage: &str) -> AppResult<Value> {
    let title = [note["title"].as_str(), note["desc"].as_str().and_then(|d| d.lines().next())]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("RedNote {id}"));
    let images = note["imageList"].as_array().cloned().unwrap_or_default();
    let mut base = json!({
        "id": id,
        "title": title,
        "description": note["desc"],
        "uploader": note["user"]["nickName"].as_str().or_else(|| note["user"]["nickname"].as_str()),
        "uploader_id": note["user"]["userId"],
        "upload_date": note["time"].as_i64().and_then(|t| web::upload_date(t / 1000)),
        "thumbnail": images.first().and_then(|i| i["url"].as_str().or_else(|| i["urlDefault"].as_str())).map(web::https),
        "webpage_url": webpage,
        "extractor_key": "RedNote (Rabbit mobile page)",
    });

    if note["type"].as_str() == Some("video") {
        let media = &note["video"]["media"];
        if media["video"]["drmType"].as_i64().is_some_and(|d| d != 0) {
            return Err(AppError::Unsupported("this RedNote video is DRM-protected".into()));
        }
        let formats = video_formats(&media["stream"]);
        if formats.is_empty() {
            return Err(AppError::Extractor("RedNote's page lists this video without any playable stream".into()));
        }
        base["duration"] = media["video"]["duration"].clone();
        base["formats"] = Value::Array(formats);
        return Ok(base);
    }

    // Image note: the published URL is a 1080-wide resize. Without parameters the CDN serves the
    // stored original, which for recent notes is a tiled HEIC; `imageView2/2/format/jpg` (no width)
    // returns the same full resolution as JPEG — 3193×5744 for note 6acf6ece… — which opens anywhere.
    let originals: Vec<String> = images
        .iter()
        .filter_map(|i| i["url"].as_str().or_else(|| i["urlDefault"].as_str()))
        .map(|u| format!("{}?imageView2/2/format/jpg", web::https(u.split('?').next().unwrap_or(u))))
        .collect();
    if originals.is_empty() {
        return Err(AppError::Extractor("this RedNote note has no images or video".into()));
    }
    let mut entries = Vec::new();
    for (img, url) in images.iter().zip(&originals) {
        entries.push(json!({"formats": [{
            "format_id": "original",
            "ext": "jpg",
            "width": img["width"],
            "height": img["height"],
            "protocol": "https",
            "url": url,
        }]}));
        // A Live Photo's motion clip is part of the note too.
        if img["livePhoto"].as_bool() == Some(true) {
            let clip = video_formats(&img["stream"]);
            if !clip.is_empty() {
                entries.push(json!({"formats": clip}));
            }
        }
    }
    if entries.len() == 1 {
        let single = entries.remove(0);
        base["formats"] = single["formats"].clone();
        return Ok(base);
    }
    base["_type"] = json!("playlist");
    base["entries"] = Value::Array(entries);
    Ok(base)
}

/// Streams grouped by codec (`h264`, `h265`, `av1`, `h266`), as the note page lists them.
fn video_formats(stream: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    let Some(groups) = stream.as_object() else { return out };
    for (codec, list) in groups {
        for (i, s) in list.as_array().into_iter().flatten().enumerate() {
            let Some(url) = s["masterUrl"].as_str().or_else(|| s["backupUrls"][0].as_str()) else { continue };
            out.push(json!({
                "format_id": format!("{codec}-{i}"),
                "ext": s["format"].as_str().unwrap_or("mp4"),
                "vcodec": s["videoCodec"].as_str().unwrap_or(codec),
                "acodec": s["audioCodec"].as_str().filter(|c| !c.is_empty()),
                "width": s["width"],
                "height": s["height"],
                "fps": s["fps"],
                "tbr": s["avgBitrate"].as_f64().map(|b| b / 1000.0),
                "filesize": s["size"],
                "protocol": "https",
                "url": web::https(url),
                "backup_urls": s["backupUrls"].as_array().into_iter().flatten().filter_map(Value::as_str).map(web::https).filter(|b| b.as_str() != web::https(url)).collect::<Vec<_>>(),
                "http_headers": {"User-Agent": MOBILE_UA},
            }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn recognises_note_addresses() {
        assert_eq!(note_id(&u("https://www.xiaohongshu.com/explore/672451ee000000001a034bc6")).as_deref(), Some("672451ee000000001a034bc6"));
        assert_eq!(note_id(&u("https://www.xiaohongshu.com/discovery/item/6acf7b29000000000a01f86c?xsec_token=x")).as_deref(), Some("6acf7b29000000000a01f86c"));
        assert_eq!(note_id(&u("https://www.rednote.com/user/profile/5c3049570000000006034064/6acf7b29000000000a01f86c")).as_deref(), Some("6acf7b29000000000a01f86c"));
        assert_eq!(note_id(&u("https://www.xiaohongshu.com/user/profile/5c3049570000000006034064")), None);
        assert_eq!(note_id(&u("https://www.xiaohongshu.com/explore")), None);
        assert!(handles(&u("http://xhslink.com/a/RVjdnIaGmsi2")));
        assert!(handles(&u("https://xhslink.cn/o/8FKGfNCXZpB")));
    }

    #[test]
    fn video_streams_become_formats() {
        // Field shapes observed on the mobile page for note 6acf7b29000000000a01f86c.
        let stream = json!({"h264": [{"masterUrl": "http://sns-v11.rednotecdn.com/stream/1.mp4", "backupUrls": ["http://b/1.mp4"], "width": 720, "height": 1280,
            "videoCodec": "h264", "audioCodec": "aac", "avgBitrate": 2062048, "size": 4883704, "fps": 30, "format": "mp4"}], "h265": [], "av1": []});
        let f = video_formats(&stream);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0]["url"], "https://sns-v11.rednotecdn.com/stream/1.mp4");
        let n = crate::extract::normalize::normalize(&json!({"title": "t", "duration": 19, "formats": f}), true).unwrap();
        let rec = n.options.iter().find(|(o, _)| o.recommended).unwrap();
        assert_eq!((rec.0.height, rec.0.size, rec.1.expect_audio), (Some(1280), Some(4883704), Some(true)));
    }
}
