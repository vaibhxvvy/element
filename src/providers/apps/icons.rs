//! Shortcut-specific icon helpers — `.lnk` resolution and the shortcut icon
//! preference rule (`.ico` path > target executable).
//!
//! The generic cache + `IShellItemImageFactory` extraction lives in
//! [`crate::providers::icon`]; this module only adds the shortcut layer on top.

use std::path::Path;

use crate::providers::icon::{cached_icon_for_path, is_ico_path};

/// A resolved shortcut: the launchable target and optional explicit icon file.
pub(crate) struct ShortcutInfo {
    pub(crate) target_path: String,
    pub(crate) icon_path: Option<String>,
}

/// Icon for a shortcut: prefer the `.ico` recorded by the shortcut, otherwise
/// the target executable. Cache hit first, extraction second.
pub(crate) fn cached_icon(
    shortcut: &ShortcutInfo,
    cache_dir: &Path,
) -> Option<(Vec<u8>, u32, u32)> {
    let executable_path = Path::new(&shortcut.target_path);
    let source_path = shortcut
        .icon_path
        .as_deref()
        .map(Path::new)
        .filter(|path| path.is_file() && is_ico_path(path))
        .unwrap_or(executable_path);
    cached_icon_for_path(source_path, cache_dir)
}

/// Resolve the application and optional `.ico` source recorded by a `.lnk` shortcut.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_shortcut(lnk_path: &Path) -> Option<ShortcutInfo> {
    use std::ffi::c_void;
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
pub(crate) fn resolve_shortcut(_lnk_path: &Path) -> Option<ShortcutInfo> {
    None
}
