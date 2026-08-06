//! File index — walks the configured directories (default: curated user
//! folders) and produces a flat, capped, alphabetically sorted entry list.
//!
//! Exclusions guard against indexing junk: hidden entries, common build/
//! dependency folders, phone-backup/media dumps, caches, and Windows AppData.
//! Depth and count caps keep the index bounded.

use std::path::Path;

/// Maximum directory depth walked (0 = root only).
const MAX_DEPTH: usize = 14;
/// Hard cap on indexed entries.
const MAX_ENTRIES: usize = 50_000;

/// Directory names (case-insensitive) never indexed.
const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".hg",
    ".svn",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    ".cache",
    ".cargo",
    ".rustup",
    "AppData",
    "$Recycle.Bin",
    "System Volume Information",
    "temp",
    "tmp",
];

/// Substrings (case-insensitive) that disqualify a directory name — catches
/// cache/media/backup/photo trees wherever they live (e.g.
/// `Android/data/.../cache`, `WhatsApp/Media`, `site-packages`,
/// `curseforge/.../libraries`, phone `DCIM` dumps).
const EXCLUDED_DIR_SUBSTRINGS: &[&str] = &[
    "cache",
    "backup",
    "media",
    "libraries",
    "site-packages",
    "dcim",
    "firefox",
];

/// Android's exported photo/_dump folders (`100PINT`, `100MEDIA`, `100ANDRO`)
/// that appear under `Pictures`/`DCIM` on phones copied to the PC.
const EXCLUDED_ANDROID_DUMP_DIRS: &[&str] =
    &["100pint", "100media", "100andro", "100ncfc", "100anni"];

fn is_excluded_dir(name: &str) -> bool {
    if EXCLUDED_DIRS.iter().any(|d| d.eq_ignore_ascii_case(name)) {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    EXCLUDED_DIR_SUBSTRINGS
        .iter()
        .any(|needle| lower.contains(needle))
        || EXCLUDED_ANDROID_DUMP_DIRS.contains(&lower.as_str())
}

/// Version-numbered directories (`0.1.0`, `1.0.32`, `v2.0`) — package/module
/// install trees pollute the index without useful names.
fn is_version_dir(name: &str) -> bool {
    let trimmed = name.trim_start_matches('v').trim_start_matches('V');
    if trimmed.is_empty() {
        return false;
    }
    let mut digits_seen = false;
    let mut other_seen = false;
    for c in trimmed.chars() {
        if c.is_ascii_digit() {
            digits_seen = true;
        } else if matches!(c, '.' | '-' | '_') {
            // separators are fine as long as digits exist elsewhere
        } else {
            other_seen = true;
            break;
        }
    }
    digits_seen && !other_seen && trimmed.contains('.')
}

/// True for hidden entries (leading dot).
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Walk `dir` at relative `depth` (root = 0), collecting entries into `out`.
fn walk(dir: &Path, depth: usize, out: &mut Vec<super::FileEntry>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTRIES {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        if out.len() >= MAX_ENTRIES {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_hidden(&name) || is_excluded_dir(&name) || is_version_dir(&name) {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        out.push(super::FileEntry {
            name,
            path: path.to_string_lossy().into_owned(),
            is_dir,
        });
        if is_dir {
            walk(&path, depth + 1, out);
        }
    }
}

/// Default roots: curated top-level user folders. Indexing the entire home
/// folder would flood the index with toolchains (`miniconda3`, `curseforge`,
/// `.cargo`), so we start from the folders users actually keep files in.
/// Falling back to the home folder when none of them exist.
pub(crate) fn resolve_roots(root_dirs: &[String]) -> Vec<String> {
    if !root_dirs.is_empty() {
        return root_dirs.to_vec();
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if home.is_empty() {
        return Vec::new();
    }
    let home = Path::new(&home);
    let candidates = [
        "Desktop",
        "Documents",
        "Downloads",
        "Pictures",
        "Music",
        "Videos",
    ];
    let mut roots: Vec<String> = Vec::new();
    for name in candidates {
        let dir = home.join(name);
        if dir.is_dir() {
            roots.push(dir.to_string_lossy().into_owned());
        }
    }
    if roots.is_empty() {
        roots.push(home.to_string_lossy().into_owned());
    }
    roots
}

/// Index the configured directories (or the home folder when none are
/// configured), deduped by path, capped, sorted by name.
pub(crate) fn scan_files(root_dirs: &[String]) -> Vec<super::FileEntry> {
    let roots = resolve_roots(root_dirs);

    let mut by_path: std::collections::HashMap<String, super::FileEntry> =
        std::collections::HashMap::new();
    for root in roots {
        let mut batch: Vec<super::FileEntry> = Vec::new();
        walk(Path::new(&root), 0, &mut batch);
        for entry in batch {
            by_path.insert(entry.path.clone(), entry);
        }
        if by_path.len() >= MAX_ENTRIES {
            break;
        }
    }

    let mut entries: Vec<super::FileEntry> = by_path.into_values().collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
    entries.truncate(MAX_ENTRIES);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_junk_dirs_are_excluded() {
        for name in ["node_modules", "target", ".git", "AppData", "$RECYCLE.Bin"] {
            assert!(is_excluded_dir(name), "{name} should be excluded");
            assert!(
                is_excluded_dir(&name.to_uppercase()),
                "{name} case-insensitive"
            );
        }
    }

    #[test]
    fn user_dirs_are_not_excluded() {
        for name in ["Documents", "Downloads", "Projects", "Music"] {
            assert!(!is_excluded_dir(name), "{name} should be indexed");
        }
    }

    #[test]
    fn hidden_entries_are_skipped() {
        assert!(is_hidden(".git"));
        assert!(is_hidden(".DS_Store"));
        assert!(!is_hidden("report.txt"));
    }

    #[test]
    fn version_dirs_are_skipped() {
        assert!(is_version_dir("0.1.0"));
        assert!(is_version_dir("1.0.32"));
        assert!(is_version_dir("v2.0"));
        assert!(!is_version_dir("Documents"));
        assert!(!is_version_dir("100PINT"));
        assert!(!is_version_dir("notes"));
        assert!(!is_version_dir("3d files"));
    }

    #[test]
    fn android_dump_dirs_are_skipped() {
        assert!(is_excluded_dir("100PINT"));
        assert!(is_excluded_dir("100MEDIA"));
        assert!(is_excluded_dir("100pint"));
        assert!(!is_excluded_dir("100th"));
    }

    #[test]
    fn substring_exclusions_are_case_insensitive() {
        assert!(is_excluded_dir("DCIM"));
        assert!(is_excluded_dir("Site-Packages"));
        assert!(is_excluded_dir("Backup"));
        assert!(is_excluded_dir("Media"));
        assert!(!is_excluded_dir("My Documents"));
    }
}
