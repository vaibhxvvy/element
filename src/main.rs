#![windows_subsystem = "windows"]

mod app;
mod config;
mod database;
mod ui;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use iced::{window, Theme};

use crate::app::SearchEngine;

pub(crate) static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
pub(crate) static HOTKEY_ARMED: AtomicBool = AtomicBool::new(true);
pub(crate) static HIDE_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static VISIBLE: AtomicBool = AtomicBool::new(false);
pub(crate) static RESIZE_HEIGHT: AtomicU32 = AtomicU32::new(56);
pub(crate) static RESIZE_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn main() -> iced::Result {
    #[cfg(target_os = "windows")]
    std::thread::spawn(|| {
        #[link(name = "user32")]
        extern "system" {
            fn GetAsyncKeyState(v: i32) -> i16;
            fn FindWindowW(cn: *const u16, wn: *const u16) -> isize;
            fn ShowWindow(h: isize, n: i32) -> i32;
            fn SetWindowPos(h: isize, ha: isize, x: i32, y: i32, cx: i32, cy: i32, f: u32) -> i32;
            fn GetSystemMetrics(n: i32) -> i32;
        }
        loop {
            let alt_down = (unsafe { GetAsyncKeyState(0x12) } as i32 & 0x8000) != 0;
            let space_new = (unsafe { GetAsyncKeyState(0x20) } as i32 & 1) != 0;

            if HOTKEY_ARMED.load(Ordering::Relaxed) && alt_down && space_new {
                HOTKEY_ARMED.store(false, Ordering::Relaxed);
                let wide: Vec<u16> = "Element\0".encode_utf16().collect();
                let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
                if hwnd != 0 {
                    let was = VISIBLE.swap(true, Ordering::SeqCst);
                    if was {
                        unsafe { ShowWindow(hwnd, 0) };
                        VISIBLE.store(false, Ordering::SeqCst);
                    } else {
                        unsafe {
                            ShowWindow(hwnd, 8);
                            let cx = GetSystemMetrics(0);
                            let cy = GetSystemMetrics(1);
                            let x = (cx - 580) / 2;
                            let y = cy.saturating_sub(420) / 3;
                            SetWindowPos(hwnd, 0, x, y, 0, 0, 0x0004 | 0x0001);
                        }
                        HOTKEY_TRIGGERED.store(true, Ordering::SeqCst);
                    }
                }
            }
            if !alt_down {
                HOTKEY_ARMED.store(true, Ordering::Relaxed);
            }

            if HIDE_REQUESTED.swap(false, Ordering::SeqCst) {
                let wide: Vec<u16> = "Element\0".encode_utf16().collect();
                let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
                if hwnd != 0 {
                    unsafe { ShowWindow(hwnd, 0) };
                    VISIBLE.store(false, Ordering::SeqCst);
                }
            }

            if RESIZE_REQUESTED.swap(false, Ordering::SeqCst) {
                let wide: Vec<u16> = "Element\0".encode_utf16().collect();
                let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
                if hwnd != 0 {
                    let h = RESIZE_HEIGHT.load(Ordering::Relaxed) as i32;
                    unsafe { SetWindowPos(hwnd, 0, 0, 0, 580, h, 0x0004 | 0x0002) };
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });

    iced::application("Element", ui::update, ui::view)
        .theme(|_| Theme::Light)
        .window(window::Settings {
            decorations: false,
            level: window::Level::AlwaysOnTop,
            size: iced::Size::new(580.0, 56.0),
            visible: false,
            ..Default::default()
        })
        .subscription(ui::subscription)
        .run_with(|| {
            let config = config::Config::load();
            let db = Arc::new(database::Database::new());
            let engine = SearchEngine::new(&config, db);
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
