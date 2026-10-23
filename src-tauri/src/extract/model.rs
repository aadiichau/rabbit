use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::engine::platform::Platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Video,
    Audio,
    Image,
    Gallery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionGroup {
    Video,
    Audio,
    Image,
    Subtitle,
}

/// What the UI shows for one downloadable choice. Built only from real extracted formats.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOption {
    pub id: String,
    pub group: OptionGroup,
    pub label: String,
    pub container: String,
    pub codec: String,
    pub fps: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
    pub size_estimated: bool,
    pub note: String,
    /// True when Rabbit must transcode (e.g. MP3); false for stream copy / remux.
    pub conversion: bool,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub analysis_id: String,
    pub source_url: String,
    pub platform: Platform,
    pub platform_name: &'static str,
    pub extractor: String,
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub upload_date: Option<String>,
    pub view_count: Option<u64>,
    pub thumbnail: Option<String>,
    pub kind: MediaKind,
    pub item_count: Option<usize>,
    pub options: Vec<DownloadOption>,
    pub elapsed_ms: u64,
    /// Set when a signed-in session was involved, e.g. "your Bilibili session (name)", or a
    /// guest fallback explained ("guest qualities — your Bilibili session has expired…").
    pub session: Option<String>,
}

/// How one stream is fetched. Never sent to the renderer: URLs and headers stay in Rust.
#[derive(Debug, Clone)]
pub enum Transfer {
    /// `mirrors`: the same file on other hosts (the source's backup URLs), tried in order when
    /// `url` fails.
    Native { url: String, headers: BTreeMap<String, String>, mirrors: Vec<String> },
    /// Fragmented protocols (HLS/DASH segments) are delegated to yt-dlp with this format spec.
    YtDlp { format_spec: String },
}

#[derive(Debug, Clone)]
pub struct StreamPlan {
    pub transfer: Transfer,
    pub ext: String,
    pub size: Option<u64>,
    /// Sub-path inside a gallery folder; `None` for single-file downloads.
    pub item_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Post {
    None,
    /// Stream-copy video + audio into `out_ext`.
    Merge,
    /// Transcode the single audio stream to MP3.
    ToMp3,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub streams: Vec<StreamPlan>,
    pub post: Post,
    pub out_ext: String,
    pub expected_duration: Option<f64>,
    pub gallery: bool,
    /// Some(true): the source declares audio, so verification requires it. Some(false): known
    /// silent. None: the extractor didn't say, so verification doesn't insist either way.
    pub expect_audio: Option<bool>,
    /// The resolution the option promises (short side). Verification rejects a file that comes out
    /// clearly smaller, so a mislabelled stream can't pass as the quality the user picked.
    pub min_short_side: Option<u32>,
}
