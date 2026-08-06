//! Shared icon pipeline — on-disk PNG cache and 32×32 RGBA extraction for
//! *any* shell path (`.exe`, plain files, folders).
//!
//! 1. Prefer a cached decode: `~/.element/cache/icons/v2-<hash>.png`.
//! 2. Else extract via `IShellItemImageFactory` at 32×32 (`.ico` files are
//!    decoded directly).
//! 3. Persist the decode back to the cache.

use std::ffi::c_void;
use std::path::{Path, PathBuf};

pub(crate) fn icon_cache_dir() -> PathBuf {
    let mut p = if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        PathBuf::from(profile)
    } else {
        PathBuf::from(".")
    };
    p.push(".element");
    p.push("cache");
    p.push("icons");
    let _ = std::fs::create_dir_all(&p);
    p
}

pub(crate) fn is_ico_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ico"))
}

fn icon_cache_path(cache_dir: &Path, source_path: &Path) -> PathBuf {
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

/// Icon for an arbitrary path: hit the PNG cache, else extract and cache.
///
/// Works for executables, plain files, and folders — anything
/// `SHCreateItemFromParsingName` can produce a shell item for.
pub(crate) fn cached_icon_for_path(
    source_path: &Path,
    cache_dir: &Path,
) -> Option<(Vec<u8>, u32, u32)> {
    let cache_path = icon_cache_path(cache_dir, source_path);
    if cache_path.exists() {
        if let Some(icon) = load_icon_cache(&cache_path) {
            return Some(icon);
        }
    }

    let icon = if is_ico_path(source_path) {
        load_icon_file(source_path)
    } else {
        shell_item_icon(source_path, 32)
    };

    if let Some(icon) = &icon {
        save_icon_cache(cache_dir, source_path, &icon.0, icon.1, icon.2);
    }
    icon
}

fn load_icon_file(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    load_icon_cache(path)
}

/// Extract a 32-bit RGBA icon from `path` at `size` via `IShellItemImageFactory`.
#[cfg(target_os = "windows")]
fn shell_item_icon(path: &Path, size: u32) -> Option<(Vec<u8>, u32, u32)> {
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

    let wide: Vec<u16> = OsStr::new(path.as_os_str())
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
