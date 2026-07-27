# Changelog

## v0.8.0 (unreleased)

### Added
- Provider architecture: `SearchProvider` trait + `ProviderRegistry` with `catch_unwind` isolation
- 5 search providers: apps, calculator, emoji, clipboard, websearch
- `ElementError` enum with `thiserror`
- `Registry.rs` — iterates providers, catches panics
- `Theme.rs` — named color/spacing/radius tokens used by ui
- Unit tests: 24 tests covering fuzzy scorer, frecency formula, calculator detection, config round-trip, clipboard
- CI workflow: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build`
- Release workflow: tag-triggered build + portable zip + GitHub Release
- `CHANGELOG.md`

### Changed
- Hotkey: `RegisterHotKey` + `PeekMessageW` replaces `GetAsyncKeyState` polling (zero CPU when idle)
- System tray: `Shell_NotifyIconW` with hidden message window; left-click toggle, right-click Exit
- Icons: extracted at 32×32 (was 16×16), cached as PNG to `~/.element/cache/icons/`
- Icons: `.lnk` binary parser resolves working directory, searches real icon files (PNG/ICO) before falling back to `SHGetFileInfoW`
- Config `window_width` wired to `WINDOW_WIDTH` atomic and initial window size
- `data_dir()` consolidated into `config.rs`, duplicate removed from `database.rs`
- `SearchResult` now carries `provider_id` for registry dispatch
- All inline colors/spacing replaced with `theme.rs` tokens
- Main.rs: unnecessary `unsafe` blocks removed, dead `LoadIconW` wrapper removed
