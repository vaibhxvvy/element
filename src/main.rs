#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

mod config;
mod database;
mod debug_log;
mod error;
mod hotkey;
mod orchestrator;
mod providers;
mod registry;
pub(crate) mod theme;
mod ui;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use iced::{window, Theme};

use crate::database::Database;
use crate::orchestrator::Orchestrator;

pub(crate) static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
pub(crate) static HIDE_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static RESIZE_HEIGHT: AtomicU32 = AtomicU32::new(56);
pub(crate) static RESIZE_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static WINDOW_WIDTH: AtomicU32 = AtomicU32::new(580);
pub(crate) static WINDOW_FOUND: AtomicBool = AtomicBool::new(false);

/// Our intended visibility — more reliable than racing IsWindowVisible.
#[cfg(target_os = "windows")]
static LAUNCHER_SHOWN: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static LAST_TOGGLE_MS: AtomicU32 = AtomicU32::new(0);

#[cfg(target_os = "windows")]
const SW_HIDE: i32 = 0;
#[cfg(target_os = "windows")]
const SW_RESTORE: i32 = 9;
#[cfg(target_os = "windows")]
const HWND_TOPMOST: isize = -1;
#[cfg(target_os = "windows")]
const SWP_NOSIZE: u32 = 0x0001;
#[cfg(target_os = "windows")]
const SWP_NOMOVE: u32 = 0x0002;
#[cfg(target_os = "windows")]
const SWP_NOZORDER: u32 = 0x0004;
#[cfg(target_os = "windows")]
const SWP_SHOWWINDOW: u32 = 0x0040;

#[cfg(target_os = "windows")]
fn hide_launcher(hwnd: isize) {
    debug_log!("hide_launcher(hwnd={})", hwnd);
    let ret = ShowWindow(hwnd, SW_HIDE);
    debug_log!("ShowWindow(SW_HIDE) returned: {}", ret);
    LAUNCHER_SHOWN.store(false, Ordering::SeqCst);
    HOTKEY_TRIGGERED.store(false, Ordering::SeqCst);
    debug_log!("window hidden");
}

#[cfg(target_os = "windows")]
fn show_launcher(hwnd: isize) {
    debug_log!("show_launcher(hwnd={})", hwnd);
    let width = WINDOW_WIDTH.load(Ordering::Relaxed) as i32;
    let scr_w = GetSystemMetrics(0);
    let scr_h = GetSystemMetrics(1);
    let x = (scr_w - width) / 2;
    let y = scr_h.saturating_sub(420) / 3;
    debug_log!(
        "screen: {}x{}, position: ({}, {}), width: {}",
        scr_w,
        scr_h,
        x,
        y,
        width
    );

    HIDE_REQUESTED.store(false, Ordering::SeqCst);
    let _ = ShowWindow(hwnd, SW_RESTORE);
    let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, 0, 0, SWP_NOSIZE | SWP_SHOWWINDOW);
    // Keep focus simple — AttachThreadInput previously crashed mid-toggle.
    let _ = BringWindowToTop(hwnd);
    let fg = SetForegroundWindow(hwnd);
    debug_log!("SetForegroundWindow returned: {}", fg);
    LAUNCHER_SHOWN.store(true, Ordering::SeqCst);
    HOTKEY_TRIGGERED.store(true, Ordering::SeqCst);
    debug_log!("HOTKEY_TRIGGERED set to true");
}

#[cfg(target_os = "windows")]
fn toggle_launcher() {
    let now = GetTickCount();
    let last = LAST_TOGGLE_MS.load(Ordering::SeqCst);
    if now.wrapping_sub(last) < 200 {
        debug_log!("toggle_launcher: debounced");
        return;
    }

    let hwnd = find_own_launcher_hwnd();
    if hwnd == 0 {
        debug_log!("toggle_launcher: window not ready — requeue");
        hotkey::requeue_toggle();
        return;
    }
    LAST_TOGGLE_MS.store(now, Ordering::SeqCst);
    WINDOW_FOUND.store(true, Ordering::SeqCst);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if LAUNCHER_SHOWN.load(Ordering::SeqCst) {
            debug_log!("ACTION: hiding launcher");
            hide_launcher(hwnd);
        } else {
            debug_log!("ACTION: showing launcher");
            show_launcher(hwnd);
        }
    }));
    if result.is_err() {
        debug_log!("CRITICAL: toggle_launcher panicked — suppressed");
        LAUNCHER_SHOWN.store(false, Ordering::SeqCst);
    }
}

/// Find this process's Iced window titled "Element" via EnumWindows + PID check.
#[cfg(target_os = "windows")]
fn find_own_launcher_hwnd() -> isize {
    let title = [69u16, 108, 101, 109, 101, 110, 116, 0]; // "Element"
    let my_pid = GetCurrentProcessId();
    let mut found: isize = 0;
    let mut state = EnumFindState {
        title: title.as_ptr(),
        pid: my_pid,
        out: &mut found,
    };
    EnumWindows(Some(enum_find_own_wnd), &mut state as *mut _ as isize);
    found
}

#[cfg(target_os = "windows")]
struct EnumFindState {
    title: *const u16,
    pid: u32,
    out: *mut isize,
}

#[cfg(target_os = "windows")]
extern "system" fn enum_find_own_wnd(hwnd: isize, lparam: isize) -> i32 {
    let state = unsafe { &*(lparam as *const EnumFindState) };
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid != state.pid {
        return 1; // continue
    }
    let mut buf = [0u16; 64];
    let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if n <= 0 {
        return 1;
    }
    // Compare to "Element"
    let want = unsafe { std::slice::from_raw_parts(state.title, 7) };
    if buf[..n as usize] == *want {
        unsafe { *state.out = hwnd };
        return 0; // stop
    }
    1
}

// ── DWM rounded corners (acrylic disabled — blanks wgpu UI) ─────────────

#[cfg(target_os = "windows")]
fn apply_window_effects(hwnd: isize) {
    apply_dwm_rounded_corners(hwnd);
    let vis = IsWindowVisible(hwnd);
    debug_log!(
        "window effects applied (rounded corners), IsWindowVisible={}",
        vis
    );
}

#[cfg(target_os = "windows")]
fn apply_dwm_rounded_corners(hwnd: isize) {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    let pref = DWMWCP_ROUND;
    let ret = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &pref as *const _ as *const std::ffi::c_void,
        std::mem::size_of::<u32>() as u32,
    );
    if ret != 0 {
        debug_log!("DwmSetWindowAttribute(rounded) returned: {}", ret);
    }
}

// ── Tray window procedure ───────────────────────────────────────────────

#[cfg(target_os = "windows")]
extern "system" fn tray_wnd_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
    const WM_APP: u32 = 0x8000;
    const WM_COMMAND: u32 = 0x0111;
    const WM_DESTROY: u32 = 0x0002;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_RBUTTONUP: u32 = 0x0205;
    const ID_EXIT: usize = 1001;
    const ID_AUTOSTART: usize = 1002;
    const MF_CHECKED: u32 = 0x0008;
    const MF_SEPARATOR: u32 = 0x0800;
    const TPM_RIGHTBUTTON: u32 = 2;

    match msg {
        WM_APP => {
            let mouse_msg = lparam as u32;
            if mouse_msg == WM_LBUTTONDOWN {
                debug_log!("tray left-click: toggle");
                toggle_launcher();
            } else if mouse_msg == WM_RBUTTONUP {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                let cfg = config::Config::load();
                let menu = CreatePopupMenu();
                let autostart_text = [
                    82u16, 117, 110, 32, 97, 116, 32, 115, 116, 97, 114, 116, 117, 112, 0,
                ];
                let flags = if cfg.autostart { MF_CHECKED } else { 0 };
                AppendMenuW(menu, flags, ID_AUTOSTART, autostart_text.as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                let exit_text = [69u16, 120, 105, 116, 0];
                AppendMenuW(menu, 0, ID_EXIT, exit_text.as_ptr());
                SetForegroundWindow(hwnd);
                TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
                DestroyMenu(menu);
            }
            0
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            if id == ID_EXIT {
                debug_log!("tray exit clicked");
                EXIT_REQUESTED.store(true, Ordering::SeqCst);
                PostQuitMessage(0);
            } else if id == ID_AUTOSTART {
                debug_log!("tray autostart toggle");
                let mut cfg = config::Config::load();
                cfg.autostart = !cfg.autostart;
                cfg.save();
                set_autostart(cfg.autostart);
            }
            0
        }
        WM_DESTROY => {
            debug_log!("tray window destroyed");
            EXIT_REQUESTED.store(true, Ordering::SeqCst);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ── Safe FFI wrappers ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
extern "system" fn ShowWindow(h: isize, n: i32) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(h: isize, n: i32) -> i32;
    }
    unsafe { ShowWindow(h, n) }
}

#[cfg(target_os = "windows")]
extern "system" fn IsWindowVisible(hWnd: isize) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn IsWindowVisible(hWnd: isize) -> i32;
    }
    unsafe { IsWindowVisible(hWnd) }
}

#[cfg(target_os = "windows")]
extern "system" fn SetWindowPos(
    hWnd: isize,
    hWndInsertAfter: isize,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    uFlags: u32,
) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(
            hWnd: isize,
            hWndInsertAfter: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> i32;
    }
    unsafe { SetWindowPos(hWnd, hWndInsertAfter, x, y, cx, cy, uFlags) }
}

#[cfg(target_os = "windows")]
extern "system" fn GetSystemMetrics(nIndex: i32) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }
    unsafe { GetSystemMetrics(nIndex) }
}

#[cfg(target_os = "windows")]
extern "system" fn CreatePopupMenu() -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn CreatePopupMenu() -> isize;
    }
    unsafe { CreatePopupMenu() }
}

#[cfg(target_os = "windows")]
extern "system" fn AppendMenuW(
    hMenu: isize,
    uFlags: u32,
    uIDNewItem: usize,
    lpNewItem: *const u16,
) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn AppendMenuW(hMenu: isize, uFlags: u32, uIDNewItem: usize, lpNewItem: *const u16) -> i32;
    }
    unsafe { AppendMenuW(hMenu, uFlags, uIDNewItem, lpNewItem) }
}

#[cfg(target_os = "windows")]
extern "system" fn SetForegroundWindow(hWnd: isize) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn SetForegroundWindow(hWnd: isize) -> i32;
    }
    unsafe { SetForegroundWindow(hWnd) }
}

#[cfg(target_os = "windows")]
extern "system" fn PostMessageW(hWnd: isize, Msg: u32, wParam: usize, lParam: isize) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn PostMessageW(hWnd: isize, Msg: u32, wParam: usize, lParam: isize) -> i32;
    }
    unsafe { PostMessageW(hWnd, Msg, wParam, lParam) }
}

#[cfg(target_os = "windows")]
extern "system" fn GetForegroundWindow() -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
    }
    unsafe { GetForegroundWindow() }
}

#[cfg(target_os = "windows")]
extern "system" fn BringWindowToTop(hWnd: isize) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn BringWindowToTop(hWnd: isize) -> i32;
    }
    unsafe { BringWindowToTop(hWnd) }
}

#[cfg(target_os = "windows")]
extern "system" fn TrackPopupMenu(
    hMenu: isize,
    uFlags: u32,
    x: i32,
    y: i32,
    nReserved: i32,
    hWnd: isize,
    prcRect: *const std::ffi::c_void,
) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn TrackPopupMenu(
            hMenu: isize,
            uFlags: u32,
            x: i32,
            y: i32,
            nReserved: i32,
            hWnd: isize,
            prcRect: *const std::ffi::c_void,
        ) -> i32;
    }
    unsafe { TrackPopupMenu(hMenu, uFlags, x, y, nReserved, hWnd, prcRect) }
}

#[cfg(target_os = "windows")]
extern "system" fn DestroyMenu(hMenu: isize) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn DestroyMenu(hMenu: isize) -> i32;
    }
    unsafe { DestroyMenu(hMenu) }
}

#[cfg(target_os = "windows")]
extern "system" fn PostQuitMessage(nExitCode: i32) {
    #[link(name = "user32")]
    extern "system" {
        fn PostQuitMessage(nExitCode: i32);
    }
    unsafe { PostQuitMessage(nExitCode) }
}

#[cfg(target_os = "windows")]
extern "system" fn GetModuleHandleW(lpModuleName: *const u16) -> isize {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(lpModuleName: *const u16) -> isize;
    }
    unsafe { GetModuleHandleW(lpModuleName) }
}

#[cfg(target_os = "windows")]
extern "system" fn RegisterClassExW(wcx: *const WNDCLASSEXW) -> u16 {
    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassExW(wcx: *const WNDCLASSEXW) -> u16;
    }
    unsafe { RegisterClassExW(wcx) }
}

#[cfg(target_os = "windows")]
extern "system" fn CreateWindowExW(
    dwExStyle: u32,
    lpClassName: *const u16,
    lpWindowName: *const u16,
    dwStyle: u32,
    x: i32,
    y: i32,
    nWidth: i32,
    nHeight: i32,
    hWndParent: isize,
    hMenu: isize,
    hInstance: isize,
    lpParam: *const std::ffi::c_void,
) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn CreateWindowExW(
            dwExStyle: u32,
            lpClassName: *const u16,
            lpWindowName: *const u16,
            dwStyle: u32,
            x: i32,
            y: i32,
            nWidth: i32,
            nHeight: i32,
            hWndParent: isize,
            hMenu: isize,
            hInstance: isize,
            lpParam: *const std::ffi::c_void,
        ) -> isize;
    }
    unsafe {
        CreateWindowExW(
            dwExStyle,
            lpClassName,
            lpWindowName,
            dwStyle,
            x,
            y,
            nWidth,
            nHeight,
            hWndParent,
            hMenu,
            hInstance,
            lpParam,
        )
    }
}

#[cfg(target_os = "windows")]
extern "system" fn DefWindowProcW(hWnd: isize, msg: u32, wParam: usize, lParam: isize) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn DefWindowProcW(hWnd: isize, msg: u32, wParam: usize, lParam: isize) -> isize;
    }
    unsafe { DefWindowProcW(hWnd, msg, wParam, lParam) }
}

#[cfg(target_os = "windows")]
extern "system" fn Shell_NotifyIconW(dwMessage: u32, lpData: *mut NOTIFYICONDATAW) -> i32 {
    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(dwMessage: u32, lpData: *mut NOTIFYICONDATAW) -> i32;
    }
    unsafe { Shell_NotifyIconW(dwMessage, lpData) }
}

#[cfg(target_os = "windows")]
extern "system" fn GetCursorPos(lpPoint: *mut POINT) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    }
    unsafe { GetCursorPos(lpPoint) }
}

#[cfg(target_os = "windows")]
extern "system" fn DwmSetWindowAttribute(
    hwnd: isize,
    dwAttribute: u32,
    pvAttribute: *const std::ffi::c_void,
    cbAttribute: u32,
) -> i32 {
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            dwAttribute: u32,
            pvAttribute: *const std::ffi::c_void,
            cbAttribute: u32,
        ) -> i32;
    }
    unsafe { DwmSetWindowAttribute(hwnd, dwAttribute, pvAttribute, cbAttribute) }
}

#[cfg(target_os = "windows")]
extern "system" fn GetCurrentProcessId() -> u32 {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcessId() -> u32;
    }
    unsafe { GetCurrentProcessId() }
}

#[cfg(target_os = "windows")]
extern "system" fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32;
    }
    unsafe { GetWindowThreadProcessId(hWnd, lpdwProcessId) }
}

#[cfg(target_os = "windows")]
extern "system" fn GetWindowTextW(hWnd: isize, lpString: *mut u16, nMaxCount: i32) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetWindowTextW(hWnd: isize, lpString: *mut u16, nMaxCount: i32) -> i32;
    }
    unsafe { GetWindowTextW(hWnd, lpString, nMaxCount) }
}

#[cfg(target_os = "windows")]
extern "system" fn EnumWindows(
    lpEnumFunc: Option<unsafe extern "system" fn(isize, isize) -> i32>,
    lParam: isize,
) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(
            lpEnumFunc: Option<unsafe extern "system" fn(isize, isize) -> i32>,
            lParam: isize,
        ) -> i32;
    }
    unsafe { EnumWindows(lpEnumFunc, lParam) }
}

#[cfg(target_os = "windows")]
extern "system" fn GetTickCount() -> u32 {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetTickCount() -> u32;
    }
    unsafe { GetTickCount() }
}

/// Exit Windows "Alt menu" / system-menu mode on the focused window.
/// Alt alone still reaches apps before Space; without this, Alt+Space often
/// only works after clicking the desktop to dismiss that mode.
#[cfg(target_os = "windows")]
fn cancel_sysmenu_mode() {
    const WM_CANCELMODE: u32 = 0x001F;
    let fg = GetForegroundWindow();
    if fg != 0 {
        PostMessageW(fg, WM_CANCELMODE, 0, 0);
    }
    let hwnd = find_own_launcher_hwnd();
    if hwnd != 0 && hwnd != fg {
        PostMessageW(hwnd, WM_CANCELMODE, 0, 0);
    }
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct WNDCLASSEXW {
    cbSize: u32,
    style: u32,
    lpfnWndProc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: isize,
    hIcon: isize,
    hCursor: isize,
    hbrBackground: isize,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
    hIconSm: isize,
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct NOTIFYICONDATAW {
    cbSize: u32,
    hWnd: isize,
    uID: u32,
    uFlags: u32,
    uCallbackMessage: u32,
    hIcon: isize,
    szTip: [u16; 128],
    dwState: u32,
    dwStateMask: u32,
    szInfo: [u16; 256],
    _uVersion: u32,
    szInfoTitle: [u16; 64],
    dwInfoFlags: u32,
    guidItem: [u8; 16],
    hBalloonIcon: isize,
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct POINT {
    x: i32,
    y: i32,
}

// ── Autostart (registry) ────────────────────────────────────────────────

#[cfg(target_os = "windows")]
extern "system" fn RegOpenKeyExW(
    hKey: isize,
    lpSubKey: *const u16,
    ulOptions: u32,
    samDesired: u32,
    phkResult: &mut isize,
) -> i32 {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            hKey: isize,
            lpSubKey: *const u16,
            ulOptions: u32,
            samDesired: u32,
            phkResult: &mut isize,
        ) -> i32;
    }
    unsafe { RegOpenKeyExW(hKey, lpSubKey, ulOptions, samDesired, phkResult) }
}

#[cfg(target_os = "windows")]
extern "system" fn RegSetValueExW(
    hKey: isize,
    lpValueName: *const u16,
    Reserved: u32,
    dwType: u32,
    lpData: *const u8,
    cbData: u32,
) -> i32 {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegSetValueExW(
            hKey: isize,
            lpValueName: *const u16,
            Reserved: u32,
            dwType: u32,
            lpData: *const u8,
            cbData: u32,
        ) -> i32;
    }
    unsafe { RegSetValueExW(hKey, lpValueName, Reserved, dwType, lpData, cbData) }
}

#[cfg(target_os = "windows")]
extern "system" fn RegDeleteValueW(hKey: isize, lpValueName: *const u16) -> i32 {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegDeleteValueW(hKey: isize, lpValueName: *const u16) -> i32;
    }
    unsafe { RegDeleteValueW(hKey, lpValueName) }
}

#[cfg(target_os = "windows")]
extern "system" fn RegCloseKey(hKey: isize) -> i32 {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegCloseKey(hKey: isize) -> i32;
    }
    unsafe { RegCloseKey(hKey) }
}

#[cfg(target_os = "windows")]
const HKEY_CURRENT_USER: isize = 0x80000001;
#[cfg(target_os = "windows")]
const KEY_SET_VALUE: u32 = 0x0002;
#[cfg(target_os = "windows")]
const REG_SZ: u32 = 1;
#[cfg(target_os = "windows")]
const ERROR_SUCCESS: i32 = 0;

#[cfg(target_os = "windows")]
fn set_autostart(enabled: bool) {
    let run_key = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let value_name = "Element\0".encode_utf16().collect::<Vec<_>>();
    let mut hkey: isize = 0;
    let ret = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        run_key.as_ptr(),
        0,
        KEY_SET_VALUE,
        &mut hkey,
    );
    if ret != ERROR_SUCCESS {
        debug_log!("autostart: RegOpenKeyExW failed: {}", ret);
        return;
    }
    if enabled {
        let exe_path = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut exe_utf16: Vec<u16> = exe_path.encode_utf16().collect();
        exe_utf16.push(0);
        let ret2 = RegSetValueExW(
            hkey,
            value_name.as_ptr(),
            0,
            REG_SZ,
            exe_utf16.as_ptr() as *const u8,
            (exe_utf16.len() * 2) as u32,
        );
        if ret2 == ERROR_SUCCESS {
            debug_log!("autostart: enabled (path={})", exe_path);
        } else {
            debug_log!("autostart: RegSetValueExW failed: {}", ret2);
        }
    } else {
        let ret2 = RegDeleteValueW(hkey, value_name.as_ptr());
        if ret2 == ERROR_SUCCESS {
            debug_log!("autostart: disabled");
        } else {
            debug_log!("autostart: RegDeleteValueW failed: {}", ret2);
        }
    }
    RegCloseKey(hkey);
}

#[allow(clippy::manual_dangling_ptr)]
pub fn main() -> iced::Result {
    debug_log::init();
    debug_log!("=== Element v{} starting ===", env!("CARGO_PKG_VERSION"));

    let config = Arc::new(config::Config::load());
    WINDOW_WIDTH.store(config.window_width as u32, Ordering::Relaxed);
    let init_win_size = iced::Size::new(config.window_width, 56.0);
    debug_log!("config loaded, window_width={}", config.window_width);

    #[cfg(target_os = "windows")]
    set_autostart(config.autostart);

    static DWM_APPLIED: AtomicBool = AtomicBool::new(false);

    #[cfg(target_os = "windows")]
    {
        let preferred_hotkey = config.hotkey.clone();
        std::thread::spawn(move || {
            #[repr(C)]
            #[allow(clippy::upper_case_acronyms)]
            struct MSG {
                hwnd: isize,
                message: u32,
                wParam: usize,
                lParam: isize,
                time: u32,
                pt: POINT,
            }

            #[link(name = "user32")]
            extern "system" {
                fn PeekMessageW(
                    msg: *mut MSG,
                    hWnd: isize,
                    wMsgFilterMin: u32,
                    wMsgFilterMax: u32,
                    wRemoveMsg: u32,
                ) -> i32;
                fn DispatchMessageW(msg: *const MSG) -> isize;
                fn LoadIconW(hInstance: isize, lpIconName: *const u16) -> isize;
            }

            const WM_HOTKEY: u32 = 0x0312;
            const WM_QUIT: u32 = 0x0012;
            const PM_REMOVE: u32 = 0x0001;
            const NIM_ADD: u32 = 0;
            const NIF_MESSAGE: u32 = 1;
            const NIF_ICON: u32 = 2;
            const NIF_TIP: u32 = 4;
            const HWND_MESSAGE: isize = -3;

            let tip_hotkey = hotkey::install(&preferred_hotkey);

            // Create hidden message window for tray callbacks
            unsafe {
                let class_name = "ElementTrayClass\0".encode_utf16().collect::<Vec<_>>();
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: 0,
                    lpfnWndProc: Some(tray_wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: GetModuleHandleW(std::ptr::null()),
                    hIcon: 0,
                    hCursor: 0,
                    hbrBackground: 0,
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: 0,
                };
                RegisterClassExW(&wc);
                let tray_hwnd = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    std::ptr::null(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    0,
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                debug_log!("tray window created: hwnd={}", tray_hwnd);

                let mut nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: tray_hwnd,
                    uID: 1,
                    uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                    uCallbackMessage: 0x8000,
                    hIcon: LoadIconW(GetModuleHandleW(std::ptr::null()), 1 as *const u16),
                    szTip: [0u16; 128],
                    dwState: 0,
                    dwStateMask: 0,
                    szInfo: [0u16; 256],
                    _uVersion: 0,
                    szInfoTitle: [0u16; 64],
                    dwInfoFlags: 0,
                    guidItem: [0u8; 16],
                    hBalloonIcon: 0,
                };
                let tip = format!("Element ({tip_hotkey})");
                let tip_utf16: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
                let copy_len = (tip_utf16.len() - 1).min(nid.szTip.len() - 1);
                nid.szTip[..copy_len].copy_from_slice(&tip_utf16[..copy_len]);
                Shell_NotifyIconW(NIM_ADD, &mut nid);
                debug_log!("tray icon added (tip='{}')", tip);
            }

            let mut loop_count = 0u64;
            loop {
                let mut msg = std::mem::MaybeUninit::<MSG>::uninit();
                let has_msg = unsafe { PeekMessageW(msg.as_mut_ptr(), 0, 0, 0, PM_REMOVE) } != 0;

                if has_msg {
                    let msg = unsafe { msg.assume_init() };
                    if msg.message == WM_QUIT {
                        debug_log!("BACKGROUND THREAD: received WM_QUIT – exiting");
                        break;
                    }
                    if msg.message == WM_HOTKEY {
                        debug_log!(
                            "WM_HOTKEY TRIGGERED at loop iteration {} ({})",
                            loop_count,
                            tip_hotkey
                        );
                        cancel_sysmenu_mode();
                        toggle_launcher();
                    } else {
                        unsafe { DispatchMessageW(&msg as *const MSG) };
                    }
                }

                // LL hook only sets a flag — toggle here so the hook stays fast.
                if hotkey::take_pending_toggle() {
                    debug_log!("LL hook pending — toggling launcher");
                    cancel_sysmenu_mode();
                    toggle_launcher();
                }

                // Apply DWM effects once the Iced window is created — keep it hidden
                // until the user presses the hotkey.
                if !DWM_APPLIED.load(Ordering::Relaxed) {
                    let hwnd = find_own_launcher_hwnd();
                    if hwnd != 0 {
                        WINDOW_FOUND.store(true, Ordering::SeqCst);
                        debug_log!("Iced window found (hwnd={}) – applying DWM effects", hwnd);
                        apply_window_effects(hwnd);
                        ShowWindow(hwnd, SW_HIDE);
                        LAUNCHER_SHOWN.store(false, Ordering::SeqCst);
                        DWM_APPLIED.store(true, Ordering::SeqCst);
                        let vis = IsWindowVisible(hwnd);
                        debug_log!("startup: forced hidden, IsWindowVisible={}", vis);
                    } else if loop_count.is_multiple_of(100) {
                        debug_log!(
                            "waiting for Iced window to be created... (loop {})",
                            loop_count
                        );
                    }
                }

                if HIDE_REQUESTED.swap(false, Ordering::SeqCst) {
                    let hwnd = find_own_launcher_hwnd();
                    debug_log!("HIDE_REQUESTED: hwnd={}", hwnd);
                    if hwnd != 0 {
                        hide_launcher(hwnd);
                    }
                }

                if RESIZE_REQUESTED.swap(false, Ordering::SeqCst) {
                    let hwnd = find_own_launcher_hwnd();
                    if hwnd != 0 {
                        let ww = WINDOW_WIDTH.load(Ordering::Relaxed) as i32;
                        let h = RESIZE_HEIGHT.load(Ordering::Relaxed) as i32;
                        debug_log!("RESIZE_REQUESTED: SetWindowPos({}x{})", ww, h);
                        SetWindowPos(hwnd, 0, 0, 0, ww, h, SWP_NOZORDER | SWP_NOMOVE);
                    }
                }

                if !has_msg && !hotkey::has_pending_toggle() {
                    // Keep the LL-hook thread responsive; Windows may drop slow hooks.
                    let delay = if hotkey::hook_active() { 10 } else { 50 };
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
                loop_count += 1;
            }

            hotkey::uninstall();
            debug_log!("BACKGROUND THREAD: exited message loop");
        });
    }

    debug_log!("starting Iced application...");

    let window_icon =
        iced::window::icon::from_file_data(include_bytes!("../brandkit/windows/element.ico"), None)
            .ok();
    debug_log!("window icon loaded: {}", window_icon.is_some());

    iced::application("Element", ui::update, ui::view)
        .theme(|_| Theme::Dark)
        .window(window::Settings {
            decorations: false,
            level: window::Level::AlwaysOnTop,
            size: init_win_size,
            visible: false,
            icon: window_icon,
            ..Default::default()
        })
        .subscription(ui::subscription)
        .run_with(move || {
            let db = Arc::new(Database::new());
            let engine = Orchestrator::new(config, db);
            debug_log!("Iced application started – Orchestrator initialized");
            (
                ui::ElementApp {
                    engine,
                    input: String::new(),
                    results: Vec::new(),
                    selected_index: -1,
                    status: None,
                    search_revision: 0,
                },
                iced::Task::none(),
            )
        })
}
