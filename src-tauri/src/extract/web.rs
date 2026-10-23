//! Shared HTTP helpers for Rabbit's own extractors (Bilibili, RedNote, Kuaishou).
//! They read the same public pages and APIs a signed-out browser does: no accounts, no tokens.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, LOCATION, SET_COOKIE, USER_AGENT};
use serde_json::Value;
use url::Url;

use crate::error::{AppError, AppResult};

pub const DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
pub const MOBILE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";

const MAX_PAGE: usize = 8 * 1024 * 1024;
const MAX_HOPS: usize = 6;

/// A client that never follows redirects on its own: short links are resolved hop by hop so a
/// hop can be stopped or rewritten (RedNote's mainland host is unreachable from some networks).
#[derive(Clone)]
pub struct Web {
    client: reqwest::Client,
}

pub struct Page {
    pub status: u16,
    pub body: String,
    pub cookies: Vec<(String, String)>,
}

impl Web {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(25))
            .build()
            .map_err(|e| AppError::Network(e.to_string()))?;
        Ok(Web { client })
    }

    fn headers(ua: &str, extra: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(USER_AGENT, HeaderValue::from_str(ua).expect("static user agent"));
        for (k, v) in extra {
            if let (Ok(k), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
                h.insert(k, v);
            }
        }
        h
    }

    /// Sends a GET, retrying once after a connection-level failure (some platform hosts drop
    /// connections intermittently from outside mainland China). HTTP answers are never retried here.
    async fn send(&self, url: &str, headers: HeaderMap) -> AppResult<reqwest::Response> {
        match self.client.get(url).headers(headers.clone()).send().await {
            Ok(r) => Ok(r),
            Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
                tracing::info!("retrying a request after a connection error: {}", e.without_url());
                tokio::time::sleep(Duration::from_millis(700)).await;
                Ok(self.client.get(url).headers(headers).send().await?)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// GET one URL (no redirects) and return its body as text.
    pub async fn get(&self, url: &str, ua: &str, extra: &[(&str, &str)]) -> AppResult<Page> {
        let resp = self.send(url, Self::headers(ua, extra)).await?;
        let status = resp.status().as_u16();
        let cookies = resp
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|c| c.split(';').next()?.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
            .collect();
        if resp.content_length().is_some_and(|l| l as usize > MAX_PAGE) {
            return Err(AppError::Extractor("the page is unexpectedly large".into()));
        }
        let bytes = resp.bytes().await?;
        if bytes.len() > MAX_PAGE {
            return Err(AppError::Extractor("the page is unexpectedly large".into()));
        }
        Ok(Page { status, body: String::from_utf8_lossy(&bytes).into_owned(), cookies })
    }

    /// GET a JSON API endpoint. Non-2xx answers become `AppError::Http`.
    pub async fn get_json(&self, url: &str, ua: &str, extra: &[(&str, &str)]) -> AppResult<Value> {
        let page = self.get(url, ua, extra).await?;
        if !(200..300).contains(&page.status) {
            return Err(AppError::Http(page.status));
        }
        serde_json::from_str(&page.body).map_err(|_| AppError::Extractor("the platform answered with something other than JSON".into()))
    }

    /// Follows redirects from `start` until `stop` accepts a URL or there are no more hops.
    pub async fn resolve(&self, start: &Url, ua: &str, stop: impl Fn(&Url) -> bool) -> AppResult<Url> {
        let mut url = start.clone();
        for _ in 0..MAX_HOPS {
            if stop(&url) {
                return Ok(url);
            }
            let resp = self.send(url.as_str(), Self::headers(ua, &[])).await?;
            if !resp.status().is_redirection() {
                return Ok(url);
            }
            let Some(loc) = resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()) else { return Ok(url) };
            url = url.join(loc).map_err(|_| AppError::Extractor("the short link redirected to an invalid address".into()))?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err(AppError::Extractor("the short link redirected to an app-only address".into()));
            }
        }
        Ok(url)
    }
}

/// A user's own cookies for one site, and the only hosts they may be sent to
/// (docs/DESIGN_SIGNED_IN_SESSIONS.md §3). Debug output never shows the values.
pub struct SessionCookies {
    pub site: crate::sys::cookies::Site,
    header: String,
    allowed_hosts: Vec<String>,
}

impl std::fmt::Debug for SessionCookies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SessionCookies({:?}, <redacted>, {:?})", self.site, self.allowed_hosts)
    }
}

impl SessionCookies {
    /// `None` when there are no cookies to send.
    pub fn new(site: crate::sys::cookies::Site, cookies: &[crate::sys::cookies::Cookie], allowed_hosts: &[&str]) -> Option<Self> {
        (!cookies.is_empty()).then(|| SessionCookies {
            site,
            header: crate::sys::cookies::header(cookies),
            allowed_hosts: allowed_hosts.iter().map(|h| h.to_ascii_lowercase()).collect(),
        })
    }

    /// Exact host match: the session goes to these hosts and nowhere else.
    pub fn allows(&self, url: &str) -> bool {
        Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_ascii_lowercase)).is_some_and(|h| self.allowed_hosts.contains(&h))
    }
}

impl Web {
    /// `get_json` carrying the user's session — refused for any host outside its allow-list.
    pub async fn get_json_with_session(&self, url: &str, ua: &str, extra: &[(&str, &str)], session: &SessionCookies) -> AppResult<Value> {
        if !session.allows(url) {
            let host = Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
            return Err(AppError::Permission(format!("refusing to send your {} session to {host}", session.site.name())));
        }
        let mut headers = extra.to_vec();
        headers.push(("cookie", &session.header));
        self.get_json(url, ua, &headers).await
    }
}

/// The JSON object that starts at the first `{` after `marker` in an HTML page (a
/// `window.X = {...}` assignment). Strings and escapes are respected while matching braces.
pub fn object_after<'a>(html: &'a str, marker: &str) -> Option<&'a str> {
    let at = html.find(marker)?;
    let start = at + html[at..].find('{')?;
    let bytes = html.as_bytes();
    let (mut depth, mut in_str, mut esc) = (0usize, false, false);
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            match b {
                _ if esc => esc = false,
                b'\\' => esc = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&html[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parses a JavaScript object literal that is JSON apart from `undefined` values and
/// `new Set([...])` wrappers (RedNote's `__INITIAL_STATE__`). String contents are left untouched.
pub fn parse_js_object(src: &str) -> Option<Value> {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.char_indices();
    let (mut in_str, mut esc) = (false, false);
    let skip = |chars: &mut std::str::CharIndices, n: usize| {
        for _ in 1..n {
            chars.next();
        }
    };
    while let Some((i, c)) = chars.next() {
        if in_str {
            out.push(c);
            match c {
                _ if esc => esc = false,
                '\\' => esc = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        let at_word = !src[..i].ends_with(|p: char| p.is_ascii_alphanumeric() || p == '_');
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            'u' if at_word && src[i..].starts_with("undefined") => {
                out.push_str("null");
                skip(&mut chars, "undefined".len());
            }
            'n' if at_word && src[i..].starts_with("new Set(") => skip(&mut chars, "new Set(".len()),
            // JSON has no parentheses outside strings: this closes a `new Set(`.
            ')' => {}
            _ => out.push(c),
        }
    }
    serde_json::from_str(&out).ok()
}

/// Upgrades protocol-relative and plain-HTTP CDN links to HTTPS.
pub fn https(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

/// yt-dlp's `upload_date` form (YYYYMMDD, UTC) from a Unix timestamp in seconds.
pub fn upload_date(secs: i64) -> Option<String> {
    if secs <= 0 {
        return None;
    }
    // Days to civil date (Howard Hinnant's algorithm).
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!("{y:04}{m:02}{d:02}"))
}

pub fn host_is(url: &Url, domain: &str) -> bool {
    url.host_str().is_some_and(|h| h == domain || h.strip_suffix(domain).is_some_and(|r| r.ends_with('.')))
}

/// Image container from the first bytes of a file.
pub fn sniff_image(head: &[u8]) -> Option<&'static str> {
    match head {
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("webp"),
        [_, _, _, _, b'f', b't', b'y', b'p', rest @ ..] if rest.starts_with(b"avif") || rest.starts_with(b"avis") => Some("avif"),
        [_, _, _, _, b'f', b't', b'y', b'p', rest @ ..] if rest.starts_with(b"hei") || rest.starts_with(b"mif1") => Some("heic"),
        _ => None,
    }
}

/// Width × height of the first video track in an MP4 whose header (`moov`) comes first, read from
/// its `tkhd` box (display size, 16.16 fixed point). Audio tracks have a zero size and are skipped.
pub fn mp4_video_size(head: &[u8]) -> Option<(u32, u32)> {
    let mut from = 0;
    while let Some(rel) = head[from..].windows(4).position(|w| w == b"tkhd") {
        let body = from + rel + 4;
        let version = *head.get(body)?;
        // version, flags, times, track id, reserved, duration, reserved, layer, group, volume,
        // reserved, matrix — then width and height.
        let offset = body + if version == 1 { 4 + 8 + 8 + 4 + 4 + 8 } else { 4 + 4 + 4 + 4 + 4 + 4 } + 8 + 2 + 2 + 2 + 2 + 36;
        let w = u32::from_be_bytes(head.get(offset..offset + 4)?.try_into().ok()?) >> 16;
        let h = u32::from_be_bytes(head.get(offset + 4..offset + 8)?.try_into().ok()?) >> 16;
        if w > 0 && h > 0 {
            return Some((w, h));
        }
        from = body;
    }
    None
}

impl Web {
    /// The first `n` bytes of a media URL (a range request), for sniffing headers.
    pub async fn first_bytes(&self, url: &str, ua: &str, extra: &[(&str, &str)], n: usize) -> Option<Vec<u8>> {
        let range = format!("bytes=0-{}", n.saturating_sub(1));
        let mut headers: Vec<(&str, &str)> = extra.to_vec();
        headers.push(("range", &range));
        let resp = self.client.get(url).headers(Self::headers(ua, &headers)).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let bytes = resp.bytes().await.ok()?;
        Some(bytes[..bytes.len().min(n)].to_vec())
    }

    /// Reads the first bytes of an image to learn its real container (the URL doesn't say).
    pub async fn image_ext(&self, url: &str, ua: &str, extra: &[(&str, &str)]) -> Option<&'static str> {
        sniff_image(&self.first_bytes(url, ua, extra, 32).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_assigned_object() {
        let html = r#"<script>window.__INITIAL_STATE__={"a":{"b":"x}{\"y"},"c":[1,{"d":2}]}</script>"#;
        assert_eq!(object_after(html, "__INITIAL_STATE__"), Some(r#"{"a":{"b":"x}{\"y"},"c":[1,{"d":2}]}"#));
        assert_eq!(object_after("no state", "__INITIAL_STATE__"), None);
        assert_eq!(object_after("window.X={\"a\":1", "X"), None, "unterminated object");
    }

    #[test]
    fn js_object_with_undefined() {
        let v = parse_js_object(r#"{"a":undefined,"b":"undefined","c":[undefined],"undefinedKey":1,"s":new Set(["x","(y)"]),"e":new Set([])}"#).unwrap();
        assert!(v["a"].is_null());
        assert_eq!(v["b"], "undefined");
        assert!(v["c"][0].is_null());
        assert_eq!(v["undefinedKey"], 1);
        assert_eq!(v["s"], serde_json::json!(["x", "(y)"]));
        assert_eq!(v["e"], serde_json::json!([]));
    }

    #[test]
    fn image_sniffing() {
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_image(b"\0\0\0\x1cftypheic"), Some("heic"));
        assert_eq!(sniff_image(b"\0\0\0\x1cftypavif"), Some("avif"));
        assert_eq!(sniff_image(b"<html>"), None);
    }

    #[test]
    fn mp4_track_size() {
        // A minimal v0 tkhd for an audio track (0×0) followed by a video track (1080×608).
        let tkhd = |w: u32, h: u32| {
            let mut b = b"tkhd".to_vec();
            b.extend([0u8; 4 + 20 + 8 + 8 + 36]);
            b.extend((w << 16).to_be_bytes());
            b.extend((h << 16).to_be_bytes());
            b
        };
        let mut head = b"\0\0\0\x20ftypisom".to_vec();
        head.extend(tkhd(0, 0));
        head.extend(tkhd(1080, 608));
        assert_eq!(mp4_video_size(&head), Some((1080, 608)));
        assert_eq!(mp4_video_size(b"no boxes here"), None);
    }

    #[test]
    fn dates_from_timestamps() {
        assert_eq!(upload_date(0), None);
        assert_eq!(upload_date(951_782_400).as_deref(), Some("20000229"));
        assert_eq!(upload_date(1_791_982_377).as_deref(), Some("20261014"));
    }

    #[test]
    fn upgrades_to_https() {
        assert_eq!(https("//i0.hdslb.com/a.jpg"), "https://i0.hdslb.com/a.jpg");
        assert_eq!(https("http://sns-v11.rednotecdn.com/x"), "https://sns-v11.rednotecdn.com/x");
        assert_eq!(https("https://a/b"), "https://a/b");
    }
}
