<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brandkit/wordmark/wordmark-horizontal-light-purple.png">
    <source media="(prefers-color-scheme: light)" srcset="brandkit/wordmark/wordmark-horizontal-dark.png">
    <img src="brandkit/wordmark/wordmark-horizontal-dark.png" alt="Element" width="400">
  </picture>
</p>

<p align="center">
  <strong>Floating search bar · Unified launcher · Global hotkey</strong><br>
  <em>for Windows — built natively in Rust with Slint.</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.77+-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/platform-windows-lightgrey" alt="Platform">
</p>

---

Element is a global hotkey-activated floating search bar for Windows. Press **Alt+Space** to summon a search bar anywhere, type your query, and press Enter. It runs as a background daemon — zero UI until you need it.

## Features

- **Global hotkey** — Toggle the overlay with `Alt+Space` (configurable). Low-level keyboard hook (`WH_KEYBOARD_LL`) for reliable capture. **Escape** closes the window.
- **App launcher** — Searches installed applications from the Start Menu with proper **app icons** extracted from `.lnk` shortcuts.
- **Web search** — Fallback that opens your browser with the configured search URL.
- **Calculator** — Type `2+2`, `(3*4)/2`, press Enter to copy result to clipboard.
- **Emoji search** — Type `emoji` or `:` followed by a search term.
- **Clipboard history** — Type `cbhist` or `clip` to browse recent clipboard entries.
- **Debounced search** — Results appear after you stop typing (configurable delay).
- **Always-on-top overlay** — Acrylic-blur backdrop, centered, no decorations.

## Architecture

```
element/
├── src/
│   ├── main.rs       # Slint entry, hotkey hook, timer polling, Escape handling
│   ├── app.rs        # SearchEngine + Win32 icon extraction from .lnk files
│   ├── config.rs     # TOML config (hotkey, search URL, window size, debounce)
│   └── database.rs   # SQLite-backed clipboard history
├── ui/
│   └── main.slint    # Slint UI — search bar, results list with icons, Escape
├── build.rs          # slint-build compilation
└── Cargo.toml
```

### Search system

Every query runs through `SearchEngine::search()` which checks, in order:

1. **Calculator** — math expression detection and evaluation
2. **Emoji** — `emoji` or `:` prefix triggers emoji database search
3. **Clipboard** — `cbhist` or `clip` prefix loads recent clipboard entries
4. **Apps** — fuzzy-match installed app names with extracted icons
5. **Web search** — always present as a fallback

### Hotkey system

Uses `SetWindowsHookExW` with `WH_KEYBOARD_LL`. The hook proc checks for the configured hotkey combo and for Escape (when window is visible). Flags are polled by a Slint `Timer` every 200ms.

### Debounce

A Slint `Timer` in `SingleShot` mode is restarted on every keystroke. When the timer fires (after `debounce_delay_ms`), the search is performed. This avoids re-rendering on every keystroke.

## Installation

```bash
git clone https://github.com/vaibhxvvy/element.git
cd element
cargo run --release
```

### Requirements

- Rust 1.77 or newer
- Windows

## Configuration

Config is stored at `%USERPROFILE%\.element\config.toml` and auto-created on first run (migrates from old `config.json` automatically):

```toml
hotkey = "Alt+Space"
window_width = 580.0
window_height = 420.0
debounce_delay_ms = 150
search_url = "https://duckduckgo.com/search?q=%s"
search_dirs = []
clipboard_max_entries = 100
```

| Key | Description |
|-----|-------------|
| `hotkey` | Key combination — modifiers joined by `+`. Supported: `Alt`, `Ctrl`, `Shift`, `Space` |
| `window_width` | Width of the search bar in pixels |
| `window_height` | Height of the search bar in pixels |
| `debounce_delay_ms` | Delay before search triggers after you stop typing |
| `search_url` | URL template for web searches (`%s` = query) |
| `search_dirs` | Additional directories to scan for applications |
| `clipboard_max_entries` | Max clipboard entries to store in SQLite |

## Usage

| Action | Input |
|--------|-------|
| Toggle overlay | `Alt+Space` (or configured hotkey) |
| Select result | Click or Enter |
| Close overlay | `Escape` |

## Build from source

```bash
cargo build --release
./target/release/element
```

## License

MIT
