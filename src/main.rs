#![windows_subsystem = "windows"]

mod app;
mod config;
mod database;

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

slint::include_modules!();
use slint::{Image, Model, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::app::SearchEngine;

static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
static ESCAPE_TRIGGERED: AtomicBool = AtomicBool::new(false);
static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false);
static NAV_UP: AtomicBool = AtomicBool::new(false);
static NAV_DOWN: AtomicBool = AtomicBool::new(false);

fn main() {
    let config = config::Config::load();
    let db = Arc::new(database::Database::new());

    install_hotkey(&config.hotkey);

    let ui = SearchWindow::new().unwrap();
    let win_w = config.window_width as i32;
    let win_h = config.window_height as i32;

    let engine = SearchEngine::new(&config, db.clone());
    let _ = ui.window().hide();

    let engine_hot = engine.clone();
    let ui_hot = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(50),
        move || {
            if let Some(ui) = ui_hot.upgrade() {
                if HOTKEY_TRIGGERED.swap(false, Ordering::Relaxed) {
                    if ui.window().is_visible() {
                        let _ = ui.window().hide();
                        WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                    } else {
                        engine_hot.refresh_apps();
                        WINDOW_VISIBLE.store(true, Ordering::Relaxed);
                        ui.window().show().ok();
                        ui.set_input_text(String::new().into());
                        ui.set_selected_index(-1);
                        ui.set_results(Rc::new(VecModel::from(vec![])).into());
                        center_window(win_w, win_h);
                    }
                }
                if ESCAPE_TRIGGERED.swap(false, Ordering::Relaxed) {
                    let _ = ui.window().hide();
                    WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                }
                if NAV_UP.swap(false, Ordering::Relaxed) {
                    let idx = ui.get_selected_index();
                    let count = ui.get_result_count();
                    if count > 0 {
                        let new = if idx <= 0 { count - 1 } else { idx - 1 };
                        ui.set_selected_index(new);
                    }
                }
                if NAV_DOWN.swap(false, Ordering::Relaxed) {
                    let idx = ui.get_selected_index();
                    let count = ui.get_result_count();
                    if count > 0 {
                        let new = if idx >= count - 1 { 0 } else { idx + 1 };
                        ui.set_selected_index(new);
                    }
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
                    let count = results.len() as i32;
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
                    ui.set_result_count(count);
                    ui.set_results(Rc::new(VecModel::from(model)).into());
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

    ui.on_navigate_up(move || {});
    ui.on_navigate_down(move || {});

    if cfg!(target_os = "windows") {
        apply_dwm_blur();
    }

    ui.run().unwrap();
}

fn get_window_hwnd() -> Option<isize> {
    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(f: unsafe extern "system" fn(isize, isize) -> i32, l: isize) -> i32;
        fn GetWindowThreadProcessId(h: isize, p: *mut u32) -> u32;
    }
    let mut hwnd = None;
    unsafe {
        extern "system" fn ep(h: isize, l: isize) -> i32 {
            let mut pid: u32 = 0;
            unsafe { GetWindowThreadProcessId(h, &mut pid); }
            if pid == std::process::id() {
                unsafe { *(l as *mut Option<isize>) = Some(h); }
                return 0;
            }
            1
        }
        EnumWindows(ep, &mut hwnd as *mut _ as isize);
    }
    hwnd
}

#[cfg(target_os = "windows")]
fn center_window(w: i32, h: i32) {
    if let Some(hwnd) = get_window_hwnd() {
        #[link(name = "user32")]
        extern "system" {
            fn GetSystemMetrics(nIndex: i32) -> i32;
            fn SetWindowPos(h: isize, hAfter: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
        }
        const SM_CXSCREEN: i32 = 0;
        const SM_CYSCREEN: i32 = 1;
        const SWP_NOZORDER: u32 = 0x0004;
        const SWP_NOSIZE: u32 = 0x0001;
        unsafe {
            let cx = GetSystemMetrics(SM_CXSCREEN);
            let cy = GetSystemMetrics(SM_CYSCREEN);
            let x = (cx - w) / 2;
            let y = cy.saturating_sub(h) / 3;
            SetWindowPos(hwnd, 0, x, y, 0, 0, SWP_NOZORDER | SWP_NOSIZE);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn center_window(_w: i32, _h: i32) {}

fn rgba_to_image(data: &[u8], width: u32, height: u32) -> Option<Image> {
    if data.len() < (width * height * 4) as usize {
        return None;
    }
    // skip if all-white icon (failed extraction)
    let count = data.chunks_exact(4).filter(|p| p[0] == 255 && p[1] == 255 && p[2] == 255 && p[3] == 255).count();
    if count as u32 == width * height {
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
    if let Some(hwnd) = get_window_hwnd() {
        #[link(name = "dwmapi")]
        extern "system" {
            fn DwmSetWindowAttribute(h: isize, a: u32, v: *const std::ffi::c_void, s: u32) -> i32;
        }
        unsafe {
            let backdrop: u32 = 2;
            DwmSetWindowAttribute(hwnd, 38, &backdrop as *const _ as *const std::ffi::c_void, 4);
        }
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
        fn SetWindowsHookExW(i: i32, f: unsafe extern "system" fn(i32, usize, isize) -> isize, m: isize, t: u32) -> isize;
        fn CallNextHookEx(h: isize, c: i32, w: usize, l: isize) -> isize;
        fn GetModuleHandleW(m: *const u16) -> isize;
        fn GetAsyncKeyState(v: i32) -> i16;
    }

    unsafe extern "system" fn proc(c: i32, w: usize, l: isize) -> isize {
        if c >= 0 && (w as u32 == 0x100 || w as u32 == 0x104) {
            let vk = *(l as *const u32);
            let vis = WINDOW_VISIBLE.load(Ordering::SeqCst);
            if vis && vk == 0x1B {
                ESCAPE_TRIGGERED.store(true, Ordering::SeqCst);
                return 1;
            }
            if vis {
                match vk {
                    0x26 => { NAV_UP.store(true, Ordering::SeqCst); return 1; }
                    0x28 => { NAV_DOWN.store(true, Ordering::SeqCst); return 1; }
                    _ => {}
                }
            }
            if vk == KEY_VK.load(Ordering::Relaxed) as u32 {
                if let Some(mv) = MOD_VKS.get() {
                    if mv.iter().all(|v| (GetAsyncKeyState(*v as i32) as i32 & 0x8000) != 0) {
                        HOTKEY_TRIGGERED.store(true, Ordering::Relaxed);
                        return 1;
                    }
                }
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
