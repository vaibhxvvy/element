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

/// Current system master volume, 0-100, via Core Audio
/// (`IAudioEndpointVolume`). This is the same volume the Windows volume
/// flyout shows — `waveOut` only controls a legacy mixer and does NOT
/// change the real system volume on modern Windows.
pub fn system_volume() -> Result<u32, String> {
    match volume_access(None) {
        Ok(Some(level)) => Ok(level),
        Ok(None) => Err("no default audio endpoint".into()),
        Err(e) => Err(e),
    }
}

/// Set the system master volume (clamped to 0-100) via Core Audio.
/// 0 is effectively mute; setting any level > 0 also unmutes.
pub fn set_system_volume(level: u32) -> Result<(), String> {
    match volume_access(Some(level.min(100))) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Turn the display off (any input wakes it back up).
///
/// Debounced: a second call within 2 s is ignored so a double-Enter can't
/// toggle the monitor back on. Uses `SendMessageTimeoutW` (abort if hung)
/// instead of `PostMessageW` so the monitor command is delivered exactly
/// once and cannot be re-sent by a queued message on hide.
pub fn turn_screen_off() -> Result<(), String> {
    static LAST_OFF_MS: AtomicU64 = AtomicU64::new(0);
    const DEBOUNCE_MS: u64 = 2000;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if now.wrapping_sub(LAST_OFF_MS.load(Ordering::SeqCst)) < DEBOUNCE_MS {
        return Ok(());
    }
    LAST_OFF_MS.store(now, Ordering::SeqCst);

    // HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER (off = 2),
    // SMTO_ABORTIFHUNG, 300 ms timeout.
    let mut result: isize = 0;
    unsafe {
        SendMessageTimeoutW(
            -1,
            0x0112,
            0xF170,
            2,
            0x0002,
            300,
            &mut result as *mut isize,
        )
    };
    Ok(())
}

/// Capture the whole virtual desktop (all monitors) as RGBA pixels.
/// Returns `(rgba, width, height)` in physical pixels.
pub fn capture_screen_bitmap() -> Result<(Vec<u8>, u32, u32), String> {
    let x = unsafe { GetSystemMetrics(76) }; // SM_XVIRTUALSCREEN
    let y = unsafe { GetSystemMetrics(77) }; // SM_YVIRTUALSCREEN
    let w = unsafe { GetSystemMetrics(78) }; // SM_CXVIRTUALSCREEN
    let h = unsafe { GetSystemMetrics(79) }; // SM_CYVIRTUALSCREEN
    if w <= 0 || h <= 0 {
        return Err("no display available".into());
    }
    let hdc = unsafe { GetDC(0) };
    if hdc == 0 {
        return Err("GetDC failed".into());
    }
    let mem_dc = unsafe { CreateCompatibleDC(hdc) };
    let bmp = unsafe { CreateCompatibleBitmap(hdc, w, h) };
    let prev = unsafe { SelectObject(mem_dc, bmp) };
    // SRCCOPY | CAPTUREBLT (captures layered windows too)
    let copied = unsafe { BitBlt(mem_dc, 0, 0, w, h, hdc, x, y, 0x40CC0020) };
    if copied == 0 {
        unsafe {
            SelectObject(mem_dc, prev);
            DeleteObject(bmp);
            DeleteDC(mem_dc);
            ReleaseDC(0, hdc);
        }
        return Err("BitBlt failed".into());
    }
    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    let mut bmi = BITMAPINFO {
        bmi_header: BITMAPINFOHEADER {
            bi_size: 40,
            bi_width: w,
            bi_height: -h, // top-down rows
            bi_planes: 1,
            bi_bit_count: 32,
            bi_compression: 0, // BI_RGB
            bi_size_image: buf.len() as u32,
            bi_x_pels_per_meter: 0,
            bi_y_pels_per_meter: 0,
            bi_clr_used: 0,
            bi_clr_important: 0,
        },
        bmi_colors: [0u32; 3],
    };
    let lines = unsafe {
        GetDIBits(
            mem_dc,
            bmp,
            0,
            h as u32,
            buf.as_mut_ptr(),
            &mut bmi as *mut BITMAPINFO as *mut BITMAPINFOHEADER,
            0,
        )
    };
    unsafe {
        SelectObject(mem_dc, prev);
        DeleteObject(bmp);
        DeleteDC(mem_dc);
        ReleaseDC(0, hdc);
    }
    if lines == 0 {
        return Err("GetDIBits failed".into());
    }
    // DIB rows are BGRA; convert to RGBA.
    for px in buf.chunks_exact_mut(4) {
        (px[0], px[2]) = (px[2], px[0]);
    }
    Ok((buf, w as u32, h as u32))
}

/// The tray icon's hidden message window, registered by main.rs so background
/// threads can talk to it (e.g. timer notifications).
pub fn set_tray_hwnd(hwnd: isize) {
    TRAY_HWND.store(hwnd, Ordering::SeqCst);
}

/// Custom message the timer thread posts to the tray window when a countdown
/// finishes; `wparam` carries the total seconds.
pub const WM_APP_TIMER_DONE: u32 = 0x8001;

/// Start a countdown; when it elapses the tray window shows a notification.
pub fn start_timer(seconds: u32) -> Result<(), String> {
    if TRAY_HWND.load(Ordering::SeqCst) == 0 {
        return Err("tray is not available".into());
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(seconds as u64));
        let hwnd = TRAY_HWND.load(Ordering::SeqCst);
        if hwnd != 0 {
            unsafe { PostMessageW(hwnd, WM_APP_TIMER_DONE, seconds as usize, 0) };
        }
    });
    Ok(())
}

// ── Toast popup (custom notification window) ───────────────────────────
//
// Tray balloons are suppressed on Windows 11, so notifications get their
// own small topmost window (bottom-right of the work area). It never takes
// focus (WS_EX_NOACTIVATE), auto-dismisses after 5 s, and clicking it closes
// it. Each toast runs on its own thread with a private message loop so it
// does not depend on the tray thread's pump.

const TOAST_WINDOW_NAME: &[u16] = &[
    'E' as u16, 'l' as u16, 'e' as u16, 'm' as u16, 'e' as u16, 'n' as u16, 't' as u16, 'T' as u16,
    'o' as u16, 'a' as u16, 's' as u16, 't' as u16, 'C' as u16, 'l' as u16, 'a' as u16, 's' as u16,
    's' as u16, 0,
];

/// Show a small topmost notification (title + body) above the taskbar's
/// right corner. Never steals focus; slides in right→left; dismissed by a
/// click or after ~5 s. A previous toast is closed first so they never
/// stack.
pub fn show_toast(title: &str, body: &str) {
    let title: Vec<u16> = title.encode_utf16().collect();
    let body: Vec<u16> = body.encode_utf16().collect();
    std::thread::spawn(move || {
        // Close the previous toast, if any.
        let previous = LAST_TOAST_HWND.swap(0, Ordering::SeqCst);
        if previous != 0 {
            unsafe { PostMessageW(previous, WM_CLOSE, 0, 0) };
        }

        let Some((hwnd, rest_x, y, work_right)) = create_toast_window(&title, &body) else {
            return;
        };
        LAST_TOAST_HWND.store(hwnd, Ordering::SeqCst);
        // Auto-dismiss after 5 s (WM_TIMER → WM_CLOSE in the wndproc).
        unsafe {
            SetTimer(hwnd, 1, 5000, 0);
        }

        // Slide in from just off the right edge with an ease-out curve.
        // SWP_SHOWWINDOW on the first frame; afterwards only x changes.
        const HWND_TOPMOST: isize = -1;
        const SWP_SHOWWINDOW: u32 = 0x0040;
        const SWP_NOACTIVATE: u32 = 0x0010;
        const SWP_NOSIZE: u32 = 0x0001;
        const STEPS: i32 = 22;
        const STEP_MS: u64 = 15;
        let start_x = work_right + 8;
        for i in 0..=STEPS {
            let t = i as f32 / STEPS as f32;
            let eased = 1.0 - (1.0 - t).powi(3); // ease-out cubic
            let x = start_x as f32 - (start_x - rest_x) as f32 * eased;
            let flags = SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOSIZE;
            unsafe {
                SetWindowPos(hwnd, HWND_TOPMOST, x as i32, y, 0, 0, flags);
            }
            std::thread::sleep(std::time::Duration::from_millis(STEP_MS));
        }
        // Land exactly on the resting spot.
        unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                rest_x,
                y,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE,
            );
        }

        // Private message loop — the toast works regardless of what the
        // tray thread is doing.
        loop {
            let mut msg = MSG {
                hwnd: 0,
                message: 0,
                w_param: 0,
                l_param: 0,
                time: 0,
                pt: POINT { x: 0, y: 0 },
            };
            let r = unsafe { GetMessageW(&mut msg, 0, 0, 0) };
            if r == 0 || r == -1 {
                break;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let _ = LAST_TOAST_HWND.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
            if v == hwnd {
                None
            } else {
                Some(v)
            }
        });
    });
}

/// Create the toast window (hidden) at its resting position, bottom-right
/// of the primary work area. Returns `(hwnd, rest_x, y, work_right)`.
fn create_toast_window(title: &[u16], body: &[u16]) -> Option<(isize, i32, i32, i32)> {
    // Register the class once (per process).
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    let mut class_registered = false;
    REGISTERED.call_once(|| {
        let wc = WNDCLASSW {
            style: 0,
            lpfn_wnd_proc: toast_wnd_proc as *const () as usize,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: 0,
            h_icon: 0,
            h_cursor: 0,
            hbr_background: 0,
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: TOAST_WINDOW_NAME.as_ptr(),
        };
        class_registered = unsafe { RegisterClassW(&wc) } != 0;
    });
    if !class_registered {
        // Already registered by an earlier toast thread — fine.
        unsafe { GetClassInfoW(0, TOAST_WINDOW_NAME.as_ptr(), std::ptr::null_mut()) };
    }

    // Measure the text to size the window.
    let hdc = unsafe { GetDC(0) };
    if hdc == 0 {
        return None;
    }
    let fmt = DT_CALCRECT as u32 | DT_LEFT as u32 | DT_SINGLELINE as u32 | DT_NOPREFIX as u32;
    let mut title_rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        DrawTextW(
            hdc,
            title.as_ptr(),
            title.len() as i32,
            &mut title_rect,
            fmt,
        );
    }
    let mut body_rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        DrawTextW(hdc, body.as_ptr(), body.len() as i32, &mut body_rect, fmt);
    }
    let text_w = title_rect.right.max(body_rect.right);
    let width = (text_w + 48).clamp(220, 460);
    let height = 66;
    unsafe { ReleaseDC(0, hdc) };

    // Bottom-right of the primary work area.
    let mut wa = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { SystemParametersInfoW(0x0030, 0, &mut wa as *mut RECT as *mut std::ffi::c_void, 0) };
    // SPI_GETWORKAREA
    let rest_x = wa.right - width - 14;
    let rest_y = wa.bottom - height - 14;
    let work_right = wa.right;

    // Keep the strings alive for the window's lifetime.
    let data = Box::new(Toast {
        title: title.to_vec(),
        body: body.to_vec(),
    });
    let data_ptr = Box::into_raw(data) as isize;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            TOAST_WINDOW_NAME.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            rest_x,
            rest_y,
            width,
            height,
            0,
            0,
            0,
            data_ptr as *const std::ffi::c_void,
        )
    };
    if hwnd == 0 {
        // Recover the leaked box.
        unsafe {
            drop(Box::from_raw(data_ptr as *mut Toast));
        }
        return None;
    }
    Some((hwnd, rest_x, rest_y, work_right))
}

/// Window procedure for the toast window.
extern "system" fn toast_wnd_proc(hwnd: isize, msg: u32, w_param: usize, l_param: isize) -> isize {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let data: *const Toast =
                unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA as u32) as *const Toast };
            unsafe {
                let left = ps.rc_paint.left;
                let top = ps.rc_paint.top;
                let right = ps.rc_paint.right;
                let bottom = ps.rc_paint.bottom;
                if right > left && bottom > top {
                    // Background.
                    let brush = CreateSolidBrush(0x003c3c3c);
                    let prev = SelectObject(hdc, brush);
                    let _ = Rectangle(hdc, left, top, right, bottom);
                    SelectObject(hdc, prev);
                    DeleteObject(brush);
                    // Accent bar on the left.
                    let bar = CreateSolidBrush(accent_colorref());
                    let prev = SelectObject(hdc, bar);
                    let _ = Rectangle(hdc, left, top, left + 4, bottom);
                    SelectObject(hdc, prev);
                    DeleteObject(bar);
                    // Text.
                    if !data.is_null() {
                        let d = &*data;
                        let font = GetStockObject(14); // DEFAULT_GUI_FONT
                        let prev_font = SelectObject(hdc, font);
                        SetBkMode(hdc, 1); // TRANSPARENT
                        let mut title_rect = RECT {
                            left: 16,
                            top: 8,
                            right: right - 10,
                            bottom: 40,
                        };
                        SetTextColor(hdc, 0x00e8e8e8);
                        let _ = DrawTextW(
                            hdc,
                            d.title.as_ptr(),
                            d.title.len() as i32,
                            &mut title_rect,
                            DT_LEFT as u32 | DT_SINGLELINE as u32 | DT_NOPREFIX as u32,
                        );
                        let mut body_rect = RECT {
                            left: 16,
                            top: 41,
                            right: right - 12,
                            bottom: bottom - 6,
                        };
                        SetTextColor(hdc, 0x00b0b0b0);
                        let _ = DrawTextW(
                            hdc,
                            d.body.as_ptr(),
                            d.body.len() as i32,
                            &mut body_rect,
                            DT_LEFT as u32 | DT_SINGLELINE as u32 | DT_NOPREFIX as u32,
                        );
                        SelectObject(hdc, prev_font);
                    }
                }
            }
            unsafe {
                EndPaint(hwnd, &ps);
            }
            0
        }
        WM_LBUTTONDOWN | WM_TIMER => {
            let _ = w_param;
            let _ = l_param;
            unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            0
        }
        WM_DESTROY => {
            let data: *mut Toast =
                unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA as u32) as *mut Toast };
            if !data.is_null() {
                unsafe {
                    drop(Box::from_raw(data));
                }
            }
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) },
    }
}

fn accent_colorref() -> u32 {
    let c = crate::theme::accent();
    let r = (c.r * 255.0) as u32;
    let g = (c.g * 255.0) as u32;
    let b = (c.b * 255.0) as u32;
    r | (g << 8) | (b << 16)
}

/// Generate a cryptographically random password from a printable charset.
/// Length is clamped to 8-64 characters.
pub fn generate_password(len: usize) -> Result<String, String> {
    const CHARS: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}";
    let len = len.clamp(8, 64);
    let mut buf = vec![0u8; len];
    if unsafe { BCryptGenRandom(0, buf.as_mut_ptr(), len as u32, 0x2) } != 0 {
        return Err("BCryptGenRandom failed".into());
    }
    let out: Vec<u8> = buf
        .iter()
        .map(|b| CHARS[*b as usize % CHARS.len()])
        .collect();
    String::from_utf8(out).map_err(|_| "password encoding failed".into())
}

// ── Sound playback (MCI, plays mp3) ─────────────────────────────────────

/// Locate a named sound file (e.g. `oi.mp3`) inside one of the known
/// `sounds/` folders: current dir (cargo run / portable install), next to
/// the executable (and its ancestors, so `target\debug\element.exe` finds
/// `<repo>\sounds`), or the data dir. Returns the first that exists.
pub fn find_sound_file(name: &str) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("sounds").join(name));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("sounds").join(name));
            for ancestor in dir.ancestors().take(3) {
                candidates.push(ancestor.join("sounds").join(name));
            }
        }
    }
    candidates.push(crate::config::data_dir().join("sounds").join(name));
    candidates.into_iter().find(|p| p.is_file())
}

/// Play an mp3 (or any MCI-playable media file) once, asynchronously.
/// Uses `mciSendStringW` on a background thread; errors are ignored.
/// Takes ownership of the path so the thread can outlive the caller.
pub fn play_sound_file(path: String) {
    log_sound_event(&format!("play_sound_file requested: {}", path));
    std::thread::spawn(move || {
        mci_play(&path);
    });
}

/// Diagnose audio issues: append one line per attempt to
/// `{data_dir}/sound-debug.log`. Called by the MCI helper and by timer
/// code when it picks the fallback beep.
pub fn log_sound_event(line: &str) {
    let path = crate::config::data_dir().join("sound-debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{} {}", chrono_now(), line);
    }
}

/// "now" without pulling in a date crate: seconds since the Unix epoch.
fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => format!("t+{:.1}s", d.as_secs_f64()),
        Err(_) => "t+?".to_string(),
    }
}

/// Run the MCI open/play/close sequence synchronously; returns the MCI
/// error codes so callers (and tests) can diagnose failures.
fn mci_play(path: &str) -> (u32, u32, u32) {
    // Clear a stale `element_timer` alias left by an interrupted play —
    // otherwise `open` fails with MCIERR_DEVICE_ID_IN_USE (305).
    let mut stale: Vec<u16> = "close element_timer"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe { mciSendStringW(stale.as_mut_ptr(), std::ptr::null_mut(), 0, 0) };

    let open_cmd = format!("open \"{}\" alias element_timer", path);
    let mut wide_open: Vec<u16> = open_cmd.encode_utf16().chain(std::iter::once(0)).collect();
    let open_ret = unsafe { mciSendStringW(wide_open.as_mut_ptr(), std::ptr::null_mut(), 0, 0) };
    let mut wide_play: Vec<u16> = "play element_timer wait"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let play_ret = unsafe { mciSendStringW(wide_play.as_mut_ptr(), std::ptr::null_mut(), 0, 0) };
    let mut wide_close: Vec<u16> = "close element_timer"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let close_ret = mci_send_close(wide_close.as_mut_ptr());
    mci_log_result(path, open_ret, play_ret, close_ret);
    (open_ret, play_ret, close_ret)
}

fn mci_log_result(path: &str, open: u32, play: u32, close: u32) {
    if open != 0 || play != 0 || close != 0 {
        log_sound_event(&format!(
            "MCI NOT OK: open={} play={} close={} path={}",
            open, play, close, path
        ));
    } else if std::env::var("ELEMENT_VERBOSE_SOUND").is_ok() {
        log_sound_event(&format!("MCI ok: path={}", path));
    }
}

fn mci_send_close(cmd: *mut u16) -> u32 {
    unsafe { mciSendStringW(cmd, std::ptr::null_mut(), 0, 0) }
}

// ── Core Audio volume (COM, hand-rolled vtables) ───────────────────────

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct CLSID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[allow(non_upper_case_globals)]
const CLSID_MMDEVICE_ENUMERATOR: CLSID = CLSID {
    data1: 0xbcde_0395,
    data2: 0xe52f,
    data3: 0x467c,
    data4: [0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e],
};
#[allow(non_upper_case_globals)]
const IID_IMMDEVICE_ENUMERATOR: CLSID = CLSID {
    data1: 0xa956_64d2,
    data2: 0x9614,
    data3: 0x4f35,
    data4: [0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6],
};
#[allow(non_upper_case_globals)]
const IID_IAUDIO_ENDPOINT_VOLUME: CLSID = CLSID {
    data1: 0x5cdf_2c82,
    data2: 0x841e,
    data3: 0x4546,
    data4: [0x97, 0x22, 0x0c, 0xf7, 0x40, 0x78, 0x22, 0x9a],
};

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;
const COINIT_APARTMENTTHREADED: u32 = 0x2;
const CLSCTX_ALL: u32 = 23;

// Absolute vtable slots (IUnknown 0-2 + interface offset).
const VT_RELEASE: usize = 2;
const VT_IMMDEVICE_ENUMERATOR_GET_DEFAULT: usize = 4;
const VT_IMMDEVICE_ACTIVATE: usize = 3;
const VT_ENDPOINT_SET_SCALAR: usize = 7;
const VT_ENDPOINT_GET_SCALAR: usize = 9;
const VT_ENDPOINT_SET_MUTE: usize = 16;
const VT_ENDPOINT_GET_MUTE: usize = 17;

/// Get (None) or set (Some 0-100) the system volume through Core Audio.
fn volume_access(set_level: Option<u32>) -> Result<Option<u32>, String> {
    let initialization = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
    let must_uninitialize = initialization == S_OK || initialization == S_FALSE;
    if !must_uninitialize && initialization != RPC_E_CHANGED_MODE {
        return Err(format!("CoInitializeEx failed: 0x{initialization:08x}"));
    }

    let result = (|| {
        // CoCreateInstance(CLSID_MMDeviceEnumerator, …, IID_IMMDeviceEnumerator)
        let mut enumerator: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = unsafe {
            CoCreateInstance(
                &CLSID_MMDEVICE_ENUMERATOR,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IMMDEVICE_ENUMERATOR,
                &mut enumerator,
            )
        };
        if hr != S_OK || enumerator.is_null() {
            return Err(format!("IMMDeviceEnumerator hr=0x{hr:08x}"));
        }

        // enumerator->GetDefaultAudioEndpoint(eRender=0, eConsole=0, &mut device)
        let vtable = unsafe { *(enumerator as *const *const usize) };
        let get_default: unsafe extern "system" fn(
            *mut std::ffi::c_void,
            i32,
            i32,
            *mut *mut std::ffi::c_void,
        ) -> i32 = unsafe { std::mem::transmute(*vtable.add(VT_IMMDEVICE_ENUMERATOR_GET_DEFAULT)) };
        let release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32 =
            unsafe { std::mem::transmute(*vtable.add(VT_RELEASE)) };

        let mut device: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = unsafe { get_default(enumerator, 0, 0, &mut device) };
        unsafe { release(enumerator) };
        if hr != S_OK || device.is_null() {
            return Err(format!("GetDefaultAudioEndpoint hr=0x{hr:08x}"));
        }

        // device->Activate(IID_IAudioEndpointVolume, CLSCTX_ALL, …, &mut volume)
        let vtable = unsafe { *(device as *const *const usize) };
        let activate: unsafe extern "system" fn(
            *mut std::ffi::c_void,
            *const CLSID,
            u32,
            *mut std::ffi::c_void,
            *mut *mut std::ffi::c_void,
        ) -> i32 = unsafe { std::mem::transmute(*vtable.add(VT_IMMDEVICE_ACTIVATE)) };
        let release_device: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32 =
            unsafe { std::mem::transmute(*vtable.add(VT_RELEASE)) };

        let mut volume: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = unsafe {
            activate(
                device,
                &IID_IAUDIO_ENDPOINT_VOLUME,
                CLSCTX_ALL,
                std::ptr::null_mut(),
                &mut volume,
            )
        };
        unsafe { release_device(device) };
        if hr != S_OK || volume.is_null() {
            return Err(format!("IAudioEndpointVolume activate hr=0x{hr:08x}"));
        }

        let vtable = unsafe { *(volume as *const *const usize) };
        let get_scalar: unsafe extern "system" fn(*mut std::ffi::c_void, *mut f32) -> i32 =
            unsafe { std::mem::transmute(*vtable.add(VT_ENDPOINT_GET_SCALAR)) };
        let set_scalar: unsafe extern "system" fn(
            *mut std::ffi::c_void,
            f32,
            *const std::ffi::c_void,
        ) -> i32 = unsafe { std::mem::transmute(*vtable.add(VT_ENDPOINT_SET_SCALAR)) };
        let get_mute: unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32) -> i32 =
            unsafe { std::mem::transmute(*vtable.add(VT_ENDPOINT_GET_MUTE)) };
        let set_mute: unsafe extern "system" fn(
            *mut std::ffi::c_void,
            i32,
            *const std::ffi::c_void,
        ) -> i32 = unsafe { std::mem::transmute(*vtable.add(VT_ENDPOINT_SET_MUTE)) };
        let release_volume: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32 =
            unsafe { std::mem::transmute(*vtable.add(VT_RELEASE)) };

        let result = (|| {
            match set_level {
                Some(level) => {
                    // Unmute first, or the scalar set stays silent.
                    let mut muted: i32 = 0;
                    unsafe { get_mute(volume, &mut muted) };
                    if muted != 0 {
                        unsafe { set_mute(volume, 0, std::ptr::null()) };
                    }
                    let scalar = level as f32 / 100.0;
                    let hr = unsafe { set_scalar(volume, scalar, std::ptr::null()) };
                    if hr != S_OK {
                        return Err(format!("SetMasterVolumeLevelScalar hr=0x{hr:08x}"));
                    }
                    Ok(Some(level))
                }
                None => {
                    let mut scalar: f32 = 0.0;
                    let hr = unsafe { get_scalar(volume, &mut scalar) };
                    if hr != S_OK {
                        return Err(format!("GetMasterVolumeLevelScalar hr=0x{hr:08x}"));
                    }
                    Ok(Some(((scalar * 100.0).round() as u32).min(100)))
                }
            }
        })();
        unsafe { release_volume(volume) };
        result
    })();

    if must_uninitialize {
        unsafe { CoUninitialize() };
    }
    result
}

// ── Clipboard writes with retry ─────────────────────────────────────────

/// Run `f` until it returns `true` (up to ~500 ms), sleeping 20 ms between
/// attempts. The clipboard watcher polls every 800 ms and briefly holds the
/// clipboard open, so writers must be prepared to retry.
fn retry_clipboard(mut f: impl FnMut() -> bool) -> bool {
    for _ in 0..25 {
        if f() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

/// Allocate a `GMEM_MOVEABLE` block and copy `bytes` into it.
fn clipboard_alloc(bytes: &[u8]) -> Result<isize, String> {
    let h_mem = unsafe { GlobalAlloc(0x0002 | 0x0040, bytes.len()) };
    if h_mem == 0 {
        return Err("GlobalAlloc failed".into());
    }
    let ptr = unsafe { GlobalLock(h_mem) };
    if ptr.is_null() {
        unsafe { GlobalFree(h_mem) };
        return Err("GlobalLock failed".into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        GlobalUnlock(h_mem);
    }
    Ok(h_mem)
}

/// Put several `(format, bytes)` pairs on the clipboard with one atomic
/// `EmptyClipboard` + `SetClipboardData` sequence.
///
/// The clipboard is ALWAYS closed again even when a `SetClipboardData`
/// fails — a previous bug left it open forever, silently breaking every
/// later copy/paste. Handles that were rejected are freed; ones that were
/// accepted are owned by the clipboard and left alone.
fn set_clipboard_multi(items: &[(u32, &[u8])]) -> Result<(), String> {
    if items.is_empty() {
        return Err("nothing to copy".into());
    }
    let mut handles = Vec::with_capacity(items.len());
    for (_, bytes) in items {
        handles.push(clipboard_alloc(bytes)?);
    }
    let mut sent = vec![false; items.len()];
    let ok = retry_clipboard(|| unsafe {
        if OpenClipboard(0) == 0 {
            return false;
        }
        let mut success = EmptyClipboard() != 0;
        if success {
            for (i, ((fmt, _), h)) in items.iter().zip(handles.iter()).enumerate() {
                if SetClipboardData(*fmt, *h) == 0 {
                    success = false;
                } else {
                    sent[i] = true;
                }
            }
        }
        let _ = CloseClipboard();
        success
    });
    if ok {
        return Ok(());
    }
    for (i, h) in handles.iter().enumerate() {
        if !sent[i] {
            unsafe { GlobalFree(*h) };
        }
    }
    Err("clipboard transfer failed".into())
}

/// Put UTF-16 text on the clipboard as `CF_UNICODETEXT` (retrying while the
/// clipboard is locked by another window/thread).
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
    set_clipboard_multi(&[(CF_UNICODETEXT, bytes)])
}

/// How a captured clipboard bitmap was stored on the clipboard. `Png` means
/// the bytes are already a PNG file (apps like Chrome/Edge put `image/png`
/// on the clipboard); `Dib`/`DibV5` mean a device-independent bitmap with a
/// `BITMAPINFOHEADER` (respectively `BITMAPV5HEADER`) prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardBitmapFormat {
    Dib,
    DibV5,
    Png,
}

/// Read the current clipboard image, if any, preferring `CF_DIB`, then
/// `CF_DIBV5`, then the registered `PNG` / `image/png` formats. Returns the
/// raw bytes and which format they came from. Empty when no image is present
/// or the clipboard is locked by another window.
pub fn clipboard_bitmap_bytes() -> Option<(Vec<u8>, ClipboardBitmapFormat)> {
    let png_fmt = unsafe { RegisterClipboardFormatW(PNG_WIDE.as_ptr()) };
    let png_mime = unsafe { RegisterClipboardFormatW(PNG_MIME_WIDE.as_ptr()) };
    if unsafe { OpenClipboard(0) } == 0 {
        return None;
    }
    let chosen: Option<(u32, ClipboardBitmapFormat)> = unsafe {
        if IsClipboardFormatAvailable(CF_DIB) != 0 {
            Some((CF_DIB, ClipboardBitmapFormat::Dib))
        } else if IsClipboardFormatAvailable(CF_DIBV5) != 0 {
            Some((CF_DIBV5, ClipboardBitmapFormat::DibV5))
        } else if png_fmt != 0 && IsClipboardFormatAvailable(png_fmt) != 0 {
            Some((png_fmt, ClipboardBitmapFormat::Png))
        } else if png_mime != 0 && IsClipboardFormatAvailable(png_mime) != 0 {
            Some((png_mime, ClipboardBitmapFormat::Png))
        } else {
            None
        }
    };
    let out = chosen.and_then(|(fmt, tag)| {
        let h = unsafe { GetClipboardData(fmt) };
        if h == 0 {
            return None;
        }
        let ptr = unsafe { GlobalLock(h) };
        if ptr.is_null() {
            return None;
        }
        let size = unsafe { GlobalSize(h) };
        let mut bytes = vec![0u8; size];
        unsafe {
            std::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr(), size);
            GlobalUnlock(h);
        }
        Some((bytes, tag))
    });
    unsafe {
        CloseClipboard();
    }
    out
}

/// Put raw bitmap bytes (in the given clipboard format) on the clipboard as
/// `CF_DIB`, `CF_DIBV5` or the registered `PNG` format. Used to restore an
/// image entry from history.
pub fn set_clipboard_bitmap(bytes: &[u8], format: ClipboardBitmapFormat) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("empty bitmap".into());
    }
    let fmt = match format {
        ClipboardBitmapFormat::Dib => CF_DIB,
        ClipboardBitmapFormat::DibV5 => CF_DIBV5,
        ClipboardBitmapFormat::Png => unsafe { RegisterClipboardFormatW(PNG_WIDE.as_ptr()) },
    };
    if fmt == 0 {
        return Err("unknown clipboard format".into());
    }
    set_clipboard_multi(&[(fmt, bytes)])
}

/// Put a screenshot on the clipboard in EVERY format apps understand:
/// `CF_DIB` (Paint/Photos/GDI apps) plus the registered `PNG` and
/// `image/png` formats (messengers, browsers, Win+V picture picks). Some
/// apps only pay attention to one of them, so a single-format write can
/// look like "nothing was copied".
pub fn set_clipboard_screenshot(dib: &[u8], png: &[u8]) -> Result<(), String> {
    if dib.is_empty() || png.is_empty() {
        return Err("empty screenshot".into());
    }
    let png_fmt = unsafe { RegisterClipboardFormatW(PNG_WIDE.as_ptr()) };
    let png_mime = unsafe { RegisterClipboardFormatW(PNG_MIME_WIDE.as_ptr()) };
    let mut items: Vec<(u32, &[u8])> = Vec::with_capacity(3);
    items.push((CF_DIB, dib));
    if png_fmt != 0 {
        items.push((png_fmt, png));
    }
    if png_mime != 0 {
        items.push((png_mime, png));
    }
    set_clipboard_multi(&items)
}

/// Decode `BITMAPINFOHEADER`/`BITMAPV5HEADER`-prefixed pixels (24 or 32 bpp,
/// top-down or bottom-up, `BI_RGB` or `BI_BITFIELDS` 32bpp) into RGBA.
/// Returns `(rgba, width, height)`; `None` for unsupported layouts.
pub fn dib_to_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.len() < 40 {
        return None;
    }
    let width = i32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let height = i32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let bpp = u16::from_le_bytes(bytes[14..16].try_into().ok()?);
    if width <= 0 || height == 0 {
        return None;
    }
    let top_down = height < 0;
    let w = width as u32;
    let h = height.unsigned_abs();
    let pixels = &bytes[40..];
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    match bpp {
        32 => {
            let stride = w as usize * 4;
            for row in 0..h as usize {
                let src_row = if top_down { row } else { h as usize - 1 - row };
                let start = src_row * stride;
                if start + stride > pixels.len() {
                    return None;
                }
                for i in 0..w as usize {
                    let p = start + i * 4;
                    let (b, g, r, a) = (pixels[p], pixels[p + 1], pixels[p + 2], pixels[p + 3]);
                    rgba.extend_from_slice(&[r, g, b, a]);
                }
            }
        }
        24 => {
            let stride = ((w as usize * 3) + 3) & !3;
            for row in 0..h as usize {
                let src_row = if top_down { row } else { h as usize - 1 - row };
                let start = src_row * stride;
                if start + stride > pixels.len() {
                    return None;
                }
                for i in 0..w as usize {
                    let p = start + i * 3;
                    rgba.extend_from_slice(&[pixels[p + 2], pixels[p + 1], pixels[p], 255]);
                }
            }
        }
        _ => return None,
    }
    Some((rgba, w, h))
}

/// Encode RGBA pixels as a top-down 32 bpp `BI_RGB` DIB (the most widely
/// accepted layout for `CF_DIB`).
pub fn rgba_to_dib(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixel_bytes = rgba.len() as u32;
    let mut dib = Vec::with_capacity(40 + pixel_bytes as usize);
    dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
    dib.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&(-(height as i32)).to_le_bytes()); // biHeight (top-down)
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    dib.extend_from_slice(&pixel_bytes.to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&[0u8; 16]); // biClrUsed..biClrImportant
    for px in rgba.chunks_exact(4) {
        // RGBA → BGRA
        dib.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    dib
}

const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_DIBV5: u32 = 17;
const CF_HDROP: u32 = 15;

const PNG_WIDE: [u16; 4] = ['P' as u16, 'N' as u16, 'G' as u16, 0];
const PNG_MIME_WIDE: [u16; 10] = [
    'i' as u16, 'm' as u16, 'a' as u16, 'g' as u16, 'e' as u16, '/' as u16, 'p' as u16, 'n' as u16,
    'g' as u16, 0,
];

// MSDN: OpenClipboard — https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard
#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hWndNewOwner: isize) -> i32;
    fn EmptyClipboard() -> i32;
    fn CloseClipboard() -> i32;
    fn LockWorkStation() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardData(uFormat: u32) -> isize;
    fn RegisterClipboardFormatW(lpszFormat: *const u16) -> u32;
}

// MSDN: GlobalAlloc — https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-globalalloc
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> isize;
    fn GlobalLock(hMem: isize) -> *mut u8;
    fn GlobalUnlock(hMem: isize) -> i32;
    fn GlobalFree(hMem: isize) -> isize;
    fn GlobalSize(hMem: isize) -> usize;
    fn SetClipboardData(uFormat: u32, hMem: isize) -> isize;
}

// MSDN: SetSuspendState — https://learn.microsoft.com/en-us/windows/win32/api/powrprof/nf-powrprof-setsuspendstate
#[link(name = "powrprof")]
extern "system" {
    fn SetSuspendState(hibernate: i32, force: i32, disable_wake_events: i32) -> i32;
}

// MSDN: Core Audio volume — https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume
// COM init + the endpoint unit : the ONLY setup that touches the real
// system volume on Windows Vista onward (waveOut is a legacy mixer stub).
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut std::ffi::c_void, dwCoInit: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        rclsid: *const CLSID,
        pUnkOuter: *mut std::ffi::c_void,
        dwClsContext: u32,
        riid: *const CLSID,
        ppv: *mut *mut std::ffi::c_void,
    ) -> i32;
}

// MSDN: GetSystemMetrics — https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getsystemmetrics
// MSDN: PostMessageW — https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postmessagew
// MSDN: SendMessageTimeoutW — https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw
#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(nIndex: i32) -> i32;
    fn PostMessageW(hWnd: isize, msg: u32, wParam: usize, lParam: isize) -> i32;
    fn SendMessageTimeoutW(
        hWnd: isize,
        msg: u32,
        wParam: usize,
        lParam: isize,
        fuFlags: u32,
        uTimeout: u32,
        lpdwResult: *mut isize,
    ) -> isize;
    fn GetDC(hWnd: isize) -> isize;
    fn ReleaseDC(hWnd: isize, hDC: isize) -> i32;
}

// MSDN: GDI bitmap functions — https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-getdibits
#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn DeleteDC(hdc: isize) -> i32;
    fn CreateCompatibleBitmap(hdc: isize, w: i32, h: i32) -> isize;
    fn DeleteObject(hObject: isize) -> i32;
    fn SelectObject(hdc: isize, hObject: isize) -> isize;
    fn BitBlt(
        hdcDest: isize,
        xDest: i32,
        yDest: i32,
        w: i32,
        h: i32,
        hdcSrc: isize,
        xSrc: i32,
        ySrc: i32,
        rop: u32,
    ) -> i32;
    fn GetDIBits(
        hdc: isize,
        hbmp: isize,
        start: u32,
        lines: u32,
        bits: *mut u8,
        bmi: *mut BITMAPINFOHEADER,
        usage: u32,
    ) -> i32;
}

// MSDN: BCryptGenRandom — https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(hAlgorithm: isize, pbBuffer: *mut u8, cbBuffer: u32, dwFlags: u32) -> i32;
}

// MSDN: mciSendStringW — https://learn.microsoft.com/en-us/windows/win32/api/mmsystem/nf-mmsystem-mcisendstringw
#[link(name = "winmm")]
extern "system" {
    fn mciSendStringW(
        lpszCommand: *mut u16,
        lpszReturnString: *mut u16,
        cchReturn: u32,
        hwndCallback: isize,
    ) -> u32;
}

// ── Toast window FFI ────────────────────────────────────────────────────

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;

const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_DESTROY: u32 = 0x0002;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONDOWN: u32 = 0x0201;

const DT_LEFT: i32 = 0x0000;
const DT_SINGLELINE: i32 = 0x0020;
const DT_NOPREFIX: i32 = 0x0800;
const DT_CALCRECT: i32 = 0x0400;
const GWLP_USERDATA: i32 = -21;

// Default toast text buffer; used by the helper loop.
#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct Toast {
    title: Vec<u16>,
    body: Vec<u16>,
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct MSG {
    hwnd: isize,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt: POINT,
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct PAINTSTRUCT {
    hdc: isize,
    f_erase: i32,
    rc_paint: RECT,
    f_restore: i32,
    f_inc_update: i32,
    rgb_reserved: [u8; 32],
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct WNDCLASSW {
    style: u32,
    lpfn_wnd_proc: usize,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: isize,
    h_icon: isize,
    h_cursor: isize,
    hbr_background: isize,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
}

/// Handle of the most recent toast window.
static LAST_TOAST_HWND: AtomicIsize = AtomicIsize::new(0);

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(lp_wnd_class: *const WNDCLASSW) -> u16;
    fn GetClassInfoW(
        h_instance: isize,
        lp_class_name: *const u16,
        lp_wnd_class: *mut std::ffi::c_void,
    ) -> i32;
    fn CreateWindowExW(
        dw_ex_style: u32,
        lp_class_name: *const u16,
        lp_window_name: *const u16,
        dw_style: u32,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        hwnd_parent: isize,
        h_menu: isize,
        h_instance: isize,
        lp_param: *const std::ffi::c_void,
    ) -> isize;
    fn SetWindowPos(
        hWnd: isize,
        h_wnd_insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        u_flags: u32,
    ) -> i32;
    fn SetTimer(hWnd: isize, n_id_event: usize, u_elapse: u32, lp_timer_func: usize) -> isize;
    fn GetMessageW(
        lp_msg: *mut MSG,
        hWnd: isize,
        w_msg_filter_min: u32,
        w_msg_filter_max: u32,
    ) -> i32;
    fn TranslateMessage(lp_msg: *const MSG) -> i32;
    fn DispatchMessageW(lp_msg: *const MSG) -> isize;
    fn PostQuitMessage(n_exit_code: i32);
    fn SystemParametersInfoW(
        ui_action: u32,
        ui_param: u32,
        pv_param: *mut std::ffi::c_void,
        f_win_ini: u32,
    ) -> i32;
    fn GetWindowLongPtrW(hWnd: isize, n_index: u32) -> isize;
    fn BeginPaint(hWnd: isize, lp_paint: *mut PAINTSTRUCT) -> isize;
    fn EndPaint(hWnd: isize, lp_paint: *const PAINTSTRUCT) -> i32;
    fn DefWindowProcW(hWnd: isize, msg: u32, w_param: usize, l_param: isize) -> isize;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
    fn Rectangle(hdc: isize, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn SetTextColor(hdc: isize, color: u32) -> u32;
    fn SetBkMode(hdc: isize, mode: i32) -> i32;
    fn GetStockObject(fn_object: i32) -> isize;
    fn DrawTextW(
        hdc: isize,
        lpch_text: *const u16,
        cch_text: i32,
        lprc: *mut RECT,
        format: u32,
    ) -> i32;
}

use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};

static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct BITMAPINFOHEADER {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [u32; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_list_is_rejected() {
        assert!(copy_files_to_clipboard(&[]).is_err());
    }

    #[test]
    fn dib_round_trip_preserves_pixels() {
        let rgba = [
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 40,
        ];
        let dib = rgba_to_dib(&rgba, 2, 2);
        let (out, w, h) = dib_to_rgba(&dib).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(out, rgba);
    }

    #[test]
    fn dib_24bpp_bottom_up_decodes() {
        // 2x2, bottom-up (positive height): first row in the buffer is the
        // BOTTOM row. Pixel (x=0,y=1) = red, (1,1) = green; (0,0) = blue, (1,0) = white.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&24u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        // bottom row (y=1): blue, white (+ 2 pad bytes)
        bytes.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
        // top row (y=0): red, green (+ 2 pad bytes)
        bytes.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
        let (rgba, w, h) = dib_to_rgba(&bytes).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(
            rgba,
            vec![
                255, 0, 0, 255, // y=0 red
                0, 255, 0, 255, // y=0 green
                0, 0, 255, 255, // y=1 blue
                255, 255, 255, 255 // y=1 white
            ]
        );
    }

    #[test]
    fn dib_rejects_unsupported_layouts() {
        // Truncated header
        assert!(dib_to_rgba(&[0u8; 20]).is_none());
        // 8 bpp paletted
        let mut bytes = vec![0u8; 40];
        bytes[4..8].copy_from_slice(&4i32.to_le_bytes());
        bytes[8..12].copy_from_slice(&4i32.to_le_bytes());
        bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
        bytes[14..16].copy_from_slice(&8u16.to_le_bytes());
        assert!(dib_to_rgba(&bytes).is_none());
    }

    #[test]
    #[ignore = "live: captures the real screen (run manually)"]
    fn live_screen_capture_returns_pixels() {
        let (rgba, w, h) = capture_screen_bitmap().unwrap();
        assert!(w > 0 && h > 0);
        assert_eq!(rgba.len() as u32, w * h * 4);
        let dib = rgba_to_dib(&rgba, w, h);
        set_clipboard_bitmap(&dib, ClipboardBitmapFormat::Dib).unwrap();
    }

    #[test]
    #[ignore = "live: plays a real sound (run manually)"]
    fn live_mci_plays_mp3() {
        let paths = ["element/sounds/oi.mp3", "sounds/oi.mp3"];
        let mut found = None;
        for p in paths {
            if std::path::Path::new(p).is_file() {
                found = Some(p);
                break;
            }
        }
        let path = found.expect("oi.mp3 next to the project root");
        let (open, play, close) = mci_play(path);
        eprintln!("mci open={open} play={play} close={close}");
        assert_eq!(open, 0, "MCI open failed");
        assert_eq!(play, 0, "MCI play failed");
        assert_eq!(close, 0, "MCI close failed");
    }

    #[test]
    #[ignore = "live: changes system volume (run manually)"]
    fn live_volume_roundtrip() {
        let original = system_volume().unwrap();
        set_system_volume(42).unwrap();
        assert_eq!(system_volume().unwrap(), 42);
        set_system_volume(original).unwrap();
    }
}
