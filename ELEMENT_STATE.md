# Element — Living State Document

> **Last updated:** 2026-07-26 (Phase 2 complete)
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
| Global Hotkey | `global-hotkey = "0.5"` (tauri-apps) | Mature crate for Windows/macOS/Linux X11. Wayland support deferred until needed. |
| Brand | Element, `#6D4AFA` primary / `#08060D` ink / `#FDFAFE` core white | Full brandkit at `brandkit/`. |
| Persistence | `serde` + `serde_json` → `~/.element/config.json` | Settings and module state. |
| Migration Strategy | **In-place rewrite** | Replace egui/eframe code directly with GPUI. No parallel binary targets. |
| Module System | `trait Module` + `HashMap<&str, Box<dyn Module>>` registry | No hardcoded if/else chains. Every module registers itself. |
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

### Module trait

```rust
pub trait Module {
    fn command(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn activate(&mut self, cx: &mut WindowContext);
}
```

### Data flow

```
Global hotkey pressed (Phase 3)
  → overlay.rs toggles visibility
  → User types search or "/" command
  → overlay.rs queries ModuleRegistry
  → If slash command match: Module::activate()
  → Module sets active view in ElementApp
  → If text search: triggers SQLite search (Phase 4+)
```

---

## 4. Work State

### Completed

- **Phase 1 — GPUI scaffold**: Cargo.toml rewritten, all source files created, compiles and runs.
- **Phase 2 — Text editing in GPUI** (this session):
  - Keyboard input via `on_key_down` handler with `cx.listener` — all printable chars, arrow keys, backspace/delete, home/end, enter, tab.
  - Cursor rendering with visual blink bar (white when focused, purple when unfocused).
  - Status bar: `Ln X, Col Y (modified)` + file name.
  - File operations: Ctrl+N (new), Ctrl+O (open via native dialog), Ctrl+S (save), Ctrl+Shift+S (save-as), Ctrl+D (insert date/time), Ctrl+F (toggle find bar), F3 (find next), Ctrl+Q (close with unsaved-changes guard).
  - Window icon loaded from `brandkit/app-icons/icon-256.png` via `image` crate.
  - Find bar visual (display-only, interactive input deferred).
  - Editor focused on startup via `Window::focus()`, `Focusable` trait imported.

### Active

- Polish for Phase 2 commit: suppress expected dead-code warnings, update this document.

### Blocked

- Nothing currently blocked.

---

## 5. Next Moves (in order)

1. **Global hotkey overlay**: Wire `global-hotkey` crate for Ctrl+Space → toggle `OverlayView` visibility over the editor.
2. **Interactive find bar**: Route key events to find query when find bar is visible; add Escape to close.
3. **Module stubs**: Implement 2-3 built-in modules (`/help`, `/time`, `/calc`) to demonstrate the trait registry.
4. **Overlay query input**: Make the overlay accept typed input, query `ModuleRegistry`, display results.
5. **Phase 3 — SQLite index**: Add `rusqlite` for local search index of notes/files.
6. **Phase 4 — Mind map view**: Add canvas-based mind map module.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `gpui-unofficial` API differs from docs | Pin exact version; reference Zed source at tag `v1.11.3` as ground truth |
| No `EditableText` widget in GPUI for find bar | Build via `on_key_down` routing; study `zed/crates/editor` |
| Global hotkey registration fails | Log warning; accept remapping via `config.rs` |
| Window blur unsupported | Opaque background; runtime detection, graceful degrade |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **GPUI source:** `https://github.com/vaibhxvvy/element` (Zed mirror at tag v1.11.3)
- **Global hotkey crate:** `https://crates.io/crates/global-hotkey`
- **Brandkit:** `brandkit/README.md`
- **Phase 0 code:** `src/main.rs` (pre-rewrite, 442 lines, egui/eframe)
