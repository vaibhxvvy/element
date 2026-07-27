# Element — Living State Document

> **Last updated:** 2026-07-27 (Phase 8 — Iced 0.13 migration, frecency, scored fuzzy, icons, adaptive height)
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
| Global Hotkey | `GetAsyncKeyState` polling thread | Background thread polls every 20ms, directly calls `ShowWindow`. No hooks. |
| Keyboard in-app | `keyboard::on_key_pressed` subscription | Native event handling — Escape, arrows, Enter all work. |
| Config | **TOML** via `toml` crate | `~/.element/config.toml`, migrates from old `config.json`. |
| App scanning | **walkdir** recursive scan | `%ProgramData%` + `%APPDATA%` Start Menu folders, walks subdirectories. |
| App ranking | **Frecency** (SQLite) + **scored fuzzy match** | `count / (days_since + 1)` hybrid score boosts frequently used apps. |
| Fuzzy search | Scored character-level matcher | Word boundary, camelCase, consecutive, early-match bonuses + gap penalty. |
| Icons | **GDI** `CreateDIBSection` + `DrawIconEx` | HICON → RGBA → `iced::widget::image::Handle::from_rgba`. |
| Auto-focus | `text_input::focus("search")` | Search bar focused immediately on window show. |
| Adaptive height | Win32 `SetWindowPos` via atomics | `52 + min(results, 10) × 42` px, capped at 500. |
| Web search | `webbrowser` crate | Opens configured `search_url` with query substituted for `%s`. |
| Calculator | `evalexpr = "11"` | Detects math expressions, evaluates, copies result. |
| Emoji | `emojis = "0.6"` | Search by name or shortcode on `emoji`/`:` prefix. |
| Clipboard | `arboard` (write only) | Copy results to clipboard. |
| Clipboard DB | `rusqlite` (bundled) | `clipboard_entries` table in `~/.element/element.db`. |
| Window centering | `SetWindowPos` post-show | Centered horizontally, ⅓ from top of monitor. |
| Living Doc | `ELEMENT_STATE.md` | This file. |

---

## 3. Architecture

```
element/
├── src/
│   ├── main.rs        # Entry point: hotkey thread (GetAsyncKeyState), Iced bootstrap
│   ├── app.rs         # SearchEngine: app scan, fuzzy scorer, frecency, icon extraction
│   ├── config.rs      # TOML config with JSON migration
│   ├── database.rs    # SQLite — clipboard_entries + frecency tables
│   └── ui/
│       └── mod.rs     # Iced views: search bar, result rows with icons, styles
├── brandkit/          # Brand assets
├── Cargo.toml
├── README.md
└── ELEMENT_STATE.md
```

### Hotkey + navigation flow

```
Background thread (20ms loop):
  GetAsyncKeyState(VK_MENU + VK_SPACE) → if armed → toggle
    ShowWindow(SW_SHOWNA / SW_HIDE) via HWND
    SetWindowPos for centering
    Sets HOTKEY_TRIGGERED / VISIBLE / RESIZE_REQUESTED atomics

Iced subscription (30ms Tick):
  Consumes HOTKEY_TRIGGERED → clears input, refreshes apps
  Returns text_input::focus("search") → search bar auto-focused

Iced keyboard subscription:
  key_pressed(Escape) → HIDE_REQUESTED = true
  key_pressed(ArrowUp/Down) → selected_index ± 1
  key_pressed(Enter) → activate selected

Iced update → InputChanged:
  engine.search(query) → scored fuzzy match × frecency boost
  RESIZE_HEIGHT = adaptive_height(results)
  RESIZE_REQUESTED = true → thread calls SetWindowPos

Iced view:
  TextInput(id="search") → on_input(InputChanged)
  Scrollable (result rows) → each: indicator, icon, title, subtitle
```

---

## 4. Work State

### Completed

- **Phase 8 — Iced 0.13 migration**:
  - Replaced egui/Slint with Iced 0.13 wgpu backend.
  - Background thread polls `GetAsyncKeyState` every 20ms for Alt+Space hotkey.
  - Window starts hidden, shown via `ShowWindow` from the thread.
  - `keyboard::on_key_pressed` subscription handles Escape, arrows, Enter.
  - `Scrollable` with `height(Length::Shrink)` — scroll wheel works natively.
  - Recursive app scanning via `walkdir` — finds shortcuts in subdirectories.
  - **Scored fuzzy matching** — word boundary, camelCase, consecutive, early-match bonuses + gap penalty.
  - **Frecency ranking** — SQLite `frecency` table tracks launches; `count / (days_since + 1)` hybrid boost.
  - **Recommendations on empty search** — shows top frecency apps + all apps alphabetically.
  - **Native icons** rendered via `iced::widget::image::Handle::from_rgba` at 16×16.
  - **Adaptive window height** — `SetWindowPos` via atomics resizes to fit results (max 10 rows, capped 500px).
  - **Auto-focus** on search bar via `text_input::focus("search")` on window show.
  - `ui/` module structure — all UI code in `src/ui/mod.rs`.
  - Light theme with dark text for readability.

### Known issues

- DWM acrylic blur not yet implemented (needs winit window handle from Iced).
- Window centered at fixed position — no multi-monitor DPI awareness.
- `FindWindowW("Element")` looks up window by title — fragile if another window has same title.
- Icons extracted sequentially during app scan — no caching to disk.
- `selected_index = -1` on empty query means no Enter without typing.

---

## 5. Next Moves

1. DWM acrylic backdrop via `RunWithHandle` to get HWND.
2. System tray icon for background presence.
3. Icon caching to disk (avoid re-extraction on every launch).
4. Settings panel (in-app GUI for config.toml).
5. File search provider.
6. Plugin system (custom shell scripts).

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `FindWindowW` may find wrong HWND | PID verification via `GetWindowThreadProcessId` could be added |
| Icon extraction may fail on some .lnk files | Falls back to `None`; all-white icon filter |
| Poisoned mutex kills app | All `.lock()` replaced with `match`/`.ok()` |
| Keyboard subscription may miss on unfocused window | Escape/Enter handled via `HIDE_REQUESTED` atomic, not dependent on Iced |
| Hotkey thread polling is wasteful (20ms CPU wake) | Low priority; could switch to `RegisterHotkey` Win32 API |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **Iced docs:** `https://docs.rs/iced/`
- **Brandkit:** `brandkit/README.md`
