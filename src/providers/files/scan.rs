//! File index — walks the configured directories (default: user home) and
//! produces a flat, capped, alphabetically sorted entry list.
//!
//! Exclusions guard against indexing junk: hidden entries, common build/
//! dependency folders, and Windows AppData. Depth and count caps keep the
//! index bounded.

use std::path::Path;

/// Maximum directory depth walked (0 = root only).
const MAX_DEPTH: usize = 8;
/// Hard cap on indexed entries.
const MAX_ENTRIES: usize = 25_000;

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
    ".config",
    ".vscode",
    ".idea",
    ".local",
    "AppData",
    "$Recycle.Bin",
    "System Volume Information",
];

fn is_excluded_dir(name: &str) -> bool {
    EXCLUDED_DIRS.iter().any(|d| d.eq_ignore_ascii_case(name))
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
        if is_hidden(&name) || is_excluded_dir(&name) {
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

/// Resolve the default roots: the user home folder.
fn home_roots() -> Vec<String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if home.is_empty() {
        Vec::new()
    } else {
        vec![home]
    }
}

/// Index the configured directories (or the home folder when none are
/// configured), deduped by path, capped, sorted by name.
pub(crate) fn scan_files(root_dirs: &[String]) -> Vec<super::FileEntry> {
    let roots: Vec<String> = if root_dirs.is_empty() {
        home_roots()
    } else {
        root_dirs.to_vec()
    };

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
}
