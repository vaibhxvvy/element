use std::collections::HashSet;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::app::SearchResult;
use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider};

#[derive(Clone)]
struct InstalledApp {
    name: String,
    executable_path: String,
    icon_rgba: Option<(Vec<u8>, u32, u32)>,
}

pub struct AppsProvider {
    apps: Arc<Mutex<Vec<InstalledApp>>>,
    search_dirs: Vec<String>,
    refresh_in_progress: Arc<AtomicBool>,
    revision: Arc<AtomicU64>,
}

impl AppsProvider {
    pub fn new(search_dirs: Vec<String>) -> Self {
        let provider = Self {
            apps: Arc::new(Mutex::new(Vec::new())),
            search_dirs,
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        provider.refresh();
        provider
    }

    fn result_for(app: &InstalledApp, score: f64) -> SearchResult {
        SearchResult {
            title: app.name.clone(),
            subtitle: "Application".into(),
            kind: "app".into(),
            provider_id: "apps".into(),
            action: app.executable_path.clone(),
            icon_rgba: app.icon_rgba.clone(),
            score,
        }
    }

    fn recommendations(&self, ctx: &SearchContext, limit: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();
        let frecency = ctx.db.top_frecency(limit);
        let apps = match self.apps.lock() {
            Ok(a) => a,
            Err(_) => return results,
        };

        for (app_key, _count, _last) in &frecency {
            if let Some(app) = apps
                .iter()
                .find(|app| app.executable_path == *app_key || app.name == *app_key)
            {
                // Legacy title keys and current executable-path keys can both exist
                // for one launch. Keep one recommendation for the real action.
                if !results
                    .iter()
                    .any(|result| result.action == app.executable_path)
                {
                    results.push(Self::result_for(app, 200.0));
                }
            }
        }

        for app in apps.iter() {
            if results.len() >= limit {
                break;
            }
            if !results
                .iter()
                .any(|result| result.action == app.executable_path)
            {
                results.push(Self::result_for(app, 100.0));
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
                let frecency = ctx
                    .db
                    .frecency_score(&app.executable_path)
                    .max(ctx.db.frecency_score(&app.name));
                let boost = 1.0 + (frecency * 5.0).min(2.0);
                results.push(Self::result_for(app, score * boost));
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

    fn activate(&self, ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        let executable = Path::new(&result.action);
        if !is_executable_path(executable) || !executable.is_file() {
            return Err(ElementError::Other(format!(
                "application executable is unavailable: {}",
                result.action
            )));
        }

        let mut command = std::process::Command::new(executable);
        if let Some(working_dir) = executable.parent() {
            command.current_dir(working_dir);
        }
        command.spawn().map_err(ElementError::Io)?;
        ctx.db.record_launch(&result.action);
        Ok(())
    }

    fn refresh(&self) {
        if self
            .refresh_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let apps = Arc::clone(&self.apps);
        let search_dirs = self.search_dirs.clone();
        let refresh_in_progress = Arc::clone(&self.refresh_in_progress);
        let revision = Arc::clone(&self.revision);
        let worker = std::thread::Builder::new()
            .name("element-app-index".into())
            .spawn(move || {
                let indexed_apps =
                    std::panic::catch_unwind(|| scan_apps(&search_dirs)).unwrap_or_default();
                if let Ok(mut guard) = apps.lock() {
                    *guard = indexed_apps;
                    revision.fetch_add(1, Ordering::SeqCst);
                }
                refresh_in_progress.store(false, Ordering::SeqCst);
            });

        if worker.is_err() {
            self.refresh_in_progress.store(false, Ordering::SeqCst);
        }
    }

    fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }
}

fn scan_apps(search_dirs: &[String]) -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    #[cfg(target_os = "windows")]
    {
        let mut app_dirs = vec![
            std::env::var("ProgramData")
                .map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
            std::env::var("APPDATA")
                .map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
        ];
        app_dirs.extend(search_dirs.iter().cloned().map(Ok));
        let cache_dir = icon_cache_dir();
        let mut seen_dirs = HashSet::new();
        let mut seen_executables = HashSet::new();
        for dir in app_dirs.into_iter().filter_map(Result::ok) {
            if !seen_dirs.insert(dir.clone()) {
                continue;
            }
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
                        if let Some(shortcut) = resolve_shortcut(path) {
                            let executable_path = PathBuf::from(&shortcut.target_path);
                            if is_executable_path(&executable_path) && executable_path.is_file() {
                                if !seen_executables
                                    .insert(shortcut.target_path.to_ascii_lowercase())
                                {
                                    continue;
                                }
                                let icon_rgba = cached_icon(&shortcut, &cache_dir);
                                apps.push(InstalledApp {
                                    name,
                                    executable_path: shortcut.target_path,
                                    icon_rgba,
                                });
                            }
                        }
                    }
                }
            }
        }
        apps.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.executable_path.cmp(&b.executable_path))
        });
    }
    apps
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

struct ShortcutInfo {
    target_path: String,
    icon_path: Option<String>,
}

fn is_executable_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

fn is_ico_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ico"))
}

fn icon_cache_path(cache_dir: &Path, source_path: &Path) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source_path.hash(&mut hasher);
    cache_dir.join(format!("v2-{:016x}.png", hasher.finish()))
}

fn load_icon_cache(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::open(path).ok()?;
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w, h))
}

fn save_icon_cache(cache_dir: &Path, source_path: &Path, rgba: &[u8], w: u32, h: u32) {
    let cache_path = icon_cache_path(cache_dir, source_path);
    let _ = image::save_buffer(&cache_path, rgba, w, h, image::ColorType::Rgba8);
}

fn load_icon_file(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    load_icon_cache(path)
}

fn cached_icon(shortcut: &ShortcutInfo, cache_dir: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let executable_path = Path::new(&shortcut.target_path);
    let icon_path = shortcut
        .icon_path
        .as_deref()
        .map(Path::new)
        .filter(|path| path.is_file() && is_ico_path(path));
    let source_path = icon_path.unwrap_or(executable_path);
    let cache_path = icon_cache_path(cache_dir, source_path);
    if cache_path.exists() {
        if let Some(icon) = load_icon_cache(&cache_path) {
            return Some(icon);
        }
    }

    let icon = if is_ico_path(source_path) {
        load_icon_file(source_path)
    } else {
        shell_item_icon(executable_path, 32)
    };

    if let Some(icon) = icon {
        save_icon_cache(cache_dir, source_path, &icon.0, icon.1, icon.2);
        Some(icon)
    } else {
        None
    }
}

/// Resolve the application and optional `.ico` source recorded by a `.lnk` shortcut.
#[cfg(target_os = "windows")]
fn resolve_shortcut(lnk_path: &Path) -> Option<ShortcutInfo> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[repr(C)]
    #[allow(clippy::upper_case_acronyms)]
    struct GUID {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    #[allow(non_upper_case_globals)]
    const CLSID_ShellLink: GUID = GUID {
        data1: 0x00021401,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };
    #[allow(non_upper_case_globals)]
    const IID_IShellLinkW: GUID = GUID {
        data1: 0x000214f9,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };
    #[allow(non_upper_case_globals)]
    const IID_IPersistFile: GUID = GUID {
        data1: 0x0000010b,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };

    const STGM_READ: u32 = 0;
    const S_OK: i32 = 0;
    const S_FALSE: i32 = 1;
    const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;
    const CLSCTX_INPROC_SERVER: u32 = 1;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const SLGP_DEFAULT: u32 = 0x00000000;
    const MAX_PATH: usize = 260;

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(
            rclsid: *const GUID,
            pUnkOuter: *mut std::ffi::c_void,
            dwClsContext: u32,
            riid: *const GUID,
            ppv: *mut *mut std::ffi::c_void,
        ) -> i32;
    }

    let wide: Vec<u16> = OsStr::new(lnk_path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let initialization = unsafe { CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED) };
    let must_uninitialize = initialization == S_OK || initialization == S_FALSE;
    if !must_uninitialize && initialization != RPC_E_CHANGED_MODE {
        return None;
    }

    let result = unsafe {
        (|| {
            let mut shell_link: *mut c_void = ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_ShellLink,
                ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_IShellLinkW,
                &mut shell_link,
            );
            if hr != S_OK || shell_link.is_null() {
                return None;
            }

            type QIFn =
                unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
            type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;

            macro_rules! vtable_fn {
                ($obj:expr, $n:expr, $t:ty) => {{
                    let vtbl = *($obj as *mut *mut *mut c_void);
                    std::mem::transmute::<_, $t>(*(vtbl as *mut *mut c_void).offset($n))
                }};
            }

            let qi: QIFn = vtable_fn!(shell_link, 0, QIFn);
            let release_sl: ReleaseFn = vtable_fn!(shell_link, 2, ReleaseFn);

            let mut persist_file: *mut c_void = ptr::null_mut();
            let hr = qi(shell_link, &IID_IPersistFile, &mut persist_file);
            if hr != S_OK || persist_file.is_null() {
                release_sl(shell_link);
                return None;
            }

            // IPersistFile::Load at vtable slot 5
            let load_file: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> i32 = vtable_fn!(
                persist_file,
                5,
                unsafe extern "system" fn(*mut c_void, *const u16, u32) -> i32
            );
            let release_pf: ReleaseFn = vtable_fn!(persist_file, 2, ReleaseFn);

            let hr = load_file(persist_file, wide.as_ptr(), STGM_READ);
            release_pf(persist_file);

            if hr != S_OK {
                release_sl(shell_link);
                return None;
            }

            // IShellLinkW::GetPath is the first method after IUnknown.
            type GetPathFn =
                unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut c_void, u32) -> i32;
            type GetIconLocationFn =
                unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut i32) -> i32;
            let get_path: GetPathFn = vtable_fn!(shell_link, 3, GetPathFn);
            let get_icon_location: GetIconLocationFn =
                vtable_fn!(shell_link, 16, GetIconLocationFn);

            let mut path_buf = [0u16; MAX_PATH];
            let hr = get_path(
                shell_link,
                path_buf.as_mut_ptr(),
                MAX_PATH as i32,
                ptr::null_mut(),
                SLGP_DEFAULT,
            );

            let target_path = if hr == S_OK {
                let end = path_buf.iter().position(|&c| c == 0).unwrap_or(MAX_PATH);
                String::from_utf16_lossy(&path_buf[..end])
            } else {
                String::new()
            };

            let mut icon_buf = [0u16; MAX_PATH];
            let mut icon_index = 0;
            let icon_path = if get_icon_location(
                shell_link,
                icon_buf.as_mut_ptr(),
                MAX_PATH as i32,
                &mut icon_index,
            ) == S_OK
            {
                let end = icon_buf.iter().position(|&c| c == 0).unwrap_or(MAX_PATH);
                let path = String::from_utf16_lossy(&icon_buf[..end]);
                (!path.is_empty()).then_some(path)
            } else {
                None
            };

            release_sl(shell_link);
            (!target_path.is_empty()).then_some(ShortcutInfo {
                target_path,
                icon_path,
            })
        })()
    };

    if must_uninitialize {
        unsafe { CoUninitialize() };
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn resolve_shortcut(_lnk_path: &Path) -> Option<ShortcutInfo> {
    None
}

#[cfg(target_os = "windows")]
fn shell_item_icon(lnk_path: &Path, size: u32) -> Option<(Vec<u8>, u32, u32)> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[repr(C)]
    #[allow(clippy::upper_case_acronyms)]
    struct GUID {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    #[allow(non_upper_case_globals)]
    const CLSID_IShellItemImageFactory: GUID = GUID {
        data1: 0xbcc18b79,
        data2: 0xba16,
        data3: 0x442f,
        data4: [0x80, 0xc4, 0x8a, 0x59, 0xc3, 0x0c, 0x46, 0x3b],
    };

    const SIIGBF_RESIZETOFIT: u32 = 0x00;
    const S_OK: i32 = 0;
    const S_FALSE: i32 = 1;
    const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;

    #[repr(C)]
    #[allow(clippy::upper_case_acronyms)]
    struct SIZE {
        cx: i32,
        cy: i32,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHCreateItemFromParsingName(
            pszPath: *const u16,
            pbc: *const std::ffi::c_void,
            riid: *const GUID,
            ppv: *mut *mut std::ffi::c_void,
        ) -> i32;
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> i32;
        fn CoUninitialize();
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn GetObjectW(h: isize, c: i32, pv: *mut std::ffi::c_void) -> i32;
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
        fn GetDIBits(
            hdc: isize,
            hbmp: isize,
            start: u32,
            lines: u32,
            lpvBits: *mut u8,
            lpbmi: *mut BITMAPINFOHEADER,
            usage: u32,
        ) -> i32;
    }

    #[repr(C)]
    #[allow(clippy::upper_case_acronyms)]
    struct BITMAP {
        bmType: i32,
        bmWidth: i32,
        bmHeight: i32,
        bmWidthBytes: i32,
        bmPlanes: u16,
        bmBitsPixel: u16,
        bmBits: *mut u8,
    }

    #[repr(C)]
    #[allow(clippy::upper_case_acronyms)]
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

    let wide: Vec<u16> = OsStr::new(lnk_path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let initialization = unsafe { CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED) };
    let must_uninitialize = initialization == S_OK || initialization == S_FALSE;
    if !must_uninitialize && initialization != RPC_E_CHANGED_MODE {
        return None;
    }

    let result = unsafe {
        (|| {
            // Get IShellItem from path
            let mut shell_item: *mut std::ffi::c_void = ptr::null_mut();
            let hr = SHCreateItemFromParsingName(
                wide.as_ptr(),
                ptr::null(),
                &CLSID_IShellItemImageFactory,
                &mut shell_item,
            );
            if hr != S_OK || shell_item.is_null() {
                return None;
            }

            // COM: first field of object is vtable pointer
            // vtable slots: 0=QI, 1=AddRef, 2=Release, 3=GetImage
            let vtable_addr = *(shell_item as *mut isize);
            let vtable = vtable_addr as *const isize;

            let get_image: unsafe extern "system" fn(
                *mut std::ffi::c_void,
                SIZE,
                u32,
                *mut isize,
            ) -> i32 = std::mem::transmute(*vtable.offset(3));
            let release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32 =
                std::mem::transmute(*vtable.offset(2));

            let s = SIZE {
                cx: size as i32,
                cy: size as i32,
            };
            let mut hbmp: isize = 0;
            let hr = get_image(shell_item, s, SIIGBF_RESIZETOFIT, &mut hbmp);
            release(shell_item);

            if hr != S_OK || hbmp == 0 {
                return None;
            }

            // Get bitmap dimensions
            let mut bm: BITMAP = std::mem::zeroed();
            if GetObjectW(
                hbmp,
                std::mem::size_of::<BITMAP>() as i32,
                &mut bm as *mut _ as *mut std::ffi::c_void,
            ) == 0
            {
                DeleteObject(hbmp);
                return None;
            }

            let w = bm.bmWidth as u32;
            let h = bm.bmHeight as u32;

            // Create a DIB section to receive pixel data
            let hdc = CreateCompatibleDC(0);
            if hdc == 0 {
                DeleteObject(hbmp);
                return None;
            }

            let mut bih = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
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
            let dib = CreateDIBSection(hdc, &bih, DIB_RGB_COLORS, &mut pixels_ptr, 0, 0);
            if dib == 0 {
                DeleteDC(hdc);
                DeleteObject(hbmp);
                return None;
            }

            let old = SelectObject(hdc, dib);
            let len = (w * h * 4) as usize;
            if GetDIBits(hdc, hbmp, 0, h, pixels_ptr, &mut bih, DIB_RGB_COLORS) == 0 {
                SelectObject(hdc, old);
                DeleteObject(dib);
                DeleteDC(hdc);
                DeleteObject(hbmp);
                return None;
            }
            let bgra = std::slice::from_raw_parts(pixels_ptr, len).to_vec();
            SelectObject(hdc, old);

            DeleteObject(dib);
            DeleteDC(hdc);
            DeleteObject(hbmp);

            // BGRA -> RGBA
            let mut rgba = Vec::with_capacity(len);
            for pixel in bgra.chunks_exact(4) {
                rgba.push(pixel[2]);
                rgba.push(pixel[1]);
                rgba.push(pixel[0]);
                rgba.push(pixel[3]);
            }

            // skip all-white (failed extraction)
            let all_white = rgba
                .chunks_exact(4)
                .all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255);
            if all_white {
                return None;
            }

            Some((rgba, w, h))
        })()
    };

    if must_uninitialize {
        unsafe { CoUninitialize() };
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn shell_item_icon(_path: &Path, _size: u32) -> Option<(Vec<u8>, u32, u32)> {
    None
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

    #[test]
    fn only_executable_targets_are_indexed() {
        assert!(is_executable_path(std::path::Path::new(
            "C:\\Apps\\Element.EXE"
        )));
        assert!(!is_executable_path(std::path::Path::new(
            "C:\\Apps\\Element.lnk"
        )));
    }

    #[test]
    fn recommendations_dedupe_legacy_and_executable_frecency_keys() {
        use crate::config::Config;
        use crate::database::Database;

        let executable_path = r"C:\\Apps\\Example.exe".to_string();
        let provider = AppsProvider {
            apps: Arc::new(Mutex::new(vec![InstalledApp {
                name: "Example".into(),
                executable_path: executable_path.clone(),
                icon_rgba: None,
            }])),
            search_dirs: Vec::new(),
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        let db = Database::new_in_memory();
        db.record_launch("Example");
        db.record_launch(&executable_path);
        let config = Config::default();
        let context = SearchContext {
            config: &config,
            db: &db,
        };

        let results = provider.recommendations(&context, 12);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, executable_path);
    }
}
