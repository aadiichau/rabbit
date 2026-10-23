//! Pinterest image pins and multi-page ("story"/Idea) pins, from the public pin resource that
//! pinterest.com's own pages call (`/resource/PinResource/get/`, the call yt-dlp makes too).
//! It lists every image at its original size (`orig` / `originals`). yt-dlp only reads videos:
//! image pins fail there with "No video formats found" and story pins yield just the cover.
//! Pins with any video go to yt-dlp, which handles their HLS streams.

use serde_json::{Value, json};
use url::Url;

use super::web::{self, DESKTOP_UA, Web};
use crate::error::{AppError, AppResult};

pub fn handles(url: &Url) -> bool {
    web::host_is(url, "pin.it") || pin_id(url).is_some()
}

/// `/pin/<id>/` or `/pin/<slug>--<id>/` on any pinterest.* host.
fn pin_id(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    if !host.split('.').any(|l| l == "pinterest") {
        return None;
    }
    let segs: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let raw = match segs.as_slice() {
        ["pin", raw, ..] => *raw,
        _ => return None,
    };
    let id = raw.rsplit("--").next()?;
    (id.len() >= 6 && id.chars().all(|c| c.is_ascii_digit())).then(|| id.to_string())
}

pub async fn extract(w: &Web, input: &Url) -> AppResult<Option<Value>> {
    let url = if web::host_is(input, "pin.it") { w.resolve(input, DESKTOP_UA, |u| pin_id(u).is_some()).await? } else { input.clone() };
    let Some(id) = pin_id(&url) else { return Ok(None) };
    let data = json!({"options": {"field_set_key": "unauth_react_main_pin", "id": id}}).to_string();
    let api = format!("https://www.pinterest.com/resource/PinResource/get/?data={}", url::form_urlencoded::byte_serialize(data.as_bytes()).collect::<String>());
    let resp = w.get_json(&api, DESKTOP_UA, &[("x-pinterest-pws-handler", "www/[username].js")]).await?;
    let pin = &resp["resource_response"]["data"];
    if !pin.is_object() {
        return Err(AppError::Extractor("Pinterest says this pin doesn't exist or isn't public".into()));
    }
    let Some(mut v) = build(pin, &id) else { return Ok(None) };
    // Keep, per image, the first candidate the CDN actually serves (story-pin `originals` URLs
    // answered 403 while their 736-wide copies were served), with the type its bytes show.
    let items: Vec<&mut Value> = match v.get_mut("entries").and_then(Value::as_array_mut) {
        Some(entries) => entries.iter_mut().collect(),
        None => vec![&mut v],
    };
    for item in items {
        let candidates = item["formats"].as_array().cloned().unwrap_or_default();
        let mut chosen = None;
        for mut f in candidates {
            let Some(url) = f["url"].as_str().map(str::to_string) else { continue };
            if let Some(ext) = w.image_ext(&url, DESKTOP_UA, &[]).await {
                f["ext"] = json!(ext);
                chosen = Some(f);
                break;
            }
        }
        let Some(f) = chosen else {
            return Err(AppError::Expired(403));
        };
        item["formats"] = json!([f]);
    }
    Ok(Some(v))
}

/// Image renditions, best first: `orig`/`originals` (the upload as stored), then by width.
fn candidates(images: &Value) -> Vec<Value> {
    let Some(map) = images.as_object() else { return Vec::new() };
    let mut list: Vec<(&String, &Value)> = map.iter().filter(|(_, i)| i["url"].is_string()).collect();
    list.sort_by_key(|(k, i)| (!matches!(k.as_str(), "orig" | "originals"), std::cmp::Reverse(i["width"].as_u64().unwrap_or(0))));
    let mut seen = Vec::new();
    list.into_iter()
        .filter(|(_, i)| {
            let u = i["url"].as_str().unwrap_or_default().to_string();
            let new = !seen.contains(&u);
            seen.push(u);
            new
        })
        .take(3)
        .map(|(k, i)| json!({"format_id": k, "ext": "jpg", "width": i["width"], "height": i["height"], "protocol": "https", "url": i["url"]}))
        .collect()
}

/// `None` for pins with video (yt-dlp handles those) or without any image.
fn build(pin: &Value, id: &str) -> Option<Value> {
    if pin["videos"]["video_list"].is_object() {
        return None;
    }
    let pages = pin["story_pin_data"]["pages"].as_array().cloned().unwrap_or_default();
    let has_video_block = pages.iter().flat_map(|p| p["blocks"].as_array().cloned().unwrap_or_default()).any(|b| b["video"].is_object());
    if has_video_block {
        return None;
    }
    let mut items: Vec<Value> = pages
        .iter()
        .filter_map(|p| {
            let images = p["blocks"].as_array().and_then(|b| b.iter().find_map(|b| b["image"]["images"].as_object().map(|_| &b["image"]["images"]))).unwrap_or(&p["image"]["images"]);
            let c = candidates(images);
            (!c.is_empty()).then(|| json!({"formats": c}))
        })
        .collect();
    if items.is_empty() {
        let c = candidates(&pin["images"]);
        if c.is_empty() {
            return None;
        }
        items.push(json!({"formats": c}));
    }
    let title = [pin["title"].as_str(), pin["grid_title"].as_str(), pin["description"].as_str()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|t| !t.is_empty())
        .map(|t| t.chars().take(120).collect::<String>())
        .unwrap_or_else(|| format!("Pin {id}"));
    let mut base = json!({
        "id": id,
        "title": title,
        "description": pin["description"],
        "uploader": pin["closeup_attribution"]["full_name"].as_str().or_else(|| pin["pinner"]["full_name"].as_str()),
        "thumbnail": candidates(&pin["images"]).first().and_then(|c| c["url"].as_str().map(str::to_string)),
        "webpage_url": format!("https://www.pinterest.com/pin/{id}/"),
        "extractor_key": "Pinterest (Rabbit pin resource)",
    });
    if items.len() == 1 {
        base["formats"] = items[0]["formats"].clone();
    } else {
        base["_type"] = json!("playlist");
        base["entries"] = Value::Array(items);
    }
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::model::MediaKind;
    use crate::extract::normalize::normalize;

    #[test]
    fn recognises_pin_addresses() {
        let id = |s: &str| pin_id(&Url::parse(s).unwrap());
        assert_eq!(id("https://www.pinterest.com/pin/858146903966145189/").as_deref(), Some("858146903966145189"));
        assert_eq!(id("https://jp.pinterest.com/pin/858146904010573850/").as_deref(), Some("858146904010573850"));
        assert_eq!(id("https://pinterest.com/pin/dive-into-serenity--2885187256207927").as_deref(), Some("2885187256207927"));
        assert_eq!(id("https://www.pinterest.co.uk/pin/1477812373602736/").as_deref(), Some("1477812373602736"));
        assert_eq!(id("https://www.pinterest.com/g1952849/stuff/"), None);
        assert!(handles(&Url::parse("https://pin.it/1X4o9EJSU").unwrap()));
    }

    #[test]
    fn story_pin_pages_become_a_gallery() {
        // Shape observed for jp.pinterest.com/pin/858146904010573850 (3 image pages).
        let page = |u: &str, w: u32, h: u32| json!({"type": "storypinpage", "blocks": [{"type": "story_pin_image_block", "image": {"images": {
            "originals": {"url": format!("https://i.pinimg.com/originals/{u}.jpg"), "width": w, "height": h},
            "236x": {"url": format!("https://i.pinimg.com/236x/{u}.jpg"), "width": 236, "height": 419}}}}]});
        let pin = json!({"title": "", "grid_title": "Story", "images": {"orig": {"url": "https://i.pinimg.com/originals/c.jpg", "width": 375, "height": 666}},
            "story_pin_data": {"pages": [page("a", 225, 400), page("b", 225, 400), page("c", 675, 1200)]}});
        let mut v = build(&pin, "858146904010573850").unwrap();
        assert_eq!(v["entries"][2]["formats"][0]["url"], "https://i.pinimg.com/originals/c.jpg");
        assert_eq!(v["entries"][2]["formats"][1]["url"], "https://i.pinimg.com/236x/c.jpg", "fallback candidate kept");
        // extract() keeps one served candidate per image; normalize sees that.
        for e in v["entries"].as_array_mut().unwrap() {
            let first = e["formats"][0].clone();
            e["formats"] = json!([first]);
        }
        let n = normalize(&v, true).unwrap();
        assert_eq!((n.kind, n.item_count), (MediaKind::Gallery, Some(3)));
    }

    #[test]
    fn image_pin_and_video_pins() {
        let pin = json!({"title": "Origami", "images": {"236x": {"url": "https://i.pinimg.com/236x/d.jpg", "width": 236}, "orig": {"url": "https://i.pinimg.com/originals/d.jpg", "width": 399, "height": 600}}});
        let v = build(&pin, "858146903966145189").unwrap();
        let n = normalize(&v, true).unwrap();
        assert_eq!(n.kind, MediaKind::Image);
        assert_eq!(v["formats"][0]["url"], "https://i.pinimg.com/originals/d.jpg");
        assert_eq!(v["formats"][1]["url"], "https://i.pinimg.com/236x/d.jpg");
        assert!(build(&json!({"videos": {"video_list": {}}}), "1").is_none(), "video pins go to yt-dlp");
    }
}
