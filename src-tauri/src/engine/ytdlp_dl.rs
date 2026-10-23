//! yt-dlp-managed downloads for fragmented protocols (HLS / DASH segments).

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

use super::http::Counters;
use crate::error::{AppError, AppResult};
use crate::extract::ytdlp::{base_args, classify_error};
use crate::sys::process::{command, spawn_contained};
use crate::sys::tools::Tools;

const MARK: &str = "RBTP ";
const PATH_MARK: &str = "RBTF ";

/// Downloads `format_spec` of `url` to `stem_path` + extension chosen by yt-dlp
/// (or `merge_ext` when streams are merged). Returns the final file path.
pub async fn download(tools: &Tools, url: &str, format_spec: &str, stem_path: &Path, merge_ext: Option<&str>, counters: &Counters, cancel: &CancellationToken) -> AppResult<PathBuf> {
    let exe = tools.ytdlp.as_deref().ok_or_else(|| AppError::Tool("yt-dlp is missing".into()))?;
    // Output templates treat % as a field marker; escape it in the literal path.
    let template = format!("{}.%(ext)s", stem_path.display().to_string().replace('%', "%%"));
    let mut args = base_args(tools);
    args.extend([
        "-f".into(),
        format_spec.into(),
        "-o".into(),
        template,
        "--newline".into(),
        "--progress".into(),
        "--no-mtime".into(),
        "-N".into(),
        "4".into(),
        "--progress-template".into(),
        format!("download:{MARK}%(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s"),
        "--print".into(),
        format!("after_move:{PATH_MARK}%(filepath)s"),
    ]);
    if let Some(ext) = merge_ext {
        args.extend(["--merge-output-format".into(), ext.into()]);
    }
    args.extend(["--".into(), url.into()]);

    let mut proc = spawn_contained(command(exe, &args)).map_err(|e| AppError::Tool(format!("couldn't start yt-dlp: {e}")))?;
    let killer = proc.killer();
    let stdout = proc.child.stdout.take().expect("piped");
    let stderr = proc.child.stderr.take().expect("piped");

    let mut final_path: Option<PathBuf> = None;
    let mut err_tail: Vec<String> = Vec::new();
    let (mut base, mut last_done, mut last_total) = (0u64, 0u64, 0u64);

    let mut out_lines = BufReader::new(stdout).lines();
    let mut err_lines = BufReader::new(stderr).lines();
    let (mut out_open, mut err_open) = (true, true);
    while out_open || err_open {
        let line = tokio::select! {
            l = out_lines.next_line(), if out_open => match l { Ok(Some(l)) => l, _ => { out_open = false; continue } },
            l = err_lines.next_line(), if err_open => match l { Ok(Some(l)) => l, _ => { err_open = false; continue } },
            _ = cancel.cancelled() => { killer.kill(); return Err(AppError::Cancelled); }
        };
        if let Some(rest) = line.strip_prefix(MARK) {
            let mut it = rest.split_whitespace().map(|x| x.parse::<f64>().ok().map(|f| f as u64));
            let done = it.next().flatten().unwrap_or(0);
            let total = it.next().flatten().or(it.next().flatten()).unwrap_or(0);
            if done + 1024 < last_done {
                // A new stream started (video then audio): keep the finished one's bytes.
                base += last_total.max(last_done);
            }
            last_done = done;
            last_total = total;
            counters.done.store(base + done, Ordering::Relaxed);
            counters.total.store(if total > 0 { base + total } else { 0 }, Ordering::Relaxed);
        } else if let Some(p) = line.strip_prefix(PATH_MARK) {
            final_path = Some(PathBuf::from(p.trim()));
        } else if line.starts_with("ERROR") || line.starts_with("WARNING") {
            err_tail.push(line);
            if err_tail.len() > 8 {
                err_tail.remove(0);
            }
        }
    }
    let status = proc.child.wait().await.map_err(|e| AppError::Tool(e.to_string()))?;
    if !status.success() {
        return Err(classify_error(&err_tail.join("\n"), Some(url)));
    }
    let path = final_path.ok_or_else(|| AppError::Tool("yt-dlp finished without reporting the output file".into()))?;
    let size = tokio::fs::metadata(&path).await?.len();
    counters.done.store(size, Ordering::Relaxed);
    counters.total.store(size, Ordering::Relaxed);
    Ok(path)
}
