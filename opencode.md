# Element — OpenCode Session Log

## 2026-08-06 Session (2) — Phase 17: File & Folder Search (Raycast-style)

### Goal
Add a file/folder search mode like Raycast's File Search: type `file <query>` or
`folder <query>` in the launcher to fuzzy-match files/folders from your home
directory and open them. Extract the fuzzy scorer and icon pipeline into shared
modules so both apps and files providers use them.

### Shared modules extracted

- `src/providers/apps/fuzzy.rs` → `src/providers/fuzzy.rs` (git mv) — used by
  apps and files via `super::fuzzy::fuzzy_score`.
- `src/providers/icon.rs` (new) — the generic icon core moved out of
  `apps/icons.rs`: `icon_cache_dir`, `is_ico_path`, cache load/save, PNG cache
  path, `shell_item_icon` (IShellItemImageFactory), and new
  `cached_icon_for_path(path, cache_dir)` which works for **any** shell path —
  executables, plain files, folders.
- `src/providers/apps/icons.rs` slimmed to the shortcut layer: `ShortcutInfo`,
  `resolve_shortcut` (.lnk COM), and `cached_icon(shortcut)` which resolves the
  `.ico` preference then delegates to the shared pipeline.

### FilesProvider (`src/providers/files/`)

- **Prefix gating** (`should_run`): only `file`, `files`, `folder`, `folders`
  prefixes (case-insensitive) — never shadows normal app search.
- **Background index** (`scan.rs`): walks configured dirs or curated user
  folders (Desktop/Documents/Downloads/...), skipping hidden entries, junk
  dirs (node_modules, target, .git, AppData, site-packages, DCIM/100PINT
  phone dumps, version dirs, caches/media/backup/libraries/firefox by
  substring), capped at depth 8 and 25k entries, deduped by path.
- **Scoring**: matches `120 + fuzzy × 2` (+10 for folders); recommendations
  190–200 (folders first). Config `file_search_dirs` overrides the default home.
- **Lazy icons**: `search()` attaches icons from an in-memory cache; a worker
  thread extracts the top-12 missing icons via the shared pipeline and bumps
  the provider revision so the UI re-runs the query and renders them — no COM
  on the UI thread (same revision-loop the apps provider uses).
- **Activation**: `explorer.exe <path>` — opens folders in Explorer, files with
  their default handler, no console flash.
- Registered in `Orchestrator::new()` with `config.file_search_dirs`.

### Follow-up fix (same session — empty result list bug)

User reported "can't see files or folders". Diagnosis via diagnostic tests
showed the provider worked but the index was flooded: the home dir contains
phone-backup trees (`Desktop\pvt\...` with `DCIM/100PINT`, WhatsApp/Telegram
media dumps, `miniconda3`, `curseforge`, PowerShell module version dirs) that
swallowed the 25k entry cap with junk before real files were indexed, and the
bare `file` query surfaced that junk.

Fixes:
- Default roots = **curated user folders** (Desktop, Documents, Downloads,
  Pictures, Music, Videos) instead of the whole home dir; falls back to home.
- Junk exclusions hardened: version dirs (`0.1.0`/`v2.0`), Android dump dirs
  (`100PINT`, `100MEDIA`, ...), and substring matches for `cache`, `backup`,
  `media`, `libraries`, `site-packages`, `dcim`, `firefox`.
- Bare `file`/`folder` query now recommends the scan roots (Desktop,
  Documents, ...) as openable folder results instead of first-alphabetical
  junk — verified: 6 root recs, `file report` → 30 real matches.
- Clean index on the real machine: 6,140 entries (was 25,000 junk-capped).

### Verification

```bash
cargo check               # clean
cargo test                # 44 tests, all pass (13 new: prefix parsing, folder mode, exclusions)
cargo clippy -- -D warnings  # clean
cargo fmt                 # clean
```

### Files changed

| File | Change |
|------|--------|
| `src/providers/fuzzy.rs` | Moved from apps/fuzzy.rs (shared) |
| `src/providers/icon.rs` | New — shared icon pipeline for any path |
| `src/providers/apps/icons.rs` | Slimmed to .lnk layer (delegates extraction) |
| `src/providers/apps/{mod,scan}.rs` | Imports rewired to shared modules |
| `src/providers/files/{mod,scan}.rs` | New — FilesProvider + home-dir scanner |
| `src/config.rs` | New `file_search_dirs` field |
| `src/orchestrator.rs` | Registers FilesProvider |
| `AGENTS.md`, `ELEMENT_STATE.md` | New tree, 40 tests, files provider docs |
| `opencode.md` | This session log |

## 2026-08-06 Session — Phase 16: Module/Domain Split + Orchestrator

### Goal
Give the source tree proper feature folders (hotkey, app search, emoji, calculator, …) and a
single **Orchestrator** that takes the user's request and performs it. Update the docs.

### Restructure (pure file moves + module rewiring)

**Before → After:**

```
src/app.rs                  → src/orchestrator.rs         (SearchEngine → Orchestrator)
src/hotkey.rs               → src/hotkey/mod.rs           (folder module)
src/providers/apps.rs       → src/providers/apps/mod.rs   + scan.rs + fuzzy.rs + icons.rs
src/providers/calculator.rs → src/providers/calculator/mod.rs
src/providers/clipboard.rs  → src/providers/clipboard/mod.rs
src/providers/emoji.rs      → src/providers/emoji/mod.rs
src/providers/websearch.rs  → src/providers/websearch/mod.rs
```

### Orchestrator (`src/orchestrator.rs`)

- Public API: `handle(Request) -> Outcome`.
  - `Request::Search(query)` → `Outcome::Results(Vec<SearchResult>)`
  - `Request::Activate(result)` → `Outcome::Activated(Result<(), ElementError>)`
  - `Request::Refresh` → `Outcome::Refreshed(u64)` (new data revision)
- Owns `Arc<Config>`, `Arc<Database>`, `ProviderRegistry`; registers all five
  providers in `new()`. The UI only ever calls `engine.handle(...)`.

### Other changes

- `SearchResult` moved from `app.rs` into `src/providers/mod.rs` (lives with the
  provider contract); derives `Debug + Clone`.
- UI routes every action through the Orchestrator: search, activation, and the
  refresh-on-hotkey path (which now stores the returned revision).
- `apps.rs` (975 lines) split by concern: `mod.rs` (provider + frecency +
  recommendations), `scan.rs` (Start Menu walk, `.lnk` → exe, dedup),
  `fuzzy.rs` (scorer + 10 tests), `icons.rs` (`.ico`/IShellItemImageFactory + cache).
- `.gitignore`: added `/winget-pkgs/` (vendored fork clone, maintained outside the repo).

### Verification

```bash
cargo check               # clean
cargo test                # 31 tests, all pass (10 fuzzy, 2 apps, calc, config, DB, clipboard, URL)
cargo clippy -- -D warnings  # clean
cargo fmt                 # clean
```

### Files changed

| File | Change |
|------|--------|
| `src/orchestrator.rs` | New — Request/Outcome API, provider registration (renamed from app.rs) |
| `src/hotkey/mod.rs` | New folder module (moved from hotkey.rs) |
| `src/providers/mod.rs` | SearchResult moved here from app.rs |
| `src/providers/apps/{mod,scan,fuzzy,icons}.rs` | Split of the apps monolith |
| `src/providers/{calculator,emoji,clipboard,websearch}/mod.rs` | Folder modules (moved) |
| `src/main.rs`, `src/ui/mod.rs`, `src/registry.rs` | Imports/paths rewired |
| `AGENTS.md`, `ELEMENT_STATE.md` | New tree, Orchestrator, 31 tests |
| `opencode.md` | This session log |

## 2026-07-28 Session — Phase 15: DWM Fix, Dark UI, Debug Logging

### Root Cause: Invisible Window
The search box was invisible because of a **layered window ordering bug** in `apply_acrylic_blur()` (`src/main.rs:98-150`):

1. The function set `WS_EX_LAYERED` on the window **BEFORE** calling `SetWindowCompositionAttribute`
2. If `SetWindowCompositionAttribute` **failed** (common on many Win10/Win11 configurations), the window was left in a layered state **without** proper alpha composition
3. Result: window is fully transparent/invisible despite `ShowWindow` + `SetWindowPos` succeeding
4. Additionally, `Theme::Light` gave a white background that bled through the semi-transparent container

### Changes Made

#### 1. Critical DWM Acrylic Fix (`src/main.rs`)
- **Reordered** `apply_acrylic_blur()` so `SetWindowCompositionAttribute` is called **FIRST**
- `WS_EX_LAYERED` is only set **AFTER** the API succeeds
- If `SetWindowCompositionAttribute` fails, the window stays visible with Iced-rendered background
- Changed `gradient_color` from `0x803C3C3C` to `0x7F3C3C3C` for proper 50% opacity #3c3c3c tint
- Added detailed logging at every step (function pointer load, API call, result)

#### 2. Theme: `Theme::Light` → `Theme::Dark` (`src/main.rs:739`)
- Switched to `Theme::Dark` so the window has a dark background if DWM acrylic is unavailable
- Prevents white background showing through semi-transparent container

#### 3. Dark UI Design (`src/theme.rs`)
Applied the requested design:
- Background: `#3c3c3c` at 35% opacity (lets DWM acrylic show through)
- Selected row: `#4d4d4d` at 50% opacity
- Input field: `#1e1e1e` at 40% opacity
- Border: `#4d4d4d` **full opacity**, 2px width
- Rounded corners: 12px container, 6px input
- All text: light gray on dark (`#dcdcdc` primary, `#a0a0a0` muted)

#### 4. Comprehensive Debug Logging
**New atomic flags** (`src/main.rs`):
- `HOTKEY_REGISTERED` — tracks if RegisterHotKey succeeded
- `WINDOW_FOUND` — tracks if FindWindowW found the Iced window

**Enhanced logging** at every check point:
- `RegisterHotKey` result (SUCCESS/FAILED + conflict suggestion)
- Window creation and DWM application
- `WM_HOTKEY` event with loop iteration count
- `FindWindowW` return value for every lookup
- `IsWindowVisible` before show/hide decisions
- Background thread loop iteration counter
- UI Tick handler events (HOTKEY_TRIGGERED, EXIT_REQUESTED)
- Keyboard events (Escape pressed)

#### 5. Enhanced Debug Script (`debug.ps1`)
- Hotkey conflict detection (checks for PowerToys, Teams, Spotify, AutoHotkey, etc.)
- Auto-kill stale Element processes before launch
- Build + launch + monitor workflow (`-Build -Release` flags)
- Session summary with event counts (Alt+Space presses, shows, hides, errors)
- 30-second timeout with descriptive error messages
- Colorized output for different log levels

### How to Diagnose Issues

1. **If Alt+Space does nothing:**
   - Run `.\debug.ps1 -Build` (starts with monitoring)
   - Press Alt+Space and watch the log
   - Check for `CRITICAL: RegisterHotKey(Alt+Space) FAILED`
   - Check for `FindWindowW returned 0`
   - Check for `SetWindowCompositionAttribute FAILED`

2. **If application shows but no search box:**
   - Look for `CRITICAL: FindWindowW returned 0`
   - Check `Iced window not yet created or title mismatch`
   - The window title must be exactly "Element"

3. **If DWM acrylic not working (window appears but no blur):**
   - Log will show `SetWindowCompositionAttribute FAILED — acrylic blur not available`
   - The app falls back to `Theme::Dark` background — still usable

#### 6. Low-Level Keyboard Hook Fallback (`src/main.rs`)
- When `RegisterHotKey` fails (another app claimed the combo), installs `WH_KEYBOARD_LL` hook
- Hook intercepts the key event *before* the competing app sees it, swallows it (return 1)
- Prevents key-repeat toggles via `HOOK_KEY_HELD` atomic
- Falls back through known-safe combos if both RegisterHotKey and LL hook fail

#### 7. Single-Instance Guard (`src/main.rs`)
- Named mutex (`Local\ElementLauncherSingleInstance`) prevents duplicate processes
- Second instance activates the first via `find_any_launcher_hwnd()` + `show_launcher()`
- Mutex handle kept open for process lifetime (kernel auto-releases on exit)

#### 8. PID-Based Window Finding (`src/main.rs`)
- `find_own_launcher_hwnd()` uses `EnumWindows` + `GetWindowThreadProcessId` instead of `FindWindowW`
- Avoids targeting another process's window that happens to be titled "Element"
- `find_any_launcher_hwnd()` uses `FindWindowW` for cross-instance activation

#### 9. Comprehensive Doc Comments (All Source Files)
Added detailed Rust doc comments (`///`) and internal `//` comments across every source file:

| File | Comment Coverage |
|------|-----------------|
| `src/main.rs` | Module architecture, all atomics (read/write ownership), every Win32 constant, every FFI wrapper with MSDN links, entry-point flow, message loop stages, struct fields, single-instance, LL hook, hotkey tiers, DWM rationale |
| `src/app.rs` | SearchEngine + SearchResult struct/method docs |
| `src/config.rs` | Config fields, hotkey parsing, fallback candidates, migration |
| `src/database.rs` | All methods, all 12 tests |
| `src/debug_log.rs` | Logger, macro, lazy-init file |
| `src/error.rs` | Each ElementError variant |
| `src/registry.rs` | Provider dispatch, catch_unwind isolation |
| `src/theme.rs` | Every color, layout constant, DWM rationale |
| `src/providers/mod.rs` | SearchProvider trait + SearchContext docs |
| `src/providers/apps.rs` | Fuzzy scorer, icon pipeline, COM resolution, scanning |
| `src/providers/calculator.rs` | Math detection, eval, clipboard copy |
| `src/providers/emoji.rs` | Shortcode/name search, scoring decay |
| `src/providers/clipboard.rs` | SQLite history, substring filter, dedup |
| `src/providers/websearch.rs` | URL encoding, always-runs, score=-1 |
| `src/ui/mod.rs` | Iced Sandbox, all widget functions, resize logic |

### Files Changed

| File | Change |
|------|--------|
| `src/main.rs` | DWM acrylic reorder, Theme::Dark, verbose logging, new atomics, LL hook, single-instance, PID EnumWindows, doc comments |
| `src/app.rs` | Detailed doc comments |
| `src/config.rs` | Detailed doc comments, hotkey parsing docs, tests |
| `src/database.rs` | Detailed doc comments, all tests documented |
| `src/debug_log.rs` | Logger + macro docs |
| `src/error.rs` | Variant docs |
| `src/registry.rs` | Provider dispatch docs |
| `src/theme.rs` | Color/layout constant docs, DWM rationale |
| `src/providers/mod.rs` | Trait + context docs |
| `src/providers/apps.rs` | Fuzzy scorer, icon pipeline, COM docs |
| `src/providers/calculator.rs` | Math eval + clipboard docs |
| `src/providers/emoji.rs` | Shortcode search docs |
| `src/providers/clipboard.rs` | SQLite history docs |
| `src/providers/websearch.rs` | URL encoding docs |
| `src/ui/mod.rs` | Iced Sandbox docs |
| `debug.ps1` | Hotkey conflict detection, session summary, auto-build |
| `opencode.md` | This session log |

### Verification

```bash
cargo test              # 27 tests
cargo fmt --check       # formatting
cargo clippy -- -D warnings  # lint (clean)
cargo build             # dev build succeeds
cargo build --release   # release build succeeds
```

### Remaining Intentional Limitations
- DWM acrylic is a best-effort enhancement; app works without it (solid #3c3c3c bg)
- Hotkey is configurable (config.toml) with fallback strategy if taken
- Multi-monitor DPI not handled
- Window centered at fixed position (primary monitor only)
- No file search provider yet
- No clipboard monitor (OS clipboard watch)