# Changelog

## v1.4.0 (unreleased)

### Added
- Everyday quick actions in the system provider:
  - `volume 40` / `mute` — set the system volume (0-100); bare `volume` shows the current level
  - `screen off` — turn the display off without sleeping
  - `timer 10` / `timer 5m` / `timer 30s` / `timer 1h` — countdown with a tray balloon notification when it finishes
  - `password` / `password 24` — cryptographically random password (BCryptGenRandom) copied to the clipboard
  - `screenshot` — full virtual desktop capture (all monitors) to the clipboard as CF_DIB
- `future.md` — feature roadmap & tracker (checkbox per feature, built-in version, blocked reason)
- Live smoke tests for volume roundtrip and screen capture (`--ignored`)

### Changed
- System provider now parses parameterized commands (`volume:`, `timer:`, `password:` actions)
- Volume now uses Core Audio (`IAudioEndpointVolume`) — `waveOut` only touched a legacy mixer that doesn't change the real Windows volume (fixed "volume 20" not working)
- Tray balloons: fixed `NOTIFYICONDATAW` (missing `dwTimeout` shifted all balloon fields) + `NIM_SETVERSION` (NOTIFYICON_VERSION_4) so Win10/11 show them, plus a `MessageBeep` fallback
- Screen off debounced (2 s) and switched to `SendMessageTimeoutW` (single, synchron deliver, abort-if-hung) to stop on/off flicker
- Clipboard writes (`set_clipboard_bitmap`, new `set_clipboard_text`) retry up to 500 ms while the watcher holds the clipboard open
- Migrated to Iced 0.14 (wgpu): boot-first `iced::application`, `widget::operation` tasks for focus/scroll, unified `widget::Id`, `event::listen_with` subscription (0.14's TextInput captures Escape, so ignored-only keyboard events are no longer enough)
- `screenshot` now encodes the frame once — the PNG written to `Pictures\Screenshots` is the same buffer placed on the clipboard

### Fixed
- Clipboard images never appeared in history: the watcher bailed out (`continue`) whenever the clipboard held an image instead of text, so screenshots and copied pictures were never captured
- `screenshot` stuttered before producing output — two full-screen PNG encodes ran back to back (clipboard + file)

## v1.0.0 (2026-07-29)

### Added
- Window icon from brand kit (`element.ico`) embedded in the executable via `include_bytes!`
- Windows version info resource (`brandkit/windows/element.rc`) with proper metadata
- MIT `LICENSE` file
- Comprehensive `.gitignore` for Rust/IDE/build artifacts
- Inno Setup installer script (`installer.iss`)
- Winget publish manifests under `winget/vaibhxvvy.Element/`
- Window `transparent: true` for proper DWM acrylic/alpha-capable swap chain

### Fixed
- UI rendering on Windows 11 24H2+ where `SetWindowCompositionAttribute` reports success but doesn't apply acrylic — window now uses an alpha-capable swap chain via Iced's `transparent: true`, rendering correctly regardless of acrylic state
- Theme colors `BG_PRIMARY`, `BG_SELECTED`, `BG_INPUT` made fully opaque to ensure content is always visible

### Changed
- Version bumped to `1.0.0` — first stable release
- Window `background_color` set to transparent for proper alpha blending with DWM effects

## v0.8.0 (unreleased)

### Added
- Provider architecture: `SearchProvider` trait + `ProviderRegistry` with `catch_unwind` isolation
- 5 search providers: apps, calculator, emoji, clipboard, websearch
- `ElementError` enum with `thiserror`
- `Registry.rs` — iterates providers, catches panics
- `Theme.rs` — named color/spacing/radius tokens used by ui
- Unit tests: 27 tests covering fuzzy scorer, frecency, calculator, config, clipboard, URL encoding
- CI workflow: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build`
- Release workflow: tag-triggered build + portable zip + GitHub Release
- `CHANGELOG.md`
- `debug_log.rs` — file-based debug logger (~/.element/debug.log) with `debug_log!` macro
- Low-level keyboard hook fallback (`WH_KEYBOARD_LL`): claims hotkey when `RegisterHotKey` fails
- Single-instance guard: named mutex (`CreateMutexW`) prevents duplicate processes
- PID-based window finding: `EnumWindows` + `GetWindowThreadProcessId` replaces `FindWindowW`
- Safe FFI wrappers: every Win32 API call wrapped with `#[link(name = "...")]`, no `unsafe` at call sites
- Comprehensive Rust doc comments across all 15 source files
- `debug.ps1` — PowerShell debug monitor with hotkey conflict detection, session summary

### Changed
- Hotkey: `RegisterHotKey` + `PeekMessageW` replaces `GetAsyncKeyState` polling (zero CPU when idle)
- Hotkey strategy: three-tier fallback (RegisterHotKey → LL hook → fallback combos)
- System tray: `Shell_NotifyIconW` with hidden message window; left-click toggle, right-click Exit
- Icons: extracted at 32×32 (was 16×16), cached as PNG to `~/.element/cache/icons/`
- Icons: `.lnk` binary parser resolves working directory, searches real icon files (PNG/ICO) before falling back to `SHGetFileInfoW`
- Config `window_width` wired to `WINDOW_WIDTH` atomic and initial window size
- `data_dir()` consolidated into `config.rs`, duplicate removed from `database.rs`
- `SearchResult` now carries `provider_id` for registry dispatch
- All inline colors/spacing replaced with `theme.rs` tokens
- DWM acrylic fix: `SetWindowCompositionAttribute` called before `WS_EX_LAYERED`; graceful fallback
- Theme: `Theme::Light` → `Theme::Dark`, full dark palette (#3c3c3c bg, #4d4d4d border, #1e1e1e input)
- Window effects: DWM rounded corners (DWMWCP_ROUND) replacing acrylic blur
- Window: `transparent: true` to fix wgpu rendering on Win32 layered windows
- Main.rs: unnecessary `unsafe` blocks removed, dead `LoadIconW` wrapper removed
- AGENTS.md: updated with LL hook, single-instance, EnumWindows, safe FFI patterns
- ELEMENT_STATE.md: updated tech stack, architecture, risks
- opencode.md: full session log with root cause analysis
