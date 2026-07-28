# Element — Agent Reference

This doc is the single source of truth for AI agents working on Element.
Read it first before making any changes.

## Project Overview

Element is a **global-hotkey launcher for Windows** — press `Alt+Space` for a floating
search bar. Type to fuzzy-find apps, calculate math, search emoji, browse clipboard
history, or search the web. Built in Rust with Iced 0.13 (wgpu).

Runs in the system tray. Zero UI until summoned.

## Codebase Layout

```
src/
├── main.rs           # Entry point. Spawns background thread, boots Iced.
│                     # Hotkey: RegisterHotKey(NULL, 1, MOD_ALT|MOD_NOREPEAT, VK_SPACE)
│                     # Tray: hidden message-only window, left-click toggle, right-click Exit
│                     # Atomics: HOTKEY_TRIGGERED, HIDE_REQUESTED, EXIT_REQUESTED,
│                     #          RESIZE_HEIGHT, RESIZE_REQUESTED, WINDOW_WIDTH
│
├── app.rs            # SearchEngine — thin registry wrapper.
│                     # Holds Arc<Config>, Arc<Database>, ProviderRegistry.
│                     # search() → creates SearchContext, delegates to registry.
│                     # activate() → matches result.provider_id to registered provider.
│
├── config.rs         # ~/.element/config.toml. Full Default impl.
│                     # Migrates from legacy config.json on first run.
│                     # pub(crate) fn data_dir() shared by database.rs.
│
├── database.rs       # SQLite via rusqlite (bundled). Mutex<Connection>.
│                     # Tables: clipboard_entries, frecency.
│                     # Methods: load_clipboard, record_launch, top_frecency, frecency_score.
│                     # fn new_in_memory() for tests.
│
├── error.rs          # ElementError (thiserror). Variants: Config, Database, Io, Icon,
│                     # Provider { provider, detail }, Other.
│
├── registry.rs       # ProviderRegistry. Holds Vec<Box<dyn SearchProvider>>.
│                     # search() → catch_unwind per provider, merge results, sort by score.
│                     # activate() → catch_unwind per provider lookup by provider_id.
│                     # refresh_all() → catch_unwind per provider.
│
├── theme.rs          # Named constants: BG_PRIMARY, BG_SELECTED, BG_INPUT,
│                     # Brand-kit dark-surface colors plus TEXT_PRIMARY, TEXT_MUTED, TEXT_ERROR,
│                     # ACCENT, RESULT_HEIGHT, ICON_SIZE, SPACING_*, etc.
│
├── providers/
│   ├── mod.rs        # SearchProvider trait + SearchContext<'a> { config, db }.
│   ├── apps.rs       # Background app scan → fuzzy match → frecency → icons.
│   ├── calculator.rs # evalexpr. should_run: contains digits/math ops.
│   ├── emoji.rs      # emojis crate. should_run: starts with "emoji" or ":".
│   ├── clipboard.rs  # SQLite clipboard table. should_run: "cbhist" or "clip".
│   └── websearch.rs  # webbrowser + config.search_url. Always runs, score=-1 (bottom).
│
└── ui/
    └── mod.rs        # Iced views. Embedded brand mark, focused TextInput, Scrollable.
```

## Provider System

### Trait

```rust
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;              // "apps", "calculator", etc.
    fn priority(&self) -> i32 { 0 }             // higher = ranked first on tie
    fn should_run(&self, query: &str) -> bool;  // cheap gate check
    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult>;
    fn activate(&self, ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError>;
    fn refresh(&self) {}                         // request a provider refresh
    fn revision(&self) -> u64 { 0 }              // data revision after an async refresh
}
```

### SearchResult

```rust
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: String,         // metadata tag — used for UI feedback after activation
    pub provider_id: String,  // must match provider.id() for activation dispatch
    pub action: String,       // exact provider-owned data to activate the result
    pub icon_rgba: Option<(Vec<u8>, u32, u32)>,  // raw RGBA pixels, width, height
    pub score: f64,
}
```

### Adding a new provider

1. Create `src/providers/your_thing.rs`, implement `SearchProvider + Send + Sync`.
2. Add `pub mod your_thing;` to `src/providers/mod.rs`.
3. Register in `app.rs::SearchEngine::new()`: `registry.add(Box::new(YourProvider::new()));`
4. If it needs I/O on overlay open, make `refresh()` non-blocking and increment
   `revision()` only after publishing new data.
5. Add unit tests for any non-trivial logic.
6. Run `cargo fmt && cargo clippy -- -D warnings && cargo test`.

### Important: providers run inside `catch_unwind`

Do NOT let `?` or panics propagate out of `search()` / `activate()` — the registry
catches them, but logging has already happened. Return empty vec on error instead.

### Scoring for sort order

Results from all providers are merged and sorted by `score` descending (then title
alphabetically). Priority is **not** currently used in sort, only as future tiebreaker.
Use scores strategically:
- Calculator: 1000 (always on top when matched)
- Emoji: 500 - index (decaying, up to 20)
- Apps: fuzzy_score × frecency_boost (0–~200)
- Clipboard: 200
- Web search: -1 (always last)

## Key Design Decisions

### RegisterHotKey + PeekMessageW — not GetAsyncKeyState

The background thread registers a real hotkey and runs a PeekMessageW loop. When the
queue is empty it sleeps 50ms instead of busy-waiting. This means **zero CPU usage**
when idle. Look at the loop in `main.rs:354-408`.

### Hidden message window for tray

A message-only window (`HWND_MESSAGE`, class `ElementTrayClass`) hosts the tray icon
so `Shell_NotifyIconW` has an HWND for callbacks. The window procedure `tray_wnd_proc`
handles WM_APP (left-click toggle, right-click menu) and WM_COMMAND (Exit).

### Adaptive window height

`adaptive_height()` formula: `52 + min(results, 10) × 42 + 8`, plus a 24 px status row
when activation feedback is visible; it is capped at 500, min 56.
The result is stored in `RESIZE_HEIGHT` atomic and `RESIZE_REQUESTED` is set. The
background thread reads both and calls `SetWindowPos`. This avoids Iced needing to
know about Win32 window management.

### WINDOW_WIDTH atomic

The hotkey thread needs the window width to center the window. Iced owns the real
window size. So `config.window_width` is copied to the `WINDOW_WIDTH` atomic at
startup, and the thread reads that when positioning.

### data_dir() in config.rs

Single source of truth: `~/.element/`. Database and icon cache paths are derived
from this. Do not duplicate it.

### Icon pipeline

1. Resolve the Start Menu `.lnk` through `IShellLink` to its target executable and
   optional icon location.
2. Require an existing `.exe` target; app activation starts that executable directly.
3. Prefer a valid `.ico` path from the shortcut; otherwise extract the target
   executable's embedded icon through `IShellItemImageFactory` at 32×32.
4. Cache decoded RGBA pixels as `~/.element/cache/icons/v2-<source-hash>.png`.

### Fuzzy scorer

Character-level sequential match with bonuses:
- Base: +10 per matched character
- Consecutive: +15 (prev_matched)
- Word boundary: +30 (space, `-`, `_`, `/`, `\` or start of string)
- CamelCase: +20 (lowercase→uppercase transition)
- Separator after: +15 (non-alphanumeric before match)
- Early match: +50 if first match is at position 0, else scaled
- Gap penalty: −2 per unmatched character
- Normalized: divide by query length

### Frecency formula

SQL query: `count / (julianday('now') - julianday(last_used) + 1)`

App search multiplies score by: `1.0 + (frecency_score × 5.0)`, capped at 3×.

## Build & Test

```bash
cargo build              # debug build
cargo build --release    # release (slow — LTO takes ~5min)
cargo test               # 27 tests (fuzzy, frecency, app-result deduplication, calc, config, clipboard, URL encoding)
cargo fmt                # format
cargo clippy -- -D warnings   # lint (blocking on CI)
```

## Platform Code

Window and tray FFI lives in `main.rs`. Shortcut resolution and icon extraction stay
isolated in `providers/apps.rs`, where COM is initialized only for the helper call.

Do not scatter `#[cfg(target_os = "windows")]` through providers or UI code.
If a provider needs platform-specific logic, isolate it behind a helper.

## Future Stages

Do not build these unless the trigger condition is met:

| Stage | Trigger | Scope |
|-------|---------|-------|
| Workspace split (core/providers/platform/ui crates) | Compile times hurt or 2+ devs working in parallel | Pure file move — crate boundaries already exist |
| Command registry (VS Code–style) | 8+ providers with overlapping actions | Named composable commands replacing ad-hoc `activate()` |
| Event bus | Multiple subsystems need same event | Formal Event enum + subscriber list |
| Daemon/IPC (elementd) | Measured cold-start latency is actually bad | Background process keeps index/db warm |
| Platform trait abstraction | Actually porting to macOS/Linux | Extract platform/windows/ behind traits |
| Plugin SDK | 8+ built-in providers + users asking for custom ones | element-plugin crate, Plugin trait, trust-based |
| Marketplace + sandboxing | External plugin authors submitting code | Manifest verification + sandboxed execution |
| AI provider / notes app | v1 has real users and retention | Full long-term vision |

## Known Issues

- DWM acrylic blur not implemented (needs winit HWND handle from Iced).
- Window centered at fixed position — no multi-monitor DPI awareness.
- `FindWindowW("Element")` looks up by title — fragile if another window has same title.
- Start Menu shortcuts without an existing direct `.exe` target are intentionally skipped.

## Common Pitfalls

- Adding `unsafe` around safe wrapper FFI functions will cause warnings. The wrappers
  in `main.rs` are safe (`extern "system" fn`, not `unsafe extern`), so no `unsafe` block
  is needed to call them.
- Do NOT add `new implementation/` docs to the repo root — that folder is deleted.
  ARCHITECTURE.md content lives in `AGENTS.md` now.
- When adding a provider dependency to `Cargo.toml`, remember `bundled` for rusqlite.
- The `SearchResult` must have `provider_id` set correctly or `activate()` will fail
  with "no provider registered with id".
- Every `SearchResult` must include the exact provider-owned `action` data. Never
  recover a selected app by its visible title because titles are not unique.
- Tests that touch the database must use `Database::new_in_memory()`, not `new()`.
