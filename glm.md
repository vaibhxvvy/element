# Element — Debug & Fix Log

> **Date**: 2026-07-28
> **Build**: `cargo build --release` (target: Windows, Iced 0.13 / wgpu)

---

## Critical Issues Found

### 1. Hotkey Registration Not Checked
**File**: `src/main.rs:435`  
**Problem**: `RegisterHotKey()` return value is ignored. If another app (PowerToys, AutoHotkey, VS Code, Windows itself) has already registered `Alt+Space`, the hotkey silently fails. The tray icon runs but the hotkey does nothing.  
**Fix**: Added debug log of the return value.

### 2. Window Lookup Without Logging
**File**: `src/main.rs:509`  
**Problem**: `FindWindowW(NULL, "Element")` could fail if Iced/winit creates the window with a different title or class. If it returns 0, the hotkey handler does nothing — silently.  
**Fix**: Added debug logging around every `FindWindowW`, `IsWindowVisible`, `ShowWindow`, and `SetWindowPos` call.

### 3. Raw Win32 ShowWindow Might Conflict With Iced/winit
**File**: `src/main.rs:44-61`  
**Problem**: The background thread calls `ShowWindow(hwnd, SW_RESTORE)` and `ShowWindow(hwnd, SW_HIDE)` directly via Win32. Iced 0.13 uses winit which maintains its own internal visibility state. If winit detects the window was externally hidden/shown, it may revert it during its event loop. The window could flash briefly then disappear.
**Symptoms**: "Can't see the search box" despite the process running.  
**Fix**: Added debug logging around all show/hide operations so the user can verify whether `FindWindowW` and `ShowWindow` succeed. See `~/.element/debug.log`.

### 4. No Error/Debug Logging At All
**File**: Entire codebase  
**Problem**: Zero runtime diagnostics. If anything fails at runtime, there is no way to know.  
**Fix**: Added `src/debug_log.rs` — logs to both `~/.element/debug.log` (appended, timestamped) and stderr. All hotkey operations, window lookups, and DWM effects are now logged.

### 5. Iced Rendering With `visible: false`
**File**: `src/main.rs:547-553`  
**Problem**: The window is created with `visible: false`. Some winit/Iced versions may skip rendering or GPU work for hidden windows, meaning when `ShowWindow` is called, there's no rendered content to display.  
**Workaround**: Not changed (the raw Win32 approach is the standard for launchers). Logging will confirm whether this is the issue.

---

## UI/Rendering Issues Fixed

### 6. Theme Updated to Dark Gray (#3c3c3c)
**File**: `src/theme.rs`  
- Background: `rgba(60, 60, 60, 0.5)` — dark gray at 50% opacity for DWM acrylic
- Selected: `rgba(77, 77, 77, 0.6)` — slightly lighter on hover/selection
- Input bg: `rgba(30, 30, 30, 0.3)` — subtle dark input
- Text: light gray `#dcdcdc` for readability on dark
- Accent: blue-gray `#569cd6` (VS Code–style selection)
- Border: `#4d4d4d` at full opacity, 2px width
- Container radius: 12px (rounded corners)
- Removed branding icon/title for clean minimal look

### 7. DWM Acrylic Blur Added
**File**: `src/main.rs:apply_acrylic_blur()`  
- Loads `SetWindowCompositionAttribute` dynamically from `user32.dll` (for compatibility)
- Sets `ACCENT_ENABLE_ACRYLIC_BLURBEHIND` with tint `#3c3c3c` at 50% opacity
- Sets `WS_EX_LAYERED` window style for transparency support
- Falls back gracefully if the API is unavailable (no blur, solid background)

### 8. Rounded Window Corners
**File**: `src/main.rs:apply_dwm_rounded_corners()`  
- Uses `DWMWA_WINDOW_CORNER_PREFERENCE` with `DWMWCP_ROUND` via `DwmSetWindowAttribute`
- Gives the window native rounded corners (Win11+)

### 9. Removed Branding Icon
**File**: `src/ui/mod.rs`  
- Removed the brand icon and "element" text from the header (cleaner minimal look)
- Search input now fills the entire header width

---

## New Files Created

### `src/debug_log.rs` — Debug Logger
- Logs to `~/.element/debug.log` with Unix timestamps
- Also prints to stderr for terminal debugging
- Thread-safe via `OnceLock<Mutex<File>>`
- Call with `debug_log!("message {}", var)` macro

### `debug.ps1` — PowerShell Debug Monitor
- Kills existing Element process
- Starts a fresh instance
- Tails `~/.element/debug.log` in real-time
- Run with: `.\debug.ps1` from the repo root

---

## How To Debug

### Step 1: Run the debug monitor
```powershell
cd C:\Users\vaibh\Desktop\element
.\debug.ps1
```

### Step 2: Press Alt+Space
The monitor will show live log entries like:
```
[1234567890] RegisterHotKey(Alt+Space) SUCCESS
[1234567890] WM_HOTKEY: FindWindowW returned hwnd=123456
[1234567890] WM_HOTKEY: IsWindowVisible=0
[1234567890] show_launcher(hwnd=123456)
[1234567890] SetWindowPos returned: 1
[1234567890] SetForegroundWindow returned: 1
[1234567890] HOTKEY_TRIGGERED set to true
```

### Step 3: Read the log after the fact
```powershell
Get-Content $env:USERPROFILE\.element\debug.log
```

### Diagnosis Table
| Log Entry | Meaning |
|-----------|---------|
| `RegisterHotKey FAILED` | Another app has Alt+Space — close PowerToys/AutoHotkey/etc. |
| `FindWindowW returned 0` | Iced window not found by title — Iced might not have started. |
| `IsWindowVisible=1` on WM_HOTKEY | Window is already visible (toggle behavior) |
| `ShowWindow returned: 0` | ShowWindow failed — check permissions. |
| `acrylic blur applied successfully` | DWM blur is working — check window transparency. |
| `CRITICAL: RegisterHotKey(Alt+Space) FAILED` | Hotkey conflict — fix by freeing Alt+Space. |

---

## All Issues Found (Code Quality)

| # | Issue | Location | Severity |
|---|-------|----------|----------|
| 1 | `RegisterHotKey` return unchecked | `main.rs:435` | **High** — silent failure |
| 2 | `FindWindowW` result unchecked | `main.rs:509` | **High** — silent failure |
| 3 | No runtime logging anywhere | Entire codebase | **High** — blind debugging |
| 4 | Raw Win32 may conflict with winit | `main.rs:44-61` | **Medium** — window not showing |
| 5 | `#![allow(non_snake_case)]` is crate-wide | `main.rs:2` | **Low** — style |
| 6 | `#![allow(dead_code)]` is crate-wide | `error.rs:1` | **Low** — style |
| 7 | Icon uses default IDI_APPLICATION | `main.rs:477` | **Low** — no custom tray icon |
| 8 | No `Drop` cleanup for COM/icon handles | `providers/apps.rs` | **Low** — minor leak |
| 9 | Window y-position hardcoded | `main.rs:53` | **Low** — not DPI-aware |
| 10 | `unwrap()` on window_title encoding | `main.rs:397` | **Low** — OK in practice |
| 11 | `search_dirs` from config unused | `app.rs:31` | Info — default is empty vec |
| 12 | Frecency SQL uses `julianday` which is per-connection | `database.rs` | Info — works per-session |

---

## Build Commands
```bash
cargo build                 # debug build
cargo build --release       # release (with LTO)
cargo fmt && cargo clippy -- -D warnings
cargo test                  # 27+ tests
```

## Dependencies Note
Added `"transparent"` feature to Iced 0.13 to enable per-pixel alpha for DWM acrylic blur.