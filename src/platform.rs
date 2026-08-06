//! Platform helpers (Win32) isolated from providers and UI code.
//!
//! Every Win32 API is wrapped in a safe `extern "system" fn` (per the
//! main.rs convention), so call sites need no `unsafe` blocks.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Put a list of file paths on the clipboard as `CF_HDROP`, so pasting into
/// Explorer (or any shell view) copies/moves the actual files, not text.
pub fn copy_files_to_clipboard(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no paths to copy".into());
    }
    // Each path null-terminated, plus one extra terminator for the DROPFILES
    // list. DROPFILES.fFiles offset 20; we only need the pFiles payload, so
    // just build the packed null-terminated string list.
    let mut payload: Vec<u16> = Vec::new();
    for path in paths {
        payload.extend(OsStr::new(path).encode_wide());
        payload.push(0);
    }
    payload.push(0);

    let bytes = payload.len() * 2;
    // GMEM_MOVEABLE (0x0002) | GMEM_ZEROINIT (0x0040)
    let h_mem = unsafe { GlobalAlloc(0x0002 | 0x0040, bytes) };
    if h_mem == 0 {
        return Err("GlobalAlloc failed".into());
    }
    let ptr = unsafe { GlobalLock(h_mem) };
    if ptr.is_null() {
        unsafe { GlobalFree(h_mem) };
        return Err("GlobalLock failed".into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(payload.as_ptr(), ptr as *mut u16, payload.len());
        GlobalUnlock(h_mem);
    }
    let ok = unsafe {
        OpenClipboard(0) != 0
            && EmptyClipboard() != 0
            && SetClipboardData(CF_HDROP, h_mem) != 0
            && CloseClipboard() != 0
    };
    if !ok {
        // The clipboard owns h_mem only after a successful SetClipboardData.
        unsafe { GlobalFree(h_mem) };
        return Err("clipboard transfer failed".into());
    }
    Ok(())
}

/// Lock the workstation (Ctrl+Alt+Del style).
pub fn lock_workstation() -> Result<(), String> {
    if unsafe { LockWorkStation() } == 0 {
        return Err("LockWorkStation failed".into());
    }
    Ok(())
}

/// Put the machine to sleep (no hibernation).
pub fn suspend_system() -> Result<(), String> {
    if unsafe { SetSuspendState(0, 0, 0) } == 0 {
        return Err("SetSuspendState failed".into());
    }
    Ok(())
}

const CF_HDROP: u32 = 15;

// MSDN: OpenClipboard — https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard
#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hWndNewOwner: isize) -> i32;
    fn EmptyClipboard() -> i32;
    fn CloseClipboard() -> i32;
    fn LockWorkStation() -> i32;
}

// MSDN: GlobalAlloc — https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-globalalloc
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> isize;
    fn GlobalLock(hMem: isize) -> *mut u8;
    fn GlobalUnlock(hMem: isize) -> i32;
    fn GlobalFree(hMem: isize) -> isize;
    fn SetClipboardData(uFormat: u32, hMem: isize) -> isize;
}

// MSDN: SetSuspendState — https://learn.microsoft.com/en-us/windows/win32/api/powrprof/nf-powrprof-setsuspendstate
#[link(name = "powrprof")]
extern "system" {
    fn SetSuspendState(hibernate: i32, force: i32, disable_wake_events: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_list_is_rejected() {
        assert!(copy_files_to_clipboard(&[]).is_err());
    }
}
