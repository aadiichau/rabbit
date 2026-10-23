use std::path::{Path, PathBuf};

use serde::Serialize;

use super::updater;

/// Paths to external tools. Sidecars are resolved next to Rabbit.exe (where Tauri places
/// `externalBin` files). Debug builds may fall back to PATH so development works before
/// every sidecar is bundled; release builds never search PATH.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    /// The yt-dlp in use: a verified update when one is active, otherwise the bundled copy.
    pub ytdlp: Option<PathBuf>,
    pub bundled_ytdlp: Option<PathBuf>,
    /// Version of the active updated copy (from its verified manifest); `None` when bundled.
    pub ytdlp_update: Option<updater::Installed>,
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    /// `RUNTIME:PATH` value for yt-dlp's `--js-runtimes` (needed for full YouTube support).
    pub js_runtime: Option<String>,
}

impl Tools {
    pub fn discover() -> Self {
        Self::discover_with(None)
    }

    /// `update_dir` is where verified yt-dlp updates are kept (see `sys::updater`).
    pub fn discover_with(update_dir: Option<&Path>) -> Self {
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let local = |name: &str| exe_dir.as_ref().map(|d| d.join(name)).filter(|p| p.is_file());
        let find = |name: &str| local(name).or_else(|| if cfg!(debug_assertions) { on_path(name) } else { None });

        let js_runtime = local("deno.exe")
            .map(|p| format!("deno:{}", p.display()))
            .or_else(|| local("node.exe").map(|p| format!("node:{}", p.display())))
            .or_else(|| if cfg!(debug_assertions) { on_path("node.exe").map(|p| format!("node:{}", p.display())) } else { None });

        let bundled = find("yt-dlp.exe");
        let mut tools = Tools { ytdlp: bundled.clone(), bundled_ytdlp: bundled, ytdlp_update: None, ffmpeg: find("ffmpeg.exe"), ffprobe: find("ffprobe.exe"), js_runtime };
        if let Some(dir) = update_dir {
            tools.apply_update_dir(dir);
        }
        tools
    }

    /// Re-reads the update folder: uses the verified update if one is active, else the bundled copy.
    pub fn apply_update_dir(&mut self, dir: &Path) {
        match updater::active(dir) {
            Some((installed, path)) => {
                self.ytdlp = Some(path);
                self.ytdlp_update = Some(installed);
            }
            None => {
                self.ytdlp = self.bundled_ytdlp.clone();
                self.ytdlp_update = None;
            }
        }
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(';').map(|d| Path::new(d).join(name)).find(|p| p.is_file())
}
