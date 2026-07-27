# Element — Living State Document

> **Last updated:** 2026-07-28 (Phase 9 — Provider architecture, register-based hotkey, tray icon, theme tokens)
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
| Keyboard in-app | `keyboard::on_key_pressed` subscription | Native event handling — Escape, arrows, Enter all work. |
| Config | **TOML** via `toml` crate | `~/.element/config.toml`, migrates from old `config.json`. |
| App scanning | **walkdir** recursive scan | `%ProgramData%` + `%APPDATA%` Start Menu folders, walks subdirectories. |
| App ranking | **Frecency** (SQLite) + **scored fuzzy match** | `count / (days_since + 1)` hybrid score boosts frequently used apps. |
| Fuzzy search | Scored character-level matcher | Word boundary, camelCase, consecutive, early-match bonuses + gap penalty. |
| Icons | **Real file search** first (PNG/ICO), fallback `SHGetFileInfoW` + GDI | Extracts 32×32, cached as PNG to disk. Flutter app aware. |
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
| Testing | `cargo test` (24 tests) | fuzzy scorer, frecency, calculator detection, config round-trip, clipboard. |
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
  on WM_HOTKEY → ShowWindow(SW_SHOWNA), SetWindowPos centering
  on WM_QUIT → exit thread
  on HIDE_REQUESTED atomic → ShowWindow(SW_HIDE)
  on RESIZE_REQUESTED atomic → SetWindowPos resize

Hidden tray window (class "ElementTrayClass"):
  on WM_APP + WM_LBUTTONDOWN → toggle overlay visibility
  on WM_APP + WM_RBUTTONUP → TrackPopupMenu with "Exit"

Iced subscription (30ms Tick):
  Consumes HOTKEY_TRIGGERED → clears input, refreshes all providers
  Returns text_input::focus("search") → search bar auto-focused

Iced keyboard subscription:
  key_pressed(Escape) → HIDE_REQUESTED = true
  key_pressed(ArrowUp/Down) → selected_index ± 1
  key_pressed(Enter) → engine.activate(selected)

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
- **Recommendations on empty search**: shows top frecency apps + remaining apps alphabetically.
- **Native icons**: `.lnk` binary parser extracts working directory → real file search (PNG/ICO in app folders, including Flutter subdirs) → fallback `SHGetFileInfoW` at 32×32. Cached as PNG to `~/.element/cache/icons/`.
- **Adaptive window height**: formula `52 + min(results,10)×42 + 8`, capped at 500, min 56. WIRED to `RESIZE_HEIGHT` atomic.
- **Config `window_width`**: wired to `WINDOW_WIDTH` atomic and initial window `Size`.
- **Provider architecture**: `SearchProvider` trait, `SearchContext`, 5 providers (apps, calculator, emoji, clipboard, websearch), `ProviderRegistry` with `catch_unwind` isolation.
- **`elementError` enum**: thiserror-based covering Config, Database, Io, Icon, Provider, Other.
- **`data_dir()` consolidated**: single source in `config.rs`, imported by `database.rs`.
- **Theme tokens**: `theme.rs` with named colors (BG_PRIMARY, TEXT_PRIMARY, ACCENT, etc.), spacing, sizing, radius. All inline values replaced in `ui/mod.rs`.
- **Tests**: 24 unit tests covering fuzzy scorer, frecency formula, calculator detection, config round-trip/JSON-migration, clipboard ordering, empty states.
- **CI workflow**: `.github/workflows/ci.yml` with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build` on Windows.
- **Release workflow**: `.github/workflows/release.yml` triggered on tag push, builds release binary, packages portable zip.

### Known issues

- DWM acrylic blur not yet implemented (needs winit window handle from Iced).
- Window centered at fixed position — no multi-monitor DPI awareness.
- `FindWindowW("Element")` looks up window by title — fragile if another window has same title.
- `selected_index = -1` on empty query means no Enter without typing.

---

## 5. Next Moves

1. DWM acrylic backdrop via `RunWithHandle` to get HWND.
2. Settings panel (in-app GUI for config.toml).
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
| Keyboard subscription may miss on unfocused window | Escape/Enter handled via `HIDE_REQUESTED` atomic, not dependent on Iced |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **Iced docs:** `https://docs.rs/iced/`
- **Brandkit:** `brandkit/README.md`
