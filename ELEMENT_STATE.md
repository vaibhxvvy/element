# Element — Living State Document

> **Last updated:** 2026-07-26
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
| Brand | Element, `#6D4AFA` primary / `#08060D` ink / `#FDFAFE` core white | Full brandkit at `brandkit/`. Window title: "Element". |
| Persistence | `serde` + `serde_json` → `~/.element/config.json` | Settings and module state. |
| Migration Strategy | **In-place rewrite** | Replace egui/eframe code directly with GPUI. No parallel binary targets. |
| Module System | `trait Module` + `HashMap<&str, Box<dyn Module>>` registry | No hardcoded if/else chains. Every module registers itself. |
| Glass Effect | Colored semi-transparent `div` + shadow first; GPU texture blur via GPUI shader later | Deferred to Phase 2. DWM/kwin behind-window blur detected at runtime. |
| Living Doc | `ELEMENT_STATE.md` | This file — mandatory, always up to date. |

---

## 3. Architecture

```
src/
├── main.rs         # gpui-unofficial entry point
├── app.rs          # App state, module registry, overlay ↔ editor coordination
├── module.rs       # Module trait + HashMap registry
├── overlay.rs      # Floating command palette (global hotkey → popup)
├── config.rs       # Theme (Dark/Light), settings, serde persistence
└── editor/
    ├── mod.rs      # Editor module — re-exports
    ├── buffer.rs   # Text buffer: open, save, cursor, selection, undo
    ├── view.rs     # GPUI Render impl for the editor
    └── find.rs     # Find/replace logic, match iteration
```

### Module trait (pseudocode)

```
trait Module {
    fn command(&self) -> &str;       // e.g. "/notes"
    fn name(&self) -> &str;          // e.g. "Notes"
    fn activate(&mut self, cx: &mut WindowContext);
}
```

### Data flow

```
Global hotkey pressed
  → overlay.rs toggles window visibility
  → User types search or "/" command
  → overlay.rs queries ModuleRegistry
  → If slash command match: Module::activate()
  → Module sets active view in ElementApp
  → If text search: triggers SQLite search (Phase 3+)
```

---

## 4. Work State

### Completed

- Phase 0 text editor: egui/eframe GUI, open/save/save-as, find bar, word wrap, dark/light mode, custom window icon, standard shortcuts, unsaved-changes protection.
- Git repo initialized, committed, pushed to `https://github.com/vaibhxvvy/element`.
- README positioned as "Rust-cast-style launcher · Notepad · Mind map · Local search".
- Cargo.toml description updated.
- Brandkit complete: icons, wordmarks, social previews, Windows tiles, Linux hicolor, favicon set, color palette spec.
- Key input double-fire bug fixed (filtering `KeyEventKind` in egui).
- GPUI research complete: entity/model/view architecture understood.
- Decision: `gpui-unofficial = "1.11.3"`, `global-hotkey = "0.5"`, in-place rewrite.
- This document created.

### Active

- **Plan approved.** Execution beginning: Cargo.toml → module.rs → config.rs → editor/ → overlay.rs → app.rs → main.rs.

### Blocked

- Nothing currently blocked.

---

## 5. Next Moves (in order)

1. Update `Cargo.toml`: replace `eframe` + `rfd` with `gpui-unofficial` + `global-hotkey` + `serde` + `serde_json`.
2. Create `module.rs`: `Module` trait + `ModuleRegistry` (HashMap-based).
3. Create `config.rs`: `ElementConfig`, `Theme` enum, load/save from `~/.element/config.json`.
4. Create `editor/buffer.rs`: extract `TextBuffer` from current main.rs egui code.
5. Create `editor/find.rs`: find/replace logic, match iteration (unchanged from main.rs).
6. Create `editor/view.rs`: GPUI `View<EditorView>`, `impl Render`, keyboard via `EntityInputHandler`.
7. Create `editor/mod.rs`: re-export buffer, view, find.
8. Create `overlay.rs`: global hotkey listener, floating popup, search input, `UniformList` results, slash-command detection.
9. Create `app.rs`: `ElementApp` holding registry, active view, config; coordinate overlay ↔ editor.
10. Rewrite `main.rs`: `Application::new().run(...)` — init hotkey, open window, attach overlay.
11. Compile, test, commit, push.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `gpui-unofficial` API differs from docs | Pin exact version; reference Zed source at tag `v1.11.3` as ground truth |
| No TextEdit widget in GPUI | Build editor via `EntityInputHandler` + `ShapedLine`; study `zed/crates/editor` |
| Global hotkey registration fails | Log warning; accept remapping via `config.rs` |
| Window blur unsupported | Opaque background; runtime detection, graceful degrade |

---

## 7. Reference

- **Repository:** `https://github.com/vaibhxvvy/element`
- **GPUI source:** `https://github.com/vaibhxvvy/element` (Zed mirror at tag v1.11.3)
- **Global hotkey crate:** `https://crates.io/crates/global-hotkey`
- **Brandkit:** `brandkit/README.md`
- **Phase 0 code:** `src/main.rs` (pre-rewrite, 442 lines, egui/eframe)
