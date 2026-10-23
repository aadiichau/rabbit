pub mod bilibili;
pub mod fixups;
pub mod kuaishou;
pub mod model;
pub mod normalize;
pub mod pinterest;
pub mod rednote;
pub mod suno;
pub mod web;
pub mod x;
pub mod ytdlp;

use serde_json::Value;
use url::Url;

use crate::engine::platform::Platform;
use crate::error::AppResult;

/// Rabbit's own extractors, for platforms where yt-dlp has no working extractor
/// (docs/ARCHITECTURE.md, "Extraction chain"). Each returns yt-dlp-shaped JSON for the normalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Native {
    Bilibili,
    RedNote,
    Kuaishou,
    X,
    Suno,
    Pinterest,
}

impl Native {
    /// The native extractor that claims this address, if any.
    pub fn for_url(platform: Platform, url: &Url) -> Option<Native> {
        match platform {
            Platform::Bilibili if bilibili::handles(url) => Some(Native::Bilibili),
            Platform::RedNote if rednote::handles(url) => Some(Native::RedNote),
            Platform::Kuaishou if kuaishou::handles(url) => Some(Native::Kuaishou),
            Platform::Twitter if x::handles(url) => Some(Native::X),
            Platform::Suno if suno::handles(url) => Some(Native::Suno),
            Platform::Pinterest if pinterest::handles(url) => Some(Native::Pinterest),
            _ => None,
        }
    }

    /// Whether yt-dlp is worth trying when this extractor fails. yt-dlp has Bilibili, Xiaohongshu
    /// and Twitter extractors, but none for Kuaishou and it refuses suno.com.
    pub fn ytdlp_fallback(self) -> bool {
        !matches!(self, Native::Kuaishou | Native::Suno)
    }

    /// `Ok(None)`: the address turned out not to be one this extractor reads (a short link
    /// that led elsewhere), so yt-dlp should have it.
    /// `bilibili_session`: the user's own Bilibili session, when they switched it on.
    pub async fn extract(self, w: &web::Web, url: &Url, bilibili_session: Option<&web::SessionCookies>) -> AppResult<Option<Value>> {
        match self {
            Native::Bilibili => bilibili::extract(w, url, bilibili_session).await,
            Native::RedNote => rednote::extract(w, url).await,
            Native::Kuaishou => kuaishou::extract(w, url).await.map(Some),
            Native::X => x::extract(w, url).await,
            Native::Suno => suno::extract(w, url).await.map(Some),
            Native::Pinterest => pinterest::extract(w, url).await,
        }
    }
}
