#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

mod app;
mod config;
mod database;
mod debug_log;
mod error;
mod providers;
mod registry;
pub(crate) mod theme;
mod ui;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use iced::{window, Theme};

use crate::app::SearchEngine;
use crate::database::Database;

pub(crate) static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
pub(crate) static HIDE_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static RESIZE_HEIGHT: AtomicU32 = AtomicU32::new(56);
pub(crate) static RESIZE_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static WINDOW_WIDTH: AtomicU32 = AtomicU32::new(580);
pub(crate) static HOTKEY_REGISTERED: AtomicBool = AtomicBool::new(false);
pub(crate) static WINDOW_FOUND: AtomicBool = AtomicBool::new(false);

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
    debug_log!("screen: {}x{}, position: ({}, {}), width: {}", scr_w, scr_h, x, y, width);

    HIDE_REQUESTED.store(false, Ordering::SeqCst);
    let ret1 = ShowWindow(hwnd, SW_RESTORE);
    debug_log!("ShowWindow(SW_RESTORE) returned: {}", ret1);
    let ret2 = SetWindowPos(hwnd, HWND_TOPMOST, x, y, 0, 0, SWP_NOSIZE | SWP_SHOWWINDOW);
    debug_log!("SetWindowPos returned: {}", ret2);
    let ret3 = SetForegroundWindow(hwnd);
    debug_log!("SetForegroundWindow returned: {}", ret3);
    HOTKEY_TRIGGERED.store(true, Ordering::SeqCst);
    debug_log!("HOTKEY_TRIGGERED set to true");
}

// ── DWM acrylic blur + rounded corners ──────────────────────────────────

#[cfg(target_os = "windows")]
fn apply_window_effects(hwnd: isize) {
    apply_dwm_rounded_corners(hwnd);
    apply_acrylic_blur(hwnd);
    let vis = IsWindowVisible(hwnd);
    debug_log!("window effects applied, IsWindowVisible={}", vis);
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

#[cfg(target_os = "windows")]
fn apply_acrylic_blur(hwnd: isize) {
    const WS_EX_LAYERED: u32 = 0x00080000;
    const GWL_EXSTYLE: i32 = -20;

    // Load SetWindowCompositionAttribute dynamically for compatibility
    type SWCAFn = unsafe extern "system" fn(isize, *mut WINDOWCOMPOSITIONATTRIBDATA) -> i32;

    unsafe {
        let module = GetModuleHandleW(
            "user32\0".encode_utf16().collect::<Vec<_>>().as_ptr(),
        );
        if module == 0 {
            debug_log!("GetModuleHandleW(user32) failed — acrylic not available");
            return;
        }
        let fn_name = c"SetWindowCompositionAttribute".as_ptr();
        let fn_addr = GetProcAddress(module, fn_name);
        if fn_addr.is_null() {
            debug_log!("SetWindowCompositionAttribute not found — acrylic not available");
            return;
        }
        let func: SWCAFn = std::mem::transmute(fn_addr);

        const WCA_ACCENT_POLICY: u32 = 19;
        const ACCENT_ENABLE_ACRYLIC_BLURBEHIND: u32 = 4;

        let mut data = WINDOWCOMPOSITIONATTRIBDATA {
            attribute: WCA_ACCENT_POLICY,
            data: &ACCENTPOLICY {
                accent_state: ACCENT_ENABLE_ACRYLIC_BLURBEHIND,
                accent_flags: 0,
                gradient_color: 0x7F3C3C3C, // 50% opacity #3c3c3c
                animation_id: 0,
            },
            size: std::mem::size_of::<ACCENTPOLICY>() as u32,
        };

        // Call SetWindowCompositionAttribute FIRST — do NOT pre-set WS_EX_LAYERED.
        // The API manages the layered state internally. Setting it manually before
        // the call can leave the window invisible if the API fails.
        let ret = func(hwnd, &mut data);
        if ret == 0 {
            debug_log!("SetWindowCompositionAttribute FAILED — acrylic blur not available on this system");
            debug_log!("window will use Iced-rendered background instead (still visible)");
            return;
        }
        debug_log!("SetWindowCompositionAttribute succeeded (ret={})", ret);

        // Only now, after the API succeeded, ensure WS_EX_LAYERED is set
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        debug_log!("window ex_style after composition: {:#x}", ex_style);
        if (ex_style & WS_EX_LAYERED as isize) == 0 {
            let new_style = ex_style | WS_EX_LAYERED as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
            debug_log!("WS_EX_LAYERED added to window");
        }
        debug_log!("acrylic blur applied successfully — 60px blur, 50% opacity #3c3c3c");
    }
}

#[repr(C)]
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
struct ACCENTPOLICY {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
struct WINDOWCOMPOSITIONATTRIBDATA {
    attribute: u32,
    data: *const ACCENTPOLICY,
    size: u32,
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
    const TPM_RIGHTBUTTON: u32 = 2;

    match msg {
        WM_APP => {
            let mouse_msg = lparam as u32;
            if mouse_msg == WM_LBUTTONDOWN {
                let title = [69u16, 108, 101, 109, 101, 110, 116, 0];
                let h = FindWindowW(std::ptr::null(), title.as_ptr());
                debug_log!("tray left-click: FindWindowW returned {}", h);
                if h != 0 {
                    let vis = IsWindowVisible(h);
                    debug_log!("tray toggle: IsWindowVisible = {}", vis);
                    if vis != 0 {
                        hide_launcher(h);
                    } else {
                        show_launcher(h);
                    }
                }
            } else if mouse_msg == WM_RBUTTONUP {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                let menu = CreatePopupMenu();
                let exit_text = [69u16, 120, 105, 116, 0];
                AppendMenuW(menu, 0, ID_EXIT, exit_text.as_ptr());
                SetForegroundWindow(hwnd);
                TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
                DestroyMenu(menu);
            }
            0
        }
        WM_COMMAND => {
            if (wparam & 0xFFFF) == ID_EXIT {
                debug_log!("tray exit clicked");
                EXIT_REQUESTED.store(true, Ordering::SeqCst);
                PostQuitMessage(0);
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
extern "system" fn FindWindowW(cn: *const u16, wn: *const u16) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(cn: *const u16, wn: *const u16) -> isize;
    }
    unsafe { FindWindowW(cn, wn) }
}

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
extern "system" fn GetWindowLongPtrW(hWnd: isize, nIndex: i32) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn GetWindowLongPtrW(hWnd: isize, nIndex: i32) -> isize;
    }
    unsafe { GetWindowLongPtrW(hWnd, nIndex) }
}

#[cfg(target_os = "windows")]
extern "system" fn SetWindowLongPtrW(hWnd: isize, nIndex: i32, dwNewLong: isize) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowLongPtrW(hWnd: isize, nIndex: i32, dwNewLong: isize) -> isize;
    }
    unsafe { SetWindowLongPtrW(hWnd, nIndex, dwNewLong) }
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
extern "system" fn GetProcAddress(hModule: isize, lpProcName: *const i8) -> *mut std::ffi::c_void {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetProcAddress(hModule: isize, lpProcName: *const i8) -> *mut std::ffi::c_void;
    }
    unsafe { GetProcAddress(hModule, lpProcName) }
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

pub fn main() -> iced::Result {
    debug_log::init();
    debug_log!("=== Element v{} starting ===", env!("CARGO_PKG_VERSION"));

    let config = Arc::new(config::Config::load());
    WINDOW_WIDTH.store(config.window_width as u32, Ordering::Relaxed);
    let init_win_size = iced::Size::new(config.window_width, 56.0);
    debug_log!("config loaded, window_width={}", config.window_width);

    static DWM_APPLIED: AtomicBool = AtomicBool::new(false);

    #[cfg(target_os = "windows")]
    {
        let window_title = "Element\0".encode_utf16().collect::<Vec<_>>();
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
                fn RegisterHotKey(hWnd: isize, id: i32, fsModifiers: u32, vk: u32) -> i32;
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
            const MOD_ALT: u32 = 0x0001;
            const MOD_NOREPEAT: u32 = 0x4000;
            const VK_SPACE: u32 = 0x20;
            const PM_REMOVE: u32 = 0x0001;
            const NIM_ADD: u32 = 0;
            const NIF_MESSAGE: u32 = 1;
            const NIF_ICON: u32 = 2;
            const NIF_TIP: u32 = 4;
            const HWND_MESSAGE: isize = -3;

            // Register Alt+Space hotkey
            let hk_ret = unsafe { RegisterHotKey(0, 1, MOD_ALT | MOD_NOREPEAT, VK_SPACE) };
            if hk_ret == 0 {
                debug_log!("CRITICAL: RegisterHotKey(Alt+Space) FAILED – another app has the hotkey");
                debug_log!("Check: PowerToys, Teams, Spotlight, or other launcher may own Alt+Space");
                HOTKEY_REGISTERED.store(false, Ordering::SeqCst);
            } else {
                debug_log!("RegisterHotKey(Alt+Space) SUCCESS");
                HOTKEY_REGISTERED.store(true, Ordering::SeqCst);
            }

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
                    hIcon: LoadIconW(0, 32512 as *const u16),
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
                let tip = [
                    69u16, 108, 101, 109, 101, 110, 116, 32, 76, 97, 117, 110, 99, 104, 101, 114,
                ];
                let mut i = 0;
                while i < tip.len() {
                    nid.szTip[i] = tip[i];
                    i += 1;
                }
                Shell_NotifyIconW(NIM_ADD, &mut nid);
                debug_log!("tray icon added");
            }

            let mut loop_count = 0u64;
            loop {
                let mut msg = std::mem::MaybeUninit::<MSG>::uninit();
                let has_msg =
                    unsafe { PeekMessageW(msg.as_mut_ptr(), 0, 0, 0, PM_REMOVE) } != 0;

                if has_msg {
                    let msg = unsafe { msg.assume_init() };
                    if msg.message == WM_QUIT {
                        debug_log!("BACKGROUND THREAD: received WM_QUIT – exiting");
                        break;
                    }
                    if msg.message == WM_HOTKEY {
                        debug_log!("WM_HOTKEY (Alt+Space) TRIGGERED at loop iteration {}", loop_count);
                        let hwnd = FindWindowW(std::ptr::null(), window_title.as_ptr());
                        debug_log!("FindWindowW returned hwnd={}", hwnd);
                        if hwnd != 0 {
                            WINDOW_FOUND.store(true, Ordering::SeqCst);
                            let vis = IsWindowVisible(hwnd);
                            debug_log!("IsWindowVisible={}", vis);
                            if vis != 0 {
                                debug_log!("ACTION: hiding launcher (was visible)");
                                hide_launcher(hwnd);
                            } else {
                                debug_log!("ACTION: showing launcher (was hidden)");
                                show_launcher(hwnd);
                            }
                        } else {
                            WINDOW_FOUND.store(false, Ordering::SeqCst);
                            debug_log!("CRITICAL: FindWindowW returned 0 – Iced window not yet created or title mismatch");
                            debug_log!("Iced window title must be exactly 'Element' for FindWindowW to find it");
                        }
                    } else {
                        unsafe { DispatchMessageW(&msg as *const MSG) };
                    }
                }

                // Apply DWM effects once the Iced window is created
                if !DWM_APPLIED.load(Ordering::Relaxed) {
                    let hwnd = FindWindowW(std::ptr::null(), window_title.as_ptr());
                    if hwnd != 0 {
                        WINDOW_FOUND.store(true, Ordering::SeqCst);
                        debug_log!("Iced window found (hwnd={}) – applying DWM effects", hwnd);
                        apply_window_effects(hwnd);
                        DWM_APPLIED.store(true, Ordering::SeqCst);
                        let vis = IsWindowVisible(hwnd);
                        debug_log!("after DWM effects: IsWindowVisible={}", vis);
                    } else if loop_count.is_multiple_of(100) {
                        debug_log!("waiting for Iced window to be created... (loop {})", loop_count);
                    }
                }

                if HIDE_REQUESTED.swap(false, Ordering::SeqCst) {
                    let hwnd = FindWindowW(std::ptr::null(), window_title.as_ptr());
                    debug_log!("HIDE_REQUESTED: FindWindowW returned {}", hwnd);
                    if hwnd != 0 {
                        hide_launcher(hwnd);
                    }
                }

                if RESIZE_REQUESTED.swap(false, Ordering::SeqCst) {
                    let hwnd = FindWindowW(std::ptr::null(), window_title.as_ptr());
                    if hwnd != 0 {
                        let ww = WINDOW_WIDTH.load(Ordering::Relaxed) as i32;
                        let h = RESIZE_HEIGHT.load(Ordering::Relaxed) as i32;
                        debug_log!("RESIZE_REQUESTED: SetWindowPos({}x{})", ww, h);
                        SetWindowPos(hwnd, 0, 0, 0, ww, h, SWP_NOZORDER | SWP_NOMOVE);
                    }
                }

                if !has_msg {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                loop_count += 1;
            }
            debug_log!("BACKGROUND THREAD: exited message loop");
        });
    }

    debug_log!("starting Iced application...");

    iced::application("Element", ui::update, ui::view)
        .theme(|_| Theme::Dark)
        .window(window::Settings {
            decorations: false,
            level: window::Level::AlwaysOnTop,
            size: init_win_size,
            visible: false,
            ..Default::default()
        })
        .subscription(ui::subscription)
        .run_with(move || {
            let db = Arc::new(Database::new());
            let engine = SearchEngine::new(config, db);
            debug_log!("Iced application started – SearchEngine initialized");
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