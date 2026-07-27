#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

mod app;
mod config;
mod database;
mod error;
mod providers;
mod registry;
mod ui;
pub(crate) mod theme;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use iced::{window, Theme};

use crate::app::SearchEngine;
use crate::database::Database;

pub(crate) static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
pub(crate) static HIDE_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static VISIBLE: AtomicBool = AtomicBool::new(false);
pub(crate) static RESIZE_HEIGHT: AtomicU32 = AtomicU32::new(56);
pub(crate) static RESIZE_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static WINDOW_WIDTH: AtomicU32 = AtomicU32::new(580);

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
                let title = [69u16, 108, 101, 109, 101, 110, 116, 0]; // "Element\0"
                let h = FindWindowW(std::ptr::null(), title.as_ptr());
                if h != 0 {
                    let was = VISIBLE.swap(true, Ordering::SeqCst);
                    if was {
                        ShowWindow(h, 0);
                        VISIBLE.store(false, Ordering::SeqCst);
                    } else {
                        ShowWindow(h, 8);
                        HOTKEY_TRIGGERED.store(true, Ordering::SeqCst);
                    }
                }
            } else if mouse_msg == WM_RBUTTONUP {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                let menu = CreatePopupMenu();
                let exit_text = [69u16, 120, 105, 116, 0]; // "Exit\0"
                AppendMenuW(menu, 0, ID_EXIT, exit_text.as_ptr());
                SetForegroundWindow(hwnd);
                TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
                DestroyMenu(menu);
            }
            0
        }
        WM_COMMAND => {
            if (wparam & 0xFFFF) == ID_EXIT {
                PostQuitMessage(0);
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

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
extern "system" fn CreatePopupMenu() -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn CreatePopupMenu() -> isize;
    }
    unsafe { CreatePopupMenu() }
}

#[cfg(target_os = "windows")]
extern "system" fn AppendMenuW(hMenu: isize, uFlags: u32, uIDNewItem: usize, lpNewItem: *const u16) -> i32 {
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
extern "system" fn TrackPopupMenu(hMenu: isize, uFlags: u32, x: i32, y: i32, nReserved: i32, hWnd: isize, prcRect: *const std::ffi::c_void) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn TrackPopupMenu(hMenu: isize, uFlags: u32, x: i32, y: i32, nReserved: i32, hWnd: isize, prcRect: *const std::ffi::c_void) -> i32;
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
    x: i32, y: i32,
    nWidth: i32, nHeight: i32,
    hWndParent: isize,
    hMenu: isize,
    hInstance: isize,
    lpParam: *const std::ffi::c_void,
) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn CreateWindowExW(
            dwExStyle: u32, lpClassName: *const u16, lpWindowName: *const u16, dwStyle: u32,
            x: i32, y: i32, nWidth: i32, nHeight: i32, hWndParent: isize, hMenu: isize,
            hInstance: isize, lpParam: *const std::ffi::c_void,
        ) -> isize;
    }
    unsafe { CreateWindowExW(dwExStyle, lpClassName, lpWindowName, dwStyle, x, y, nWidth, nHeight, hWndParent, hMenu, hInstance, lpParam) }
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

#[repr(C)]
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
struct POINT {
    x: i32,
    y: i32,
}

pub fn main() -> iced::Result {
    let config = Arc::new(config::Config::load());
    WINDOW_WIDTH.store(config.window_width as u32, Ordering::Relaxed);
    let init_win_size = iced::Size::new(config.window_width, 56.0);

    #[cfg(target_os = "windows")]
    {
        let window_title = "Element\0".encode_utf16().collect::<Vec<_>>();
        std::thread::spawn(move || {
            #[repr(C)]
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
                fn PeekMessageW(msg: *mut MSG, hWnd: isize, wMsgFilterMin: u32, wMsgFilterMax: u32, wRemoveMsg: u32) -> i32;
                fn DispatchMessageW(msg: *const MSG) -> isize;
                fn SetWindowPos(h: isize, ha: isize, x: i32, y: i32, cx: i32, cy: i32, f: u32) -> i32;
                fn GetSystemMetrics(n: i32) -> i32;
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
            unsafe { RegisterHotKey(0, 1, MOD_ALT | MOD_NOREPEAT, VK_SPACE); }

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
                    0, class_name.as_ptr(), std::ptr::null(), 0,
                    0, 0, 0, 0, HWND_MESSAGE, 0,
                    GetModuleHandleW(std::ptr::null()), std::ptr::null(),
                );

                let mut nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: tray_hwnd,
                    uID: 1,
                    uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                    uCallbackMessage: 0x8000, // WM_APP
                    hIcon: LoadIconW(0, 32512 as *const u16), // IDI_APPLICATION
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
                let tip = [69u16, 108, 101, 109, 101, 110, 116, 32, 76, 97, 117, 110, 99, 104, 101, 114];
                let mut i = 0;
                while i < tip.len() {
                    nid.szTip[i] = tip[i];
                    i += 1;
                }
                Shell_NotifyIconW(NIM_ADD, &mut nid);
            }

            loop {
                let mut msg = std::mem::MaybeUninit::<MSG>::uninit();
                let has_msg = unsafe { PeekMessageW(msg.as_mut_ptr(), 0, 0, 0, PM_REMOVE) } != 0;

                if has_msg {
                    let msg = unsafe { msg.assume_init() };
                    if msg.message == WM_QUIT {
                        break;
                    }
                    if msg.message == WM_HOTKEY {
                        let hwnd = FindWindowW(std::ptr::null(), window_title.as_ptr());
                        if hwnd != 0 {
                            let was = VISIBLE.swap(true, Ordering::SeqCst);
                            if was {
                                ShowWindow(hwnd, 0);
                                VISIBLE.store(false, Ordering::SeqCst);
                            } else {
                                let ww = WINDOW_WIDTH.load(Ordering::Relaxed) as i32;
                                unsafe {
                                    ShowWindow(hwnd, 8);
                                    let cx = GetSystemMetrics(0);
                                    let cy = GetSystemMetrics(1);
                                    let x = (cx - ww) / 2;
                                    let y = cy.saturating_sub(420) / 3;
                                    SetWindowPos(hwnd, 0, x, y, ww, 0, 0x0004 | 0x0001);
                                }
                                HOTKEY_TRIGGERED.store(true, Ordering::SeqCst);
                            }
                        }
                    } else {
                        unsafe { DispatchMessageW(&msg as *const MSG) };
                    }
                }

                if HIDE_REQUESTED.swap(false, Ordering::SeqCst) {
                    let hwnd = FindWindowW(std::ptr::null(), window_title.as_ptr());
                    if hwnd != 0 {
                        ShowWindow(hwnd, 0);
                        VISIBLE.store(false, Ordering::SeqCst);
                    }
                }

                if RESIZE_REQUESTED.swap(false, Ordering::SeqCst) {
                    let hwnd = FindWindowW(std::ptr::null(), window_title.as_ptr());
                    if hwnd != 0 {
                        let ww = WINDOW_WIDTH.load(Ordering::Relaxed) as i32;
                        let h = RESIZE_HEIGHT.load(Ordering::Relaxed) as i32;
                        unsafe { SetWindowPos(hwnd, 0, 0, 0, ww, h, 0x0004 | 0x0002) };
                    }
                }

                if !has_msg {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        });
    }

    iced::application("Element", ui::update, ui::view)
        .theme(|_| Theme::Light)
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
            (
                ui::ElementApp {
                    engine,
                    input: String::new(),
                    results: Vec::new(),
                    selected_index: -1,
                },
                iced::Task::none(),
            )
        })
}
