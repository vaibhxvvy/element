# Element — Living State Document

> **Last updated:** 2026-07-26 (Phase 2c — interactive find, brand, modules)
> **Purpose:** Single source of truth for architecture decisions, project state, and next moves. Must be kept accurate after every session.

---

## 1. Objective

Build **Element**: a Raycast-style global launcher + local knowledge tool for Windows and Linux, written in Rust with GPUI for GPU-accelerated UI. The text editor is Phase 0 — a foundation module, not the product.

Core interaction model: global hotkey → floating frosted-glass overlay → search-as-you-type across files/notes/nodes/apps + slash commands route to built-in modules (`/notes`, `/timer`, `/mindmap`, `/ask`) via a trait-based registry.

---

## 2. Tech Stack & Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI Framework | **GPUI** (via `gpui-unofficial = "1.11.3"`) | Zed's GPU-accelerated framework. `gpui-unofficial` is a community mirror that tracks Zed releases (currently v1.11.3) and supports Windows + Linux + macOS. Official `gpui` v0.2.2 lacks Windows support. |
| Windowing | Built into `gpui-unofficial` | Platform backends included: Windows (Win32 + DirectWrite), Linux (x11 + wayland features), macOS (font-kit). |
| Global Hotkey | `global-hotkey = "0.5"` (tauri-apps) | Default `Alt+Space`, configurable via `ElementConfig.hotkey`. Parsed from string: `Alt+Space`, `Super+Space` (Windows key), `Ctrl+Shift+F`, etc. Polled via `AsyncWindowContext.background_executor().timer()`. Wayland support deferred. |
| Brand | Element, `#6D4AFA` primary / `#08060D` ink / `#FDFAFE` core white | Full brandkit at `brandkit/`. |
| Persistence | `serde` + `serde_json` → `~/.element/config.json` | Settings and module state. |
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
├── module.rs       # Module trait + HashMap<&'static str, Box<dyn Module>>
├── overlay.rs      # Floating command palette (placeholder, not yet wired)
├── config.rs       # Theme (Dark/Light), settings, serde persistence
└── editor/
    ├── mod.rs      # Re-exports EditorView
    ├── buffer.rs   # TextBuffer: text, cursor, open/save, dirty tracking
    ├── view.rs     # EditorView: GPUI Render, FocusHandle, key input, status bar
    └── find.rs     # FindState: find-next, match counting, cursor positioning
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
- **Phase 2c — Interactive find bar + brand + /notes module** (this session):
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

1. **Phase 3 — SQLite index**: Add `rusqlite` for local search index of notes/files.
2. **Phase 4 — Mind map view**: Add canvas-based mind map module.
3. **Module stubs**: Implement `/timer`, `/calc`, `/ask` modules.
4. **Save dialog**: Wire Save/Discard/Cancel buttons with keyboard shortcuts.
5. **Match navigation**: Auto-scroll editor to show the current find match.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `gpui-unofficial` API differs from docs | Pin exact version; reference Zed source at tag `v1.11.3` as ground truth |
| No `EditableText` widget in GPUI | Built interactive find bar via `on_key_down` routing with `show_find` flag |
| Global hotkey registration fails | Log warning; accept remapping via `config.rs` |
| Window blur unsupported | Opaque background; runtime detection, graceful degrade |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **GPUI source:** `https://github.com/vaibhxvvy/element` (Zed mirror at tag v1.11.3)
- **Global hotkey crate:** `https://crates.io/crates/global-hotkey`
- **Brandkit:** `brandkit/README.md`
- **Phase 0 code:** `src/main.rs` (pre-rewrite, 442 lines, egui/eframe)
