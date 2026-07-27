# Element — Living State Document

> **Last updated:** 2026-07-27 (Phase 7 — egui migration)
> **Purpose:** Single source of truth for architecture decisions, project state, and next moves.

---

## 1. Objective

Build **Element**: a Raycast-style global launcher for Windows, written in Rust with **egui** for GPU-accelerated immediate-mode UI.

Core interaction: global hotkey → floating acrylic search bar → type to search across apps/web/calc/emoji/clipboard → Enter to act.

---

## 2. Tech Stack & Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI Framework | **egui 0.31** (via eframe) | Immediate-mode, native GPU, simple keyboard API, scrollable natively. |
| Windowing | eframe (winit backend) | Borderless, always-on-top, transparent, starts hidden. |
| Global Hotkey + Keys | `GetAsyncKeyState` polling thread | Background thread polls every 20ms, directly calls `ShowWindow`. No hooks needed. |
| Keyboard in-app | egui `ctx.input().key_pressed()` | Native event handling — Escape, arrows, Enter all work without workarounds. |
| Config | **TOML** via `toml` crate | `~/.element/config.toml`, migrates from old `config.json`. |
| App scanning | **walkdir** recursive scan | `%ProgramData%` + `%APPDATA%` Start Menu folders, walks subdirectories. |
| Icon extraction | GDI `CreateDIBSection` + `DrawIconEx` | Converts HICON → RGBA pixel buffer → `egui::ColorImage`. |
| Debounce | Done in search function (no timer needed) | Search runs on input change via `TextEdit::changed()`. |
| Web search | `webbrowser` crate | Opens configured `search_url` with query substituted for `%s`. |
| Calculator | `evalexpr = "11"` | Detects math expressions, evaluates, copies result. |
| Emoji | `emojis = "0.6"` | Search by name or shortcode on `emoji`/`:` prefix. |
| Clipboard | `arboard` (write only) | Copy results to clipboard. |
| Clipboard DB | `rusqlite` (bundled) | `clipboard_entries` table in `~/.element/element.db`. |
| DWM Blur | TBD (not yet implemented in egui) | |
| Window icon | `LoadImageW` + `WM_SETICON` | Applied after show (when HWND exists). |
| Window centering | `SetWindowPos` post-show | Centered horizontally, ⅓ from top of monitor. |
| App matching | substring, stem (spaceless), sequential chars | "pwsh" → "WindowsPowerShell" works. |
| Living Doc | `ELEMENT_STATE.md` | This file. |

---

## 3. Architecture

```
element/
├── src/
│   ├── main.rs       # egui Application + hotkey thread (GetAsyncKeyState polling)
│   ├── app.rs        # SearchEngine: app scan, icon extraction, calc, emoji, clipboard, web
│   ├── config.rs     # TOML config with JSON migration
│   └── database.rs   # SQLite — clipboard_entries table
├── brandkit/         # Brand assets
├── Cargo.toml
└── ELEMENT_STATE.md
```

### Hotkey + navigation flow

```
Background thread (20ms loop):
  GetAsyncKeyState(VK_MENU + VK_SPACE) → if armed → toggle
    ShowWindow(SW_SHOWNA / SW_HIDE) directly via HWND
    SetWindowPos for centering
    Sets VISIBLE / RESIZE_REQUESTED atomics

egui App::update (on ShowWindow → winit event):
  Consumes RESIZE_REQUESTED → resets input, refreshes apps
  Checks VISIBLE → if false, sleeps 30ms and returns
  
  Keyboard (native egui):
    ctx.input().key_pressed(Escape) → HIDE_REQUESTED = true
    ctx.input().key_pressed(ArrowUp/Down) → selected_index ± 1
    ctx.input().key_pressed(Enter) → activate selected

  UI:
    TextEdit (search bar) → on change → engine.search()
    ScrollArea (results list) → native scroll wheel support
    Each row: icon, title, subtitle, click handler
```

---

## 4. Work State

### Completed

- **Phase 7 — egui migration**:
  - Replaced Slint with egui/eframe 0.31.
  - Background thread polls `GetAsyncKeyState` every 20ms for Alt+Space hotkey.
  - Window starts hidden (`with_visible(false)`), shown via `ShowWindow` from the thread.
  - Escape, arrows, Enter handled natively via `ctx.input().key_pressed()`.
  - `ScrollArea::vertical()` with `max_height(400.0)` — scroll wheel works natively.
  - Recursive app scanning via `walkdir` — finds shortcuts in subdirectories.
  - Fuzzy matching: substring, stem (spaceless), sequential characters.
  - Window icon, centering, always-on-top, borderless, transparent background.
  - Dark theme matching Slint version.

### Known issues

- DWM acrylic blur not yet implemented (needs winit window handle access in egui).
- Window initially hidden — Alt+Space shows it. After hide, the event loop continues at reduced rate.
- `FindWindowW("Element")` looks up the window by title — fragile if another window has same title.

---

## 5. Next Moves

1. DWM acrylic backdrop — get `HWND` from egui's winit window and apply `DwmSetWindowAttribute`.
2. System tray icon for background presence.
3. Auto-start via registry.
4. Settings panel.
5. File search.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| egui immediate-mode consumes GPU when visible | `request_repaint_after(16ms)` limits to ~60fps; no repaint when hidden |
| `FindWindowW` may find wrong HWND | PID verification via `GetWindowThreadProcessId` could be added |
| Icon extraction may fail on some .lnk files | Falls back to `None`; all-white icon filter |
| Poisoned mutex kills app | All `.lock().unwrap()` replaced with `match`/`.ok()` |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **egui docs:** `https://docs.rs/egui/`
- **Brandkit:** `brandkit/README.md`
