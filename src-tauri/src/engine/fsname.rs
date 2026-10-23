use std::path::{Path, PathBuf};

const MAX_STEM_CHARS: usize = 150;
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "CONIN$", "CONOUT$",
];

/// Turns untrusted media titles into a safe Windows filename stem (no extension).
pub fn sanitize_stem(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if c.is_control() => ' ',
            // Bidi overrides can disguise extensions in Explorer.
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => ' ',
            c => c,
        })
        .collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > MAX_STEM_CHARS {
        s = s.chars().take(MAX_STEM_CHARS).collect();
    }
    let s = s.trim_end_matches(['.', ' ']).trim_start_matches(['.', ' ']).to_string();
    if s.is_empty() {
        return "download".to_string();
    }
    let base = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&base.as_str()) { format!("_{s}") } else { s }
}

/// Returns `dir/stem.ext`, or `dir/stem (n).ext` for the first n that doesn't exist yet.
/// `reserved` lets callers avoid names already claimed by in-flight tasks.
pub fn unique_path(dir: &Path, stem: &str, ext: &str, reserved: &dyn Fn(&Path) -> bool) -> PathBuf {
    let ext = ext.trim_start_matches('.');
    let make = |n: u32| {
        let name = if n <= 1 { format!("{stem}.{ext}") } else { format!("{stem} ({n}).{ext}") };
        dir.join(name)
    };
    let mut n = 1;
    loop {
        let p = make(n);
        if !p.exists() && !reserved(&p) {
            return p;
        }
        n += 1;
    }
}

/// Like `unique_path` but for a folder (no extension).
pub fn unique_dir(dir: &Path, stem: &str, reserved: &dyn Fn(&Path) -> bool) -> PathBuf {
    let mut n = 1;
    loop {
        let p = if n == 1 { dir.join(stem) } else { dir.join(format!("{stem} ({n})")) };
        if !p.exists() && !reserved(&p) {
            return p;
        }
        n += 1;
    }
}

/// True when `candidate` stays inside `dir` after normalisation (guards against crafted names).
pub fn is_within(dir: &Path, candidate: &Path) -> bool {
    candidate.parent().is_some_and(|p| p == dir) && candidate.file_name().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_illegal_characters() {
        assert_eq!(sanitize_stem(r#"a<b>c:d"e/f\g|h?i*j"#), "a b c d e f g h i j");
        assert_eq!(sanitize_stem("line\nbreak\ttab"), "line break tab");
    }

    #[test]
    fn reserved_names_and_dots() {
        assert_eq!(sanitize_stem("CON"), "_CON");
        assert_eq!(sanitize_stem("nul.txt"), "_nul.txt");
        assert_eq!(sanitize_stem("...hidden..."), "hidden");
        assert_eq!(sanitize_stem("   "), "download");
        assert_eq!(sanitize_stem("../../etc/passwd"), "etc passwd");
    }

    #[test]
    fn keeps_unicode_and_truncates() {
        assert_eq!(sanitize_stem("刚出新手村，遇到了顶级魅魔 🐰"), "刚出新手村，遇到了顶级魅魔 🐰");
        let long = "é".repeat(400);
        assert_eq!(sanitize_stem(&long).chars().count(), MAX_STEM_CHARS);
        assert_eq!(sanitize_stem("evil\u{202E}4pm.exe"), "evil 4pm.exe");
    }

    #[test]
    fn unique_path_appends_counter() {
        let dir = tempfile::tempdir().unwrap();
        let none = |_: &Path| false;
        let a = unique_path(dir.path(), "clip", "mp4", &none);
        assert_eq!(a.file_name().unwrap(), "clip.mp4");
        std::fs::write(&a, b"x").unwrap();
        let b = unique_path(dir.path(), "clip", "mp4", &none);
        assert_eq!(b.file_name().unwrap(), "clip (2).mp4");
        let taken = b.clone();
        let c = unique_path(dir.path(), "clip", "mp4", &move |p: &Path| p == taken);
        assert_eq!(c.file_name().unwrap(), "clip (3).mp4");
        assert!(is_within(dir.path(), &c));
    }
}
