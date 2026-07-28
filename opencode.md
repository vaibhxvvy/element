# Element — OpenCode Session Log

## 2026-07-28 Session — Phase 15: DWM Fix, Dark UI, Debug Logging

### Root Cause: Invisible Window
The search box was invisible because of a **layered window ordering bug** in `apply_acrylic_blur()` (`src/main.rs:98-150`):

1. The function set `WS_EX_LAYERED` on the window **BEFORE** calling `SetWindowCompositionAttribute`
2. If `SetWindowCompositionAttribute` **failed** (common on many Win10/Win11 configurations), the window was left in a layered state **without** proper alpha composition
3. Result: window is fully transparent/invisible despite `ShowWindow` + `SetWindowPos` succeeding
4. Additionally, `Theme::Light` gave a white background that bled through the semi-transparent container

### Changes Made

#### 1. Critical DWM Acrylic Fix (`src/main.rs`)
- **Reordered** `apply_acrylic_blur()` so `SetWindowCompositionAttribute` is called **FIRST**
- `WS_EX_LAYERED` is only set **AFTER** the API succeeds
- If `SetWindowCompositionAttribute` fails, the window stays visible with Iced-rendered background
- Changed `gradient_color` from `0x803C3C3C` to `0x7F3C3C3C` for proper 50% opacity #3c3c3c tint
- Added detailed logging at every step (function pointer load, API call, result)

#### 2. Theme: `Theme::Light` → `Theme::Dark` (`src/main.rs:739`)
- Switched to `Theme::Dark` so the window has a dark background if DWM acrylic is unavailable
- Prevents white background showing through semi-transparent container

#### 3. Dark UI Design (`src/theme.rs`)
Applied the requested design:
- Background: `#3c3c3c` at 35% opacity (lets DWM acrylic show through)
- Selected row: `#4d4d4d` at 50% opacity
- Input field: `#1e1e1e` at 40% opacity
- Border: `#4d4d4d` **full opacity**, 2px width
- Rounded corners: 12px container, 6px input
- All text: light gray on dark (`#dcdcdc` primary, `#a0a0a0` muted)

#### 4. Comprehensive Debug Logging
**New atomic flags** (`src/main.rs`):
- `HOTKEY_REGISTERED` — tracks if RegisterHotKey succeeded
- `WINDOW_FOUND` — tracks if FindWindowW found the Iced window

**Enhanced logging** at every check point:
- `RegisterHotKey` result (SUCCESS/FAILED + conflict suggestion)
- Window creation and DWM application
- `WM_HOTKEY` event with loop iteration count
- `FindWindowW` return value for every lookup
- `IsWindowVisible` before show/hide decisions
- Background thread loop iteration counter
- UI Tick handler events (HOTKEY_TRIGGERED, EXIT_REQUESTED)
- Keyboard events (Escape pressed)

#### 5. Enhanced Debug Script (`debug.ps1`)
- Hotkey conflict detection (checks for PowerToys, Teams, Spotify, AutoHotkey, etc.)
- Auto-kill stale Element processes before launch
- Build + launch + monitor workflow (`-Build -Release` flags)
- Session summary with event counts (Alt+Space presses, shows, hides, errors)
- 30-second timeout with descriptive error messages
- Colorized output for different log levels

### How to Diagnose Issues

1. **If Alt+Space does nothing:**
   - Run `.\debug.ps1 -Build` (starts with monitoring)
   - Press Alt+Space and watch the log
   - Check for `CRITICAL: RegisterHotKey(Alt+Space) FAILED`
   - Check for `FindWindowW returned 0`
   - Check for `SetWindowCompositionAttribute FAILED`

2. **If application shows but no search box:**
   - Look for `CRITICAL: FindWindowW returned 0`
   - Check `Iced window not yet created or title mismatch`
   - The window title must be exactly "Element"

3. **If DWM acrylic not working (window appears but no blur):**
   - Log will show `SetWindowCompositionAttribute FAILED — acrylic blur not available`
   - The app falls back to `Theme::Dark` background — still usable

### Files Changed

| File | Change |
|------|--------|
| `src/main.rs` | DWM acrylic reorder, Theme::Dark, verbose logging, new atomics |
| `src/theme.rs` | Dark color tokens (#3c3c3c, #4d4d4d, adjusted opacities) |
| `src/ui/mod.rs` | Debug logging in Tick/key handlers |
| `debug.ps1` | Hotkey conflict detection, session summary, auto-build |
| `opencode.md` | This session log |

### Verification

```bash
cargo test        # 27 tests
cargo fmt --check # formatting
cargo clippy -- -D warnings  # lint
```

### Remaining Intentional Limitations
- DWM acrylic is a best-effort enhancement; app works without it
- Hotkey is still hardcoded as Alt+Space (config field reserved for future)
- Multi-monitor DPI not handled
- Window centered at fixed position