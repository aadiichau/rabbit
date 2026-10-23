//! Bilibili videos through the public web API that bilibili.com's own player calls: WBI-signed
//! requests with anonymous visitor cookies (the route of yt-dlp PR #17898). Rabbit never signs
//! in, so it gets what a signed-out browser gets — currently up to 480p.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use md5::{Digest, Md5};
use serde_json::{Value, json};
use url::Url;

use super::web::{self, DESKTOP_UA, SessionCookies, Web};
use crate::error::{AppError, AppResult};

const API: &str = "https://api.bilibili.com";
const SESSION_TTL: Duration = Duration::from_secs(60 * 60);
const SHORT_HOSTS: &[&str] = &["b23.tv", "bili2233.cn", "bili.im"];

/// Character order the web client uses to derive the signing key (same table as yt-dlp's `_sign_wbi`).
const MIXIN: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1,
    60, 51, 30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

struct Session {
    cookie: String,
    mixin: String,
    at: Instant,
}

static SESSION: Mutex<Option<Arc<Session>>> = Mutex::new(None);

#[derive(Debug, PartialEq)]
enum VideoId {
    Bv(String),
    Av(u64),
}

/// Addresses this module reads: video pages and the short links that lead to them.
pub fn handles(url: &Url) -> bool {
    SHORT_HOSTS.iter().any(|h| web::host_is(url, h)) || video_id(url).is_some()
}

fn video_id(url: &Url) -> Option<(VideoId, usize)> {
    if !(web::host_is(url, "www.bilibili.com") || web::host_is(url, "m.bilibili.com") || url.host_str() == Some("bilibili.com")) {
        return None;
    }
    let mut segs = url.path_segments()?.filter(|s| !s.is_empty());
    if segs.next()? != "video" {
        return None;
    }
    let raw = segs.next()?;
    let id = if raw.len() == 12 && raw[..2].eq_ignore_ascii_case("bv") && raw[2..].chars().all(|c| c.is_ascii_alphanumeric()) {
        VideoId::Bv(format!("BV{}", &raw[2..]))
    } else {
        VideoId::Av(raw.strip_prefix("av").or_else(|| raw.strip_prefix("AV"))?.parse().ok()?)
    };
    let page = url.query_pairs().find(|(k, _)| k == "p").and_then(|(_, v)| v.parse::<usize>().ok()).filter(|p| *p >= 1).unwrap_or(1);
    Some((id, page))
}

/// Percent-encoding as JavaScript's `encodeURIComponent` does it, which the signature is computed over.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'!' | b'*' | b'\'' | b'(' | b')') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn mixin_key(img_url: &str, sub_url: &str) -> Option<String> {
    let stem = |u: &str| u.rsplit('/').next().and_then(|f| f.split('.').next()).map(str::to_string);
    let raw: Vec<char> = (stem(img_url)? + &stem(sub_url)?).chars().collect();
    if raw.len() < 64 {
        return None;
    }
    Some(MIXIN.iter().map(|&i| raw[i]).take(32).collect())
}

/// Signs a query: adds `wts`, sorts keys, strips `!'()*` from values, appends `w_rid`.
fn sign(params: &[(&str, String)], mixin: &str, wts: u64) -> String {
    let mut p: Vec<(String, String)> = params.iter().map(|(k, v)| (k.to_string(), v.chars().filter(|c| !"!'()*".contains(*c)).collect())).collect();
    p.push(("wts".into(), wts.to_string()));
    p.sort();
    let query = p.iter().map(|(k, v)| format!("{}={}", encode(k), encode(v))).collect::<Vec<_>>().join("&");
    let digest = Md5::digest(format!("{query}{mixin}").as_bytes());
    let rid: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("{query}&w_rid={rid}")
}

async fn session(w: &Web, fresh: bool) -> AppResult<Arc<Session>> {
    if !fresh && let Some(s) = SESSION.lock().unwrap().as_ref().filter(|s| s.at.elapsed() < SESSION_TTL) {
        return Ok(s.clone());
    }
    // A first visit hands out anonymous device cookies; the fingerprint endpoint supplies buvid3/4.
    let home = w.get("https://www.bilibili.com/", DESKTOP_UA, &[]).await?;
    let mut jar: Vec<(String, String)> = home.cookies.into_iter().filter(|(k, _)| !k.is_empty()).collect();
    let referer = [("referer", "https://www.bilibili.com/")];
    if let Ok(spi) = w.get_json(&format!("{API}/x/frontend/finger/spi"), DESKTOP_UA, &referer).await {
        for (key, field) in [("buvid3", "b_3"), ("buvid4", "b_4")] {
            if let Some(v) = spi["data"][field].as_str().filter(|v| !v.is_empty()) {
                jar.retain(|(k, _)| k != key);
                jar.push((key.into(), v.into()));
            }
        }
    }
    let cookie = jar.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
    // Signed out, `nav` answers code -101 but still carries the day's signing key.
    let nav = w.get_json(&format!("{API}/x/web-interface/nav"), DESKTOP_UA, &[("referer", "https://www.bilibili.com/"), ("cookie", &cookie)]).await?;
    let img = &nav["data"]["wbi_img"];
    let mixin = mixin_key(img["img_url"].as_str().unwrap_or(""), img["sub_url"].as_str().unwrap_or(""))
        .ok_or_else(|| AppError::Extractor("Bilibili's API no longer publishes the request-signing key in the expected form".into()))?;
    let s = Arc::new(Session { cookie, mixin, at: Instant::now() });
    *SESSION.lock().unwrap() = Some(s.clone());
    Ok(s)
}

/// Bilibili wraps every answer in `{code, message, data}`; non-zero codes are failures.
fn check(v: &Value) -> AppResult<&Value> {
    let code = v["code"].as_i64().unwrap_or(-1);
    let msg = v["message"].as_str().unwrap_or("").to_string();
    match code {
        0 => Ok(&v["data"]),
        // -400 is what a well-formed but unknown BV id gets.
        -400 | -404 | 62002 | 62004 => Err(AppError::Extractor(format!("the video doesn't exist, was removed, or is still under review (Bilibili code {code})"))),
        62012 => Err(AppError::LoginRequired(format!("Only the uploader can see this video (Bilibili code {code})"))),
        87008 | 87007 => Err(AppError::LoginRequired(format!("This video is only for the uploader's paying supporters (Bilibili code {code})"))),
        -10403 | 6002003 => Err(AppError::Unsupported(format!("Bilibili doesn't serve this video in your region (code {code})"))),
        -101 | -403 => Err(AppError::LoginRequired(format!("Bilibili refused a signed-out request: {msg} (code {code})"))),
        _ => Err(AppError::Extractor(format!("Bilibili's API answered code {code}: {msg}"))),
    }
}

/// Risk-control answers worth one retry with a fresh visitor session and signing key.
fn is_risk_control(e: &AppError) -> bool {
    matches!(e, AppError::Http(412)) || matches!(e, AppError::Extractor(m) if m.contains("code -352"))
}

/// The only hosts a user's Bilibili session is sent to. Media comes from the CDN without cookies.
pub const SESSION_HOSTS: &[&str] = &["api.bilibili.com", "www.bilibili.com"];

/// Who Bilibili sees: an anonymous visitor (cached device cookies) or the user's own session.
enum Caller<'a> {
    Guest(Arc<Session>),
    User { session: &'a SessionCookies, mixin: String },
}

impl Caller<'_> {
    fn mixin(&self) -> &str {
        match self {
            Caller::Guest(s) => &s.mixin,
            Caller::User { mixin, .. } => mixin,
        }
    }

    async fn get(&self, w: &Web, url: &str, referer: &str) -> AppResult<Value> {
        let base = [("referer", referer), ("origin", "https://www.bilibili.com")];
        match self {
            Caller::Guest(s) => w.get_json(url, DESKTOP_UA, &[base[0], base[1], ("cookie", s.cookie.as_str())]).await,
            Caller::User { session, .. } => w.get_json_with_session(url, DESKTOP_UA, &base, session).await,
        }
    }
}

/// What `nav` says about a user session.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub logged_in: bool,
    pub name: Option<String>,
    /// 大会员: 1080P+, 4K and HDR where the video offers them.
    pub premium: bool,
}

/// Checks a user session (`nav`), which also carries the day's signing key.
pub async fn account(w: &Web, session: &SessionCookies) -> AppResult<(Account, Option<String>)> {
    let nav = w.get_json_with_session(&format!("{API}/x/web-interface/nav"), DESKTOP_UA, &[("referer", "https://www.bilibili.com/")], session).await?;
    let d = &nav["data"];
    let account = Account {
        logged_in: d["isLogin"].as_bool() == Some(true),
        name: d["uname"].as_str().map(str::to_string),
        premium: d["vipStatus"].as_i64() == Some(1) || d["vip"]["status"].as_i64() == Some(1),
    };
    let img = &d["wbi_img"];
    Ok((account, mixin_key(img["img_url"].as_str().unwrap_or(""), img["sub_url"].as_str().unwrap_or(""))))
}

/// `Ok(None)` when a short link leads somewhere other than a video page (yt-dlp may know it).
/// With `user`, the video is read with the user's own session; an expired session falls back to
/// guest access and says so (`_rabbit_session`).
pub async fn extract(w: &Web, input: &Url, user: Option<&SessionCookies>) -> AppResult<Option<Value>> {
    let url = if SHORT_HOSTS.iter().any(|h| web::host_is(input, h)) {
        w.resolve(input, DESKTOP_UA, |u| web::host_is(u, "bilibili.com")).await?
    } else {
        input.clone()
    };
    if SHORT_HOSTS.iter().any(|h| web::host_is(&url, h)) {
        // Bilibili answers an unknown or expired short code with a -404 JSON body instead of a redirect.
        return Err(AppError::Extractor("this Bilibili short link doesn't lead anywhere — it may have expired. Copy the link again from Bilibili".into()));
    }
    let Some((id, page)) = video_id(&url) else { return Ok(None) };

    let (user_caller, note) = match user {
        None => (None, None),
        Some(session) => match account(w, session).await {
            Ok((acc, Some(mixin))) if acc.logged_in => {
                let who = acc.name.clone().unwrap_or_else(|| "signed in".into());
                let tier = if acc.premium { " · 大会员" } else { "" };
                (Some(Caller::User { session, mixin }), Some(format!("your Bilibili session ({who}{tier})")))
            }
            Ok(_) => (None, Some("guest qualities — your Bilibili session has expired or isn't signed in; sign in again in Firefox".to_string())),
            Err(e) => (None, Some(format!("guest qualities — couldn't use your Bilibili session ({e})"))),
        },
    };
    let mut v = match user_caller {
        Some(caller) => fetch(w, &id, page, &caller).await?,
        None => match fetch(w, &id, page, &Caller::Guest(session(w, false).await?)).await {
            Err(e) if is_risk_control(&e) => fetch(w, &id, page, &Caller::Guest(session(w, true).await?)).await?,
            other => other?,
        },
    };
    if let Some(n) = note {
        v["_rabbit_session"] = json!(n);
    }
    Ok(Some(v))
}

async fn fetch(w: &Web, id: &VideoId, page: usize, caller: &Caller<'_>) -> AppResult<Value> {
    let wts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let id_param = match id {
        VideoId::Bv(b) => ("bvid", b.clone()),
        VideoId::Av(a) => ("aid", a.to_string()),
    };
    let referer = match id {
        VideoId::Bv(b) => format!("https://www.bilibili.com/video/{b}/"),
        VideoId::Av(a) => format!("https://www.bilibili.com/video/av{a}/"),
    };
    let detail = caller.get(w, &format!("{API}/x/web-interface/wbi/view/detail?{}", sign(std::slice::from_ref(&id_param), caller.mixin(), wts)), &referer).await?;
    let view = &check(&detail)?["View"];
    let bvid = view["bvid"].as_str().ok_or_else(|| AppError::Extractor("Bilibili's answer has no video id".into()))?.to_string();
    let pages = view["pages"].as_array().cloned().unwrap_or_default();
    if page > pages.len().max(1) {
        return Err(AppError::Input(format!("this video has {} part(s); part {page} doesn't exist", pages.len())));
    }
    let part = pages.get(page - 1);
    let cid = part.and_then(|p| p["cid"].as_u64()).or_else(|| view["cid"].as_u64()).ok_or_else(|| AppError::Extractor("Bilibili's answer has no stream id".into()))?;

    // fnval 4048 = DASH with every codec (AVC, HEVC, AV1) and HDR/4K variants where offered.
    let play_query = sign(&[("bvid", bvid.clone()), ("cid", cid.to_string()), ("fnval", "4048".into()), ("fourk", "1".into()), ("qn", "0".into())], caller.mixin(), wts);
    // Not requested: the progressive MP4 (fnval 1). It is offered to signed-out visitors at 720p,
    // but its CDN cut transfers short, answered 503 to parallel requests and 404 to some links
    // even over one connection (docs/EXTRACTION_FAILURES.md, Bilibili), so 3 of 6 test downloads
    // failed. DASH has not failed once.
    let play = caller.get(w, &format!("{API}/x/player/wbi/playurl?{play_query}"), &referer).await?;
    let data = check(&play)?;
    let duration = duration_of(part, view);
    let mut formats = formats(data, duration);
    // A progressive-only answer (old uploads without DASH): read the real size from the file's
    // header (Bilibili puts it first), since the quality class isn't exact ("720P" was 1080×608).
    if let [p] = formats.as_mut_slice()
        && p["format_id"].as_str().is_some_and(|id| id.starts_with("mp4-"))
    {
        let url = p["url"].as_str().unwrap_or_default().to_string();
        let head = w.first_bytes(&url, DESKTOP_UA, &[("referer", "https://www.bilibili.com/")], 64 * 1024).await;
        if let Some((width, height)) = head.as_deref().and_then(web::mp4_video_size) {
            p["width"] = json!(width);
            p["height"] = json!(height);
        }
    }

    let mut title = view["title"].as_str().unwrap_or("").trim().to_string();
    let mut video_id = bvid.clone();
    let mut webpage = format!("https://www.bilibili.com/video/{bvid}/");
    if pages.len() > 1 {
        let name = part.and_then(|p| p["part"].as_str()).unwrap_or("").trim();
        title = format!("{title} - P{page} {name}").trim_end().to_string();
        video_id = format!("{bvid}_p{page}");
        webpage = format!("{webpage}?p={page}");
    }
    Ok(json!({
        "id": video_id,
        "title": title,
        "uploader": view["owner"]["name"],
        "uploader_id": view["owner"]["mid"].as_u64().map(|m| m.to_string()),
        "duration": duration,
        "thumbnail": view["pic"].as_str().map(web::https),
        "upload_date": view["pubdate"].as_i64().and_then(web::upload_date),
        "view_count": view["stat"]["view"],
        "webpage_url": webpage,
        "extractor_key": "Bilibili (Rabbit web API)",
        "playlist_count": pages.len(),
        "formats": formats,
    }))
}

fn formats(data: &Value, duration: Option<f64>) -> Vec<Value> {
    // The CDN checks the Referer; the same headers go with every media request.
    let headers = json!({"Referer": "https://www.bilibili.com/", "User-Agent": DESKTOP_UA});
    let url_of = |s: &Value| s["baseUrl"].as_str().or_else(|| s["base_url"].as_str()).map(web::https);
    let backups_of = |s: &Value| -> Vec<String> {
        s["backupUrl"].as_array().or_else(|| s["backup_url"].as_array()).into_iter().flatten().filter_map(Value::as_str).map(web::https).collect()
    };
    let approx = |bandwidth: Option<f64>| -> Option<u64> { Some((bandwidth? / 8.0 * duration?) as u64).filter(|x| *x > 0) };
    let mut out = Vec::new();
    let dash = &data["dash"];
    for v in dash["video"].as_array().into_iter().flatten() {
        let Some(url) = url_of(v) else { continue };
        let bw = v["bandwidth"].as_f64();
        out.push(json!({
            "format_id": format!("{}-{}", v["id"].as_u64().unwrap_or(0), v["codecid"].as_u64().unwrap_or(0)),
            "ext": "mp4",
            "vcodec": v["codecs"],
            "acodec": "none",
            "width": v["width"],
            "height": v["height"],
            "fps": v["frameRate"].as_str().and_then(|f| f.parse::<f64>().ok()).or_else(|| v["frame_rate"].as_str().and_then(|f| f.parse().ok())),
            "backup_urls": backups_of(v),
            "tbr": bw.map(|b| b / 1000.0),
            "filesize_approx": approx(bw),
            "protocol": "https",
            "url": url,
            "http_headers": headers,
        }));
    }
    let audios = dash["audio"].as_array().into_iter().flatten().chain(dash["flac"]["audio"].as_object().map(|_| &dash["flac"]["audio"]));
    for a in audios {
        let Some(url) = url_of(a) else { continue };
        let bw = a["bandwidth"].as_f64();
        let flac = a["codecs"].as_str().is_some_and(|c| c.starts_with("fLaC") || c.eq_ignore_ascii_case("flac"));
        out.push(json!({
            "format_id": format!("a{}", a["id"].as_u64().unwrap_or(0)),
            "ext": if flac { "flac" } else { "m4a" },
            "vcodec": "none",
            "acodec": a["codecs"],
            "abr": bw.map(|b| b / 1000.0),
            "backup_urls": backups_of(a),
            "tbr": bw.map(|b| b / 1000.0),
            "filesize_approx": approx(bw),
            "protocol": "https",
            "url": url,
            "http_headers": headers,
        }));
    }
    // Older uploads may only have a progressive file.
    if out.is_empty() {
        out.extend(progressive(data));
    }
    out
}

fn duration_of(part: Option<&Value>, view: &Value) -> Option<f64> {
    part.and_then(|p| p["duration"].as_f64()).or_else(|| view["duration"].as_f64())
}

/// A progressive (`durl`) answer as one muxed format — taken only when it is a single piece.
/// The API names the quality (qn), not the codecs: MP4 ones were H.264 + AAC in every probe
/// (4 videos, `research/tools/bilibili-mp4-url.mjs` + ffprobe); FLV ones stay "not reported".
fn progressive(data: &Value) -> Option<Value> {
    let [one] = data["durl"].as_array()?.as_slice() else { return None };
    let url = one["url"].as_str()?;
    let qn = data["quality"].as_u64().unwrap_or(0);
    // Quality numbers name the short side: 16 = 360P, 32 = 480P, 64 = 720P, 80 = 1080P.
    let height = match qn {
        16 => Some(360),
        32 => Some(480),
        64 => Some(720),
        74 | 80 | 112 | 116 => Some(1080),
        _ => None,
    };
    let flv = data["format"].as_str().is_some_and(|f| f.starts_with("flv"));
    Some(json!({
        "format_id": format!("mp4-{qn}"),
        "ext": if flv { "flv" } else { "mp4" },
        "height": height,
        "vcodec": (!flv).then_some("avc1"),
        "acodec": (!flv).then_some("mp4a.40.2"),
        "audio_ext": "m4a",
        "filesize": one["size"],
        "protocol": "https",
        "url": web::https(url),
        "http_headers": {"Referer": "https://www.bilibili.com/", "User-Agent": DESKTOP_UA},
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
        assert_eq!(video_id(&u("https://www.bilibili.com/video/BV1efJk6aEdA?t=1.6")), Some((VideoId::Bv("BV1efJk6aEdA".into()), 1)));
        assert_eq!(video_id(&u("https://m.bilibili.com/video/bv1efJk6aEdA/?p=3")), Some((VideoId::Bv("BV1efJk6aEdA".into()), 3)));
        assert_eq!(video_id(&u("https://www.bilibili.com/video/av170001")), Some((VideoId::Av(170001), 1)));
        assert_eq!(video_id(&u("https://www.bilibili.com/video/BV1efJk6aEdA/?p=0")), Some((VideoId::Bv("BV1efJk6aEdA".into()), 1)));
        assert_eq!(video_id(&u("https://www.bilibili.com/bangumi/play/ep1")), None);
        assert_eq!(video_id(&u("https://www.bilibili.tv/en/video/4794396117045760")), None);
        assert_eq!(video_id(&u("https://live.bilibili.com/6103516")), None);
        assert!(handles(&u("https://b23.tv/a9P3uZO")));
        assert!(!handles(&u("https://space.bilibili.com/1")));
    }

    #[test]
    fn signing_matches_the_web_client() {
        // Reference values from the published WBI description (img/sub keys, wts and w_rid).
        let mixin = mixin_key("https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png", "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png").unwrap();
        assert_eq!(mixin, "ea1db124af3c7062474693fa704f4ff8");
        let q = sign(&[("foo", "114".into()), ("bar", "514".into()), ("zab", "1919810".into())], &mixin, 1702204169);
        assert_eq!(q, "bar=514&foo=114&wts=1702204169&zab=1919810&w_rid=8f6f2b5b3d485fe1886cec6a0be8c5d4");
    }

    #[test]
    fn encodes_like_encode_uri_component() {
        assert_eq!(encode("a b/中"), "a%20b%2F%E4%B8%AD");
        assert_eq!(encode("-_.~"), "-_.~");
    }

    #[test]
    fn dash_formats_carry_referer() {
        let data = json!({"dash": {
            "video": [{"id": 32, "codecid": 7, "baseUrl": "http://upos.example/v.m4s", "bandwidth": 678000, "codecs": "avc1.640033", "width": 852, "height": 480, "frameRate": "29.412"}],
            "audio": [{"id": 30280, "baseUrl": "https://upos.example/a.m4s", "bandwidth": 151000, "codecs": "mp4a.40.2"}],
            "flac": null
        }});
        let f = formats(&data, Some(61.0));
        assert_eq!(f.len(), 2);
        assert_eq!(f[0]["format_id"], "32-7");
        assert_eq!(f[0]["url"], "https://upos.example/v.m4s");
        assert_eq!(f[0]["http_headers"]["Referer"], "https://www.bilibili.com/");
        assert!(f.iter().all(|x| x["http_headers"].get("Cookie").is_none()), "media requests never carry a session");
        assert_eq!(f[0]["filesize_approx"], 5_169_750);
        assert_eq!(f[1]["ext"], "m4a");
        let n = crate::extract::normalize::normalize(&json!({"title": "t", "duration": 61.0, "formats": f}), true).unwrap();
        let rec = n.options.iter().find(|(o, _)| o.recommended).unwrap();
        assert_eq!(rec.0.id, "v-32-7+a30280");
        assert_eq!(rec.0.container, "mp4");
    }

    #[test]
    fn progressive_only_answers_become_one_file() {
        // Shape of a progressive (`durl`) answer, as observed with fnval=1 for BV1efJk6aEdA.
        let data = json!({"quality": 64, "format": "mp4720", "durl": [{"url": "http://upos.example/p.mp4", "size": 10452551}]});
        let f = formats(&data, Some(61.0));
        let n = crate::extract::normalize::normalize(&json!({"title": "t", "duration": 61.0, "formats": f}), false).unwrap();
        let (o, p) = n.options.iter().find(|(o, _)| o.recommended).unwrap();
        assert_eq!((o.id.as_str(), o.label.as_str(), o.size), ("v-mp4-64", "720p", Some(10452551)));
        assert_eq!((p.post, p.expect_audio), (crate::extract::model::Post::None, Some(true)));
        assert!(progressive(&json!({"quality": 64, "durl": [{"url": "a"}, {"url": "b"}]})).is_none(), "multi-piece files aren't joined");
    }
}
