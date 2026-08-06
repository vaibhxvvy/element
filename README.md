<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brandkit/wordmark/wordmark-horizontal-light.png">
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

Element is a global hotkey-activated floating search bar for Windows. Press **Alt+Space** to summon a search bar anywhere, type your query, and press Enter. It sits in the system tray — zero UI until you need it.

## Features

- **Global hotkey** — Toggle the overlay with `Alt+Space`. Uses `RegisterHotKey` + `PeekMessageW` — zero CPU when idle. **Escape** closes the window.
- **System tray** — Icon in the notification area. Left-click toggles overlay; right-click shows Exit menu.
- **App launcher** — Resolves Start Menu shortcuts to their target `.exe` and launches that executable directly. Prefers shortcut-provided `.ico` files, then falls back to the executable's embedded icon; decoded icons are cached as PNG.
- **Scored fuzzy matching** — Word boundary, camelCase, consecutive, and early-match bonuses. Type `pwsh` → matches "PowerShell", `wn` → "Windows Notepad", `vsc` → "Visual Studio Code".
- **Frecency ranking** — Frequently and recently launched apps rise to the top. Uses a SQLite frecency table (`count / days since last use`).
- **App recommendations** — Show recently/frequently used apps when the search bar opens.
- **Auto-focus** — Search bar is focused immediately on `Alt+Space`, ready to type.
- **Adaptive window height** — Window resizes automatically to fit search results (up to 10 rows, capped at 500px).
- **Web search** — Fallback that opens your browser with the configured search URL.
- **Calculator** — Type `2+2`, `(3*4)/2`, press Enter to copy result to clipboard.
- **Emoji search** — Type `emoji` or `:` followed by a search term.
- **Clipboard history** — Type `cbhist` or `clip` to browse recent clipboard entries from SQLite.
- **File & folder search** — Raycast-style: type `file <name>` or `folder <name>` to fuzzy-match files/folders in your curated user folders (Desktop, Documents, Downloads, ... — or `file_search_dirs` in config) and open them with their default handler.
- **Always-on-top overlay** — Borderless, centered, dark theme.

## Architecture

```
element/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml         # cargo fmt --check + clippy + test + build on PR
│   │   └── release.yml    # tag-triggered build + portable zip + GitHub Release
│   ├── ISSUE_TEMPLATE/    # bug_report.md, feature_request.md
│   └── PULL_REQUEST_TEMPLATE.md
├── src/
│   ├── main.rs        # Entry point: RegisterHotKey + PeekMessageW loop,
│   │                  #   hidden tray window, Iced bootstrap
│   ├── orchestrator.rs # Orchestrator: handle(Request) → Outcome, owns providers
│   ├── config.rs      # TOML config with JSON migration, shared data_dir()
│   ├── database.rs    # SQLite — clipboard_entries + frecency tables
│   ├── error.rs       # ElementError enum (thiserror)
│   ├── registry.rs    # ProviderRegistry: catch_unwind isolation per provider
│   ├── theme.rs       # Named color/spacing/radius tokens
│   ├── hotkey/        # Global hotkey detection (RegisterHotKey → LL hook → fallbacks)
│   ├── providers/
│   │   ├── mod.rs        # SearchProvider trait + SearchContext + SearchResult
│   │   ├── fuzzy.rs      # Shared fuzzy scorer (apps + files)
│   │   ├── icon.rs       # Shared icon pipeline for any shell path
│   │   ├── apps/         # Installed-app scan + fuzzy match + frecency + icons
│   │   ├── calculator/   # evalexpr expression evaluator
│   │   ├── emoji/        # emojis crate search
│   │   ├── clipboard/    # Clipboard history from SQLite
│   │   ├── files/        # File/folder search ("file"/"folder" prefixes)
│   │   └── websearch/    # Web search fallback (always at bottom)
│   └── ui/
│       └── mod.rs     # Iced views (uses theme.rs tokens)
├── brandkit/          # Brand assets
├── AGENTS.md          # Canonical reference for AI agents
├── codex.md           # Audit and implementation log for future agents
├── CHANGELOG.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── Cargo.toml
└── README.md
```

### Provider architecture

Every capability is a `SearchProvider`:

```rust
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;              // "apps", "calculator", etc.
    fn should_run(&self, query: &str) -> bool;  // cheap gate check
    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult>;
    fn activate(&self, ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError>;
    fn refresh(&self) {}                         // reload state when overlay opens
}
```

Search results are scored and sorted descending:

| Provider | Score |
|----------|-------|
| Calculator | 1000 (always on top) |
| Emoji | 500 - index (decaying, up to 20) |
| Apps | fuzzy_score × frecency_boost |
| Clipboard | 200 |
| Files | 120 + fuzzy × 2 (+10 folders); recs 210 (roots); bare queries 10 + fuzzy × 0.5, cap 6 |
| Web search | -1 (always last) |

The `ProviderRegistry` wraps each call in `catch_unwind` — a buggy provider never crashes the overlay.

### Search flow

```
query → ProviderRegistry
  ├─ Calculator  → should_run("2+2")?  yes → search
  ├─ Emoji       → should_run(":smile")? yes → search
  ├─ Clipboard   → should_run("cbhist")? yes → search
  ├─ Files       → should_run("file x")? yes → fuzzy match home index
  ├─ Apps        → should_run(_) = true → fuzzy match + frecency
  └─ Web search  → should_run(_) = true → construct URL
       ↓
  merge + sort by score → return Vec<SearchResult>
```

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

### Frecency formula

```
SQL: count / (julianday('now') - julianday(last_used) + 1)
App score = fuzzy_score × (1.0 + frecency × 5.0), capped at 3×
```

### SearchResult

```rust
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: String,         // metadata tag
    pub provider_id: String,  // must match provider.id() for dispatch
    pub action: String,       // exact provider-owned activation data
    pub icon_rgba: Option<(Vec<u8>, u32, u32)>,  // raw RGBA pixels
    pub score: f64,
}
```

### Hotkey system

Background thread registers `RegisterHotKey(NULL, 1, MOD_ALT|MOD_NOREPEAT, VK_SPACE)` and runs a `PeekMessageW` loop with 50ms sleep when idle. On `WM_HOTKEY`, it shows/centers the Iced window via `FindWindowW` / `ShowWindow` / `SetWindowPos`. Atomic flags communicate between the thread and Iced's `update` function — no CPU wasted on polling.

### Tray icon

A hidden message-only window (class `ElementTrayClass`) hosts the tray icon. Left-click toggles the overlay. Right-click pops up a menu with "Exit" to quit.

### Panic isolation

Each provider's `search()`, `activate()`, and `refresh()` runs inside `std::panic::catch_unwind` at the registry level. A buggy provider never crashes the overlay — its results are dropped and an error is logged.

### Shared data directory

All persistent data lives under `~/.element/`. Single source of truth in `config.rs::data_dir()`:
- `config.toml` — user configuration
- `element.db` — SQLite database (frecency + clipboard history)
- `cache/icons/*.png` — cached app icons

### Adaptive height

The window starts at `config.window_width × 56` px. On each keystroke, `adaptive_height()` calculates the needed height (`52 + min(results, 10) × 42 + 8`), stores it in an atomic, and signals the hotkey thread to call `SetWindowPos`.

### Icon pipeline

1. Resolve the `.lnk` with `IShellLink` to find the target executable and optional icon location (apps only)
2. Prefer the shortcut's `.ico` file; otherwise extract a 32×32 icon for any shell path (exe, file, folder) via `IShellItemImageFactory` — shared in `src/providers/icon.rs`
3. Cache decoded RGBA pixels at `cache/icons/v2-<source-hash>.png`

## Installation

### Winget (recommended)

```bash
winget install vaibhxvvy.Element
```

### Portable zip

Download `element-1.0.0-win64.zip` from the [latest release](https://github.com/vaibhxvvy/element/releases/latest), extract `element.exe` to any folder, and run it.

### Inno Setup installer

Download `element-1.0.0-setup.exe` from the [latest release](https://github.com/vaibhxvvy/element/releases/latest) and run it.

### Build from source

```bash
git clone https://github.com/vaibhxvvy/element.git
cd element
cargo run --release
```

### Requirements

- Rust 1.77 or newer
- Windows 10 1809+ or Windows 11

## Configuration

Config is stored at `%USERPROFILE%\.element\config.toml` and auto-created on first run (migrates from old `config.json` automatically):

```toml
hotkey = "Alt+Space"
window_width = 580.0
window_height = 420.0
debounce_delay_ms = 150
search_url = "https://duckduckgo.com/search?q=%s"
search_dirs = []
file_search_dirs = []
clipboard_max_entries = 100
```

| Key | Description |
|-----|-------------|
| `hotkey` | Reserved for configurable hotkeys; the current Windows registration is `Alt+Space` |
| `window_width` | Width of the search bar in pixels |
| `window_height` | Reserved; height adapts to the result list |
| `debounce_delay_ms` | Reserved; search currently updates immediately |
| `search_url` | URL template for web searches (`%s` = query) |
| `search_dirs` | Additional directories to scan recursively for Start Menu-style `.lnk` applications |
| `file_search_dirs` | Directories indexed for file/folder search; empty = curated user folders (Desktop, Documents, Downloads, Pictures, Music, Videos) |
| `clipboard_max_entries` | Maximum clipboard history entries shown |

## Usage

| Action | Input |
|--------|-------|
| Toggle overlay | `Alt+Space` |
| Select result | Click or Enter |
| Navigate results | Arrow Up / Arrow Down |
| Search files | Type `file <name>` (e.g. `file report`) |
| Search folders | Type `folder <name>` |
| Close overlay | `Escape` |
| Quit app | Right-click tray icon → Exit |

## Build from source

```bash
cargo build              # debug build
cargo build --release    # release build (slow with LTO)
cargo test               # run all 40 tests
cargo fmt                # format code
cargo clippy -- -D warnings  # lint (blocking on CI)
```

## Contributing

See `CONTRIBUTING.md` for guidelines. If you're an AI agent, read `AGENTS.md` first — it's the canonical reference covering architecture, provider system, design decisions, and common pitfalls.

CI runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo build` on every pull request.

## License

MIT
