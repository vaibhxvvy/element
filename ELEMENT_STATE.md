# Element — Living State Document

> **Last updated:** 2026-07-26 (Phase 3 — SQLite DB, clipboard, calc, emoji)
> **Purpose:** Single source of truth for architecture decisions, project state, and next moves. Must be kept accurate after every session.

---

## 1. Objective

Build **Element**: a Raycast-style global launcher + local knowledge tool for Windows and Linux, written in Rust with GPUI for GPU-accelerated UI. The text editor is Phase 0 — a foundation module, not the product.

Core interaction model: global hotkey → floating frosted-glass overlay → search-as-you-type across files/notes/nodes/apps + slash commands route to built-in modules (`/notes`, `/calc`, `/cbhist`, `/emoji`, `/help`, `/savenote`, `/noteslist`) via a struct-based registry with closure activation.

---

## 2. Tech Stack & Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI Framework | **GPUI** (via `gpui-unofficial = "1.11.3"`) | Zed's GPU-accelerated framework. `gpui-unofficial` is a community mirror that tracks Zed releases (currently v1.11.3) and supports Windows + Linux + macOS. Official `gpui` v0.2.2 lacks Windows support. |
| Windowing | Built into `gpui-unofficial` | Platform backends included: Windows (Win32 + DirectWrite), Linux (x11 + wayland features), macOS (font-kit). |
| Global Hotkey | `global-hotkey = "0.5"` (tauri-apps) | Default `Alt+Space`, configurable via `ElementConfig.hotkey`. Parsed from string: `Alt+Space`, `Super+Space` (Windows key), `Ctrl+Shift+F`, etc. Polled via `AsyncWindowContext.background_executor().timer()`. Wayland support deferred. |
| Brand | Element, `#6D4AFA` primary / `#08060D` ink / `#FDFAFE` core white | Full brandkit at `brandkit/`. |
| Persistence | `serde` + `serde_json` → `~/.element/config.json` | Config settings. |
| SQLite | `rusqlite = "0.31"` (bundled) | `Database` struct with own `Connection`. Clipboard history and notes stored in `~/.element/element.db`. Separate connection per thread. |
| Clipboard | `arboard = "3.3"` | Monitored via background thread polling every 500ms. Stores text entries in SQLite via `ClipBoardContentType`. |
| Calculator | `evalexpr = "11"` | `/calc` module evaluates math expressions. Replaces `x` with `*`, `÷` with `/`. |
| Emoji | `emojis = "0.6"` | `/emoji` module searches by name or shortcode. |
| Migration Strategy | **In-place rewrite** | Replace egui/eframe code directly with GPUI. No parallel binary targets. |
| Module System | `struct Module` with `activate: Arc<dyn Fn(...)>` closure | `ModuleRegistry` uses `RefCell<HashMap<String, Module>>` for interior mutability via `Arc`. Modules registered with `registry.register(Module::new(...))`. |
| Glass Effect | Colored semi-transparent `div` + shadow first; GPU texture blur via GPUI shader later | Deferred. DWM/kwin behind-window blur detected at runtime. |
| Dialog Boxes | `tinyfiledialogs = "3.9"` | Cross-platform native file open/save dialogs (tiny C lib binding). |
| Window Icon | `WindowOptions.icon: Option<Arc<image::RgbaImage>>` | Loaded from `brandkit/app-icons/icon-256.png` via `image` crate. X11 only. |
| Text Input | `on_key_down` handler on focused `Div` | No IME support in Phase 2. IME via `EntityInputHandler` deferred. |
| Living Doc | `ELEMENT_STATE.md` | This file — mandatory, always up to date. |

---

## 3. Architecture

```
src/
├── main.rs         # gpui_platform::application() entry point
├── app.rs          # ElementApp shell: holds editor + overlay + config
├── config.rs       # Theme colors, clipboard config, debounce, serde persistence
├── database.rs     # SQLite init, clipboard_entries + notes tables, CRUD
├── clipboard.rs    # Background thread polling arboard every 500ms, stores to DB
├── module.rs       # Module struct + Registry + register_builtin_modules()
├── overlay.rs      # Floating command palette, keyboard input, results list
├── editor/
│   ├── mod.rs      # Re-exports EditorView
│   ├── buffer.rs   # TextBuffer: text, cursor, open/save, dirty tracking
│   ├── view.rs     # EditorView: GPUI Render, FocusHandle, key input, status bar
│   └── find.rs     # FindState: find-next/find-prev, match highlighting
└── brandkit/       # Brand assets: icons, favicon, wordmark, social
```

### Key GPUI API patterns discovered

- Entry point: `gpui_platform::application()` (not `Application::new()`)
- View handle: `Entity<V>` (not `View<V>`)
- Render: `fn render(&mut self, &mut Window, &mut Context<Self>) -> impl IntoElement`
- Prelude: must explicitly `use gpui::prelude::*` (for `FluentBuilder`, `StatefulInteractiveElement`, etc.)
- No `title` on `WindowOptions` — title set by titlebar (macOS) or native window manager
- `overflow_y_scroll()` requires `.id()` first (needs `Stateful<Div>`)
- `on_key_down` + `cx.listener()` for keyboard input on focused elements

### Module struct

```rust
pub type ActivateFn = Arc<dyn Fn(
    Entity<EditorView>, Entity<OverlayView>,
    &mut Window, &mut App,
)>;

pub struct Module {
    pub command: String,
    pub name: String,
    pub description: String,
    pub activate: ActivateFn,
}
```

### Data flow

```
Global hotkey pressed (Alt+Space)
  → overlay.rs toggles visibility, focuses overlay
  → User types "/notes" → overlay.rs queries ModuleRegistry
  → overlay.rs calls ModuleRegistry::activate()
  → Module closure receives (editor, overlay_handle, window, cx)
  → closure: editor.update(cx, |ed, _cx| ed.buffer = TextBuffer::from_string(...))
  → overlay hides, focus returns to editor
```

---

## 4. Work State

### Completed

- **Phase 1 — GPUI scaffold**: Cargo.toml rewritten, all source files created, compiles and runs.
- **Phase 2 — Text editing in GPUI**:
  - Full text editing in GPUI with keyboard input, cursor, status bar, file dialogs, find bar display, window icon.
  - Committed as `c3a0fb6`.
- **Phase 2c — Interactive find bar + brand + /notes module**:
  - Interactive find bar, brand colors, match highlighting.
  - Module system rewritten to struct-based with closure activation.
  - `/notes` and `/help` modules.
  - Committed as `70b7336`.
- **Phase 3 — SQLite database, clipboard history, calculator, emoji search** (this session):
  - **`database.rs`**: SQLite via `rusqlite` (bundled). Two tables: `clipboard_entries` (content_type, text_content, created_at) and `notes` (title, content, updated_at). Separate `Database::init()` per thread (each gets its own connection).
  - **`clipboard.rs`**: `std::thread::spawn` background polling of `arboard::Clipboard` every 500ms. Detects changes, stores text entries in SQLite, sends copy event through `std::sync::mpsc::Sender` to the GPUI main loop.
  - **Config upgrade**: `ThemeColors` (primary/ink/core/muted/surface/border as hex strings), `ClipboardConfig` (enabled, poll_interval_ms, max_entries), `debounce_delay_ms`, `window_width`, `window_height`. Hex color helpers: `primary()`, `ink()`, `core()`, `muted()`, `surface()`, `border()` — each returns `u32` for `rgb()`.
  - **`/calc` module**: Uses `evalexpr::eval()` to evaluate math expressions in the editor text. Replaces `x` with `*` and `÷` with `/`. Result appended as `= value`.
  - **`/cbhist` module**: Loads last 30 clipboard entries from SQLite, displays as numbered list in editor.
  - **`/emoji` module**: Iterates `emojis::iter()`, matches by name or shortcode. Displays emoji + shortcode in editor (up to 2000 chars).
  - **`/savenote` module**: Saves current editor content as a persistent note in SQLite. Uses first line as title.
  - **`/noteslist` module**: Shows all saved notes from SQLite.
  - **`register_builtin_modules()`**: New function in `module.rs` that registers all 7 built-in modules. Takes `(registry, editor_handle, overlay_handle, db: Arc<Database>)`.
  - **Dependencies added**: `rusqlite`, `arboard`, `evalexpr`, `emojis`, `crossbeam-channel`.
  - Inspired by `RustCast` (github.com/MystikoLab/rustcast) — an Iced-based Raycast alternative for macOS.
  - Default: Alt+Space (configurable via `ElementConfig.hotkey` string, supports Windows key via `super` prefix).
  - Implemented via `global-hotkey` crate v0.5 (tauri-apps).
  - Registered on startup via `GlobalHotKeyManager::register()`.
  - Polled every 50ms via `window.spawn()` + `AsyncWindowContext.background_executor().timer()`.
  - Hotkey event toggles `OverlayView.visible` via `Entity::update()`.
  - Config hotkey field accepts platform key names: `Alt+Space`, `Super+Space`, `Ctrl+Shift+F`, etc.

  - **Interactive find bar**: When `show_find` is true, all keyboard input routes to find query. Printable chars append to query, backspace removes, Escape closes, Enter/F3 find-next, Shift+F3 find-prev. Match count displayed as `N / M`. ◀ ▶ navigation buttons. Find input has `#6D4AFA` border when active.
  - **Match highlighting**: `FindState.matches` holds all `FindMatch { start, end }` positions. `split_highlighted()` splits each line into segments and renders matches with `rgb(0x6D4AFA)` background.
  - **Brand colors throughout**: `#6D4AFA` primary on find bar "FIND" label, line numbers, find bar border, selected overlay item, "ELEMENT" in status bar. `#08060D` ink on editor background, find input background. `#FDFAFE` core white on text, cursor. Semi-transparent overlay background.
  - **Module system rewritten**: `Module` is now a struct (not trait) with `command`, `name`, `description`, and `activate: Arc<dyn Fn(...)>` closure. `ModuleRegistry` uses `RefCell<HashMap<String, Module>>` for interior mutability. `register()` takes `&self` instead of `&mut self`, enabling registration via `Arc<ModuleRegistry>`.
  - **Overlay keyboard input**: When overlay opens, `Render::render()` focuses it via `window.focus(&self.focus_handle, cx)`. `track_focus` + `on_key_down` handler routes Escape/Enter/Backspace/Up/Down/printable chars. Results are clickable via `on_mouse_up` with `cx.listener`.
  - **`/notes` module**: Creates a new `TextBuffer` with `"# New Note\n\n"` pre-filled and sets it on the editor. Registered in `main.rs` via `module_registry.register(Module::new("/notes", ...))`.
  - **`/help` module**: Shows a command reference in the editor buffer.
  - **Overlay handles store**: `OverlayView` stores `Entity<EditorView>` and `Entity<OverlayView>` for module activation and focus routing. Set via `set_handles()` after construction.
  - **Module activation flow**: Overlay → `activate_selected()` → `ModuleRegistry::activate()` → module closure receives `(Entity<EditorView>, Entity<OverlayView>, &mut Window, &mut Context<OverlayView>)` → closure uses `editor.update(cx, |ed, _| ...)` to mutate editor state.

### Active

- **No active development — waiting for next task.**

### Blocked

- Nothing currently blocked.

---

## 5. Next Moves (in order)

1. **File search module (`/files`)**: Index files in `search_dirs` using SQLite FTS5, search by name.
2. **Web search**: Type `?query` to open browser search (like RustCast's Google search).
3. **Spotify/Media control module**: Cross-platform media control via DBus (Linux) or `windows-rs`.
4. **Mind map view**: Canvas-based mind map module.
5. **Settings panel**: In-app settings UI for hotkey, theme, clipboard config.
6. **System tray icon**: Minimize to tray with `tray-icon` crate.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `gpui-unofficial` API differs from docs | Pin exact version; reference Zed source at tag `v1.11.3` as ground truth |
| No `EditableText` widget in GPUI | Built interactive find bar via `on_key_down` routing with `show_find` flag |
| Global hotkey registration fails | Log warning; accept remapping via `config.rs` |
| Window blur unsupported | Opaque background; runtime detection, graceful degrade |
| SQLite connection not `Send` | Clipboard thread creates its own `Database::init()` connection |
| `arboard` may fail on Wayland | Graceful fallback — clipboard monitoring disabled if init fails |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **GPUI source:** `https://github.com/vaibhxvvy/element` (Zed mirror at tag v1.11.3)
- **Reference project:** `https://github.com/MystikoLab/rustcast` — Iced-based Raycast alternative for macOS (971★). Borrowed patterns: SQLite persistence, clipboard polling, calculator, config-driven themes, platform abstraction.
- **Global hotkey crate:** `https://crates.io/crates/global-hotkey`
- **Brandkit:** `brandkit/README.md`
- **Phase 0 code:** `src/main.rs` (pre-rewrite, 442 lines, egui/eframe)
