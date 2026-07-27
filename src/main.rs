#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod database;

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

slint::include_modules!();
use slint::{Image, Model, Rgba8Pixel, SharedPixelBuffer};

use crate::app::SearchEngine;

static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
static ESCAPE_TRIGGERED: AtomicBool = AtomicBool::new(false);
static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false);

fn main() {
    let config = config::Config::load();
    let db = Arc::new(database::Database::new());

    install_hotkey(&config.hotkey);

    let ui = SearchWindow::new().unwrap();

    let engine = SearchEngine::new(&config, db.clone());
    let _ = ui.window().hide();

    #[cfg(target_os = "windows")]
    apply_dwm_blur();

    let engine_hot = engine.clone();
    let ui_hot = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(200),
        move || {
            if HOTKEY_TRIGGERED.swap(false, Ordering::Relaxed) {
                if let Some(ui) = ui_hot.upgrade() {
                    if ui.window().is_visible() {
                        let _ = ui.window().hide();
                        WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                    } else {
                        engine_hot.refresh_apps();
                        let _ = ui.window().show();
                        WINDOW_VISIBLE.store(true, Ordering::Relaxed);
                        ui.set_input_text(String::new().into());
                        ui.set_selected_index(-1);
                        ui.set_results(
                            Rc::new(slint::VecModel::from(vec![])).into(),
                        );
                    }
                }
            }
            if ESCAPE_TRIGGERED.swap(false, Ordering::Relaxed) {
                if let Some(ui) = ui_hot.upgrade() {
                    let _ = ui.window().hide();
                    WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                }
            }
        },
    );

    let engine_s = engine.clone();
    let ui_s = ui.as_weak();
    let debounce_timer = Rc::new(slint::Timer::default());
    let debounce_ms = config.debounce_delay_ms;
    let dt = debounce_timer.clone();
    ui.on_input_changed(move |text| {
        if let Some(ui) = ui_s.upgrade() {
            let es = engine_s.clone();
            dt.stop();
            let dt_inner = dt.clone();
            dt_inner.start(
                slint::TimerMode::SingleShot,
                std::time::Duration::from_millis(debounce_ms),
                move || {
                    let results = es.search(&text);
                    let model: Vec<ResultItem> = results
                        .into_iter()
                        .map(|r| {
                            let icon = r.icon_rgba.and_then(|(data, w, h)| {
                                rgba_to_image(&data, w, h)
                            });
                            ResultItem {
                                title: r.title.into(),
                                subtitle: r.subtitle.into(),
                                kind: r.kind.into(),
                                icon: icon.unwrap_or_default(),
                            }
                        })
                        .collect();
                    ui.set_results(Rc::new(slint::VecModel::from(model)).into());
                    ui.set_selected_index(if text.is_empty() { -1 } else { 0 });
                },
            );
        }
    });

    let engine_a = engine.clone();
    let ui_a = ui.as_weak();
    ui.on_item_selected(move |index| {
        if let Some(ui) = ui_a.upgrade() {
            let items = ui.get_results();
            if index >= 0 && (index as usize) < items.row_count() {
                let item = items.row_data(index as usize).unwrap();
                engine_a.activate(&item.kind, &item.title, &ui.get_input_text());
            }
            let _ = ui.window().hide();
            WINDOW_VISIBLE.store(false, Ordering::Relaxed);
        }
    });

    let ui_h = ui.as_weak();
    ui.on_request_hide(move || {
        if let Some(ui) = ui_h.upgrade() {
            let _ = ui.window().hide();
            WINDOW_VISIBLE.store(false, Ordering::Relaxed);
        }
    });

    ui.run().unwrap();
}

fn rgba_to_image(data: &[u8], width: u32, height: u32) -> Option<Image> {
    if data.len() < (width * height * 4) as usize {
        return None;
    }
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let slice = buf.make_mut_slice();
    for (i, pixel) in slice.iter_mut().enumerate() {
        let off = i * 4;
        *pixel = Rgba8Pixel::new(data[off], data[off + 1], data[off + 2], data[off + 3]);
    }
    Some(Image::from_rgba8(buf))
}

#[cfg(target_os = "windows")]
fn apply_dwm_blur() {
    use std::sync::atomic::AtomicBool;
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(
            f: unsafe extern "system" fn(isize, isize) -> i32,
            l: isize,
        ) -> i32;
        fn GetWindowThreadProcessId(h: isize, p: *mut u32) -> u32;
    }
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            h: isize,
            a: u32,
            v: *const std::ffi::c_void,
            s: u32,
        ) -> i32;
    }
    unsafe {
        static mut HWND: isize = 0;
        unsafe extern "system" fn ep(h: isize, _: isize) -> i32 {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(h, &mut pid);
            if pid == std::process::id() {
                HWND = h;
                return 0;
            }
            1
        }
        EnumWindows(ep, 0);
        if HWND == 0 {
            DONE.store(false, Ordering::Relaxed);
            return;
        }
        let backdrop: u32 = 2;
        DwmSetWindowAttribute(
            HWND,
            38,
            &backdrop as *const _ as *const std::ffi::c_void,
            4,
        );
    }
}

#[cfg(target_os = "windows")]
fn install_hotkey(hotkey_str: &str) {
    use std::sync::atomic::AtomicU16;
    use std::sync::OnceLock;
    static KEY_VK: AtomicU16 = AtomicU16::new(0x20);
    static MOD_VKS: OnceLock<Vec<u16>> = OnceLock::new();

    let parts: Vec<&str> = hotkey_str.split('+').map(|p| p.trim()).collect();
    let mut mv = Vec::new();
    let mut kv = 0x20u16;
    for p in &parts {
        match p.to_lowercase().as_str() {
            "alt" => mv.push(0x12),
            "ctrl" | "control" => mv.push(0x11),
            "shift" => mv.push(0x10),
            "space" => kv = 0x20,
            _ => {}
        }
    }
    KEY_VK.store(kv, Ordering::Relaxed);
    let _ = MOD_VKS.set(mv);

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(
            i: i32,
            f: unsafe extern "system" fn(i32, usize, isize) -> isize,
            m: isize,
            t: u32,
        ) -> isize;
        fn CallNextHookEx(h: isize, c: i32, w: usize, l: isize) -> isize;
        fn GetModuleHandleW(m: *const u16) -> isize;
        fn GetAsyncKeyState(v: i32) -> i16;
    }

    unsafe extern "system" fn proc(c: i32, w: usize, l: isize) -> isize {
        if c >= 0 && (w as u32 == 0x100 || w as u32 == 0x104) {
            let vk = *(l as *const u32);
            if vk == KEY_VK.load(Ordering::Relaxed) as u32 {
                if let Some(mv) = MOD_VKS.get() {
                    if mv.iter().all(|v| (GetAsyncKeyState(*v as i32) as i32 & 0x8000) != 0) {
                        HOTKEY_TRIGGERED.store(true, Ordering::Relaxed);
                        return 1;
                    }
                }
            }
            if vk == 0x1B && WINDOW_VISIBLE.load(Ordering::Relaxed) {
                ESCAPE_TRIGGERED.store(true, Ordering::Relaxed);
                return 1;
            }
        }
        unsafe { CallNextHookEx(0, c, w, l) }
    }
    unsafe {
        SetWindowsHookExW(13, proc, GetModuleHandleW(std::ptr::null()), 0);
    }
}

#[cfg(not(target_os = "windows"))]
fn install_hotkey(_: &str) {}
