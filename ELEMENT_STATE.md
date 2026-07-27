# Element — Living State Document

> **Last updated:** 2026-07-27 (Phase 4 — Slint rewrite)
> **Purpose:** Single source of truth for architecture decisions, project state, and next moves.

---

## 1. Objective

Build **Element**: a Raycast-style global launcher for Windows, written in Rust with **Slint** for native GPU-accelerated UI.

Core interaction: global hotkey → floating acrylic search bar → type to search across apps/web/calc/emoji/clipboard → Enter to act.

---

## 2. Tech Stack & Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI Framework | **Slint 1.17** | Rust-native, GPU-accelerated, declarative UI. Minimal binary size, no browser/JS runtime. |
| Windowing | Built into Slint | Cross-platform (Win32, X11, Wayland, macOS). |
| Global Hotkey | `WH_KEYBOARD_LL` low-level hook | Reliable capture across all apps. Falls back to `GetAsyncKeyState` polling. |
| Brand | Element, `#6D4AFA` primary | Brand assets in `brandkit/`. |
| Config | `serde_json` → `~/.element/config.json` | Key: hotkey, window_width, window_height, search_url, search_dirs. |
| App scanning | `walkdir` → `%ProgramData%` / `%APPDATA%` Start Menu `*.lnk` | Windows-only. |
| Web search | `webbrowser` crate | Opens configured `search_url` with query substituted for `%s`. |
| Calculator | `evalexpr = "11"` | Detects math expressions, evaluates them. |
| Emoji | `emojis = "0.6"` | Search by name or shortcode. |
| Clipboard | `arboard` (copy only) | Copy results (calc, emoji, clipboard entries) to clipboard. |
| Clipboard DB | `rusqlite` (bundled) | `clipboard_entries` table in `~/.element/element.db`. |
| DWM Blur | `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW)` | Acrylic backdrop effect on the window. |
| Living Doc | `ELEMENT_STATE.md` | This file. |

---

## 3. Architecture

```
element/
├── src/
│   ├── main.rs       # Slint entry, hotkey hook, timer polling, DWM blur
│   ├── app.rs        # SearchEngine: app scan, calc, emoji, clipboard, web search
│   ├── config.rs     # JSON config (hotkey, window size, search_url, search_dirs)
│   └── database.rs   # SQLite — clipboard_entries table
├── ui/
│   └── main.slint    # Slint UI: Window → TextInput + placeholder + ListView
├── build.rs          # slint_build::compile("ui/main.slint")
├── brandkit/         # Brand assets
├── Cargo.toml
└── ELEMENT_STATE.md
```

### Search flow

```
User types → on_input_changed callback
  → SearchEngine::search(query)
    → 1. Calculator? (digits/operators detected)
    → 2. Emoji? (query starts with "emoji" or ":")
    → 3. Clipboard? (query is "cbhist" or starts with "clip")
    → 4. App search? (fuzzy match installed .lnk names)
    → 5. Web search fallback (always present)
  → VecModel<ResultItem> updated in ListView
```

### Activate flow

```
User presses Enter → on_item_selected(index)
  → SearchEngine::activate(kind, title, input)
    → "app":       cmd /c start "" "<path>"
    → "websearch": webbrowser::open(search_url.replace("%s", query))
    → "calc":      arboard::Clipboard::set_text(result)
    → "emoji":     arboard::Clipboard::set_text(emoji_char)
    → "clipboard": arboard::Clipboard::set_text(text)
  → window.hide()
```

### Hotkey system

`SetWindowsHookExW(WH_KEYBOARD_LL=13)` captures low-level keyboard events. Hook proc checks virtual key + modifier state (`GetAsyncKeyState`). When the combo matches, sets `AtomicBool`. A Slint `Timer(Mode::Repeated, 200ms)` polls the flag and toggles `window.show()` / `window.hide()`.

---

## 4. Work State

### Completed

- **Phase 1–3**: Original iced/GPUI prototypes with text editor, module system, notes, slash commands (all deleted).
- **Phase 4 — Slint rewrite** (current):
  - Framework swap: iced + GPUI → Slint 1.17.
  - `ui/main.slint` — Window with TextInput, placeholder Text, ListView of ResultItem.
  - `Cargo.toml` — slint, slint-build, walkdir, webbrowser, arboard, emojis, evalexpr, rusqlite.
  - `build.rs` — `slint_build::compile("ui/main.slint")`.
  - `src/main.rs` — Slint entry, WH_KEYBOARD_LL hook + timer polling, DWM acrylic backdrop.
  - `src/app.rs` — SearchEngine: app scanning (Start Menu .lnk), calculator, emoji, clipboard, web search.
  - `src/config.rs` — Config with hotkey, search_url, search_dirs, window width/height.
  - `src/database.rs` — clipboard_entries table, `load_clipboard()`.
  - Deleted: `src/editor/`, `src/module.rs`, `src/overlay.rs`, `src/clipboard.rs`, `src/style.rs`.
  - Committed as `be40ee2`.

### Active

- **Tray icon**: Not implemented — deferred to user.

### Blocked

- Nothing currently blocked.

---

## 5. Next Moves

1. **System tray icon** — `tray-icon` or `windows-rs` for background presence.
2. **Auto-start** — Windows registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
3. **File search** — Walk `search_dirs`, index filenames, add to unified search results.
4. **Settings panel** — In-app UI for hotkey, search URL, directories (or keep as config file).
5. **Escape to close** — Keyboard event handling in Slint (deferred; user will handle UI).

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| WH_KEYBOARD_LL may conflict with other hooks | Runs with `CallNextHookEx` chain; atomic flag, minimal overhead |
| DWM blur unsupported on older Windows | `DwmSetWindowAttribute` may silently fail — window still works with solid background |
| `arboard` may fail on headless/remote sessions | Clipboard operations are best-effort |
| Slint API changes between versions | Pinned to 1.17.0 in `Cargo.toml` |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **Slint docs:** `https://slint.dev/releases/1.7.0/docs/slint`
- **Brandkit:** `brandkit/README.md`
