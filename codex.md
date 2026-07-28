# Codex Audit Log

Date: 2026-07-28

## Objective

Audit Element for UI, UX, usage, and activation issues without changing working
application behavior. The user specifically called out shortcut-looking icons,
Enter not reliably opening apps, and apps sometimes opening incorrectly.

## Guardrails Followed

- Do not break current working behavior.
- Do not modify source code in this audit pass.
- Keep a detailed handoff for the next agent.
- Existing dirty worktree was present before this audit:
  - `ELEMENT_STATE.md` modified
  - `src/main.rs` modified
  - `src/providers/apps.rs` modified
  - `src/theme.rs` modified
  - `src/ui/mod.rs` modified
  - `opencode.md` untracked
- This audit intentionally added only `codex.md`.

## Material Reviewed

- Project docs:
  - `AGENTS.md`
  - `ELEMENT_STATE.md`
  - `opencode.md`
  - `README.md`
  - `CHANGELOG.md`
  - `CONTRIBUTING.md`
  - `SECURITY.md`
  - `CODE_OF_CONDUCT.md`
  - `brandkit/README.md`
  - `.github/PULL_REQUEST_TEMPLATE.md`
  - `.github/ISSUE_TEMPLATE/feature_request.md`
  - `.github/ISSUE_TEMPLATE/bug_report.md`
- Source areas:
  - `src/ui/mod.rs`
  - `src/main.rs`
  - `src/app.rs`
  - `src/registry.rs`
  - `src/config.rs`
  - `src/database.rs`
  - `src/providers/apps.rs`
  - `src/providers/calculator.rs`
  - `src/providers/emoji.rs`
  - `src/providers/clipboard.rs`
  - `src/providers/websearch.rs`
  - `src/theme.rs`

## Verification Run

- `cargo test`: passed, 24 tests.
- `cargo clippy -- -D warnings`: passed.
- Both commands printed `warn: could not canonicalize path C:\Users\vaibh`, but did
  not fail.
- No GUI smoke test was performed in this pass, so Windows shell focus, tray exit,
  and actual app launching still need manual verification.

## Architecture Notes

- UI state lives in `ElementApp` with `input`, `results`, and `selected_index`.
- `SearchResult` carries title, subtitle, kind, provider id, icon pixels, and score.
  It does not carry a stable provider-specific action payload such as app path,
  clipboard full text, or resolved executable path.
- App discovery indexes Start Menu `.lnk` files, stores app name/path internally in
  `AppsProvider`, and emits generic `SearchResult` values.
- Activation is provider-dispatched by `provider_id`, but the apps provider then
  re-finds the selected app by title.
- The hotkey/tray thread manipulates the Iced window indirectly using title-based
  `FindWindowW("Element")` and atomics.

## Findings To Fix

### P0 - Launch and Trust Issues

#### EUI-001: Enter activation hides the launcher even when activation fails

Evidence:
- `src/ui/mod.rs:78-86` calls `engine.activate()` and discards the `Result`.
- `src/ui/mod.rs:86` always sets `HIDE_REQUESTED`, even if no result was selected or
  activation failed.
- `src/providers/apps.rs:151-157` treats `ShellExecuteW` failure as an `eprintln!`
  but still returns `Ok(())`.

Impact:
- Users see the launcher disappear and assume something opened.
- Failed app launches, browser failures, and clipboard failures have no visible state.
- This directly matches the reported "Enter does not work" feeling.

Fix direction:
- Return `Err(...)` from app activation when `ShellExecuteW <= 32`.
- In UI, hide only after successful activation.
- Add a small non-invasive status/error row or inline message for failure.
- Keep the current fast success path intact.

#### EUI-002: Wrong app can launch when two shortcuts share the same title

Evidence:
- App results use `title: app.name.clone()` in `src/providers/apps.rs:40`,
  `src/providers/apps.rs:56`, and `src/providers/apps.rs:101`.
- Activation re-finds by title with `apps.iter().find(|a| a.name == result.title)` at
  `src/providers/apps.rs:133`.
- `SearchResult` does not carry the selected shortcut path.

Impact:
- If two Start Menu shortcuts have the same display name, selecting the second one can
  launch the first matching one.
- Frecency can also reinforce the wrong title because launches are recorded by title.

Fix direction:
- Add a stable action payload/id to `SearchResult`, or add an app-specific action id.
- For apps, carry the exact `.lnk` path and optionally the resolved target path.
- Record frecency against a stable app identity, while still displaying the friendly
  title.

#### EUI-003: Tray Exit likely does not exit the full app

Evidence:
- Tray menu Exit posts `PostQuitMessage(0)` in `src/main.rs:68`.
- `WM_DESTROY` posts `PostQuitMessage(0)` in `src/main.rs:73`.
- No Iced exit task or process-level shutdown signal is connected.
- No `Shell_NotifyIconW(NIM_DELETE, ...)` cleanup was found.

Impact:
- Exit can stop the tray/hotkey message loop while leaving the Iced app process alive.
- A stale tray icon can remain until Explorer refreshes.

Fix direction:
- Add an app-wide `EXIT_REQUESTED` atomic or Iced message and return `iced::exit()`.
- Remove the tray icon with `NIM_DELETE` during shutdown.
- Preserve existing left-click toggle and right-click menu behavior.

### P1 - Core UX Issues

#### EUI-004: Empty-query recommendations are implemented but not shown on open

Evidence:
- `AppsProvider.search()` returns recommendations for an empty query at
  `src/providers/apps.rs:83-89`.
- On hotkey, UI clears results instead of searching empty input:
  `src/ui/mod.rs:91-95`.
- `selected_index` is set to `-1` for empty input at `src/ui/mod.rs:46`.

Impact:
- The documented "recommendations on open" behavior does not appear immediately.
- Even when empty recommendations appear after clearing input, Enter cannot activate
  them because selection is `-1`.

Fix direction:
- On `HOTKEY_TRIGGERED`, call `engine.search("")` after refresh and select index 0
  when results exist.
- Preserve the current no-action behavior when there are truly no results.

#### EUI-005: Mouse click selection is advertised but not wired

Evidence:
- README says "Select result: Click or Enter".
- `result_row()` wraps rows in `mouse_area(item)` at `src/ui/mod.rs:203`, but there is
  no `on_press`, `on_release`, or row click message.

Impact:
- Rows look interactive but clicks do nothing.
- Users who do not rely on keyboard cannot launch results.

Fix direction:
- Add `Message::ResultClicked(usize)` and activate that exact result.
- Consider hover/pressed states so clickability is visible.

#### EUI-006: Clipboard history copies only the preview, not the full entry

Evidence:
- Clipboard search truncates the displayed title to 80 chars at
  `src/providers/clipboard.rs:32`.
- Clipboard activation copies `result.title` at `src/providers/clipboard.rs:52`.

Impact:
- Multi-line or long clipboard entries are corrupted when re-copied.
- The UI says clipboard history, but activation does not preserve original content.

Fix direction:
- Carry the full clipboard text as an action payload.
- Keep the 80-character preview only for display.

#### EUI-007: Several documented config fields are not wired into behavior

Evidence:
- Config fields exist at `src/config.rs:6-12`.
- Hotkey is hardcoded as Alt+Space at `src/main.rs:306`.
- Initial window height is hardcoded to 56 at `src/main.rs:268`.
- `debounce_delay_ms` is documented, but `InputChanged` searches immediately at
  `src/ui/mod.rs:43-50`.
- `search_dirs` is documented, but app refresh only scans ProgramData and APPDATA
  Start Menu paths at `src/providers/apps.rs:166-175`.
- `clipboard_max_entries` is documented, but clipboard search hardcodes 20 at
  `src/providers/clipboard.rs:23`.

Impact:
- Users can edit config and see no effect.
- This creates avoidable support/debug confusion.

Fix direction:
- Either wire each field or remove/mark unsupported config from user docs.
- Prioritize hotkey, search dirs, debounce, and clipboard limit.
- Keep defaults exactly as they are.

#### EUI-008: Web search URLs are not encoded

Evidence:
- Web search builds the URL with plain replacement at
  `src/providers/websearch.rs:25`.

Impact:
- Queries with spaces, `&`, `?`, `%`, `#`, non-ASCII characters, or quotes can produce
  broken or unintended URLs.

Fix direction:
- URL-encode the query before substituting `%s`.
- Add tests for spaces and special characters.

#### EUI-009: App refresh and icon extraction run synchronously when opening

Evidence:
- `src/ui/mod.rs:91-92` calls `app.engine.refresh_all()` on hotkey tick.
- `src/providers/apps.rs:175-189` walks Start Menu directories.
- `src/providers/apps.rs:189` calls `cached_icon()`, which can do disk and shell work.

Impact:
- Opening the launcher can stall, especially first run or after cache misses.
- This is high-risk for a launcher because the hotkey must feel instant.

Fix direction:
- Keep a cached app list for immediate display.
- Refresh in a background task and swap results when complete.
- Keep existing icon cache behavior, but avoid blocking the UI thread.

#### EUI-010: Auto-scroll offset likely over-scrolls by the search bar height

Evidence:
- `scroll_to_selected()` uses `SEARCH_BAR_HEIGHT + selected_index * RESULT_HEIGHT`
  at `src/ui/mod.rs:34`.
- The scrollable itself contains only the result list, not the search input.

Impact:
- Arrow navigation can scroll too far and make the selected result feel jumpy or
  partially hidden.

Fix direction:
- Use `selected_index * RESULT_HEIGHT` inside the scrollable content coordinate
  system.
- Clamp to keep the selected row visible without excessive movement.

#### EUI-011: Successful copy actions have no visible confirmation

Evidence:
- Calculator and emoji providers copy to clipboard at
  `src/providers/calculator.rs:46` and `src/providers/emoji.rs:73`.
- UI discards activation result at `src/ui/mod.rs:83`.

Impact:
- For calculator and emoji, "Enter" appears to close the launcher with no confirmation.
- Users must paste somewhere else to verify success.

Fix direction:
- Add a brief "Copied" status before hiding, or a notification-style confirmation.
- Keep instant close for app/web launches if preferred.

#### EUI-012: Icon behavior is inconsistent with docs and can preserve wrong icons

Evidence:
- Current code requests `shell_item_icon(icon_path, 32)` at
  `src/providers/apps.rs:329`.
- UI renders icons at 16 px via `src/theme.rs:23`.
- `ELEMENT_STATE.md` says 96x96 COM icons; README/CHANGELOG/AGENTS still describe
  older 32x32/binary `.lnk` parser behavior.
- If target resolution fails, `cached_icon()` falls back to the `.lnk` path at
  `src/providers/apps.rs:322-327` and caches the returned image.

Impact:
- Users can see generic shortcut icons instead of app icons.
- Bad cached icons can persist until cache deletion.
- Agent docs disagree about the active icon pipeline.

Fix direction:
- Decide target extraction size and UI render size.
- Version the icon cache or invalidate cache entries when the extraction strategy
  changes.
- Keep a provider fallback glyph for missing icons rather than blank space.

### P2 - Polish, Docs, and Contributor UX

#### EUI-013: Placeholder promises file search before file search exists

Evidence:
- Placeholder says "Search apps, files, or type anything..." at `src/ui/mod.rs:107`.
- File search is listed as a future move in `ELEMENT_STATE.md`.

Impact:
- Users expect file results and get web/app/calc/emoji/clipboard only.

Fix direction:
- Change placeholder until file provider lands, or implement the file provider.

#### EUI-014: Long titles/subtitles can degrade row readability

Evidence:
- `result_row()` creates plain text nodes for title/subtitle at
  `src/ui/mod.rs:161-169`.
- Web search subtitle is the full URL at `src/providers/websearch.rs:25`.

Impact:
- Long URLs, clipboard previews, or app names can make the compact 580 px launcher
  hard to scan.

Fix direction:
- Truncate or elide subtitles.
- Prefer useful action labels over raw full URLs in subtitles.

#### EUI-015: Window targeting and positioning are fragile

Evidence:
- `FindWindowW("Element")` is used at `src/main.rs:43`, `src/main.rs:368`,
  `src/main.rs:392`, and `src/main.rs:400`.
- Positioning uses primary monitor metrics at `src/main.rs:377-381`.
- Tray left-click show path uses `ShowWindow(h, 5)` at `src/main.rs:50` but does not
  share the hotkey centering path.

Impact:
- Another window named Element can be targeted.
- Multi-monitor/DPI behavior can be wrong.
- Tray toggle and hotkey toggle can position/focus differently.

Fix direction:
- Store the Iced window id/HWND when available.
- Verify PID if `FindWindowW` remains temporarily.
- Position relative to active monitor/cursor.

#### EUI-016: Documentation has stale references and contradictions

Evidence:
- `CONTRIBUTING.md:11`, `CONTRIBUTING.md:26`, and `CONTRIBUTING.md:43` reference
  `ARCHITECTURE.md`, but `AGENTS.md` says that content now lives in `AGENTS.md`.
- `SECURITY.md:29`, `.github/PULL_REQUEST_TEMPLATE.md:17`, and
  `.github/ISSUE_TEMPLATE/feature_request.md:16` also reference `ARCHITECTURE.md`.
- README and CHANGELOG still describe the old binary `.lnk` icon parser.

Impact:
- New contributors and future agents can follow stale instructions.
- This increases the chance of someone reintroducing old icon code.

Fix direction:
- Replace `ARCHITECTURE.md` references with `AGENTS.md`.
- Update README/CHANGELOG/AGENTS icon pipeline text to match actual code.

#### EUI-017: Invalid config is silently replaced with defaults

Evidence:
- `Config::load()` falls back to `Config::default()` and saves it at
  `src/config.rs:47-52`.
- `Config::save()` ignores write errors at `src/config.rs:59-62`.

Impact:
- A user typo can silently wipe their config.
- Troubleshooting config issues becomes harder.

Fix direction:
- On parse failure, preserve the bad file as a backup and report/log the parse error.
- Keep first-run default creation behavior.

#### EUI-018: Provider priority exists but is not used in final sort

Evidence:
- The provider trait exposes `priority()`.
- `ProviderRegistry.search()` sorts by score and title only.

Impact:
- Ranking rules are more fragile because every provider must encode ordering through
  raw scores.
- Future providers may behave unpredictably on ties.

Fix direction:
- Either remove priority from the trait until needed or include it in result ranking.
- If included, keep current scores as the primary ordering to avoid regressions.

## Suggested Fix Order

1. Fix activation correctness:
   - Carry stable action payloads.
   - Launch the selected app path, not a title lookup.
   - Return real activation errors.
   - Hide only after success.
2. Fix recommendations and selection:
   - Show empty-query recommendations on open.
   - Select the first recommendation when results exist.
   - Wire row click activation.
3. Fix clipboard/web data correctness:
   - Copy full clipboard entries.
   - URL-encode web search queries.
4. Wire or de-document config:
   - Hotkey, debounce, search dirs, clipboard max entries, window height.
5. Move heavy refresh/icon work off the UI path.
6. Clean docs after behavior is fixed so the docs describe reality.

## Notes For Next Agent

- Do not revert the existing dirty files unless the user explicitly asks. They were
  already modified before this audit.
- The current baseline passes `cargo test` and `cargo clippy -- -D warnings`.
- Be careful with Windows-specific behavior. Unit tests do not prove that
  `ShellExecuteW`, focus, tray exit, or Start Menu shortcut handling works in a real
  desktop session.
- The safest structural fix is to extend `SearchResult` with an action payload or
  stable id, because that resolves wrong app launch, clipboard truncation, and future
  provider actions in one model change.

---

## Implementation Pass: Phase 11

Date: 2026-07-28

The user asked for the main issues from this audit to be fixed while preserving working
behavior. This pass implemented the highest-impact reliability and UI corrections.

### Current Codebase Map

```
element/
├── src/
│   ├── main.rs              # Win32 hotkey/tray bridge and Iced startup
│   ├── app.rs               # SearchResult contract and SearchEngine
│   ├── providers/
│   │   ├── apps.rs          # Shortcut resolution, direct .exe launch, icon cache
│   │   ├── clipboard.rs     # Full-text clipboard activation
│   │   ├── calculator.rs    # Clipboard result activation
│   │   ├── emoji.rs         # Exact emoji activation
│   │   └── websearch.rs     # URL-encoded browser action
│   ├── theme.rs             # UI status color and sizing tokens
│   └── ui/mod.rs            # Keyboard, pointer, selection, and feedback behavior
├── AGENTS.md                # Agent contract and implementation guidance
├── ELEMENT_STATE.md         # Current architecture and known limitations
├── README.md                # User-facing behavior and configuration status
└── codex.md                 # This audit and implementation handoff log
```

### Changes Made

1. Exact result actions
   - Added `SearchResult.action` in `src/app.rs`.
   - Apps now carry their resolved executable path, clipboard results carry full text,
     calculator results carry the computed value, emoji results carry the exact emoji,
     and web results carry the final encoded URL.
   - This removes all activation-by-visible-title behavior. Duplicate shortcut names
     can no longer launch whichever matching title was indexed first.

2. Direct executable launch
   - `AppsProvider::refresh()` resolves every Start Menu `.lnk` before indexing it.
   - Only an existing direct `.exe` target is accepted. `activate()` starts that
     executable through `std::process::Command`, using its parent directory as the
     working directory.
   - A failed start returns an error and is not recorded as a successful launch.
   - Existing title-based frecency entries are still considered while users transition
     to the stable executable-path frecency key.

3. Correct shortcut and icon handling
   - Fixed the `IShellLink::GetPath` vtable slot from 5 to 3. The old slot targeted a
     different method and was the likely cause of unreliable shortcut resolution.
   - Reads `IShellLink::GetIconLocation` from slot 16.
   - Initializes COM around each resolver and shell-image helper call, then balances it
     with `CoUninitialize` only when this call initialized COM.
   - Uses a valid shortcut `.ico` file first. If no `.ico` is specified, extracts the
     resolved executable's embedded icon at 32 px.
   - Versioned cache filenames as `v2-*.png`, leaving stale shortcut-icon cache entries
     unused.
   - Fixed the GDI bitmap copy so pixels are copied before the DIB is deleted.

4. UI and interaction fixes
   - Opening the launcher now shows empty-query app recommendations and selects the
     first result when available.
   - Enter and clicking a result activate the exact selected result.
   - The launcher hides only after a non-copy action succeeds. Failed activation keeps
     it open and shows a concise error message.
   - Calculator, emoji, and clipboard actions keep the launcher open briefly with a
     `Copied to clipboard` confirmation.
   - Fixed list scroll coordinates so arrow navigation is measured from the result list,
     not from the search bar.
   - Updated the placeholder so it no longer promises file search.

5. Configuration and lifecycle corrections
   - `search_dirs` is now appended to the Start Menu directories and scanned for `.lnk`
     applications.
   - `clipboard_max_entries` controls how many history rows the clipboard provider shows.
   - Tray Exit now raises `EXIT_REQUESTED`, and the Iced update loop returns `iced::exit()`
     so the whole process terminates rather than only the background tray thread.

6. Cleanup and documentation
   - Ran `cargo fmt` over the repository. This formatted pre-existing Rust files too;
     no unrelated logic was changed.
   - Updated `AGENTS.md`, `ELEMENT_STATE.md`, and `README.md` to describe the actual
     action and icon pipeline.
   - Replaced stale `ARCHITECTURE.md` references in contributor and GitHub issue/PR
     documents with `AGENTS.md`.

### Verification After Implementation

- `cargo fmt`: passed.
- `cargo test`: passed, 26 tests.
- `cargo clippy -- -D warnings`: passed.
- No interactive Windows desktop test was run from the agent environment. The first
  manual smoke test should cover: a classic `.lnk` to an `.exe`, a shortcut with a
  custom `.ico`, duplicate app display names, calculator/emoji/clipboard confirmation,
  failed executable activation, and tray Exit.

### Remaining Intentional Limitations

- `Alt+Space` remains the actual registered hotkey; the documented configurable hotkey
  field is marked reserved instead of pretending it is wired.
- Refreshing the Start Menu index still runs synchronously when the overlay opens. A
  future async refresh should preserve the current list until fresh results are ready.
- Shortcuts that do not resolve to an existing direct `.exe` (for example, some UWP or
  argument-dependent shortcuts) are intentionally omitted to honor the direct-exe
  launch rule.

---

## Implementation Pass: Phase 12

Date: 2026-07-28

The user reported that mouse activation worked but Enter did not, and that typing after
Alt+Space could still reach the previously active application. This pass corrected focus
ownership and added the requested brand-kit visual treatment.

### Changes Made

1. `src/main.rs`
   - Calls `SetForegroundWindow` after showing Element from both Alt+Space and the tray
     left-click path. This gives the Element window keyboard ownership before the Iced
     update loop requests text-input focus.

2. `src/ui/mod.rs`
   - Added `Message::Submit` and connected `TextInput::on_submit(Message::Submit)`.
     Enter now activates the selected result through the text input itself rather than
     relying only on the global keyboard subscription.
   - Kept Escape in the global key handler; with foreground focus restored it hides the
     launcher reliably.
   - Embedded `brandkit/app-icons/icon-64.png` with `include_bytes!`, so the launcher
     logo is bundled into the executable rather than loaded from a fragile runtime path.
   - Added a compact header with the Element mark and name; kept the result list dense.

3. `src/theme.rs`
   - Switched the launcher shell to the documented brand-kit Ink, Primary, Core White,
     and Text Grey palette.
   - Improved visual hierarchy with a restrained border, 8 px outer radius, 6 px input
     radius, a 24 px brand mark, and 48 px result rows. No gradients or decorative
     effects were introduced.

4. Documentation
   - Updated `ELEMENT_STATE.md` and `AGENTS.md` for the focus and branded UI behavior.

### Verification After Phase 12

- `cargo test`: passed, 26 tests.
- `cargo fmt --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- Native focus behavior still needs a brief real-desktop smoke test because unit tests
  cannot prove Windows foreground-window policy in a user session.

---

## Implementation Pass: Phase 13

Date: 2026-07-28

The user reported two regressions: an application could appear twice after launch and
reopen because the local frecency database retained both an old display-name key and a
new executable-path key; later Alt+Space could also leave the launcher hidden.

### Changes Made

1. `src/providers/apps.rs`
   - Empty-query recommendations now deduplicate by `SearchResult.action`, which is
     the direct executable path. Legacy title keys and modern executable-path keys can
     therefore contribute frecency to one result without rendering it twice.
   - The Start Menu scan now tracks case-insensitive executable paths and indexes only
     the first shortcut for each target executable. Duplicate `.lnk` files can no
     longer produce duplicate search results.
   - Added a regression test that records both kinds of frecency key and verifies one
     recommendation with the executable action.

2. `src/main.rs`
   - Replaced the show/hide decision based solely on `VISIBLE` with `IsWindowVisible`
     and removed the now-stale `VISIBLE` atomic. The real window state now decides
     whether a hotkey or tray click should show the launcher.
   - Consolidated tray and hotkey opening in `show_launcher()`: it clears an obsolete
     hide request, restores the window, centers and raises it with `SetWindowPos`,
     brings it to the foreground, then signals Iced to refresh and focus the input.
   - Consolidated hiding in `hide_launcher()` and clears a pending focus signal when a
     user explicitly hides the overlay.

3. Documentation
   - Updated `ELEMENT_STATE.md` with the actual visibility, focus, Enter, and
     duplicate-prevention flow.

### Verification After Phase 13

- `cargo fmt --check`: passed.
- `cargo test`: passed, 27 tests.
- `cargo clippy -- -D warnings`: passed.
- Native Alt+Space behavior still requires a desktop smoke test because Windows focus
  policy cannot be proven by unit tests.

---

## Implementation Pass: Phase 14

Date: 2026-07-28

### Root Cause

Launching the debug build created an `element.exe` process with no top-level window
after more than twenty seconds. `SearchEngine::new()` synchronously called
`AppsProvider::refresh()`, which walked Start Menu directories and extracted COM icons
before Iced had created its native window. `FindWindowW("Element")` therefore had no
window to show when Alt+Space was pressed.

### Changes Made

1. `src/providers/apps.rs`
   - Moved Start Menu scanning into a named `element-app-index` worker thread.
   - Retains the previous index while a refresh runs and prevents overlapping scans.
   - Publishes an atomic revision only after it has safely swapped in a complete index.

2. `src/providers/mod.rs`, `src/registry.rs`, `src/app.rs`, and `src/ui/mod.rs`
   - Added a default `SearchProvider::revision()` contract and an aggregate registry
     revision.
   - The UI records the revision used for its current results. Its existing 30 ms tick
     re-runs the current query exactly when the worker publishes new app data.
   - Opening the launcher now requests a refresh without blocking window creation,
     foregrounding, or input focus.

3. Documentation
   - Updated `AGENTS.md` and `ELEMENT_STATE.md` for the background index lifecycle.

### Verification After Phase 14

- `cargo fmt --check`: passed.
- `cargo test`: passed, 27 tests.
- `cargo clippy -- -D warnings`: passed.
- The old diagnostic `element.exe` is stuck before Iced starts, so its tray Exit cannot
  complete shutdown. It needs a one-time force-close before a new `cargo run` can
  replace that debug executable.
