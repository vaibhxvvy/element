//! Global hotkey detection — isolated from window show/hide.
//!
//! The LL hook callback must stay tiny (set a flag and return). All real work
//! happens on the background message-loop thread via [`take_pending_toggle`].

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

use crate::config;
use crate::debug_log;

static WANT_MODS: AtomicU32 = AtomicU32::new(0);
static WANT_VK: AtomicU32 = AtomicU32::new(0);
static HOOK: AtomicIsize = AtomicIsize::new(0);
static TOGGLE_PENDING: AtomicBool = AtomicBool::new(false);
/// True while the hotkey combo is physically held down. Key-repeat keydowns
/// must NOT re-arm the toggle — re-arming made holding Alt+Space flicker the
/// window open/closed for every repeat.
static COMBO_DOWN: AtomicBool = AtomicBool::new(false);

/// True while a WH_KEYBOARD_LL hook is installed (needs a responsive message pump).
pub fn hook_active() -> bool {
    HOOK.load(Ordering::Relaxed) != 0
}

/// Consume a pending Alt+Space (or configured hotkey) press.
pub fn take_pending_toggle() -> bool {
    TOGGLE_PENDING.swap(false, Ordering::SeqCst)
}

/// Non-consuming check — for sleep-delay decisions in the message loop.
pub fn has_pending_toggle() -> bool {
    TOGGLE_PENDING.load(Ordering::Relaxed)
}

/// Re-queue a toggle that arrived before the Iced window existed.
pub fn requeue_toggle() {
    TOGGLE_PENDING.store(true, Ordering::SeqCst);
}

/// Register `preferred` via RegisterHotKey, or steal it with an LL hook on failure.
/// Returns the active hotkey label for the tray tip.
pub fn install(preferred: &str) -> String {
    const MOD_NOREPEAT: u32 = 0x4000;

    for (i, candidate) in config::hotkey_fallback_candidates(preferred)
        .into_iter()
        .enumerate()
    {
        let Some((mods, vk)) = config::parse_hotkey(&candidate) else {
            debug_log!("hotkey: skip unparseable '{}'", candidate);
            continue;
        };

        if register_hotkey(0, 1, mods | MOD_NOREPEAT, vk) != 0 {
            debug_log!("hotkey: RegisterHotKey('{}') OK", candidate);
            return candidate;
        }
        debug_log!("hotkey: RegisterHotKey('{}') failed", candidate);

        if i == 0 && install_ll_hook(mods, vk) {
            return candidate;
        }
    }

    debug_log!("hotkey: CRITICAL — nothing registered");
    preferred.to_string()
}

pub fn uninstall() {
    let hook = HOOK.swap(0, Ordering::SeqCst);
    if hook != 0 {
        unhook_windows_hook_ex(hook);
        debug_log!("hotkey: LL hook removed");
    }
    TOGGLE_PENDING.store(false, Ordering::SeqCst);
    COMBO_DOWN.store(false, Ordering::SeqCst);
}

fn install_ll_hook(mods: u32, vk: u32) -> bool {
    const WH_KEYBOARD_LL: i32 = 13;
    WANT_MODS.store(mods, Ordering::SeqCst);
    WANT_VK.store(vk, Ordering::SeqCst);
    TOGGLE_PENDING.store(false, Ordering::SeqCst);

    // hMod=0 is valid for WH_KEYBOARD_LL in a desktop EXE.
    let hook = set_windows_hook_ex_w(WH_KEYBOARD_LL, Some(ll_hook_proc), 0, 0);
    if hook == 0 {
        debug_log!("hotkey: SetWindowsHookExW failed");
        false
    } else {
        HOOK.store(hook, Ordering::SeqCst);
        debug_log!("hotkey: LL hook installed");
        true
    }
}

fn mods_held(want_mods: u32, kb_flags: u32) -> bool {
    const VK_MENU: i32 = 0x12;
    const VK_CONTROL: i32 = 0x11;
    const VK_SHIFT: i32 = 0x10;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;
    const LLKHF_ALTDOWN: u32 = 0x20;

    let alt =
        (kb_flags & LLKHF_ALTDOWN) != 0 || (get_async_key_state(VK_MENU) as u16 & 0x8000) != 0;
    let ctrl = (get_async_key_state(VK_CONTROL) as u16 & 0x8000) != 0;
    let shift = (get_async_key_state(VK_SHIFT) as u16 & 0x8000) != 0;
    let win = (get_async_key_state(VK_LWIN) as u16 & 0x8000) != 0
        || (get_async_key_state(VK_RWIN) as u16 & 0x8000) != 0;

    let mut held = 0u32;
    if alt {
        held |= config::MOD_ALT;
    }
    if ctrl {
        held |= config::MOD_CONTROL;
    }
    if shift {
        held |= config::MOD_SHIFT;
    }
    if win {
        held |= config::MOD_WIN;
    }
    (held & want_mods) == want_mods
}

/// Tiny callback — never show/hide windows or EnumWindows here.
extern "system" fn ll_hook_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    const WM_KEYDOWN: usize = 0x0100;
    const WM_KEYUP: usize = 0x0101;
    const WM_SYSKEYDOWN: usize = 0x0104;
    const WM_SYSKEYUP: usize = 0x0105;
    const LLKHF_UP: u32 = 0x80;

    if code >= 0 {
        let kb = unsafe { &*(lparam as *const KbdLlHook) };
        let want_vk = WANT_VK.load(Ordering::Relaxed);
        let want_mods = WANT_MODS.load(Ordering::Relaxed);
        let is_up = (kb.flags & LLKHF_UP) != 0 || wparam == WM_KEYUP || wparam == WM_SYSKEYUP;
        let is_down = !is_up && (wparam == WM_KEYDOWN || wparam == WM_SYSKEYDOWN);

        if kb.vk_code == want_vk {
            if is_down && mods_held(want_mods, kb.flags) {
                // Only the first keydown of a hold arms the toggle; repeats
                // are still swallowed but don't re-trigger.
                if !COMBO_DOWN.swap(true, Ordering::SeqCst) {
                    TOGGLE_PENDING.store(true, Ordering::SeqCst);
                }
                return 1;
            }
            // Swallow Space-up after we took the combo so the app doesn't get a space.
            if is_up {
                COMBO_DOWN.store(false, Ordering::SeqCst);
                if mods_held(want_mods, kb.flags | 0x20) || TOGGLE_PENDING.load(Ordering::Relaxed) {
                    return 1;
                }
            }
        }
    }
    call_next_hook_ex(HOOK.load(Ordering::Relaxed), code, wparam, lparam)
}

#[repr(C)]
struct KbdLlHook {
    vk_code: u32,
    scan_code: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

// ── Minimal FFI (hotkey module only) ────────────────────────────────────

extern "system" fn register_hotkey(hwnd: isize, id: i32, mods: u32, vk: u32) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn RegisterHotKey(hWnd: isize, id: i32, fsModifiers: u32, vk: u32) -> i32;
    }
    unsafe { RegisterHotKey(hwnd, id, mods, vk) }
}

extern "system" fn set_windows_hook_ex_w(
    id: i32,
    cb: Option<unsafe extern "system" fn(i32, usize, isize) -> isize>,
    hmod: isize,
    tid: u32,
) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(
            idHook: i32,
            lpfn: Option<unsafe extern "system" fn(i32, usize, isize) -> isize>,
            hmod: isize,
            dwThreadId: u32,
        ) -> isize;
    }
    unsafe { SetWindowsHookExW(id, cb, hmod, tid) }
}

extern "system" fn call_next_hook_ex(hhk: isize, code: i32, wp: usize, lp: isize) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn CallNextHookEx(hhk: isize, nCode: i32, wParam: usize, lParam: isize) -> isize;
    }
    unsafe { CallNextHookEx(hhk, code, wp, lp) }
}

extern "system" fn unhook_windows_hook_ex(hhk: isize) -> i32 {
    #[link(name = "user32")]
    extern "system" {
        fn UnhookWindowsHookEx(hhk: isize) -> i32;
    }
    unsafe { UnhookWindowsHookEx(hhk) }
}

extern "system" fn get_async_key_state(vk: i32) -> i16 {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(vKey: i32) -> i16;
    }
    unsafe { GetAsyncKeyState(vk) }
}
