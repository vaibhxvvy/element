# Element — Agent Reference

This doc is the single source of truth for AI agents working on Element.
Read it first before making any changes.

## Project Overview

Element is a **global-hotkey launcher for Windows** — press `Alt+Space` for a floating
search bar. Type to fuzzy-find apps and files, calculate math, search emoji, browse
clipboard history, run system commands, or search the web. Built in Rust with
Iced 0.14 (wgpu).

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
│                     # Clipboard watcher: "element-clipboard" thread polls every
│                     #   800 ms → db.save_clipboard (dedupe + trim); also captures
│                     #   bitmaps (DIB/DIBV5/PNG) → full+thumb PNG in
│                     #   cache/clipboard/ + db.save_clipboard_image (pixel-hash
│                     #   dedupe, 16 MB cap). Helpers: capture_clipboard_image,
│                     #   write_clipboard_image_files.
│                     # Atomics: HOTKEY_TRIGGERED, HIDE_REQUESTED, EXIT_REQUESTED,
│                     #          RESIZE_HEIGHT, RESIZE_REQUESTED, WINDOW_WIDTH,
│                     #          WINDOW_FOUND, LAUNCHER_SHOWN, LAST_TOGGLE_MS,
│                     #          LAUNCHER_HWND, LAUNCHER_LAST_SHOWN_MS, AUTO_HIDDEN_AT
│                     # Tray proc also handles WM_APP_TIMER_DONE (0x8001, timer
│                     #   balloon) and WM_APP_SCREENSHOT_DONE (0x8002, screenshot
│                     #   toast — wparam owns a boxed String body).
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
│                     # clipboard_newest_first (default true) seeds the clipboard
│                     #   sort direction; the settings panel flips it live.
│
├── database.rs       # SQLite via rusqlite (bundled). Mutex<Connection>.
│                     # Tables: clipboard_entries (pinned flag), clipboard_images
│                     #          (hash UNIQUE, path, width/height, pinned),
│                     #          emoji_frecency, frecency, file_frecency.
│                     # Methods: load_clipboard_filtered(limit, text_like,
│                     #          date_from, date_to, newest_first) — positional
│                     #          params numbered sequentially; sort is
│                     #          (pinned DESC, id DESC/ASC) because two rows
│                     #          captured in the same second share created_at
│                     #          (UTC, second resolution). load_clipboard /
│                     #          load_clipboard_images return (…, pinned, id)
│                     #          and are #[cfg(test)] wrappers now.
│                     #          load_clipboard_images_filtered(limit,
│                     #          date_from, date_to, newest_first); local_date
│                     #          (days_offset) computes local dates in SQLite
│                     #          (SELECT date('now','localtime',?1)) — no chrono.
│                     #          save_clipboard (dedupe/trim),
│                     #          toggle_clipboard_pinned, load/save_clipboard_images
│                     #          (dedupe by hash, trim removes cached files),
│                     #          toggle_clipboard_image_pinned, record_emoji_use,
│                     #          emoji_frecency_score, record_launch, top_frecency,
│                     #          frecency_score, record_file_open,
│                     #          file_frecency_score (case-insensitive keys).
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
│   ├── fuzzy.rs      # Shared character-level fuzzy scorer (pure, unit-tested,
│   │                 # used by apps and files).
│   ├── icon.rs       # Shared icon pipeline: PNG cache + IShellItemImageFactory
│   │                 # extraction for ANY shell path (exe, file, folder).
│   ├── apps/         # Feature folder: app search.
│   │   ├── mod.rs    #   AppsProvider: fuzzy match → frecency boost → icons, recommendations.
│   │   ├── scan.rs   #   Background Start Menu scan (walkdir, .lnk → exe, dedup).
│   │   └── icons.rs  #   Shortcut layer: .lnk resolution + .ico preference,
│   │                 #   delegates extraction to providers/icon.rs.
│   ├── calculator/   # evalexpr. should_run: contains digits/math ops.
│   │   └── mod.rs
│   ├── color/         # "#ff0000" → hex/rgb/hsl results with a rendered
│   │   └── mod.rs    #   swatch icon; Enter copies the selected variant.
│   ├── units/        # "5 km in miles" / "100 c in f" — length, mass, volume,
│   │   └── mod.rs    #   speed, data, time, temperature. Multi-separator parse.
│   ├── snippets/     # Quick text insertions from ~/.element/snippets.toml
│   │   └── mod.rs    #   (name = "text"). Exact name copies; "snip" lists.
│   │                 #   Cached in memory, reloaded on refresh() (overlay open).
│   ├── settings/     # "settings" → opens the in-launcher settings panel
│   │   └── mod.rs    #   (UI intercepts kind=="settings"; width/URL/accent/autostart).
│   ├── help/         # "help" → opens the in-launcher manual (hotkeys, providers,
│   │   └── mod.rs    #   tips). UI intercepts kind=="help" like the settings flow.
│   ├── emoji/        # emojis crate. should_run: starts with "emoji" or ":".
│   │   └── mod.rs    #   Frecency boost (≤2×) from emoji_frecency table; activate
│   │                 #   records usage via record_emoji_use.
│   ├── clipboard/    # SQLite clipboard tables. should_run: "cbhist" or "clip".
│   │   └── mod.rs    #   Text + image entries merged by (pinned, recency).
│   │                 #   Newest first by default (NEWEST_FIRST AtomicBool,
│   │                 #   seeded from config + flipped live by the settings
│   │                 #   panel); sort is (pinned, id) so same-second captures
│   │                 #   keep capture order. Query grammar after the trigger:
│   │                 #   today / yesterday / YYYY-MM-DD / last Nd filter by
│   │                 #   local date (db.local_date), sort:new / sort:old
│   │                 #   override the order, any remaining words are a text
│   │                 #   LIKE filter (escaped). Pinned rows sort first and
│   │                 #   show 📌 ("· pinned" subtitle). Image rows: kind
│   │                 #   "clipboard-image", 64×64 thumb PNG decoded into
│   │                 #   icon_rgba; Enter restores the image to the clipboard
│   │                 #   as CF_DIB (rgba_to_dib + set_clipboard_bitmap).
│   ├── system/       # System commands + everyday quick actions. should_run:
│   │   └── mod.rs    #   shutdown/restart/reboot(alias)/sleep/lock/volume/mute/
│   │                 #   screen off/timer/password/screenshot at command start
│   │                 #   (word boundary, case-insensitive). Score 300, priority 9.
│   │                 #   shutdown.exe for off/restart; platform::lock_workstation /
│   │                 #   suspend_system for lock/sleep; waveOut* for volume;
│   │                 #   GDI BitBlt for screenshot (all-monitor capture); timers
│   │                 #   post WM_APP_TIMER_DONE (0x8001) to the tray window which
│   │                 #   shows a balloon; BCryptGenRandom for passwords
│   │                 #   (kind "password" keeps the window open w/ feedback).
│   │                 #   Screenshot saves the PNG to Pictures\Screenshots and
│   │                 #   notifies the tray via platform::notify_screenshot_captured
│   │                 #   (WM_APP_SCREENSHOT_DONE 0x8002) → toast + status row.
│   ├── files/        # Raycast-style file search. Runs on "file"/"folder" prefixes
│   │                 # AND on bare queries (≥2 chars, not emoji/clipboard/math
│   │                 # domains) so typing ".png" or "pvt" finds files directly.
│   │   ├── mod.rs    #   FilesProvider: prefix/bare parsing, fuzzy match on names,
│   │   │             #   frecency boost (≤2×, same decay as apps), lazy icons via
│   │   │             #   revision loop, explorer.exe activation + record_file_open.
│   │   │             #   Bare queries: score 10 + fuzzy×0.5, cap 6 (apps stay on
│   │   │             #   top). Prefixed: 120 + fuzzy×2 (+10 folders), cap 30.
│   │   │             #   Roots published synchronously at construction (bare
│   │   │             #   `file` shows Desktop/Documents/... instantly); an
│   │   │             #   "Indexing your files…" hint fills the gap while the
│   │   │             #   first background scan is still running.
│   │   │             #   Scan limits live from settings (depth 4–32, entries
│   │   │             #   10k–200k): set_file_limits() + Request::UpdateFileIndex.
│   │   └── scan.rs    #   Background walk of curated user folders (Desktop/
│   │                 #   Documents/...): junk exclusions, depth/entry caps
│   │                 #   from config (file_index_depth / file_index_entries).
│   └── websearch/    # webbrowser + config.search_url. Always runs, score=-1 (bottom).
│       └── mod.rs    #   Bare URLs (example.com, https://…) open directly, score=850.
│                     #   Per-site shortcuts: `yt cats` → 800 (search_prefixes map
│                     #   in config, `%s` template; defaults yt/gh/w).
│
├── platform.rs       # Win32 helpers behind safe wrappers: copy_files_to_clipboard
│                     # (CF_HDROP), lock_workstation, suspend_system,
│                     # clipboard_bitmap_bytes (CF_DIB/DIBV5/PNG capture),
│                     # set_clipboard_bitmap, dib_to_rgba / rgba_to_dib.
│                     # Tray notifications: WM_APP_SCREENSHOT_DONE (0x8002) +
│                     # notify_screenshot_captured(body) — posts a boxed String
│                     # to the tray window; show_toast for custom toasts.
│
└── ui/
    └── mod.rs        # Iced views. Search TextInput, scrollable results list,
                      # status row for activation feedback, contextual hint row
                      # (files: Alt+C/F/Enter; clipboard: right-click to pin +
                      # date/sort tokens), settings + help panels (Mode enum;
                      # Esc/Back leaves). Settings includes the clipboard
                      # order picker (flips the provider's NEWEST_FIRST static
                      # live) plus width/URL/accent/autostart/index limits.
                      # Sends Request to the Orchestrator via engine.handle(Request)
                      # and matches Outcome.
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
- Units: 900
- Color: 950/940/930 (hex, rgb, hsl — explicit `#hex` intent)
- Web URL passthrough: 850 (typed destination)
- Web prefix shortcuts (`yt …`): 800 (explicit intent)
- Snippets: 700 exact name match, 500 listing (`snip …`, decaying)
- Settings / Help panels: 1000 (UI intercepts kind and switches mode)
- Emoji: 500 - index (decaying, up to 20) × frecency boost
- Apps: fuzzy_score × frecency_boost (0–~200)
- Clipboard: 200
- System commands: 300 (priority 9)
- Files: 120 + fuzzy_score × 2 (+10 for folders); recommendations 210 (scan roots)
- Files bare (no `file`/`folder` prefix): 10 + fuzzy_score × 0.5 (+5 for folders), cap 6 — apps stay on top
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
know about Win32 window management. The hint row shares the status slot — the status
row wins when both would be visible.

### Clipboard watcher thread

A background thread ("element-clipboard") polls the clipboard every 800 ms and calls
`db.save_clipboard(&text, keep)` when the text changed (dedupe, trim to
`config.clipboard_max_entries`). History rows can be pinned via right-click;
`toggle_clipboard_pinned` flips all rows with that text, pinned rows sort first and
survive trimming. Emoji usage is tracked the same way via `record_emoji_use`.

### WINDOW_WIDTH atomic

The hotkey thread needs the window width to center the window. Iced owns the real
window size. So `config.window_width` is copied to the `WINDOW_WIDTH` atomic at
startup, and the thread reads that when positioning.

### Auto-hide on focus loss (clicking away)

While the launcher is visible the background thread polls `GetForegroundWindow`
every ~10 ms. If the foreground window is no longer the launcher (user clicked
another app, the desktop, Alt+Tab, ...) and more than `FOCUS_LOSS_GRACE_MS`
(250 ms) has passed since `LAUNCHER_LAST_SHOWN_MS`, it calls `hide_launcher()`
exactly like the Alt+Space toggle. The grace period lets `SetForegroundWindow`
finish before the check becomes active. `LAUNCHER_HWND` caches the window handle
to avoid an `EnumWindows` scan on every poll.

The tray left-click is the one race: the click both steals focus (triggering
auto-hide) and posts the toggle message. If a tray toggle arrives within
`TRAY_SUPPRESS_MS` (300 ms) of an auto-hide, the toggle keeps the window hidden
instead of re-showing it — `toggle_launcher(from_tray: bool)` distinguishes
tray clicks from hotkey/LL-hook toggles, which always toggle normally.

### Hotkey fallback: RegisterHotKey → LL hook → fallback combos

Three-tier strategy in `hotkey::install()` (`src/hotkey/mod.rs`):

1. **RegisterHotKey** — cleanest approach, OS-level global hotkey. Fails if another
   app already claimed the same combo.
2. **WH_KEYBOARD_LL hook** — if RegisterHotKey fails, installs a low-level keyboard
   hook that intercepts the key event *before* the competing app sees it. Returns 1
   to swallow the event, preventing the other app's hotkey from firing.
3. **Fallback combos** — if both tier 1 and 2 fail, try `hotkey_fallback_candidates()`
   (Alt+Space, Ctrl+Space, Ctrl+Shift+Space, etc.) until one succeeds.

Key-repeat is prevented by the `TOGGLE_PENDING` + `COMBO_DOWN` atomics — the
LL hook only sets the flag on the first keydown of a hold and swallows the
matching keyup until it is consumed.

### Single-instance guard

Named mutex (`Local\ElementLauncherSingleInstance`) via `CreateMutexW`. On
`ERROR_ALREADY_EXISTS`, the second instance calls `find_any_launcher_hwnd()` +
`show_launcher()` to bring the existing window to foreground, then exits.
The first instance never calls `CloseHandle` — the kernel releases the mutex
automatically on process termination. Without this, a second copy double-
registers the hotkey (RegisterHotKey + LL-hook fallback) and both toggle the
window against each other.

### Reliable foreground steal

`SetForegroundWindow` from the hotkey thread is normally blocked by Windows'
foreground lock (the process receives no direct user input). `show_launcher`
attaches to the launcher window's thread via `AttachThreadInput`, sets
foreground, then detaches immediately. Auto-hide only fires after the window
has actually held focus at least once (`LAUNCHER_HAD_FOCUS`) — a denied
foreground steal never makes the window blink open and close.

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

The extraction + cache core is shared in `providers/icon.rs`
(`cached_icon_for_path`), which works for **any** shell path — executables,
plain files, and folders. The files provider uses it for lazy per-result icon
fetching on a worker thread, publishing new icons through the revision counter
so the UI re-renders without blocking on COM.

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
cargo test               # 101 tests (fuzzy, frecency, app-result deduplication, calc, config, clipboard pinning/dedupe/order/date-text filters, emoji frecency, system commands, help, files, URL encoding, web prefixes)
cargo fmt                # format
cargo clippy -- -D warnings   # lint (blocking on CI)

## Releases

Every change that warrants a version bump gets a new version + release
(standing instruction — do this on feature/UX/fix batches, not per commit).
Version lives in `Cargo.toml` AND `installer.iss` (`MyAppVersion`). Semver:
features → minor, bug fixes → patch. **Never delete or overwrite an existing
release/tag — old releases stay forever; ship changes as a new version.**
Full release flow:

1. `cargo build --release` (gate first: fmt + clippy + tests).
2. `& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer.iss` → `element-<ver>-setup.exe`.
3. `Compress-Archive target\release\element.exe, brandkit\windows\element.ico → element-<ver>-win64.zip`.
4. Write `element-<ver>-setup.exe.sha256` (hash + filename), hash via `Get-FileHash -Algorithm SHA256`.
5. New winget folder `winget/vaibhxvvy/Element/<ver>/` — copy the three yamls from the previous
   version, bump `PackageVersion`, set `InstallerUrl` to the new tag URL and `InstallerSha256`
   to the setup hash, update `ReleaseNotesUrl` and `ReleaseDate`. Keep old version folders.
6. `gh release create v<ver> --title "Element <ver>" --notes "..."`, then
   `gh release upload v<ver> element-<ver>-setup.exe element-<ver>-setup.exe.sha256 element-<ver>-win64.zip`.
7. Update the download filenames in `README.md` if they reference a version.
8. Commit + push everything (code, installer.iss, winget folder, README).
```

## Platform Code

Window and tray FFI lives in `main.rs`. Shortcut resolution stays isolated in
`providers/apps/icons.rs`; generic icon extraction (`IShellItemImageFactory`)
lives in `providers/icon.rs`, where COM is initialized only for the helper call.
Simple Win32 helpers (clipboard file copy via CF_HDROP, lock workstation,
suspend system, system volume via waveOut*, screen capture via GDI,
screen-off, BCryptGenRandom passwords, tray-hwnd registration + timer
notifications) live in `src/platform.rs` behind safe wrappers.

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
