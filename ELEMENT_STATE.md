# Element — Living State Document

> **Last updated:** 2026-07-27 (Phase 5 — TOML, debounce, Escape, icons)
> **Purpose:** Single source of truth for architecture decisions, project state, and next moves.

---

## 1. Objective

Build **Element**: a Raycast-style global launcher for Windows, written in Rust with **Slint** for native GPU-accelerated UI.

Core interaction: global hotkey → floating acrylic search bar → type to search across apps/web/calc/emoji/clipboard → Enter to act.

---

## 2. Tech Stack & Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI Framework | **Slint 1.17** | Rust-native, GPU-accelerated, declarative UI. |
| Windowing | Built into Slint | Cross-platform (Win32, X11, Wayland, macOS). |
| Global Hotkey | `WH_KEYBOARD_LL` low-level hook | Captures hotkey + Escape. Polled by Slint Timer every 200ms. |
| Config | **TOML** via `toml` crate | `~/.element/config.toml`, migrates from old `config.json`. |
| App scanning | `walkdir` → Start Menu `*.lnk` | Extracts **icons** via Win32 `SHGetFileInfoW` + GDI. |
| Icon extraction | GDI `CreateDIBSection` + `DrawIconEx` | Converts HICON → RGBA pixel buffer → `slint::Image`. |
| Debounce | Slint `Timer` (SingleShot) | Restarted on every keystroke; search fires after `debounce_delay_ms`. |
| Web search | `webbrowser` crate | Opens configured `search_url` with query substituted for `%s`. |
| Calculator | `evalexpr = "11"` | Detects math expressions, evaluates, copies result. |
| Emoji | `emojis = "0.6"` | Search by name or shortcode on `emoji`/`:` prefix. |
| Clipboard | `arboard` (write only) | Copy results to clipboard. |
| Clipboard DB | `rusqlite` (bundled) | `clipboard_entries` table in `~/.element/element.db`. |
| DWM Blur | `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW)` | Acrylic backdrop. |
| Image processing | `image = "0.25"` | (Installed, available for future PNG save/cache) |
| Living Doc | `ELEMENT_STATE.md` | This file. |

---

## 3. Architecture

```
element/
├── src/
│   ├── main.rs       # Slint entry, hotkey + Escape hook, timer, DWM blur, debounce
│   ├── app.rs        # SearchEngine: app scan, icon extraction, calc, emoji, clipboard, web
│   ├── config.rs     # TOML config with JSON migration
│   └── database.rs   # SQLite — clipboard_entries table
├── ui/
│   └── main.slint    # Slint UI: Window → TextInput + Image + ListView
├── build.rs          # slint_build::compile("ui/main.slint")
├── brandkit/         # Brand assets
├── Cargo.toml
└── ELEMENT_STATE.md
```

### Search flow (debounced)

```
User types → on_input_changed
  → restart debounce Timer (SingleShot, debounce_delay_ms)
  → timer fires → SearchEngine::search(query)
    → 1. Calculator?
    → 2. Emoji? ("emoji" or ":" prefix)
    → 3. Clipboard? ("cbhist" or "clip")
    → 4. App search (fuzzy match + extracted icon)
    → 5. Web search fallback
  → VecModel<ResultItem> updated with icons
```

### Icon extraction

```
.lnk path → SHGetFileInfoW(SHGFI_ICON | SHGFI_SMALLICON) → HICON
  → CreateCompatibleDC + CreateDIBSection(32bpp)
  → DrawIconEx → BGRA pixel buffer
  → BGRA→RGBA swap → slint::Image::from_rgba8()
```

### Hotkey + Escape

```
WH_KEYBOARD_LL hook proc:
  - alt+space match → HOTKEY_TRIGGERED = true
  - Escape (vk=0x1B) + WINDOW_VISIBLE → ESCAPE_TRIGGERED = true
Slint Timer::Repeated(200ms) polls both flags
```

---

## 4. Work State

### Completed

- **Phase 1–3**: Original iced/GPUI prototypes (deleted).
- **Phase 4 — Slint rewrite**: Unified search bar, app launcher, web search, calc, emoji, clipboard, DWM blur. Committed `be40ee2`.
- **Phase 5 — Polish** (current):
  - **TOML config**: `serde_json` → `toml`, auto-migration from `config.json` to `config.toml`.
  - **Debounce**: Slint `Timer(Mode::SingleShot)` restarted on each keystroke. Configurable delay.
  - **Escape to close**: Captured in `WH_KEYBOARD_LL` hook when window is visible. Uses `WINDOW_VISIBLE` atomic flag tracked on show/hide.
  - **App icons**: Win32 GDI icon extraction from `.lnk` shortcuts. `SHGetFileInfoW` → HICON → GDI `CreateDIBSection` + `DrawIconEx` → BGRA→RGBA conversion → `slint::Image::from_rgba8()`. Icons displayed as 20×20 `Image` elements in search results.
  - Committed as `...` (pending).

### Active

- Feature-complete for current scope.

### Blocked

- Nothing currently blocked.

---

## 5. Next Moves

1. **System tray icon** — `tray-icon` or `windows-rs` for background presence.
2. **Auto-start** — Windows registry `HKCU\...\Run`.
3. **File search** — Walk `search_dirs`, index filenames.
4. **Settings panel** — In-app UI for hotkey, search URL, directories.
5. **Unit conversions** — Like RustCast.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| GDI icon extraction may fail on some .lnk files | Falls back to `None`; window still works with no icon |
| WH_KEYBOARD_LL may conflict | `CallNextHookEx` chains correctly; atomic flags, minimal overhead |
| DWM blur unsupported | Falls back to solid background |
| `arboard` may fail | Best-effort clipboard operations |
| Slint API changes | Pinned to 1.17.0 |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **Slint docs:** `https://slint.dev/releases/1.7.0/docs/slint`
- **Brandkit:** `brandkit/README.md`
