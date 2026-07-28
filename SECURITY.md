# Security Policy

Element runs with a global hotkey, reads your installed applications, and stores
clipboard history locally in SQLite. Treat anything touching those paths as
security-sensitive.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Instead, report privately via GitHub's "Report a vulnerability" button under this repo's
Security tab (Security → Advisories → Report a vulnerability), or email the maintainer
directly — see the contact listed on the repo/profile.

Include:
- A description of the issue and its potential impact
- Steps to reproduce
- Affected version (`element --version` or the `Cargo.toml` version)

## What's in scope

- Arbitrary code execution via crafted input (search queries, clipboard content, config file)
- Local privilege escalation
- Clipboard/history data exposure beyond the local SQLite store
- Anything that lets a malicious app/shortcut on the system manipulate Element's behavior

## What's out of scope (for now)

- Third-party plugin sandboxing — there is no plugin system yet, see `AGENTS.md`
- Network-facing attack surface — Element does not currently make outbound requests
  except opening the configured web-search URL in the user's default browser

## Response

This is a young, community-maintained project without a dedicated security team — please
be patient. Confirmed vulnerabilities will get a fix and a note in the release changelog;
credit given if you'd like it.
