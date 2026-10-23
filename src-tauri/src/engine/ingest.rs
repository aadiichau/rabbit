use std::net::IpAddr;
use std::sync::LazyLock;

use regex::Regex;
use url::{Host, Url};

pub const MAX_URL_LEN: usize = 2048;

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Stops at whitespace, quotes, angle brackets and full-width CJK punctuation common in app share text.
    Regex::new(r#"(?i)https?://[^\s<>"'`，。！？、；：（）【】「」《》\u{3000}]+"#).unwrap()
});

const TRAILING: &[char] = &['.', ',', ';', ':', '!', '?', ']', '}', '\'', '"', '>', '…'];

/// Pulls every http(s) URL out of free text (e.g. app "share" messages), in order, without duplicates.
pub fn extract_urls(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in URL_RE.find_iter(text) {
        let mut candidate = m.as_str();
        // Strip trailing punctuation; a closing paren is kept only when it balances one in the URL
        // (Wikipedia-style paths), so "(see https://x.y/a)" loses it but ".../Rabbit_(film)" keeps it.
        loop {
            let trimmed = candidate.trim_end_matches(TRAILING);
            let unbalanced = trimmed.ends_with(')') && trimmed.matches(')').count() > trimmed.matches('(').count();
            let next = if unbalanced { &trimmed[..trimmed.len() - 1] } else { trimmed };
            if next.len() == candidate.len() {
                break;
            }
            candidate = next;
        }
        if candidate.len() > "https://".len() && !out.iter().any(|u| u == candidate) {
            out.push(candidate.to_string());
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("No link found. Paste a full address that starts with http:// or https://.")]
    NoUrl,
    #[error("That link is longer than {MAX_URL_LEN} characters.")]
    TooLong,
    #[error("That doesn't look like a valid web address.")]
    Malformed,
    #[error("Only http and https links can be downloaded.")]
    Scheme,
    #[error("Links that contain a username or password aren't accepted.")]
    Credentials,
    #[error("Links to this computer or a private network aren't accepted.")]
    LocalHost,
}

/// Validates one URL for use with external tools.
pub fn validate(raw: &str) -> Result<Url, InputError> {
    if raw.len() > MAX_URL_LEN {
        return Err(InputError::TooLong);
    }
    let url = Url::parse(raw).map_err(|_| InputError::Malformed)?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(InputError::Scheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(InputError::Credentials);
    }
    match url.host() {
        None => return Err(InputError::Malformed),
        Some(Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            if d == "localhost" || d.ends_with(".localhost") || d.ends_with(".local") || !d.contains('.') {
                return Err(InputError::LocalHost);
            }
        }
        Some(Host::Ipv4(ip)) => {
            if is_private(IpAddr::V4(ip)) {
                return Err(InputError::LocalHost);
            }
        }
        Some(Host::Ipv6(ip)) => {
            if is_private(IpAddr::V6(ip)) {
                return Err(InputError::LocalHost);
            }
        }
    }
    Ok(url)
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast()
                || v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64 // CGNAT 100.64/10
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_from_douyin_share_text() {
        let t = "7.94 复制打开抖音，看看【某某的作品】今天也要开心 https://v.douyin.com/8Tdgeu/ X@m.Dh 12/30 iPd:/";
        assert_eq!(extract_urls(t), vec!["https://v.douyin.com/8Tdgeu/"]);
    }

    #[test]
    fn extracts_from_rednote_share_text_with_cjk_punctuation() {
        let t = "看看这篇笔记，http://xhslink.com/a/RVjdnIaGmsi2，复制本条信息";
        assert_eq!(extract_urls(t), vec!["http://xhslink.com/a/RVjdnIaGmsi2"]);
    }

    #[test]
    fn trims_trailing_punctuation_and_dedupes() {
        let t = "see https://youtu.be/aAkMkVFwAoo?si=6DLR4cwaxSIGIzap. and (https://x.com/a/status/1) again https://youtu.be/aAkMkVFwAoo?si=6DLR4cwaxSIGIzap";
        assert_eq!(
            extract_urls(t),
            vec!["https://youtu.be/aAkMkVFwAoo?si=6DLR4cwaxSIGIzap", "https://x.com/a/status/1"]
        );
    }

    #[test]
    fn keeps_balanced_parens() {
        let t = "https://en.wikipedia.org/wiki/Rabbit_(disambiguation)";
        assert_eq!(extract_urls(t), vec![t]);
    }

    #[test]
    fn finds_multiple_urls_on_lines() {
        let t = "https://www.pinterest.com/pin/1477812373602736/\nhttps://pin.it/1X4o9EJSU\n";
        assert_eq!(extract_urls(t).len(), 2);
    }

    #[test]
    fn no_url() {
        assert!(extract_urls("just words, ftp://nope").is_empty());
    }

    #[test]
    fn validation() {
        assert!(validate("https://www.youtube.com/watch?v=jNQXAC9IVRw").is_ok());
        assert_eq!(validate("ftp://example.com/a"), Err(InputError::Scheme));
        assert_eq!(validate("https://user:pw@example.com/"), Err(InputError::Credentials));
        assert_eq!(validate("http://127.0.0.1:8080/x"), Err(InputError::LocalHost));
        assert_eq!(validate("http://192.168.1.4/x"), Err(InputError::LocalHost));
        assert_eq!(validate("http://[::1]/x"), Err(InputError::LocalHost));
        assert_eq!(validate("http://localhost/x"), Err(InputError::LocalHost));
        assert_eq!(validate("http://intranet/x"), Err(InputError::LocalHost));
        assert_eq!(validate(&format!("https://a.com/{}", "a".repeat(3000))), Err(InputError::TooLong));
        assert_eq!(validate("https://"), Err(InputError::Malformed));
    }
}
