# Element — Living State Document

> **Last updated:** 2026-07-28 (Phase 14 — nonblocking app indexing and startup recovery)
> **Purpose:** Single source of truth for architecture decisions, project state, and next moves.

---

## 1. Objective

Build **Element**: a Raycast-style global launcher for Windows, written in Rust with **Iced 0.13** (wgpu backend) for GPU-accelerated retained-mode UI.

Core interaction: `Alt+Space` → floating search bar with app recommendations → type to search across apps/web/calc/emoji/clipboard → Enter to act.

---

## 2. Tech Stack & Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI Framework | **Iced 0.13** (wgpu) | Retained-mode, native GPU, composable widgets, subscriptions. |
| Windowing | Iced (winit backend) | Borderless, always-on-top, starts hidden. |
| Global Hotkey | `RegisterHotKey` + `PeekMessageW` loop | No CPU polling. Background thread sleeps when idle, wakes on message. |
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
| Error handling | `ElementError` (thiserror) | Central error type covering config, DB, I/O, icon, provider. |
| Theme tokens | `theme.rs` constants | Named colors/spacing instead of inline values; ready for dark mode. |
| Web search | `webbrowser` crate | Opens configured `search_url` with query substituted for `%s`. |
| Calculator | `evalexpr = "11"` | Detects math expressions, evaluates, copies result. |
| Emoji | `emojis = "0.6"` | Search by name or shortcode on `emoji`/`:` prefix. |
| Clipboard | `arboard` (write only) | Copy results to clipboard. |
| Clipboard DB | `rusqlite` (bundled) | `clipboard_entries` table in `~/.element/element.db`. |
| Window centering | `SetWindowPos` post-show | Centered horizontally, ⅓ from top of monitor. |
| Testing | `cargo test` (27 tests) | fuzzy scorer, executable and frecency deduplication, calculator detection, config round-trip, clipboard, URL encoding. |
| Agent reference | `AGENTS.md` | Canonical doc for AI agents — architecture, provider system, design decisions. |
| Living Doc | `ELEMENT_STATE.md` | This file. |

---

## 3. Architecture

```
element/
├── src/
│   ├── main.rs        # Entry point: RegisterHotKey + PeekMessageW loop, tray window, Iced bootstrap
│   ├── app.rs         # SearchEngine: owns ProviderRegistry, holds Arc<Config> + Arc<Database>
│   ├── config.rs      # TOML config with JSON migration, shared data_dir()
│   ├── database.rs    # SQLite — clipboard_entries + frecency tables
│   ├── error.rs       # ElementError enum (thiserror)
│   ├── registry.rs    # ProviderRegistry: iterates providers, catch_unwind isolation
│   ├── theme.rs       # Named color/spacing/radius tokens
│   ├── providers/
│   │   ├── mod.rs        # SearchProvider trait + SearchContext
│   │   ├── apps.rs       # Installed-app scan + fuzzy match + frecency + icon pipeline
│   │   ├── calculator.rs # evalexpr-based calculator
│   │   ├── emoji.rs      # Emoji search via emojis crate
│   │   ├── clipboard.rs  # Clipboard history from SQLite
│   │   └── websearch.rs  # Web search fallback (always at bottom)
│   └── ui/
│       └── mod.rs     # Iced views: search bar, result rows with icons, styles (uses theme.rs tokens)
├── brandkit/          # Brand assets
├── Cargo.toml
├── README.md
└── ELEMENT_STATE.md
```

### Hotkey + navigation flow

```
Background thread (message loop, 50ms sleep when idle):
  RegisterHotKey(NULL, 1, MOD_ALT|MOD_NOREPEAT, VK_SPACE)
   on WM_HOTKEY → inspect IsWindowVisible; hide when shown, otherwise restore,
                  center with SetWindowPos(HWND_TOPMOST, SWP_SHOWWINDOW), foreground,
                  and request Iced input focus
  on tray Exit → EXIT_REQUESTED → Iced exits; WM_QUIT exits this thread
  on HIDE_REQUESTED atomic → ShowWindow(SW_HIDE)
  on RESIZE_REQUESTED atomic → SetWindowPos resize

Hidden tray window (class "ElementTrayClass"):
  on WM_APP + WM_LBUTTONDOWN → toggle using the real Win32 visibility state
  on WM_APP + WM_RBUTTONUP → TrackPopupMenu with "Exit"

Iced subscription (30ms Tick):
  Consumes HOTKEY_TRIGGERED → clears input, requests an app-index refresh, shows current recommendations, selects first result
  Detects a provider revision change → reruns the current query after new app data is published
  Returns text_input::focus("search") → search bar auto-focused
  scrollable::scroll_to("results", AbsoluteOffset) → scrolls to selected item

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
  TextInput(id="search") → on_input(InputChanged)
  Scrollable (result rows) → each: indicator, icon, title, subtitle
```

---

## 4. Work State

### Completed

- **Iced 0.13 migration**: wgpu backend, retained-mode UI, keyboard subscriptions.
- **Register-based hotkey**: `RegisterHotKey` + `PeekMessageW` loop — zero CPU when idle.
- **System tray icon**: `Shell_NotifyIconW` with hidden message-only window, left-click toggle, right-click Exit.
- **Scored fuzzy matching**: word boundary, camelCase, consecutive, early-match bonuses + gap penalty.
- **Frecency ranking**: SQLite frecency table; `count / (days_since + 1)` hybrid boost capped at 3×.
- **Recommendations on open**: shows top frecency apps plus remaining apps alphabetically, with the first result selected.
- **Direct app activation**: a result stores the resolved executable path and starts that `.exe` directly. Visible app names are never used as activation keys.
- **Native icons**: COM `IShellLink` resolves the target and optional icon location. Valid `.ico` files are decoded first; otherwise the target executable icon is extracted at 32×32. Cached as PNG to `~/.element/cache/icons/v2-*.png`.
- **Adaptive window height**: formula `52 + min(results,10)×42 + 8`, capped at 500, min 56. WIRED to `RESIZE_HEIGHT` atomic.
- **Config `window_width`**: wired to `WINDOW_WIDTH` atomic and initial window `Size`.
- **Provider architecture**: `SearchProvider` trait, `SearchContext`, 5 providers (apps, calculator, emoji, clipboard, websearch), `ProviderRegistry` with `catch_unwind` isolation.
- **`elementError` enum**: thiserror-based covering Config, Database, Io, Icon, Provider, Other.
- **`data_dir()` consolidated**: single source in `config.rs`, imported by `database.rs`.
- **COM shortcut pipeline**: `.lnk` resolution uses the correct `IShellLink::GetPath` vtable slot and reads the shortcut icon location. The helper initializes COM for its own call and releases every COM interface before returning.
- **Reliable activation**: `SearchResult.action` carries exact provider data. Failed activation keeps the launcher visible with feedback; copy actions confirm success; tray Exit signals the Iced event loop to terminate.
- **Duplicate-result prevention**: empty-query recommendations collapse legacy title-based frecency and current executable-path frecency into one action. The Start Menu index also keeps only one shortcut per case-insensitive executable path.
- **Foreground focus and submit**: Alt+Space and tray opening call `SetForegroundWindow` before Iced focuses the search input. The input owns Enter with `on_submit`; Escape remains a global close action.
- **Visibility recovery**: Alt+Space and tray left-click read `IsWindowVisible`, eliminating the stale visibility cache. Showing restores and foregrounds Element, clears an obsolete hide request, and then asks Iced to focus the search input.
- **Nonblocking app index**: `AppsProvider` launches its Start Menu/COM/icon scan on a single named worker thread. Window creation and hotkey handling never wait for that scan; a provider revision refreshes the visible query after the new index is published.
- **Branded launcher shell**: the embedded `brandkit/app-icons/icon-64.png` mark is shown in the header. The compact dark surface uses the documented Ink, Primary, Core White, and Text Grey palette.
- **Window focus fix**: `ShowWindow(SW_RESTORE)`, `SetWindowPos(hwnd, HWND_TOPMOST, ..., SWP_SHOWWINDOW)`, and `SetForegroundWindow` are used together before Iced focuses the input.
- **Auto-scroll**: `scrollable::scroll_to()` with `AbsoluteOffset` on arrow up/down and input change.
- **Hidden scrollbar**: Custom `element_scrollable_style()` with transparent scroller and no rail.
- **Clippy cleanup**: All warnings fixed across `theme.rs`, `main.rs`, `apps.rs`, `ui/mod.rs`.
- **Theme tokens**: `theme.rs` with named colors (BG_PRIMARY, TEXT_PRIMARY, ACCENT, etc.), spacing, sizing, radius. All inline values replaced in `ui/mod.rs`.
- **Tests**: 27 unit tests covering fuzzy scorer, executable target validation, legacy/current frecency deduplication, calculator detection, config round-trip/JSON-migration, clipboard ordering, and web URL encoding.
- **CI workflow**: `.github/workflows/ci.yml` with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build` on Windows.
- **Release workflow**: `.github/workflows/release.yml` triggered on tag push, builds release binary, packages portable zip.

### Known issues

- DWM acrylic blur not yet implemented (needs winit window handle from Iced).
- Window centered at fixed position — no multi-monitor DPI awareness.
- `FindWindowW("Element")` looks up window by title — fragile if another window has same title.
- Start Menu shortcuts without an existing `.exe` target are intentionally skipped, so they cannot be launched through a stale `.lnk` path.

---

## 5. Next Moves

1. Manually smoke-test representative Start Menu shortcuts (classic Win32, custom `.ico`, duplicate titles) in a real Windows desktop session.
2. DWM acrylic backdrop via `RunWithHandle` to get HWND.
3. File search provider (index user directories, `.gitignore`-style filtering).
4. Clipboard monitor (watch OS clipboard, auto-store in SQLite).
5. Plugin system (custom shell scripts / WASM plugins).

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `FindWindowW` may find wrong HWND | PID verification via `GetWindowThreadProcessId` could be added |
| Icon extraction may fail on some .lnk files | Falls back to `None`; all-white icon filter; disk caching reduces re-failures |
| Provider panic crashes overlay | `catch_unwind` at registry level per search/activate call |
| Poisoned mutex kills app | All `.lock()` replaced with `match`/`.ok()` |
| Windows foreground policy can reject focus | A show request restores, raises, and foregrounds the launcher before Iced focuses the input; real-desktop smoke testing remains required |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **Iced docs:** `https://docs.rs/iced/`
- **Brandkit:** `brandkit/README.md`
