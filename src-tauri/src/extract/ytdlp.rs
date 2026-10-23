use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::sys::process::{command, spawn_contained};
use crate::sys::tools::Tools;

const ANALYZE_TIMEOUT: Duration = Duration::from_secs(90);

/// Flags every yt-dlp invocation gets: no user config files, no plugins.
pub fn base_args(tools: &Tools) -> Vec<String> {
    let mut a: Vec<String> = vec!["--ignore-config".into(), "--no-plugin-dirs".into(), "--no-colors".into(), "--no-playlist".into()];
    if let Some(rt) = &tools.js_runtime {
        a.push("--js-runtimes".into());
        a.push(rt.clone());
    }
    if let Some(ff) = &tools.ffmpeg {
        a.push("--ffmpeg-location".into());
        a.push(ff.display().to_string());
    }
    a
}

/// Runs `yt-dlp -J` and returns the parsed info dict.
pub async fn dump_json(tools: &Tools, url: &str, cancel: &CancellationToken) -> AppResult<Value> {
    dump_json_with(tools, url, &[], cancel).await
}

/// `dump_json` with extra yt-dlp options placed before the URL.
pub async fn dump_json_with(tools: &Tools, url: &str, extra: &[&str], cancel: &CancellationToken) -> AppResult<Value> {
    let exe = tools.ytdlp.as_deref().ok_or_else(|| AppError::Tool("yt-dlp is missing from Rabbit's installation.".into()))?;
    let mut args = base_args(tools);
    args.extend(extra.iter().map(|s| s.to_string()));
    args.extend(["-J".into(), "--".into(), url.to_string()]);
    run_json(exe, &args, url, cancel).await
}

async fn run_json(exe: &Path, args: &[String], input_url: &str, cancel: &CancellationToken) -> AppResult<Value> {
    let mut proc = spawn_contained(command(exe, args)).map_err(|e| AppError::Tool(format!("couldn't start yt-dlp: {e}")))?;
    let killer = proc.killer();
    let mut stdout = proc.child.stdout.take().expect("piped");
    let mut stderr = proc.child.stderr.take().expect("piped");
    let read = async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (a, b) = tokio::join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
        a?;
        b?;
        let status = proc.child.wait().await?;
        Ok::<_, std::io::Error>((status, out, err))
    };
    let (status, out, err) = tokio::select! {
        r = read => r.map_err(|e| AppError::Tool(format!("yt-dlp I/O error: {e}")))?,
        _ = tokio::time::sleep(ANALYZE_TIMEOUT) => { killer.kill(); return Err(AppError::Network("the site took too long to respond (90 s)".into())); }
        _ = cancel.cancelled() => { killer.kill(); return Err(AppError::Cancelled); }
    };
    let err = String::from_utf8_lossy(&err);
    if !status.success() {
        return Err(classify_error(&err, Some(input_url)));
    }
    serde_json::from_slice(&out).map_err(|e| AppError::Extractor(format!("unexpected yt-dlp output ({e})")))
}

fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(|h| h.trim_start_matches("www.").to_ascii_lowercase())
}

/// Maps yt-dlp's stderr to a user-facing error. Only the last ERROR line is kept as detail.
/// `input_url` lets errors about a *different* address (a link card in a post) be explained.
pub fn classify_error(stderr: &str, input_url: Option<&str>) -> AppError {
    let line = stderr
        .lines()
        .rev()
        .find(|l| l.starts_with("ERROR:"))
        .unwrap_or_else(|| stderr.lines().last().unwrap_or("unknown error"))
        .trim_start_matches("ERROR:")
        .trim()
        .to_string();
    let l = line.to_ascii_lowercase();
    if l.contains("[liability]") || l.contains("will not be supported") {
        AppError::Unsupported("yt-dlp deliberately excludes this website, and Rabbit follows that decision".into())
    } else if l.contains("there is no video in this post") || l.contains("no video could be found") || l.contains("no video formats found") {
        AppError::Unsupported(
            "no video was found in this post. Image-only posts aren't supported yet, and expired share links or posts that need sign-in look the same".into(),
        )
    } else if l.contains("unsupported url") {
        // yt-dlp followed a link card to another site: the post itself has no media.
        let other = line.split("unsupported url:").nth(1).or_else(|| line.split("Unsupported URL:").nth(1)).map(str::trim).and_then(host_of);
        match (other, input_url.and_then(host_of)) {
            // Same site when one host is a subdomain of the other (v.douyin.com → douyin.com).
            (Some(o), Some(i)) if !o.ends_with(&i) && !i.ends_with(&o) => {
                AppError::Unsupported(format!("this post has no downloadable media of its own — it links to {o}"))
            }
            _ => AppError::Unsupported("no extractor recognises this address".into()),
        }
    } else if l.contains("login") || l.contains("sign in") || l.contains("cookies") || l.contains("private") || l.contains("authentication") {
        AppError::LoginRequired(line)
    } else if l.contains("drm") {
        AppError::Unsupported("this media is DRM-protected".into())
    } else if l.contains("http error 404") || l.contains("not found") || l.contains("unavailable") || l.contains("removed") {
        AppError::Extractor(format!("the media is unavailable — {line}"))
    } else if l.contains("nonetype") || l.contains("object is not subscriptable") || l.contains("keyerror") || l.contains("typeerror") {
        // A Python exception inside the extractor: the site changed under it.
        AppError::Extractor(format!("yt-dlp's reader for this site hit an internal error, which usually means the site changed. Check for a yt-dlp update in Settings → Tools ({line})"))
    } else if l.contains("timed out") || l.contains("unable to download webpage") || l.contains("connection") || l.contains("getaddrinfo") {
        AppError::Network(line)
    } else {
        AppError::Extractor(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_errors() {
        assert!(matches!(classify_error("ERROR: Unsupported URL: https://example.com/", Some("https://example.com/")), AppError::Unsupported(_)));
        assert!(matches!(
            classify_error("WARNING: x\nERROR: [Instagram] abc: Requested content is not available, rate-limit reached or login required. Use --cookies", None),
            AppError::LoginRequired(_)
        ));
        assert!(matches!(classify_error("ERROR: [youtube] x: Video unavailable", None), AppError::Extractor(_)));
        assert!(matches!(classify_error("ERROR: Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>", None), AppError::Network(_)));
    }

    #[test]
    fn explains_phase6_cases() {
        // Observed for an X post whose only link is a shop page.
        let e = classify_error("ERROR: Unsupported URL: https://lily-brw.com/Form/Product/ProductList.aspx?shop=0", Some("https://x.com/yua_mikami/status/1876940930139181424"));
        assert!(e.to_string().contains("links to lily-brw.com"), "{e}");
        // Observed for suno.com.
        let e = classify_error("ERROR: [Liability] This website is not supported and will not be supported.", Some("https://suno.com/song/x"));
        assert!(matches!(e, AppError::Unsupported(_)) && e.to_string().contains("deliberately excludes"));
        // Observed for an Instagram image post.
        let e = classify_error("ERROR: [Instagram] DEOGT-qyOwm: There is no video in this post", None);
        assert!(e.to_string().contains("Image-only posts"));
        // Observed for v.douyin.com: a redirect within the same site is not "a link to another site".
        let e = classify_error("ERROR: Unsupported URL: https://www.douyin.com/video/7310000000000000000", Some("https://v.douyin.com/8Tdgeu/"));
        assert!(!e.to_string().contains("links to"), "{e}");
        // Observed for bilibili.tv with yt-dlp 2026.10.21.
        let e = classify_error("ERROR: 'NoneType' object is not subscriptable", Some("https://www.bilibili.tv/en/video/4794396117045760"));
        assert!(e.to_string().contains("internal error"), "{e}");
    }
}
