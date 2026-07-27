<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brandkit/wordmark/wordmark-horizontal-light-purple.png">
    <source media="(prefers-color-scheme: light)" srcset="brandkit/wordmark/wordmark-horizontal-dark.png">
    <img src="brandkit/wordmark/wordmark-horizontal-dark.png" alt="Element" width="400">
  </picture>
</p>

<p align="center">
  <strong>Floating search bar · Unified launcher · Global hotkey</strong><br>
  <em>for Windows — built natively in Rust with Iced.</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.77+-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/platform-windows-lightgrey" alt="Platform">
</p>

---

Element is a global hotkey-activated floating search bar for Windows. Press **Alt+Space** to summon a search bar anywhere, type your query, and press Enter. It runs as a background daemon — zero UI until you need it.

## Features

- **Global hotkey** — Toggle the overlay with `Alt+Space` (configurable). Background thread polling (`GetAsyncKeyState`) for reliable capture. **Escape** closes the window.
- **App launcher** — Searches installed applications from the Start Menu with proper **app icons** extracted from `.lnk` shortcuts and rendered natively.
- **Scored fuzzy matching** — Word boundary, camelCase, consecutive, and early-match bonuses. Type `pwsh` → matches "PowerShell", `wn` → "Windows Notepad", `vsc` → "Visual Studio Code".
- **Frecency ranking** — Frequently and recently launched apps rise to the top. Uses a SQLite frecency table (`count / days since last use`).
- **App recommendations** — Show recently/frequently used apps when the search bar opens.
- **Auto-focus** — Search bar is focused immediately on `Alt+Space`, ready to type.
- **Adaptive window height** — Window resizes automatically to fit search results (up to 10 rows, capped at 500px).
- **Web search** — Fallback that opens your browser with the configured search URL.
- **Calculator** — Type `2+2`, `(3*4)/2`, press Enter to copy result to clipboard.
- **Emoji search** — Type `emoji` or `:` followed by a search term.
- **Clipboard history** — Type `cbhist` or `clip` to browse recent clipboard entries.
- **Always-on-top overlay** — Borderless, centered, light theme.

## Architecture

```
element/
├── src/
│   ├── main.rs        # Entry point: hotkey thread (GetAsyncKeyState), Iced bootstrap
│   ├── app.rs         # SearchEngine: app scan, fuzzy scorer, frecency, icon extraction
│   ├── config.rs      # TOML config with JSON migration
│   ├── database.rs    # SQLite — clipboard_entries + frecency tables
│   └── ui/
│       └── mod.rs     # Iced views: search bar, result rows with icons, styles
├── brandkit/          # Brand assets
├── Cargo.toml
└── README.md
```

### Search system

Every query runs through `SearchEngine::search()` which checks, in order:

1. **Calculator** — math expression detection and evaluation
2. **Emoji** — `emoji` or `:` prefix triggers emoji database search
3. **Clipboard** — `cbhist` or `clip` prefix loads recent clipboard entries
4. **Apps** — scored fuzzy-match installed app names with frecency boost and extracted icons
5. **Web search** — always present as a fallback

### Fuzzy match scoring

```
fuzzy_score(query, app_name) → Option<f64>
```

Each character of the query must appear in order in the app name. The score rewards:
- **Consecutive matches** — adjacent chars get +15 bonus
- **Word boundaries** — match after space, `-`, `_`, `/`, `\` gets +30
- **CamelCase boundaries** — match at uppercase after lowercase gets +20
- **Early matches** — match near the start gets up to +50
- **Gap penalty** — unmatched characters in the name reduce score by -2 each

Final score is multiplied by a frecency boost: `1.0 + (frecency_score × 5.0)` capped at 3×.

### Hotkey system

Background thread polls `GetAsyncKeyState` for the configured hotkey every 20ms. When detected, it directly calls `ShowWindow` / `SetWindowPos` on the Iced window via `FindWindowW`. Atomic flags communicate between the thread and Iced's `update` function.

### Auto-focus

When the window appears, `text_input::focus("search")` is returned from the update function, immediately focusing the search bar so the user can start typing.

### Adaptive height

The window starts at 580×56px. On each keystroke, `adaptive_height()` calculates the needed height (`52 + min(results, 10) × 42 + 8`), stores it in an atomic, and signals the hotkey thread to call `SetWindowPos`.

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
| Navigate results | Arrow Up / Arrow Down |
| Close overlay | `Escape` |

## Build from source

```bash
cargo build --release
./target/release/element
```

## License

MIT
