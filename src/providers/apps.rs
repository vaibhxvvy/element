use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::app::SearchResult;
use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider};

#[derive(Clone)]
struct InstalledApp {
    name: String,
    path: String,
    icon_rgba: Option<(Vec<u8>, u32, u32)>,
}

pub struct AppsProvider {
    apps: Arc<Mutex<Vec<InstalledApp>>>,
}

impl AppsProvider {
    pub fn new() -> Self {
        let provider = Self {
            apps: Arc::new(Mutex::new(Vec::new())),
        };
        provider.refresh();
        provider
    }

    fn recommendations(&self, ctx: &SearchContext, limit: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();
        let frecency = ctx.db.top_frecency(limit);
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
                    provider_id: "apps".into(),
                    icon_rgba: app.icon_rgba.clone(),
                    score: 200.0,
                });
            }
        }

        for app in apps.iter() {
            if results.len() >= limit {
                break;
            }
            if !results.iter().any(|r| r.title == app.name) {
                results.push(SearchResult {
                    title: app.name.clone(),
                    subtitle: "App".into(),
                    kind: "app".into(),
                    provider_id: "apps".into(),
                    icon_rgba: app.icon_rgba.clone(),
                    score: 100.0,
                });
            }
        }

        results
    }
}

impl SearchProvider for AppsProvider {
    fn id(&self) -> &'static str {
        "apps"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn should_run(&self, _query: &str) -> bool {
        true
    }

    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let q = query.trim().to_lowercase();

        if q.is_empty() {
            return self.recommendations(ctx, 12);
        }

        let apps = match self.apps.lock() {
            Ok(a) => a,
            Err(_) => return Vec::new(),
        };

        let mut results: Vec<SearchResult> = Vec::new();
        for app in apps.iter() {
            if let Some(score) = fuzzy_score(&q, &app.name) {
                let frecency = ctx.db.frecency_score(&app.name);
                let boost = 1.0 + (frecency * 5.0).min(2.0);
                results.push(SearchResult {
                    title: app.name.clone(),
                    subtitle: "App".into(),
                    kind: "app".into(),
                    provider_id: "apps".into(),
                    icon_rgba: app.icon_rgba.clone(),
                    score: score * boost,
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.title.cmp(&b.title))
        });

        results
    }

    fn activate(
        &self,
        ctx: &SearchContext,
        result: &SearchResult,
    ) -> Result<(), ElementError> {
        ctx.db.record_launch(&result.title);
        let apps = match self.apps.lock() {
            Ok(a) => a,
            Err(_) => return Err(ElementError::Other("apps lock poisoned".into())),
        };
        if let Some(app) = apps.iter().find(|a| a.name == result.title) {
            std::process::Command::new("cmd")
                .args(["/c", "start", "", &app.path])
                .spawn()
                .map_err(|e| ElementError::Io(e))?;
            Ok(())
        } else {
            Err(ElementError::Other(format!("app '{}' not found", result.title)))
        }
    }

    fn refresh(&self) {
        let mut apps = Vec::new();
        #[cfg(target_os = "windows")]
        {
            let start_menu_dirs = vec![
                std::env::var("ProgramData").map(|p| {
                    format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)
                }),
                std::env::var("APPDATA").map(|p| {
                    format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)
                }),
            ];
            let cache_dir = icon_cache_dir();
            for dir in start_menu_dirs.into_iter().filter_map(|d| d.ok()) {
                for entry in walkdir::WalkDir::new(&dir)
                    .follow_links(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path();
                    if path.extension().map(|e| e == "lnk").unwrap_or(false) {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        if !name.is_empty() && !name.starts_with('.') {
                            let icon_rgba = cached_icon(path, &cache_dir);
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
}

// ---------------------------------------------------------------------------
// Fuzzy scorer — RustCast/Sublime-like character-level matching
// ---------------------------------------------------------------------------

fn fuzzy_score(query: &str, name: &str) -> Option<f64> {
    if query.is_empty() {
        return None;
    }

    let ql = query.to_lowercase();
    let q = ql.as_bytes();
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
            score += 10.0;

            if prev_matched {
                score += 15.0;
            }
            if ti == 0 || matches!(t[ti - 1], b' ' | b'-' | b'_' | b'/' | b'\\') {
                score += 30.0;
            }
            if ti > 0 && t[ti].is_ascii_uppercase() && t[ti - 1].is_ascii_lowercase() {
                score += 20.0;
            }
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

    if let Some(pos) = first_match_pos {
        if pos == 0 {
            score += 50.0;
        } else {
            score += (1.0 - pos as f64 / t.len() as f64) * 30.0;
        }
    }

    let unmatched = t.len() - qi;
    score -= unmatched as f64 * 2.0;

    Some(score / q.len() as f64)
}

// ---------------------------------------------------------------------------
// Icon extraction & caching
// ---------------------------------------------------------------------------

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

fn icon_cache_path(cache_dir: &Path, lnk_path: &Path) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    lnk_path.hash(&mut hasher);
    cache_dir.join(format!("{:016x}.png", hasher.finish()))
}

fn load_icon_cache(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::open(path).ok()?;
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w, h))
}

fn save_icon_cache(cache_dir: &Path, lnk_path: &Path, rgba: &[u8], w: u32, h: u32) {
    let cache_path = icon_cache_path(cache_dir, lnk_path);
    let _ = image::save_buffer(&cache_path, rgba, w, h, image::ColorType::Rgba8);
}

fn cached_icon(lnk_path: &Path, cache_dir: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let cache_path = icon_cache_path(cache_dir, lnk_path);
    if cache_path.exists() {
        if let Some(icon) = load_icon_cache(&cache_path) {
            return Some(icon);
        }
    }

    // Try to find a high-quality icon from the app's installation directory
    if let Some(info) = resolve_lnk_strings(lnk_path) {
        if !info.working_dir.is_empty() {
            if let Some(icon) = find_icon_from_app_dir(&info.working_dir) {
                save_icon_cache(cache_dir, lnk_path, &icon.0, icon.1, icon.2);
                return Some(icon);
            }
        }
        if !info.icon_location.is_empty() {
            let icon_path = Path::new(&info.icon_location);
            if icon_path.is_file() {
                if let Some(icon) = load_icon_file(icon_path) {
                    save_icon_cache(cache_dir, lnk_path, &icon.0, icon.1, icon.2);
                    return Some(icon);
                }
            }
        }
    }

    if let Some(icon) = extract_icon_from_lnk(lnk_path) {
        save_icon_cache(cache_dir, lnk_path, &icon.0, icon.1, icon.2);
        Some(icon)
    } else {
        None
    }
}

struct LnkStrings {
    working_dir: String,
    icon_location: String,
}

fn resolve_lnk_strings(lnk_path: &Path) -> Option<LnkStrings> {
    let data = std::fs::read(lnk_path).ok()?;
    if data.len() < 76 {
        return None;
    }
    if &data[0..4] != b"\x4C\x00\x00\x00" {
        return None;
    }

    let flags = u32::from_le_bytes(data[0x14..0x18].try_into().ok()?);
    let mut offset: usize = 76;

    // LinkTargetIDList
    if flags & 1 != 0 {
        let sz = u16::from_le_bytes(data.get(offset..offset + 2)?.try_into().ok()?) as usize;
        offset += 2 + sz;
    }

    // LinkInfo
    if flags & 2 != 0 {
        let sz = u32::from_le_bytes(data.get(offset + 4..offset + 8)?.try_into().ok()?) as usize;
        offset += sz;
    }

    // StringData
    let has_name = flags & 4 != 0;
    let has_relpath = flags & 8 != 0;
    let has_workdir = flags & 0x10 != 0;
    let has_args = flags & 0x20 != 0;
    let has_iconloc = flags & 0x40 != 0;

    let mut working_dir = String::new();
    let mut icon_location = String::new();

    if has_name {
        offset = skip_lnk_string(&data, offset)?;
    }
    if has_relpath {
        offset = skip_lnk_string(&data, offset)?;
    }
    if has_workdir {
        let (new_off, s) = read_lnk_string(&data, offset)?;
        working_dir = s;
        offset = new_off;
    }
    if has_args {
        offset = skip_lnk_string(&data, offset)?;
    }
    if has_iconloc {
        let (_, s) = read_lnk_string(&data, offset)?;
        icon_location = s;
    }

    Some(LnkStrings {
        working_dir,
        icon_location,
    })
}

fn read_lnk_string(data: &[u8], offset: usize) -> Option<(usize, String)> {
    let count = u16::from_le_bytes(data.get(offset..offset + 2)?.try_into().ok()?) as usize;
    if count < 1 {
        return None;
    }
    let byte_len = count * 2;
    if offset + 2 + byte_len > data.len() {
        return None;
    }
    let raw = &data[offset + 2..offset + 2 + byte_len - 2];
    let u16s: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let s = String::from_utf16(&u16s).ok()?;
    Some((offset + 2 + byte_len, s))
}

fn skip_lnk_string(data: &[u8], offset: usize) -> Option<usize> {
    let count = u16::from_le_bytes(data.get(offset..offset + 2)?.try_into().ok()?) as usize;
    Some(offset + 2 + count * 2)
}

fn find_icon_from_app_dir(app_dir: &str) -> Option<(Vec<u8>, u32, u32)> {
    let dir = Path::new(app_dir);
    if !dir.is_dir() {
        return None;
    }

    let common = [
        "icon.png",
        "logo.png",
        "app_icon.png",
        "icon.ico",
        "logo.ico",
        "Icon.png",
        "Logo.png",
    ];
    for name in &common {
        let p = dir.join(name);
        if p.is_file() {
            if let Some(icon) = load_icon_file(&p) {
                return Some(icon);
            }
        }
    }

    let subdirs = [
        "assets",
        "assets\\img",
        "assets\\images",
        "res",
        "resources",
        "resources\\icons",
        "data\\flutter_assets\\assets\\img",
    ];
    for sub in &subdirs {
        let d = dir.join(sub);
        if d.is_dir() {
            for entry in walkdir::WalkDir::new(&d)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if matches!(ext, "png" | "ico") {
                        if let Some(icon) = load_icon_file(path) {
                            return Some(icon);
                        }
                    }
                }
            }
        }
    }

    None
}

fn load_icon_file(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::open(path).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w, h))
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
        if hdc == 0 {
            return None;
        }

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
        if hbmp == 0 {
            DeleteDC(hdc);
            return None;
        }

        let old = SelectObject(hdc, hbmp);
        DrawIconEx(hdc, 0, 0, hicon, size, size, 0, 0, DI_NORMAL);
        SelectObject(hdc, old);

        let len = (size * size * 4) as usize;
        let bgra = std::slice::from_raw_parts(pixels_ptr, len).to_vec();

        DeleteObject(hbmp);
        DeleteDC(hdc);

        let mut rgba = Vec::with_capacity(len);
        for pixel in bgra.chunks_exact(4) {
            rgba.push(pixel[2]);
            rgba.push(pixel[1]);
            rgba.push(pixel[0]);
            rgba.push(pixel[3]);
        }

        // skip all-white icons (failed extraction)
        let all_white = rgba
            .chunks_exact(4)
            .all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255);
        if all_white {
            return None;
        }

        Some((rgba, size as u32, size as u32))
    }
}

#[cfg(target_os = "windows")]
fn extract_icon_from_lnk(lnk_path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    const SHGFI_ICON: u32 = 0x00000100;

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
            SHGFI_ICON,
        )
    };
    if ret == 0 || shfi.hIcon == 0 {
        return None;
    }

    let icon = shfi.hIcon;
    let rgba = hicon_to_rgba(icon, 32);
    unsafe {
        destroy_icon(icon);
    }
    rgba
}

#[cfg(not(target_os = "windows"))]
fn extract_icon_from_lnk(_lnk_path: &Path) -> Option<(Vec<u8>, u32, u32)> {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_empty_query_returns_none() {
        assert!(fuzzy_score("", "Anything").is_none());
    }

    #[test]
    fn fuzzy_exact_match() {
        let score = fuzzy_score("notepad", "Notepad").unwrap();
        assert!(score > 0.0);
    }

    #[test]
    fn fuzzy_subsequence_match() {
        let score = fuzzy_score("npd", "Notepad").unwrap();
        assert!(score > 0.0);
    }

    #[test]
    fn fuzzy_no_match() {
        assert!(fuzzy_score("xyz", "Notepad").is_none());
    }

    #[test]
    fn fuzzy_word_boundary_bonus() {
        let with_bonus = fuzzy_score("ps", "Power Shell").unwrap();
        let without = fuzzy_score("ps", "Powershell").unwrap_or(0.0);
        // Word boundary should boost the score
        assert!(
            with_bonus > without,
            "expected word boundary bonus: {} <= {}",
            with_bonus,
            without
        );
    }

    #[test]
    fn fuzzy_camelcase_bonus() {
        let with_bonus = fuzzy_score("vs", "VisualStudio").unwrap_or(0.0);
        let without = fuzzy_score("vs", "visualstudio").unwrap_or(0.0);
        assert!(
            with_bonus >= without,
            "expected camelCase bonus: {} < {}",
            with_bonus,
            without
        );
    }

    #[test]
    fn fuzzy_consecutive_bonus() {
        let consecutive = fuzzy_score("wo", "Word").unwrap();
        let spread = fuzzy_score("wd", "Word").unwrap_or(0.0);
        assert!(
            consecutive > spread,
            "expected consecutive bonus: {} <= {}",
            consecutive,
            spread
        );
    }

    #[test]
    fn fuzzy_early_match_bonus() {
        let early = fuzzy_score("n", "Notepad").unwrap();
        let late = fuzzy_score("d", "Notepad").unwrap_or(0.0);
        assert!(
            early > late,
            "expected early match bonus: {} <= {}",
            early,
            late
        );
    }

    #[test]
    fn fuzzy_query_longer_than_name() {
        assert!(fuzzy_score("ThisIsWayTooLong", "Short").is_none());
    }

    #[test]
    fn fuzzy_case_insensitive() {
        let upper = fuzzy_score("NP", "Notepad").unwrap();
        let lower = fuzzy_score("np", "Notepad").unwrap();
        assert!(
            (upper - lower).abs() < f64::EPSILON,
            "should be case-insensitive: {} != {}",
            upper,
            lower
        );
    }
}
