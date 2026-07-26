<p align="center">
  <img src="logo.png" alt="Element" width="96" height="96">
</p>

<h1 align="center">Element</h1>

<p align="center">
  <strong>Rust-cast-style launcher · Notepad · Mind map · Local search</strong><br>
  <em>for Windows and Linux — built natively in Rust.</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.77+-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/platform-windows%20%7C%20linux-lightgrey" alt="Platform">
</p>

---

Element is a single, fast Rust binary that replaces the scattered tools you reach for every day. It starts as a native text editor — and grows into a local command palette, a note graph, and a search engine for your own files.

Think Raycast or Alfred, but native to Windows and Linux, with a built-in second-brain layer.

## Currently shipped (Phase 0)

The foundation is a native GUI text editor — fast, distraction-free, and ready for real work.

- **Native GUI** — Built with [egui](https://egui.rs) and [eframe](https://docs.rs/eframe). Looks and feels like a first-class citizen on every platform.
- **Fast startup** — Launches instantly. No splash screens, no waiting.
- **Word wrap** — Toggle on/off from the View menu.
- **Find & Search** — Quick find bar with match counting and next-match navigation.
- **File operations** — Open, Save, Save As with native file dialogs.
- **Unsaved changes protection** — Never lose your work; prompted to save before closing.
- **Dark & Light modes** — Respects your system theme automatically.
- **Custom icon** — Ships with a full brand kit, from window icon to Linux hicolor theme and Windows tile assets.
- **Keyboard shortcuts** — Every action has a shortcut (`Ctrl+S`, `Ctrl+O`, `Ctrl+F`, etc.).

## Roadmap

| Phase | Feature | Status |
|-------|---------|--------|
| 0 | Text editor — native, distraction-free buffer | ✅ Shipped |
| 1 | **Markdown renderer** — preview `.md` files with live rendering, not just plain-text editing | 🚧 Planned |
| 2 | **Mind map** — node-graph view for linking notes and ideas visually, backed by the same file data | 🔮 Planned |
| 3 | **Local search index (SQLite)** — background-indexed database of files, notes, and mind-map nodes, instantly searchable | 🔮 Planned |
| 4 | **Command palette shell** — global-hotkey overlay (Raycast-style) that sits on top of everything: type to search, run actions, jump into any view | 🔮 Planned |

Every future feature is designed to be triggerable from the command palette and indexed by the local search layer. Nothing is a dead-end standalone screen.

## Installation

```bash
git clone https://github.com/vaibhxvvy/element.git
cd element
cargo run -- path/to/file.txt
```

To build a release binary:

```bash
cargo build --release
./target/release/element
```

### Requirements

- Rust 1.77 or newer
- A GPU that supports WebGPU (modern integrated or discrete GPU)

## Usage

```bash
element                  # Start with a blank buffer
element notes.txt        # Open an existing file
cargo run --release      # Run the release build
```

### Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+N` | New file |
| `Ctrl+O` | Open file |
| `Ctrl+S` | Save |
| `Ctrl+Shift+S` | Save As |
| `Ctrl+F` | Toggle find bar |
| `F3` | Find next match |
| `Ctrl+D` | Insert current date & time |
| `Ctrl+Q` | Quit |
| `Ctrl+A` | Select all |

## Philosophy

Element is built on three principles:

1. **Speed over features.** It should open instantly and respond instantly. Every millisecond of startup and every frame of latency matters.
2. **Simplicity over configuration.** No plugins, no language servers, no settings files. What you see is what you get.
3. **Quality over quantity.** A small set of well-crafted features beats a thousand half-baked ones.

## Brand

Element has a full brand identity. See [`brandkit/`](brandkit/README.md) for:
- Color palette (`#6D4AFA` primary, `#08060D` ink, `#FDFAFE` core white)
- App icons, wordmarks, and social previews
- Windows tile assets and Linux hicolor icon themes
- Favicon set for the docs site

## License

MIT
