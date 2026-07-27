#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::database::Database;

#[derive(Clone)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub icon_rgba: Option<(Vec<u8>, u32, u32)>,
    pub score: f64,
}

#[derive(Clone)]
pub struct InstalledApp {
    pub name: String,
    pub path: String,
    pub icon_rgba: Option<(Vec<u8>, u32, u32)>,
}

#[derive(Clone)]
pub struct SearchEngine {
    config: Arc<Config>,
    db: Arc<Database>,
    apps: Arc<Mutex<Vec<InstalledApp>>>,
}

impl SearchEngine {
    pub fn new(config: &Config, db: Arc<Database>) -> Self {
        let engine = Self {
            config: Arc::new(config.clone()),
            db,
            apps: Arc::new(Mutex::new(Vec::new())),
        };
        engine.refresh_apps();
        engine
    }

    pub fn refresh_apps(&self) {
        let mut apps = Vec::new();
        #[cfg(target_os = "windows")]
        {
            let start_menu_dirs = vec![
                std::env::var("ProgramData")
                    .map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
                std::env::var("APPDATA")
                    .map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
            ];
            let cache_dir = icon_cache_dir();
            for dir in start_menu_dirs.into_iter().filter_map(|d| d.ok()) {
                for entry in walkdir::WalkDir::new(&dir).follow_links(true).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().map(|e| e == "lnk").unwrap_or(false) {
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        if !name.is_empty() && !name.starts_with('.') {
                            let icon_rgba = extract_icon_from_lnk(path, &cache_dir);
                            apps.push(InstalledApp {
                                name,
                                path: path.to_string_lossy().to_string(),
                                icon_rgba,
                            });
                        }
                    }
                }
            }
            apps.sort_by(|a, b| a.name.cmp(&b.name));
        }
        if let Ok(mut guard) = self.apps.lock() {
            *guard = apps;
        }
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let q = query.trim().to_lowercase();

        if q.is_empty() {
            return self.recommendations(12);
        }

        let mut results: Vec<SearchResult> = Vec::new();

        // Calculator
        if q.chars().any(|c| c.is_ascii_digit()
            || matches!(c, '+' | '-' | '*' | '/' | 'x' | '÷' | '(' | ')'))
        {
            let expr = q.replace('x', "*").replace('÷', "/");
            if let Ok(val) = evalexpr::eval(&expr) {
                results.push(SearchResult {
                    title: format!("= {}", val),
                    subtitle: format!("Calc: {}", query),
                    kind: "calc".into(),
                    icon_rgba: None,
                    score: 1000.0,
                });
            }
        }

        // Emoji search
        if q.starts_with("emoji") || q.starts_with(":") {
            let term = q.trim_start_matches("emoji").trim().trim_start_matches(':').trim();
            for emoji in emojis::iter() {
                let name = emoji.name().to_lowercase();
                let codes: Vec<String> = emoji.shortcodes().map(|s| s.to_string()).collect();
                if term.is_empty() || name.contains(term) || codes.iter().any(|c| c.contains(term)) {
                    results.push(SearchResult {
                        title: format!("{}  {}", emoji.as_str(),
                            codes.first().map(|c| format!(":{}:", c)).unwrap_or_default()),
                        subtitle: emoji.name().into(),
                        kind: "emoji".into(),
                        icon_rgba: None,
                        score: 500.0 - (results.len() as f64),
                    });
                    if results.len() > 20 { break; }
                }
            }
        }

        // Clipboard history
        if q == "cbhist" || q.starts_with("clip") {
            let entries = self.db.load_clipboard(20);
            for (text, ts) in &entries {
                let preview: String = text.lines().next().unwrap_or(text).chars().take(80).collect();
                results.push(SearchResult {
                    title: preview,
                    subtitle: format!("Clipboard · {}", ts),
                    kind: "clipboard".into(),
                    icon_rgba: None,
                    score: 200.0,
                });
            }
        }

        // App search with scored fuzzy matching
        {
            let apps = match self.apps.lock() {
                Ok(a) => a,
                Err(_) => return results,
            };
            for app in apps.iter() {
                if let Some(score) = fuzzy_score(&q, &app.name) {
                    let frecency = self.db.frecency_score(&app.name);
                    let boost = 1.0 + (frecency * 5.0).min(2.0);
                    results.push(SearchResult {
                        title: app.name.clone(),
                        subtitle: "App".into(),
                        kind: "app".into(),
                        icon_rgba: app.icon_rgba.clone(),
                        score: score * boost,
                    });
                }
            }
        }

        // Sort by score descending, then alphabetically
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.title.cmp(&b.title))
        });

        // Web search suggestion at the bottom
        results.push(SearchResult {
            title: format!("Search web for \"{}\"", query),
            subtitle: config_or_default(&self.config.search_url,
                "https://duckduckgo.com/search?q=%s").replace("%s", query),
            kind: "websearch".into(),
            icon_rgba: None,
            score: -1.0,
        });

        results
    }

    fn recommendations(&self, limit: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();
        let frecency = self.db.top_frecency(limit);
        let apps = match self.apps.lock() {
            Ok(a) => a,
            Err(_) => return results,
        };

        for (name, _count, _last) in &frecency {
            if let Some(app) = apps.iter().find(|a| a.name == *name) {
                results.push(SearchResult {
                    title: app.name.clone(),
                    subtitle: "App".into(),
                    kind: "app".into(),
                    icon_rgba: app.icon_rgba.clone(),
                    score: 200.0,
                });
            }
        }

        for app in apps.iter() {
            if results.len() >= limit { break; }
            if !results.iter().any(|r| r.title == app.name) {
                results.push(SearchResult {
                    title: app.name.clone(),
                    subtitle: "App".into(),
                    kind: "app".into(),
                    icon_rgba: app.icon_rgba.clone(),
                    score: 100.0,
                });
            }
        }

        results
    }

    pub fn activate(&self, kind: &str, title: &str, input: &str) {
        match kind {
            "app" => {
                self.db.record_launch(title);
                let apps = match self.apps.lock() {
                    Ok(a) => a,
                    Err(_) => return,
                };
                if let Some(app) = apps.iter().find(|a| a.name == title) {
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", &app.path])
                        .spawn();
                }
            }
            "websearch" => {
                let _ = webbrowser::open(&self.config.search_url.replace("%s", input));
            }
            "calc" => {
                let _ = arboard::Clipboard::new()
                    .and_then(|mut c| c.set_text(title.trim_start_matches("= ")));
            }
            "emoji" => {
                if let Some(emoji_char) = title.chars().next() {
                    let _ = arboard::Clipboard::new()
                        .and_then(|mut c| c.set_text(emoji_char.to_string()));
                }
            }
            "clipboard" => {
                let _ = arboard::Clipboard::new()
                    .and_then(|mut c| c.set_text(title));
            }
            _ => {}
        }
    }
}

fn config_or_default(config: &str, default: &str) -> String {
    if config.is_empty() { default.into() } else { config.into() }
}

/// Scored fuzzy match — RustCast/Sublime-like character-level matching.
/// Returns `Some(score)` if all query chars appear in order in name, `None` otherwise.
fn fuzzy_score(query: &str, name: &str) -> Option<f64> {
    if query.is_empty() {
        return None;
    }

    let q = query.as_bytes();
    let t = name.as_bytes();
    let tl = name.to_lowercase();
    let t_lower = tl.as_bytes();

    if q.len() > t.len() {
        return None;
    }

    let mut score = 0.0;
    let mut qi = 0;
    let mut prev_matched = false;
    let mut first_match_pos: Option<usize> = None;

    for (ti, &ch) in t_lower.iter().enumerate() {
        if qi < q.len() && ch == q[qi] {
            qi += 1;

            // Base score for each matched character
            score += 10.0;

            // Bonus for consecutive matches
            if prev_matched {
                score += 15.0;
            }

            // Bonus for matching at word boundary (space, -, _, /, \)
            if ti == 0 || matches!(t[ti - 1], b' ' | b'-' | b'_' | b'/' | b'\\') {
                score += 30.0;
            }

            // Bonus for camelCase boundary
            if ti > 0
                && t[ti].is_ascii_uppercase()
                && t[ti - 1].is_ascii_lowercase()
            {
                score += 20.0;
            }

            // Bonus for matching after a separator
            if ti > 0 && !t[ti - 1].is_ascii_alphanumeric() {
                score += 15.0;
            }

            if first_match_pos.is_none() {
                first_match_pos = Some(ti);
            }

            prev_matched = true;
        } else {
            prev_matched = false;
        }
    }

    if qi != q.len() {
        return None;
    }

    // Bonus for early match (matches closer to start are better)
    if let Some(pos) = first_match_pos {
        if pos == 0 {
            score += 50.0;
        } else {
            score += (1.0 - pos as f64 / t.len() as f64) * 30.0;
        }
    }

    // Penalize for unmatched characters in target
    let matched = qi;
    let unmatched = t.len() - matched;
    score -= unmatched as f64 * 2.0;

    // Normalize: higher score per query char is better
    Some(score / q.len() as f64)
}

fn icon_cache_dir() -> std::path::PathBuf {
    let mut p = if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home)
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        std::path::PathBuf::from(profile)
    } else {
        std::path::PathBuf::from(".")
    };
    p.push(".element");
    p.push("cache");
    p.push("icons");
    let _ = std::fs::create_dir_all(&p);
    p
}

#[cfg(target_os = "windows")]
fn hicon_to_rgba(hicon: isize, size: i32) -> Option<(Vec<u8>, u32, u32)> {
    use std::ptr;
    #[repr(C)]
    struct BITMAPINFOHEADER {
        biSize: u32,
        biWidth: i32,
        biHeight: i32,
        biPlanes: u16,
        biBitCount: u16,
        biCompression: u32,
        biSizeImage: u32,
        biXPelsPerMeter: i32,
        biYPelsPerMeter: i32,
        biClrUsed: u32,
        biClrImportant: u32,
    }

    const BI_RGB: u32 = 0;
    const DIB_RGB_COLORS: u32 = 0;
    const DI_NORMAL: u32 = 3;

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: isize) -> isize;
        fn DeleteDC(hdc: isize) -> i32;
        fn CreateDIBSection(
            hdc: isize,
            pbmi: *const BITMAPINFOHEADER,
            usage: u32,
            ppvBits: *mut *mut u8,
            hSection: isize,
            offset: u32,
        ) -> isize;
        fn SelectObject(hdc: isize, h: isize) -> isize;
        fn DeleteObject(h: isize) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn DrawIconEx(
            hdc: isize,
            xLeft: i32,
            yTop: i32,
            hicon: isize,
            cxWidth: i32,
            cyWidth: i32,
            istepIfAniCur: u32,
            hbrFlickerFreeDraw: isize,
            diFlags: u32,
        ) -> i32;
    }

    unsafe {
        let hdc = CreateCompatibleDC(0);
        if hdc == 0 { return None; }

        let bih = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };

        let mut pixels_ptr: *mut u8 = ptr::null_mut();
        let hbmp = CreateDIBSection(hdc, &bih, DIB_RGB_COLORS, &mut pixels_ptr, 0, 0);
        if hbmp == 0 { DeleteDC(hdc); return None; }

        let old = SelectObject(hdc, hbmp);
        DrawIconEx(hdc, 0, 0, hicon, size, size, 0, 0, DI_NORMAL);
        SelectObject(hdc, old);

        let len = (size * size * 4) as usize;
        let bgra = std::slice::from_raw_parts(pixels_ptr, len).to_vec();

        DeleteObject(hbmp);
        DeleteDC(hdc);

        let mut rgba = Vec::with_capacity(len);
        for pixel in bgra.chunks_exact(4) {
            rgba.push(pixel[2]); // R
            rgba.push(pixel[1]); // G
            rgba.push(pixel[0]); // B
            rgba.push(pixel[3]); // A
        }

        // skip all-white icons (failed extraction)
        let all_white = rgba.chunks_exact(4).all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255);
        if all_white {
            return None;
        }

        Some((rgba, size as u32, size as u32))
    }
}

#[cfg(target_os = "windows")]
fn extract_icon_from_lnk(
    lnk_path: &std::path::Path,
    _cache_dir: &std::path::Path,
) -> Option<(Vec<u8>, u32, u32)> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    const SHGFI_ICON: u32 = 0x00000100;
    const SHGFI_SMALLICON: u32 = 0x00000001;

    #[repr(C)]
    struct SHFILEINFOW {
        hIcon: isize,
        iIcon: i32,
        dwAttributes: u32,
        szDisplayName: [u16; 260],
        szTypeName: [u16; 80],
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHGetFileInfoW(
            pszPath: *const u16,
            dwFileAttributes: u32,
            psfi: *mut SHFILEINFOW,
            cbFileInfo: u32,
            uFlags: u32,
        ) -> usize;
    }

    let wide: Vec<u16> = OsStr::new(lnk_path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut shfi: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut shfi,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    };
    if ret == 0 || shfi.hIcon == 0 {
        return None;
    }

    let icon = shfi.hIcon;
    let rgba = hicon_to_rgba(icon, 16);
    unsafe { destroy_icon(icon); }
    rgba
}

#[cfg(not(target_os = "windows"))]
fn extract_icon_from_lnk(
    _lnk_path: &std::path::Path,
    _cache_dir: &std::path::Path,
) -> Option<(Vec<u8>, u32, u32)> {
    None
}

#[cfg(target_os = "windows")]
unsafe fn destroy_icon(hicon: isize) {
    #[link(name = "user32")]
    extern "system" {
        fn DestroyIcon(hIcon: isize) -> i32;
    }
    DestroyIcon(hicon);
}
