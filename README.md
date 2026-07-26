<p align="center">
  <img src="logo.png" alt="Element" width="96" height="96">
</p>

<h1 align="center">Element</h1>

<p align="center">
  <strong>A native text editor for the modern terminal — fast, focused, and built in Rust.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.97+-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/platform-windows%20%7C%20macos%20%7C%20linux-lightgrey" alt="Platform">
</p>

---

**Element** is a lightweight, native GUI text editor written in Rust. It's designed for people who want a fast, distraction-free editing experience without the overhead of Electron or the complexity of terminal-based editors.

## Features

- **Native GUI** — Built with [egui](https://egui.rs) and [eframe](https://docs.rs/eframe), Element looks and feels like a first-class citizen on every platform.
- **Fast startup** — Launches instantly, no splash screens, no waiting.
- **Word wrap** — Toggle on/off from the View menu.
- **Find & Search** — Quick find bar with match counting and next-match navigation.
- **File operations** — Open, Save, Save As with native file dialogs.
- **Unsaved changes protection** — Never lose your work; prompted to save before closing.
- **Dark & Light modes** — Respects your system theme automatically.
- **Custom icon** — Ships with a brand identity, right down to the window icon.
- **Keyboard shortcuts** — Every action has a shortcut (Ctrl+S, Ctrl+O, Ctrl+F, etc.).

## Installation

### From source

```bash
git clone https://github.com/vaibhav/element.git
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
element                       # Start with a blank buffer
element notes.txt             # Open an existing file
element --release notes.txt   # Run the release build
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

## Tech stack

| Layer | Technology |
|-------|------------|
| Language | Rust |
| GUI framework | [egui](https://egui.rs) / [eframe](https://docs.rs/eframe) |
| File dialogs | [rfd](https://docs.rs/rfd) (native) |
| Rendering | WebGPU / OpenGL |
| Binary size | ~15 MB (release) |

## License

MIT
