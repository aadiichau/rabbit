//! Suno songs from their public song page. The page's clip record lists what Suno publishes for
//! signed-out visitors: the song video (`video_url`, an MP4 with the full song as its sound track,
//! made for sharing) and the cover image. It deliberately withholds the audio file
//! (`audio_url` points at `…/api/forbidden`, and the CDN answers 403 for the MP3), so Rabbit
//! offers the video and cover only — no audio taken from the player's stream, no MP3 conversion.
//! (yt-dlp lists suno.com as a site that "would be a liability to the project"; that is a project
//! decision, not an access restriction, so Rabbit reads public pages itself.)

use serde_json::{Value, json};
use url::Url;

use super::web::{self, DESKTOP_UA, Web};
use crate::error::{AppError, AppResult};

pub fn handles(url: &Url) -> bool {
    song_id(url).is_some() || share_code(url)
}

fn on_suno(url: &Url) -> bool {
    web::host_is(url, "suno.com") || web::host_is(url, "suno.ai")
}

/// `/song/<uuid>` (also on app.suno.ai).
fn song_id(url: &Url) -> Option<String> {
    if !on_suno(url) {
        return None;
    }
    let segs: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    match segs.as_slice() {
        ["song", id, ..] if is_uuid(id) => Some(id.to_ascii_lowercase()),
        _ => None,
    }
}

/// Share links: `suno.com/s/<code>`, which redirect to the song page.
fn share_code(url: &Url) -> bool {
    on_suno(url) && url.path_segments().is_some_and(|mut s| s.next() == Some("s") && s.next().is_some_and(|c| !c.is_empty()))
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.chars().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() })
}

pub async fn extract(w: &Web, input: &Url) -> AppResult<Value> {
    let url = if share_code(input) { w.resolve(input, DESKTOP_UA, |u| song_id(u).is_some()).await? } else { input.clone() };
    let id = song_id(&url).ok_or_else(|| AppError::Extractor("this Suno share link doesn't lead to a song — it may have expired".into()))?;
    let page = w.get(&format!("https://suno.com/song/{id}"), DESKTOP_UA, &[]).await?;
    if page.status == 404 {
        return Err(AppError::Extractor("Suno says this song doesn't exist (it may have been deleted)".into()));
    }
    if !(200..300).contains(&page.status) {
        return Err(AppError::Http(page.status));
    }
    let clip = find_clip(&page.body, &id).ok_or_else(|| {
        AppError::LoginRequired("Suno didn't show this song to a signed-out visitor. It may be private or removed".into())
    })?;
    build(&clip, &id)
}

/// The song page is a Next.js app: its data arrives as `self.__next_f.push([1,"…"])` string chunks.
/// The clip record is the JSON object with this song's `id` and a `video_url` or `audio_url`.
fn find_clip(html: &str, id: &str) -> Option<Value> {
    let mut flight = String::new();
    let marker = "self.__next_f.push([1,";
    let mut rest = html;
    while let Some(at) = rest.find(marker) {
        rest = &rest[at + marker.len()..];
        let end = string_end(rest)?;
        if let Ok(Value::String(s)) = serde_json::from_str::<Value>(&rest[..end]) {
            flight.push_str(&s);
        }
        rest = &rest[end..];
    }
    let needle = format!("\"id\":\"{id}\"");
    let mut search_from = 0;
    while let Some(rel) = flight[search_from..].find(&needle) {
        let at = search_from + rel;
        // Try each enclosing `{` until one parses into the clip record.
        let mut start = flight[..at].rfind('{');
        while let Some(s) = start {
            if let Some(obj) = super::web::object_after(&flight[s..], "{").and_then(|o| serde_json::from_str::<Value>(o).ok())
                && obj["id"] == id
                && (obj.get("video_url").is_some() || obj.get("audio_url").is_some())
            {
                return Some(obj);
            }
            start = flight[..s].rfind('{');
            if at - start.unwrap_or(0) > 20_000 {
                break;
            }
        }
        search_from = at + needle.len();
    }
    None
}

/// Length of the JSON string literal at the start of `s` (including both quotes).
fn string_end(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.first() != Some(&b'"') {
        return None;
    }
    let mut esc = false;
    for (i, &c) in b.iter().enumerate().skip(1) {
        match c {
            _ if esc => esc = false,
            b'\\' => esc = true,
            b'"' => return Some(i + 1),
            _ => {}
        }
    }
    None
}

fn build(clip: &Value, id: &str) -> AppResult<Value> {
    if clip["is_public"].as_bool() == Some(false) || clip["is_trashed"].as_bool() == Some(true) {
        return Err(AppError::LoginRequired("This Suno song isn't public".into()));
    }
    let video = clip["video_url"].as_str().filter(|u| u.starts_with("https://") && !u.contains("/forbidden"));
    let Some(video) = video else {
        return Err(AppError::Unsupported(
            "Suno doesn't publish a downloadable file for this song to signed-out visitors (its audio file is marked forbidden, and there's no song video)".into(),
        ));
    };
    let title = clip["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).unwrap_or("Suno song");
    Ok(json!({
        "id": id,
        "title": title,
        "description": clip["metadata"]["tags"],
        "uploader": clip["display_name"],
        "uploader_id": clip["handle"],
        "duration": clip["metadata"]["duration"],
        "upload_date": clip["created_at"].as_str().and_then(|d| d.get(..10)).map(|d| d.replace('-', "")),
        "view_count": clip["play_count"],
        "thumbnail": clip["image_large_url"].as_str().or_else(|| clip["image_url"].as_str()),
        "webpage_url": format!("https://suno.com/song/{id}"),
        "extractor_key": "Suno (Rabbit song page)",
        // Suno withholds the audio file; don't turn the video into one.
        "_rabbit_no_audio_conversion": true,
        "formats": [{
            "format_id": "video",
            "ext": "mp4",
            // Every probed song video was H.264 + AAC (research/tools/suno-probe.mjs + ffprobe).
            "vcodec": "avc1",
            "acodec": "mp4a.40.2",
            "protocol": "https",
            "url": video,
        }],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn recognises_song_addresses() {
        assert_eq!(song_id(&u("https://suno.com/song/8e94d88d-912d-4bda-a5b4-42d9e04d74c4")).as_deref(), Some("8e94d88d-912d-4bda-a5b4-42d9e04d74c4"));
        assert_eq!(song_id(&u("https://app.suno.ai/song/b27c29f6-8ab4-47eb-81fd-efb85c848ada/")).as_deref(), Some("b27c29f6-8ab4-47eb-81fd-efb85c848ada"));
        assert!(handles(&u("https://suno.com/s/AQrKvtqiIoeTE9RI")));
        assert!(!handles(&u("https://suno.com/@bojian")));
        assert!(!handles(&u("https://suno.com/song/not-a-uuid")));
    }

    #[test]
    fn finds_the_clip_in_flight_data() {
        // Shape observed on suno.com/song/8e94d88d-… (Next.js flight chunks with escaped JSON).
        let id = "8e94d88d-912d-4bda-a5b4-42d9e04d74c4";
        let clip = format!(r#"{{"status":"complete","title":"迷航","id":"{id}","video_url":"https://cdn1.suno.ai/{id}.mp4","audio_url":"https://studio-api.prod.suno.com/api/forbidden","is_public":true,"display_name":"BoJian","handle":"bojian","created_at":"2024-11-22T13:16:10.040Z","metadata":{{"duration":203.99,"tags":"Pop"}}}}"#);
        let chunk = serde_json::to_string(&format!(r#"5:["$","div",null,{{"clip":{clip}}}]"#)).unwrap();
        let html = format!(r#"<script>self.__next_f.push([1,"1:\"other\""])</script><script>self.__next_f.push([1,{chunk}])</script>"#);
        let found = find_clip(&html, id).expect("clip record");
        let v = build(&found, id).unwrap();
        assert_eq!(v["upload_date"], "20241122");
        let n = crate::extract::normalize::normalize(&v, true).unwrap();
        assert!(n.options.iter().any(|(o, _)| o.container == "mp4"));
        assert!(n.options.iter().all(|(o, _)| !o.conversion), "no MP3 from a song whose audio Suno withholds");
    }

    #[test]
    fn private_or_audio_only_songs_are_explained() {
        let e = build(&json!({"is_public": false}), "x").unwrap_err();
        assert_eq!(e.kind(), "loginRequired");
        let e = build(&json!({"is_public": true, "audio_url": "https://studio-api.prod.suno.com/api/forbidden"}), "x").unwrap_err();
        assert!(e.to_string().contains("forbidden"), "{e}");
    }
}
