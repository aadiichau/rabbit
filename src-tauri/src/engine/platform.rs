use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    YouTube,
    TikTok,
    Douyin,
    Bilibili,
    Instagram,
    Facebook,
    Twitter,
    Pinterest,
    Kuaishou,
    RedNote,
    Suno,
    Other,
}

impl Platform {
    pub fn display_name(self) -> &'static str {
        match self {
            Platform::YouTube => "YouTube",
            Platform::TikTok => "TikTok",
            Platform::Douyin => "Douyin",
            Platform::Bilibili => "Bilibili",
            Platform::Instagram => "Instagram",
            Platform::Facebook => "Facebook",
            Platform::Twitter => "X / Twitter",
            Platform::Pinterest => "Pinterest",
            Platform::Kuaishou => "Kuaishou",
            Platform::RedNote => "RedNote",
            Platform::Suno => "Suno",
            Platform::Other => "Other",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Platform::YouTube => "youtube",
            Platform::TikTok => "tiktok",
            Platform::Douyin => "douyin",
            Platform::Bilibili => "bilibili",
            Platform::Instagram => "instagram",
            Platform::Facebook => "facebook",
            Platform::Twitter => "twitter",
            Platform::Pinterest => "pinterest",
            Platform::Kuaishou => "kuaishou",
            Platform::RedNote => "rednote",
            Platform::Suno => "suno",
            Platform::Other => "other",
        }
    }
}

/// Platforms Rabbit declines up front, with the reason shown to the user (docs/PLATFORM_COMPATIBILITY.md).
/// None at the moment: every detected platform has an extractor.
pub fn unsupported_reason(_p: Platform) -> Option<&'static str> {
    None
}

/// Adds platform-specific context to an extraction error, based on causes observed in testing.
pub fn explain(p: Platform, e: AppError) -> AppError {
    match (p, &e) {
        (Platform::TikTok, AppError::Extractor(m) | AppError::Network(m)) => AppError::Extractor(format!(
            "TikTok didn't return the video. TikTok is blocked in some countries (including India), and private or removed videos can't be downloaded ({m})"
        )),
        (Platform::RedNote, AppError::Network(_)) => AppError::Network(
            "RedNote closed the connection. It blocks some networks and asks automated visitors to pass a verification check, which Rabbit doesn't do".into(),
        ),
        (Platform::Bilibili, AppError::Extractor(m)) if m.contains("initial state") => AppError::Extractor(
            "Bilibili changed its pages and the installed yt-dlp can't read this kind of Bilibili page yet (yt-dlp issue #17894). Ordinary video links (BV/av) work through Rabbit's own reader; for others, check for a yt-dlp update in Settings → Tools".into(),
        ),
        _ => e,
    }
}

fn host_is(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|rest| rest.ends_with('.'))
}

/// Pinterest uses many country TLDs (pinterest.co.uk, pinterest.de, …).
fn is_pinterest_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    labels.iter().position(|l| *l == "pinterest").is_some_and(|i| {
        let tail = &labels[i + 1..];
        !tail.is_empty() && tail.len() <= 2 && tail.iter().all(|t| t.len() <= 3 && t.chars().all(|c| c.is_ascii_alphabetic()))
    })
}

pub fn detect(url: &Url) -> Platform {
    let Some(host) = url.host_str() else { return Platform::Other };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let h = host.as_str();
    let any = |domains: &[&str]| domains.iter().any(|d| host_is(h, d));

    if any(&["youtube.com", "youtu.be", "youtube-nocookie.com"]) {
        Platform::YouTube
    } else if any(&["tiktok.com"]) {
        Platform::TikTok
    } else if any(&["douyin.com", "iesdouyin.com"]) {
        Platform::Douyin
    } else if any(&["bilibili.com", "b23.tv", "bili2233.cn", "bili.im", "bilibili.tv"]) {
        Platform::Bilibili
    } else if any(&["instagram.com", "instagr.am"]) {
        Platform::Instagram
    } else if any(&["facebook.com", "fb.watch", "fb.com"]) {
        Platform::Facebook
    } else if any(&["x.com", "twitter.com"]) {
        Platform::Twitter
    } else if any(&["pin.it"]) || is_pinterest_host(h) {
        Platform::Pinterest
    } else if any(&["kuaishou.com", "gifshow.com", "chenzhongtech.com", "kwai.com"]) {
        Platform::Kuaishou
    } else if any(&["xiaohongshu.com", "xhslink.com", "xhslink.cn", "rednote.com"]) {
        Platform::RedNote
    } else if any(&["suno.com", "suno.ai"]) {
        Platform::Suno
    } else {
        Platform::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Platform {
        detect(&Url::parse(s).unwrap())
    }

    #[test]
    fn reference_url_formats() {
        // Every example URL from docs/REFERENCE_RESEARCH.md §4.
        let cases = [
            ("https://vm.tiktok.com/ZMLp2UG/", Platform::TikTok),
            ("https://www.tiktok.com/@xx/video/680586780545", Platform::TikTok),
            ("https://m.tiktok.com/v/680586780545.html", Platform::TikTok),
            ("https://www.tiktok.com/@evynhuynh/photo/7367130722806648106", Platform::TikTok),
            ("https://v.douyin.com/8Tdgeu/", Platform::Douyin),
            ("https://www.douyin.com/video/705187728920004", Platform::Douyin),
            ("https://www.douyin.com/shipin/7344374652132968484", Platform::Douyin),
            ("https://www.douyin.com/discover?modal_id=7069543727328398622", Platform::Douyin),
            ("https://v.kuaishou.com/5fkjkb", Platform::Kuaishou),
            ("https://www.kuaishou.com/f/X-9XB3YvenaCTkCE", Platform::Kuaishou),
            ("https://www.kuaishou.com/short-video/3x88nbkyjtga6zk", Platform::Kuaishou),
            ("https://www.kwai.com/@vokic313/video/5220666502875224403", Platform::Kuaishou),
            ("https://v.m.chenzhongtech.com/fw/photo/3xjegz725scedme?fid=1", Platform::Kuaishou),
            ("https://m.gifshow.com/fw/photo/3xjegz725scedme", Platform::Kuaishou),
            ("https://www.xiaohongshu.com/explore/672451ee000000001a034bc6", Platform::RedNote),
            ("https://www.xiaohongshu.com/discovery/item/6749f3370000000007031c8e", Platform::RedNote),
            ("http://xhslink.com/a/RVjdnIaGmsi2", Platform::RedNote),
            ("https://xhslink.cn/o/8FKGfNCXZpB", Platform::RedNote),
            ("https://www.rednote.com/discovery/item/6ad1f61a0000000018013f51?xsec_token=x", Platform::RedNote),
            ("https://bili2233.cn/UiMF2lm", Platform::Bilibili),
            ("https://b23.tv/a9P3uZO", Platform::Bilibili),
            ("https://bili.im/w4OlGpA", Platform::Bilibili),
            ("https://live.bilibili.com/6103516?live_from=84001", Platform::Bilibili),
            ("https://www.bilibili.tv/en/video/4794396117045760?s_locale=en_US", Platform::Bilibili),
            ("https://www.bilibili.tv/en/play/1051287", Platform::Bilibili),
            ("https://www.bilibili.com/video/BV1T44FezE7a/", Platform::Bilibili),
            ("https://www.youtube.com/watch?v=vuCJJFdOOrw", Platform::YouTube),
            ("https://www.youtube.com/shorts/r-rLeLMwvpE", Platform::YouTube),
            ("https://youtube.com/shorts/5mdloh263PI?si=LjRKvpJibS6v6UY6", Platform::YouTube),
            ("https://youtu.be/aAkMkVFwAoo?si=6DLR4cwaxSIGIzap", Platform::YouTube),
            ("https://www.instagram.com/tv/Cc50Ti7rGHi", Platform::Instagram),
            ("https://www.instagram.com/reel/DBBEGXpvwNF/?igsh=ZzM3", Platform::Instagram),
            ("https://www.instagram.com/p/DEOGT-qyOwm/", Platform::Instagram),
            ("https://www.instagram.com/stories/re_mini_scene/3535907102071999752", Platform::Instagram),
            ("https://www.facebook.com/share/v/12L6D2pvco7/", Platform::Facebook),
            ("https://www.facebook.com/reel/1355164985501660", Platform::Facebook),
            ("https://m.facebook.com/story.php?story_fbid=pfbid0A&id=1", Platform::Facebook),
            ("https://x.com/yua_mikami/status/1876940930139181424", Platform::Twitter),
            ("https://twitter.com/a/status/1", Platform::Twitter),
            ("https://www.pinterest.com/pin/1477812373602736/", Platform::Pinterest),
            ("https://pin.it/1X4o9EJSU", Platform::Pinterest),
            ("https://www.pinterest.co.uk/pin/1/", Platform::Pinterest),
            ("https://suno.com/s/AQrKvtqiIoeTE9RI", Platform::Suno),
            ("https://suno.com/song/8e94d88d-912d-4bda-a5b4-42d9e04d74c4", Platform::Suno),
            ("https://app.suno.ai/song/b27c29f6-8ab4-47eb-81fd-efb85c848ada", Platform::Suno),
        ];
        for (url, expected) in cases {
            assert_eq!(d(url), expected, "{url}");
        }
    }

    #[test]
    fn look_alike_hosts_are_other() {
        assert_eq!(d("https://youtube.com.evil.test/watch?v=1"), Platform::Other);
        assert_eq!(d("https://notyoutube.com/watch?v=1"), Platform::Other);
        assert_eq!(d("https://x.com.example.org/a"), Platform::Other);
        assert_eq!(d("https://pinterest.example.com/pin/1"), Platform::Other);
        assert_eq!(d("https://vimeo.com/76979871"), Platform::Other);
    }
}
