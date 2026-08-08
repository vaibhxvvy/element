# Element — Living State Document

> **Last updated:** 2026-08-08 (Phase 18+ — Iced 0.14 migration, clipboard search/order, screenshot toast)
> **Purpose:** Single source of truth for architecture decisions, project state, and next moves.

---

## 1. Objective

Build **Element**: a Raycast-style global launcher for Windows, written in Rust with **Iced 0.14** (wgpu backend) for GPU-accelerated retained-mode UI.

Core interaction: `Alt+Space` → floating search bar with app recommendations → type to search across apps/web/calc/emoji/clipboard → Enter to act.

---

## 2. Tech Stack & Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI Framework | **Iced 0.14** (wgpu) | Retained-mode, native GPU, composable widgets, subscriptions. 0.14: boot-first `iced::application`, `widget::operation` tasks, unified `widget::Id`, `event::listen_with`. |
| Windowing | Iced (winit backend) | Borderless, always-on-top, starts hidden. |
| Global Hotkey | `RegisterHotKey` + `PeekMessageW` loop | No CPU polling. Background thread sleeps when idle, wakes on message. |
| Hotkey Fallback | **WH_KEYBOARD_LL** hook | When RegisterHotKey fails, LL hook intercepts key before competing app sees it. Three tiers: RegisterHotKey → LL hook → fallback combos. |
| Single Instance | Named mutex (`CreateMutexW`) | Second instance activates first via FindWindowW + show_launcher, then exits. |
| Window Finding | **EnumWindows** + PID for own window; FindWindowW for cross-instance | PID verification avoids title collisions with other processes. |
| FFI Safety | Safe `extern "system" fn` wrappers | Every Win32 API wrapped — no `unsafe` at call sites. All wrappers include MSDN doc links. |
| Tray Icon | `Shell_NotifyIconW` with hidden message-only window | Left-click toggles overlay; right-click shows Exit menu. |
| Keyboard in-app | Text-input submit + keyboard subscription | Enter submits the selected result; Escape and arrows are handled globally. |
| Config | **TOML** via `toml` crate | `~/.element/config.toml`, migrates from old `config.json`. |
| App scanning | **walkdir** background worker | `%ProgramData%` + `%APPDATA%` Start Menu folders are indexed off the Iced UI thread. |
| App ranking | **Frecency** (SQLite) + **scored fuzzy match** | `count / (days_since + 1)` hybrid score boosts frequently used apps. |
| Fuzzy search | Scored character-level matcher | Word boundary, camelCase, consecutive, early-match bonuses + gap penalty. |
| Icons | **COM `IShellLink`** → `.ico` or `IShellItemImageFactory` + PNG cache | Resolves `.lnk` targets, prefers the shortcut's `.ico`, then extracts the target `.exe` icon at 32×32. |
| Icon caching | `~/.element/cache/icons/<hash>.png` | Skip re-extraction on subsequent launches. |
| Auto-focus | `text_input::focus("search")` | Search bar focused immediately on window show. |
| Adaptive height | Win32 `SetWindowPos` via atomics | `52 + min(results, 10) × 42` px, capped at 500. |
| Provider arch | **SearchProvider trait** + ProviderRegistry | Each capability isolated behind a trait, `catch_unwind` per provider. |
| Orchestration | **Orchestrator** (`src/orchestrator.rs`) | Single entry point: `handle(Request) → Outcome`. UI sends `Request::Search/Activate/Refresh`, never touches providers directly. |
| Error handling | `ElementError` (thiserror) | Central error type covering config, DB, I/O, icon, provider. |
| Theme tokens | `theme.rs` constants | Named colors (#3c3c3c bg, #505050 selected, #1e1e1e input, #4d4d4d border) with 12px radius. |
| Web search | `webbrowser` crate | Opens configured `search_url` with query substituted for `%s`. |
| Calculator | `evalexpr = "11"` | Detects math expressions, evaluates, copies result. |
| Emoji | `emojis = "0.6"` | Search by name or shortcode on `emoji`/`:` prefix. |
| Clipboard | Win32 clipboard API + watcher thread (800 ms poll) | Copy/restore results and capture history (text + images) into SQLite. |
| Clipboard DB | `rusqlite` (bundled) | `clipboard_entries` + `clipboard_images` tables in `~/.element/element.db`; local-date filters computed in SQLite (`date('now','localtime',?)`). |
| Screenshot toast | `WM_APP_SCREENSHOT_DONE` (0x8002) → tray window | `screenshot` posts a boxed String body; the tray proc shows it via `show_toast` and sets the UI status row. |
| Window centering | `SetWindowPos` post-show | Centered horizontally, ⅓ from top of primary monitor. |
| Window effects | **DWM rounded corners** (DWMWCP_ROUND) | Solid #3c3c3c bg + small-radius corners avoids DWM acrylic fragility. |
| Testing | `cargo test` (101 tests) | Fuzzy scorer, calculator, config, clipboard (dedupe/pinning/order/date-text filters), frecency, files, URL encoding, system commands. |
| Debug logging | File-based (`~/.element/debug.log`) | Timestamped output via `debug_log!` macro; enabled in debug builds or ELEMENT_DEBUG=1. |
| Agent reference | `AGENTS.md` | Canonical doc for AI agents — architecture, provider system, design decisions. |
| Living Doc | `ELEMENT_STATE.md` | This file. |

---

## 3. Architecture

```
element/
├── src/
│   ├── main.rs        # Entry point: RegisterHotKey + PeekMessageW loop, LL hook fallback,
│   │                  #   single-instance guard, PID-based EnumWindows, tray window,
│   │                  #   clipboard watcher thread, Iced bootstrap
│   ├── orchestrator.rs # Orchestrator: single entry point — handle(Request) → Outcome;
│   │                  #   owns Arc<Config> + Arc<Database> + ProviderRegistry, registers providers
│   ├── config.rs      # TOML config with JSON migration, shared data_dir(), hotkey parsing
│   ├── database.rs    # SQLite — clipboard_entries + clipboard_images + frecency tables
│   ├── debug_log.rs   # File-based debug logger (~/.element/debug.log)
│   ├── error.rs       # ElementError enum (thiserror)
│   ├── platform.rs    # Win32 helpers: clipboard (CF_HDROP/DIB), lock/suspend, volume,
│   │                  #   screen capture, tray notifications (WM_APP_*)
│   ├── registry.rs    # ProviderRegistry: iterates providers, catch_unwind isolation
│   ├── theme.rs       # Named color/spacing/radius tokens (dark palette #3c3c3c)
│   ├── hotkey/        # Global hotkey detection (RegisterHotKey → LL hook → fallbacks)
│   │   └── mod.rs
│   ├── providers/
│   │   ├── mod.rs        # SearchProvider trait + SearchContext + SearchResult
│   │   ├── fuzzy.rs      # Shared fuzzy scorer (apps + files)
│   │   ├── icon.rs       # Shared icon pipeline (any shell path) + PNG cache
│   │   ├── apps/         # App search feature folder
│   │   │   ├── mod.rs    #   AppsProvider: fuzzy match → frecency boost → recommendations
│   │   │   ├── scan.rs   #   Start Menu scan (walkdir, .lnk → exe, dedup)
│   │   │   └── icons.rs  #   Shortcut layer: .lnk resolution + .ico preference
│   │   ├── calculator/   # evalexpr-based calculator
│   │   ├── color/        # #hex → hex/rgb/hsl with rendered swatch
│   │   ├── units/        # Unit conversion (5 km in miles)
│   │   ├── snippets/     # Quick text insertions from ~/.element/snippets.toml
│   │   ├── settings/     # In-launcher settings panel (UI intercepts kind=="settings")
│   │   ├── help/         # In-launcher manual (UI intercepts kind=="help")
│   │   ├── emoji/        # Emoji search via emojis crate (+ frecency boost)
│   │   ├── clipboard/    # Clipboard history (text + images): newest-first order,
│   │   │   └── mod.rs    #   date/text filters, sort:new/sort:old, pinning
│   │   ├── files/        # Raycast-style file/folder search ("file"/"folder" prefixes)
│   │   │   ├── mod.rs    #   FilesProvider: prefix parsing, fuzzy names, lazy icons, explorer.exe
│   │   │   └── scan.rs   #   Walk of curated user folders: exclusions, caps
│   │   ├── system/       # System commands: shutdown/sleep/lock/volume/screen off/timer/password/screenshot
│   │   └── websearch/    # Web search fallback (always at bottom) + URL passthrough + prefixes
│   └── ui/
│       └── mod.rs     # Iced views: search bar, result rows with icons, status feedback,
│                      #   settings + help panels
├── brandkit/          # Brand assets
├── Cargo.toml
├── README.md
├── AGENTS.md          # Canonical AI agent reference
└── ELEMENT_STATE.md
```

### Hotkey + navigation flow

```
Background thread (message loop, 50ms sleep when idle):
  register_launcher_hotkey():
    1. RegisterHotKey(NULL, 1, MOD_ALT|MOD_NOREPEAT, VK_SPACE)  ← preferred
    2. If fails, install WH_KEYBOARD_LL hook that swallows the event before
       the competing app sees it
    3. If both fail, try fallback combos (Alt+Space, Ctrl+Space, ...)

  on WM_HOTKEY → toggle_launcher():
    find_own_launcher_hwnd() using EnumWindows + PID
    inspect IsWindowVisible; hide when shown, otherwise restore,
    center with SetWindowPos(HWND_TOPMOST, SWP_SHOWWINDOW), foreground,
    and set HOTKEY_TRIGGERED atomic

  on tray Exit → EXIT_REQUESTED → Iced exits; WM_QUIT exits this thread
  on HIDE_REQUESTED atomic → ShowWindow(SW_HIDE)
  on RESIZE_REQUESTED atomic → SetWindowPos resize

  ensure_single_instance():
    CreateMutexW("Local\ElementLauncherSingleInstance")
    on ERROR_ALREADY_EXISTS → find_any_launcher_hwnd() + show_launcher() → exit
    Keep mutex open for process lifetime

Hidden tray window (class "ElementTrayClass"):
  on WM_APP + WM_LBUTTONDOWN → toggle using IsWindowVisible
  on WM_APP + WM_RBUTTONUP → TrackPopupMenu with "Exit"

Iced subscription (100ms Tick):
  Consumes HOTKEY_TRIGGERED → clears input, requests an app-index refresh,
    shows current recommendations, selects first result
  Detects a provider revision change → reruns current query
  Auto-dismisses status message after 2 seconds

Iced keyboard + input handling:
  TextInput.on_submit(Enter) → engine.activate(exact selected result)
  key_pressed(Escape) → HIDE_REQUESTED = true
  key_pressed(ArrowUp/Down) → selected_index ± 1
  row click → engine.activate(exact selected result)

Iced update → InputChanged:
  engine.search(query) → ProviderRegistry iterates providers in catch_unwind
  RESIZE_HEIGHT = adaptive_height(results)
  RESIZE_REQUESTED = true → thread calls SetWindowPos

Iced view:
  TextInput → on_input(InputChanged), on_submit(Enter)
  Scrollable (result rows) → each: icon, title, subtitle
  Status row → shows "Copied!", "Launched...", errors (auto-dismiss 2s)
```

---

## 4. Work State

### Completed

- **Iced 0.13 migration**: wgpu backend, retained-mode UI, keyboard subscriptions.
- **Register-based hotkey**: `RegisterHotKey` + `PeekMessageW` loop — zero CPU when idle.
- **Low-level keyboard hook fallback**: WH_KEYBOARD_LL installed when RegisterHotKey fails; intercepts key before competing app, swallows event. Key-repeat prevented via HOOK_KEY_HELD atomic.
- **Single-instance guard**: named mutex (`Local\ElementLauncherSingleInstance`); second instance activates the first via FindWindowW + show_launcher then exits.
- **PID-based window finding**: `find_own_launcher_hwnd()` uses EnumWindows + GetWindowThreadProcessId instead of FindWindowW to avoid title collisions.
- **Safe FFI wrappers**: every Win32 API wrapped in `extern "system" fn` with `#[link(...)]` — no `unsafe` at call sites, all wrappers include MSDN doc links.
- **System tray icon**: `Shell_NotifyIconW` with hidden message-only window, left-click toggle, right-click Exit.
- **Scored fuzzy matching**: word boundary, camelCase, consecutive, early-match bonuses + gap penalty.
- **Frecency ranking**: SQLite frecency table; `count / (days_since + 1)` hybrid boost capped at 3×.
- **Scored fuzzy matching**: word boundary, camelCase, consecutive, early-match bonuses + gap penalty.
- **Frecency ranking**: SQLite frecency table; `count / (days_since + 1)` hybrid boost capped at 3×.
- **Recommendations on open**: shows top frecency apps plus remaining apps alphabetically, with the first result selected.
- **Direct app activation**: a result stores the resolved executable path and starts that `.exe` directly. Visible app names are never used as activation keys.
- **Native icons**: shared pipeline in `providers/icon.rs` — `IShellItemImageFactory` extraction at 32×32 for any shell path (exe, file, folder), PNG-cached to `~/.element/cache/icons/v2-*.png`. Apps layer resolves `.lnk` → `.ico`/`.exe`; files provider fetches lazily on a worker thread.
- **Adaptive window height**: formula `52 + min(results,10)×42 + 8`, capped at 500, min 56. WIRED to `RESIZE_HEIGHT` atomic.
- **Config `window_width`**: wired to `WINDOW_WIDTH` atomic and initial window `Size`.
- **Provider architecture**: `SearchProvider` trait, `SearchContext`, **6** providers (apps, calculator, emoji, clipboard, **files**, websearch), `ProviderRegistry` with `catch_unwind` isolation.
- **File search**: `file <q>` / `folder <q>` prefixes fuzzy-match a background home-dir index; `explorer.exe` opens results with their default handler; `config.file_search_dirs` overrides the default home root.
- **`elementError` enum**: thiserror-based covering Config, Database, Io, Icon, Provider, Other.
- **`data_dir()` consolidated**: single source in `config.rs`, imported by `database.rs`.
- **COM shortcut pipeline**: `.lnk` resolution uses the correct `IShellLink::GetPath` vtable slot and reads the shortcut icon location. The helper initializes COM for its own call and releases every COM interface before returning.
- **Reliable activation**: `SearchResult.action` carries exact provider data. Failed activation keeps the launcher visible with feedback; copy actions confirm success; tray Exit signals the Iced event loop to terminate.
- **Duplicate-result prevention**: empty-query recommendations collapse legacy title-based frecency and current executable-path frecency into one action. The Start Menu index also keeps only one shortcut per case-insensitive executable path.
- **Foreground focus and submit**: Alt+Space and tray opening call `SetForegroundWindow` before Iced focuses the search input. The input owns Enter with `on_submit`; Escape remains a global close action.
- **Visibility recovery**: Alt+Space and tray left-click read `IsWindowVisible`, eliminating the stale visibility cache. Showing restores and foregrounds Element, clears an obsolete hide request, and then asks Iced to focus the search input.
- **Nonblocking app index**: `AppsProvider` launches its Start Menu/COM/icon scan on a single named worker thread. Window creation and hotkey handling never wait for that scan; a provider revision refreshes the visible query after the new index is published.
- **DWM acrylic fix**: reordered SetWindowCompositionAttribute before WS_EX_LAYERED; falls back to opaque bg on failure. Switched Theme::Light → Theme::Dark to prevent white bleed-through.
- **Dark UI design**: solid #3c3c3c bg, #505050 selected rows, #1e1e1e input, #4d4d4d 2px border stroke, 12px DWM rounded corners.
- **Comprehensive debug logging**: file-based debug_log! macro, timestamped output to ~/.element/debug.log, detailed logging at every Win32 API boundary.
- **Hotkey fallback strategy**: RegisterHotKey → WH_KEYBOARD_LL hook → fallback combos. LL hook swallows key event before competing app sees it. Key-repeat prevented by HOOK_KEY_HELD atomic.
- **Single-instance guard**: named mutex with second-instance activation via find_any_launcher_hwnd() + show_launcher().
- **PID-based window finding**: own-window lookup via EnumWindows + GetWindowThreadProcessId (not FindWindowW), avoiding title collisions. find_any_launcher_hwnd() retains FindWindowW for cross-instance activation.
- **Safe FFI wrappers**: every Win32 API wrapped in `extern "system" fn` with `#[link(name = "...")]` — no `unsafe` at call sites. All wrappers include MSDN doc references.
- **Doc comments**: full Rust doc comments across all 15 source files — architecture overviews, design rationales, field docs, unit test docs.
- **opencode.md** updated with full session log.
- **AGENTS.md** updated with new atomics, LL hook, single-instance, EnumWindows, safe FFI patterns.
- **ELEMENT_STATE.md** updated with new tech stack entries, architecture, and risk mitigations.
- **Module/domain split (Phase 16)**: `app.rs` → `orchestrator.rs` with `Request`/`Outcome` API; `hotkey.rs` → `hotkey/` folder; providers split into per-feature folders (`apps/{mod,scan,icons}`, `calculator/`, `emoji/`, `clipboard/`, `files/`, `websearch/`); `SearchResult` moved into `providers/mod.rs`. UI routes all actions through `engine.handle(Request)`.
- **File search (Phase 17)**: shared `providers/fuzzy.rs` (moved out of apps) and `providers/icon.rs` (generic path extraction) extracted; new `files/` provider with `file`/`folder` prefix gating, background index of curated user folders (`file_search_dirs` config, junk exclusions incl. version dirs/Android dumps, depth/entry caps), lazy icons via revision loop, `explorer.exe` activation, root-folder recommendations on empty query. 13 new tests (prefix parsing, folder mode, exclusions, ranking, root recs).
- **v1.3.0 — Clipboard image history**: watcher captures bitmaps (DIB/DIBV5/PNG) to `cache/clipboard/` full+thumb PNGs, pixel-hash dedupe, 16 MB cap; images shown as 64×64 thumbs in history, Enter restores to clipboard as CF_DIB; pinning for both text and image rows; trimming removes cached files.
- **v1.4.0 — Everyday quick actions**: `volume`/`mute` (Core Audio), `screen off`, `timer` (tray balloon via `WM_APP_TIMER_DONE`), `password` (BCryptGenRandom), `screenshot` (all-monitor GDI capture → clipboard CF_DIB + PNG to `Pictures\Screenshots`). 5 live smoke tests (`--ignored`).
- **Iced 0.14 migration**: boot-first `iced::application`, `widget::operation` tasks for focus/scroll, unified `widget::Id`, `event::listen_with` subscription, `Theme::Dark` value, 0.14 widget APIs (checkbox/pick_list/scrollable style types). Migration found and fixed two real bugs: clipboard watcher skipped image capture when the clipboard held an image; screenshot double-encoded the frame PNG.
- **Clipboard ordering + search (v1.4.0)**: history sorts by `(pinned, id)` — same-second captures keep capture order (was: alphabetical tie-break put the newest second); `clipboard_newest_first` config + live "Clipboard order" settings picker (`NEWEST_FIRST` static); query grammar `today` / `yesterday` / `YYYY-MM-DD` / `last Nd` (local dates via SQLite) and a text LIKE filter; `sort:new`/`sort:old` per-query override.
- **Screenshot toast (v1.4.0)**: `platform::notify_screenshot_captured` posts `WM_APP_SCREENSHOT_DONE` (0x8002) with a boxed String to the tray window → `show_toast` + UI status row.

### Known issues

- DWM acrylic blur not yet implemented (needs winit window handle from Iced). Current approach: solid #3c3c3c bg with DWM rounded corners.
- Window centered at fixed position — no multi-monitor DPI awareness.
- Start Menu shortcuts without an existing `.exe` target are intentionally skipped, so they cannot be launched through a stale `.lnk` path.
- `find_any_launcher_hwnd()` still uses `FindWindowW` for cross-instance activation — fragile if another window has same title.

---

## 5. Next Moves

1. Smoke-test v1.4.0 on a real desktop: clipboard `clip today`/`sort:old`/text filter, screenshot toast, timer balloon, settings clipboard-order picker.
2. Snippets that type anywhere (low-level keyboard hook typing into the focused app).
3. Wi-Fi / Bluetooth / brightness (WinRT helper layer).
4. Window switcher / tab finder (WindowStation APIs).
5. Plugin system (custom shell scripts / WASM plugins) when 8+ providers overlap.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `FindWindowW` may find wrong HWND | PID verification via `EnumWindows` + `GetWindowThreadProcessId` used for own-window lookup; FindWindowW only used for cross-instance activation |
| Icon extraction may fail on some .lnk files | Falls back to `None`; disk caching reduces re-failures |
| Provider panic crashes overlay | `catch_unwind` at registry level per search/activate call |
| Poisoned mutex kills app | All `.lock()` replaced with `match`/`.ok()` |
| Windows foreground policy can reject focus | A show request restores, raises, and foregrounds the launcher before Iced focuses the input; real-desktop smoke testing remains required |
| Hotkey conflict with other apps | Three-tier fallback: RegisterHotKey → LL hook (swallows event) → fallback combos |
| Unsafe FFI misuse | All Win32 APIs wrapped in safe `extern "system" fn` — no `unsafe` at call sites |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **Iced docs:** `https://docs.rs/iced/`
- **Brandkit:** `brandkit/README.md`
