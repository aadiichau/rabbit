//! FFmpeg / ffprobe steps. Stream copy whenever possible; transcoding only for MP3.

use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::sys::process::{command, spawn_contained};
use crate::sys::tools::Tools;

async fn run(exe: &Path, args: Vec<OsString>, cancel: &CancellationToken, what: &str) -> AppResult<Vec<u8>> {
    let mut proc = spawn_contained(command(exe, &args)).map_err(|e| AppError::Tool(format!("couldn't start {what}: {e}")))?;
    let killer = proc.killer();
    let mut stdout = proc.child.stdout.take().expect("piped");
    let mut stderr = proc.child.stderr.take().expect("piped");
    let work = async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (a, b) = tokio::join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
        a?;
        b?;
        Ok::<_, std::io::Error>((proc.child.wait().await?, out, err))
    };
    let (status, out, err) = tokio::select! {
        r = work => r.map_err(|e| AppError::Tool(format!("{what} I/O error: {e}")))?,
        _ = cancel.cancelled() => { killer.kill(); return Err(AppError::Cancelled); }
    };
    if !status.success() {
        let tail: String = String::from_utf8_lossy(&err).lines().rev().take(3).collect::<Vec<_>>().join(" | ");
        return Err(AppError::Tool(format!("{what} failed: {tail}")));
    }
    Ok(out)
}

fn ffmpeg(tools: &Tools) -> AppResult<&Path> {
    tools.ffmpeg.as_deref().ok_or_else(|| AppError::Tool("FFmpeg is required for this format but isn't available.".into()))
}

/// Stream-copies the first video stream of `video` and the first audio stream of `audio` into `out`.
pub async fn merge(tools: &Tools, video: &Path, audio: &Path, out: &Path, cancel: &CancellationToken) -> AppResult<()> {
    let mut args: Vec<OsString> = ["-hide_banner", "-nostdin", "-loglevel", "error", "-y", "-i"].map(OsString::from).to_vec();
    args.push(video.into());
    args.push("-i".into());
    args.push(audio.into());
    args.extend(["-map", "0:v:0", "-map", "1:a:0", "-c", "copy"].map(OsString::from));
    if out.extension().is_some_and(|e| e == "mp4" || e == "m4a" || e == "mov") {
        args.extend(["-movflags", "+faststart"].map(OsString::from));
    }
    args.push(out.into());
    run(ffmpeg(tools)?, args, cancel, "FFmpeg merge").await.map(|_| ())
}

/// Transcodes the first audio stream to MP3 (LAME VBR quality 2, ~190 kbps).
pub async fn to_mp3(tools: &Tools, input: &Path, out: &Path, cancel: &CancellationToken) -> AppResult<()> {
    let mut args: Vec<OsString> = ["-hide_banner", "-nostdin", "-loglevel", "error", "-y", "-i"].map(OsString::from).to_vec();
    args.push(input.into());
    args.extend(["-vn", "-map", "0:a:0", "-c:a", "libmp3lame", "-q:a", "2"].map(OsString::from));
    args.push(out.into());
    run(ffmpeg(tools)?, args, cancel, "FFmpeg MP3 conversion").await.map(|_| ())
}

#[derive(Debug)]
pub struct Probe {
    pub duration: Option<f64>,
    pub video: usize,
    pub audio: usize,
    /// Coded size of the first video stream (cover art attached to audio files is skipped).
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
}

impl Probe {
    /// The resolution class the file really has: the shorter side, so portrait 720×1280 is 720.
    pub fn short_side(&self) -> Option<u32> {
        Some(self.width?.min(self.height?))
    }
}

pub async fn probe(tools: &Tools, file: &Path, cancel: &CancellationToken) -> AppResult<Probe> {
    let exe = tools.ffprobe.as_deref().ok_or_else(|| AppError::Tool("ffprobe isn't available".into()))?;
    let mut args: Vec<OsString> = ["-v", "error", "-show_entries", "format=duration:stream=codec_type,codec_name,width,height:stream_disposition=attached_pic", "-of", "json"]
        .map(OsString::from)
        .to_vec();
    args.push(file.into());
    let out = run(exe, args, cancel, "ffprobe").await?;
    let v: Value = serde_json::from_slice(&out).map_err(|e| AppError::Verification(format!("unreadable ffprobe output: {e}")))?;
    let streams: Vec<&Value> = v["streams"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let is_cover = |s: &Value| s["disposition"]["attached_pic"].as_i64() == Some(1);
    let videos: Vec<&Value> = streams.iter().copied().filter(|s| s["codec_type"] == "video" && !is_cover(s)).collect();
    let audios: Vec<&Value> = streams.iter().copied().filter(|s| s["codec_type"] == "audio").collect();
    let dim = |k: &str| videos.first().and_then(|s| s[k].as_u64()).map(|x| x as u32).filter(|x| *x > 0);
    let codec = |list: &[&Value]| list.first().and_then(|s| s["codec_name"].as_str()).map(str::to_string);
    Ok(Probe {
        duration: v["format"]["duration"].as_str().and_then(|d| d.parse().ok()),
        video: videos.len(),
        audio: audios.len(),
        width: dim("width"),
        height: dim("height"),
        vcodec: codec(&videos),
        acodec: codec(&audios),
    })
}

/// Checks a finished media file. `expect_video`/`expect_audio` come from the chosen option.
/// `min_short_side` is the resolution the option promised (short side, exact sizes only).
pub async fn verify(
    tools: &Tools,
    file: &Path,
    expected_duration: Option<f64>,
    expect_video: bool,
    expect_audio: bool,
    min_short_side: Option<u32>,
    cancel: &CancellationToken,
) -> AppResult<()> {
    let len = tokio::fs::metadata(file).await?.len();
    if len == 0 {
        return Err(AppError::Verification("the file is empty".into()));
    }
    if tools.ffprobe.is_none() {
        return Ok(());
    }
    let p = probe(tools, file, cancel).await?;
    if expect_video && p.video == 0 {
        return Err(AppError::Verification("no video stream in the result".into()));
    }
    if expect_audio && p.audio == 0 {
        return Err(AppError::Verification("no audio stream in the result".into()));
    }
    // A few pixels of difference are encoder padding (1080 vs 1088), not a different quality.
    if let (Some(want), Some(got)) = (min_short_side.filter(|_| expect_video), p.short_side())
        && f64::from(got) < f64::from(want) * 0.9
    {
        return Err(AppError::Verification(format!("the video is {got}p, but the chosen option promised {want}p")));
    }
    if let (Some(want), Some(got)) = (expected_duration, p.duration) {
        let tolerance = (want * 0.02).max(2.0);
        if (want - got).abs() > tolerance {
            return Err(AppError::Verification(format!("duration is {got:.1} s, expected {want:.1} s")));
        }
    }
    Ok(())
}
