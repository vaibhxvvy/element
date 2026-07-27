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

- **Global hotkey** — Toggle the overlay with `Alt+Space` (configurable). Uses a low-level keyboard hook (`WH_KEYBOARD_LL`) for reliable capture across all apps.
- **App launcher** — Searches installed applications from the Start Menu (`*.lnk` files in `%ProgramData%` and `%APPDATA%`).
- **Web search** — Anything that doesn't match a local action opens your default browser with the configured search URL.
- **Calculator** — Type a math expression (`2+2`, `(3*4)/2`) and press Enter to copy the result to your clipboard.
- **Emoji search** — Type `emoji` or `:` followed by a term to find and copy emojis.
- **Clipboard history** — Type `cbhist` or `clip` to browse recent clipboard entries.
- **Always-on-top overlay** — Acrylic-blur backdrop, centered, no decorations.

## Architecture

```
element/
├── src/
│   ├── main.rs       # Slint entry point, hotkey hook, timer polling
│   ├── app.rs        # SearchEngine — app scan, calc, emoji, clipboard, web search
│   ├── config.rs     # JSON config (hotkey, search URL, window size)
│   └── database.rs   # SQLite-backed clipboard history
├── ui/
│   └── main.slint    # Slint UI — search bar + results list
├── build.rs          # slint-build compilation
└── Cargo.toml
```

### Search system

Every query runs through `SearchEngine::search()` which checks, in order:

1. **Calculator** — if the query looks like a math expression, evaluate and show the result.
2. **Emoji** — if the query starts with `emoji` or `:`, search the emoji database.
3. **Clipboard** — if the query is `cbhist` or starts with `clip`, load recent clipboard entries.
4. **Apps** — fuzzy-match installed application names.
5. **Web search** — always present as a fallback.

### Hotkey system

Uses `SetWindowsHookExW` with `WH_KEYBOARD_LL` (low-level keyboard hook) to capture the configured hotkey combination. A flag is set on capture and polled by a Slint `Timer` every 200ms to toggle the window.

## Installation

```bash
git clone https://github.com/vaibhxvvy/element.git
cd element
cargo run --release
```

The binary runs silently in the background. Press your hotkey to summon the overlay.

### Requirements

- Rust 1.77 or newer
- Windows

## Configuration

Config is stored at `%USERPROFILE%\.element\config.json` and auto-created on first run:

```json
{
  "hotkey": "Alt+Space",
  "window_width": 580,
  "window_height": 48,
  "debounce_delay_ms": 150,
  "search_url": "https://duckduckgo.com/?q=%s",
  "search_dirs": []
}
```

| Key | Description |
|-----|-------------|
| `hotkey` | Key combination — modifiers joined by `+`. Supported: `Alt`, `Ctrl`/`Control`, `Shift`, `Space`, etc. |
| `window_width` | Width of the search bar in pixels |
| `window_height` | Height of the search bar in pixels |
| `debounce_delay_ms` | Delay before search triggers after typing stops |
| `search_url` | URL template for web searches (`%s` is replaced with the query) |
| `search_dirs` | Additional directories to scan for applications |

## Usage

| Action | Input |
|--------|-------|
| Toggle overlay | `Alt+Space` (or configured hotkey) |
| Select result | Click or arrow keys + Enter |
| Close overlay | `Escape` |

## Build from source

```bash
cargo build --release
./target/release/element
```

## License

MIT
