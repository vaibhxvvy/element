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
│                     #   Fallback: WH_KEYBOARD_LL hook if RegisterHotKey fails
│                     #   Single-instance: named mutex guard
│                     # Tray: hidden message-only window, left-click toggle, right-click Exit
│                     # Window: PID-based EnumWindows (not FindWindowW)
│                     # Atomics: HOTKEY_TRIGGERED, HIDE_REQUESTED, EXIT_REQUESTED,
│                     #          RESIZE_HEIGHT, RESIZE_REQUESTED, WINDOW_WIDTH,
│                     #          WINDOW_FOUND, LAUNCHER_SHOWN, LAST_TOGGLE_MS
│
├── orchestrator.rs   # Orchestrator — the single entry point for user requests.
│                     # handle(Request) → Outcome: Search(query) → Results,
│                     # Activate(result) → Activated(Ok/Err), Refresh → Refreshed(rev).
│                     # Owns Arc<Config>, Arc<Database>, ProviderRegistry; registers
│                     # all providers in new(). UI never touches providers directly.
│
├── hotkey/           # Global hotkey detection (isolated from window show/hide).
│   └── mod.rs        #   install()/uninstall()/take_pending_toggle()/hook_active()
│                     #   RegisterHotKey → LL hook → fallback combos (three tiers).
│                     #   LL hook callback stays tiny — only sets a flag.
│
├── config.rs         # ~/.element/config.toml. Full Default impl.
│                     # Migrates from legacy config.json on first run.
│                     # pub(crate) fn data_dir() shared by database.rs.
│                     # Hotkey parsing: parse_hotkey(), hotkey_fallback_candidates().
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
├── theme.rs          # Named constants: BG_PRIMARY (#3c3c3c solid), BG_SELECTED (#505050),
│                     # BG_INPUT (#1e1e1e), BORDER_COLOR (#4d4d4d), CONTAINER_RADIUS (12px),
│                     # Brand-kit dark-surface colors plus TEXT_PRIMARY, TEXT_MUTED, TEXT_ERROR,
│                     # ACCENT, RESULT_HEIGHT, ICON_SIZE, SPACING_*, etc.
│
├── providers/
│   ├── mod.rs        # SearchProvider trait + SearchContext<'a> { config, db }
│   │                 # + SearchResult (provider contract lives with the providers).
│   ├── apps/         # Feature folder: app search.
│   │   ├── mod.rs    #   AppsProvider: fuzzy match → frecency boost → icons, recommendations.
│   │   ├── scan.rs   #   Background Start Menu scan (walkdir, .lnk → exe, dedup).
│   │   ├── fuzzy.rs  #   Character-level fuzzy scorer (pure, unit-tested).
│   │   └── icons.rs  #   Icon pipeline: .ico preference, IShellItemImageFactory, PNG cache.
│   ├── calculator/   # evalexpr. should_run: contains digits/math ops.
│   │   └── mod.rs
│   ├── emoji/        # emojis crate. should_run: starts with "emoji" or ":".
│   │   └── mod.rs
│   ├── clipboard/    # SQLite clipboard table. should_run: "cbhist" or "clip".
│   │   └── mod.rs
│   └── websearch/    # webbrowser + config.search_url. Always runs, score=-1 (bottom).
│       └── mod.rs
│
└── ui/
    └── mod.rs        # Iced views. Search TextInput, scrollable results list,
                      # status row for activation feedback. Sends Request to the
                      # Orchestrator via engine.handle(Request) and matches Outcome.
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

1. Create `src/providers/your_thing/mod.rs`, implement `SearchProvider + Send + Sync`.
2. Add `pub mod your_thing;` to `src/providers/mod.rs`.
3. Register in `orchestrator.rs::Orchestrator::new()`: `registry.add(Box::new(YourProvider::new()));`
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
when idle. Look at the loop in `main.rs` (the message loop inside `fn main`).

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

### Hotkey fallback: RegisterHotKey → LL hook → fallback combos

Three-tier strategy in `hotkey::install()` (`src/hotkey/mod.rs`):

1. **RegisterHotKey** — cleanest approach, OS-level global hotkey. Fails if another
   app already claimed the same combo.
2. **WH_KEYBOARD_LL hook** — if RegisterHotKey fails, installs a low-level keyboard
   hook that intercepts the key event *before* the competing app sees it. Returns 1
   to swallow the event, preventing the other app's hotkey from firing.
3. **Fallback combos** — if both tier 1 and 2 fail, try `hotkey_fallback_candidates()`
   (Alt+Space, Ctrl+Space, Ctrl+Shift+Space, etc.) until one succeeds.

Key-repeat is prevented by the `TOGGLE_PENDING` atomic — the LL hook only sets the
flag on the first keydown and swallows the matching keyup until it is consumed.

### Single-instance guard

Named mutex (`Local\ElementLauncherSingleInstance`) via `CreateMutexW`. On
`ERROR_ALREADY_EXISTS`, the second instance calls `find_any_launcher_hwnd()` +
`show_launcher()` to bring the existing window to foreground, then exits.
The first instance never calls `CloseHandle` — the kernel releases the mutex
automatically on process termination.

### PID-based window finding — not FindWindowW

`find_own_launcher_hwnd()` uses `EnumWindows` + `GetWindowThreadProcessId` + PID
verification instead of `FindWindowW`. This avoids accidentally targeting another
process's window that happens to be titled "Element".
`find_any_launcher_hwnd()` still uses `FindWindowW` for cross-instance activation
(second instance activating the first).

### Safe FFI wrappers — no `unsafe` at call sites

Every Win32 API function is wrapped in an `extern "system" fn` that declares the
external symbol with `#[link(name = "...")]` and calls it inside an `unsafe { }` block.
The wrapper itself is safe (`extern "system" fn`, not `unsafe extern`), keeping
`unsafe` out of all call sites. Each wrapper includes a doc comment with the MSDN link.

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
cargo test               # 31 tests (fuzzy, frecency, app-result deduplication, calc, config, clipboard, URL encoding)
cargo fmt                # format
cargo clippy -- -D warnings   # lint (blocking on CI)
```

## Platform Code

Window and tray FFI lives in `main.rs`. Shortcut resolution and icon extraction stay
isolated in `providers/apps/icons.rs`, where COM is initialized only for the helper call.

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
