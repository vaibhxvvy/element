# Contributing to Element

Thanks for considering it. A heads up before you dive in: large parts of this codebase
were built with AI assistance. That's not a secret and it's not a problem — but it means
the usual "read the code, the code is the truth" approach is extra important here, since
comments or naming can occasionally be more confident than the logic backing them. If
something looks wrong, it might genuinely be wrong. Flag it.

## Before you start

- Read [`AGENTS.md`](./AGENTS.md) — it explains the provider model and why
  certain "obvious" bigger ideas (plugins, daemon, multi-crate) aren't there yet.
- For anything bigger than a small bugfix, open an issue first. Saves you writing code
  that doesn't fit the current sequencing.

## Setup

```bash
git clone https://github.com/vaibhxvvy/element.git
cd element
cargo build
cargo test
```

Requirements: Rust 1.77+, Windows (Element is Windows-only for now — see
`AGENTS.md` for why cross-platform isn't in scope yet).

## Before opening a PR

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs all three on every PR — a red check will block review, so it's worth catching
locally first.

## Adding a feature

Most new capabilities (a new search source, a new prefix command) should be a new
`SearchProvider`, not a change to `SearchEngine` or the UI. See "Adding a new provider"
in `AGENTS.md` for the pattern. If your idea doesn't fit that shape, say so in the
issue/PR — it might mean the trait needs to grow, which is a valid conversation.

## Reporting bugs

Use the bug report template when opening an issue. The more reproducible, the faster it
gets fixed — "Alt+Space sometimes doesn't focus the search bar" is much harder to act on
than exact repro steps + your Windows build + what else was running.

## Commit style

Small, focused commits. `fix: `, `feat: `, `refactor: `, `docs: ` prefixes are appreciated
but not enforced. One logical change per commit — makes bisecting regressions possible,
which matters more here than in a codebase with heavier test coverage.

## Code of conduct

See [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md).
