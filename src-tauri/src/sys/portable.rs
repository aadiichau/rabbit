//! Portable mode. The portable ZIP ships a `portable.txt` marker next to `rabbit.exe`; when it's there, Rabbit
//! keeps its database, settings, yt-dlp updates and WebView2 profile in a `data` folder beside the exe instead
//! of the per-user AppData folders, so the whole folder can move between PCs or live on a USB drive.
//! Downloads still default to the user's Downloads\Rabbit folder.

use std::path::{Path, PathBuf};

pub const MARKER: &str = "portable.txt";

/// The portable data folder, if this copy of Rabbit is portable and the folder is writable.
pub fn data_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    data_dir_for(exe.parent()?)
}

fn data_dir_for(exe_dir: &Path) -> Option<PathBuf> {
    if !exe_dir.join(MARKER).is_file() {
        return None;
    }
    let data = exe_dir.join("data");
    match std::fs::create_dir_all(&data) {
        Ok(()) => Some(data),
        Err(e) => {
            // e.g. unpacked into a read-only folder: behave like an installed copy rather than fail to start.
            tracing::warn!(error = %e, dir = %data.display(), "portable data folder isn't writable; using AppData");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_switches_data_to_the_exe_folder() {
        let dir = std::env::temp_dir().join(format!("rabbit-portable-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(data_dir_for(&dir), None);
        std::fs::write(dir.join(MARKER), "").unwrap();
        assert_eq!(data_dir_for(&dir), Some(dir.join("data")));
        assert!(dir.join("data").is_dir());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
