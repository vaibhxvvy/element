# Element — Feature Roadmap & Tracker

Every feature is tracked with a checkbox, a plain-language "what it does", a
status, the version it was first built in, and (if stuck) *why* it's stuck.

## Legend

| Field | Meaning |
|-------|---------|
| `- [ ]` | Not built yet (open box) |
| `- [x]` | Built and working (ticked box) |
| **Status** | `Planned` → `Building` → `Done` / `Blocked` |
| **Built in** | The `vX.Y.Z` release that shipped it (filled when **Done**) |
| **Blocked by** | The reason work stopped (only filled when **Blocked**) |

If an item is **Blocked**, the box stays `- [ ]` and the blocker is documented
so it can be picked back up.

---

## 🗝️ Everyday quick actions

The launcher as a control panel — type a thing, it happens.

- [x] **Shut down / restart / sleep / lock** — type `shutdown`, `restart`, `sleep`, `lock`.
  - **Status:** Done | **Built in:** v1.0.0
- [x] **Volume** — `volume 40` sets the volume; `mute` / `volume` shows the current level.
  - **Status:** Done | **Built in:** v1.4.0
- [x] **Screen off** — `screen off` turns the display off without sleeping the PC.
  - **Status:** Done | **Built in:** v1.4.0
- [x] **Timer** — `timer 10` pings you after 10 minutes with a tray notification.
  - **Status:** Done | **Built in:** v1.4.0
- [x] **Strong password** — `password` puts a random 16-char password on your clipboard; `password 24` for length.
  - **Status:** Done | **Built in:** v1.4.0
- [x] **Screenshot** — `screenshot` grabs the whole screen into the clipboard, no extra tool.
  - **Status:** Done | **Built in:** v1.4.0
- [ ] **Wi-Fi toggle** — `wifi on` / `wifi off`.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** Windows radio toggling needs WinRT APIs (a `pip install` is not possible; heavy FFI) — schedule after the WinRT helper layer exists.
- [ ] **Bluetooth toggle** — `bluetooth on` / `bluetooth off`.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** same WinRT dependency as Wi-Fi.
- [ ] **Brightness** — `brightness 50` sets screen brightness.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** needs WinRT/display API; bundled with the Wi-Fi/Bluetooth work.
- [ ] **Weather** — `weather today` shows a 7-day forecast inline.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** needs a small HTTP client + a free weather API key.
- [ ] **Currency** — `100 usd in inr` with live rates (offline fallback to last-known).
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** needs network + rate source; same HTTP client as weather.
- [ ] **Time zones** — `9am in tokyo` converts across time zones.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** needs a time-zone database crate (`iana` data) in the build.
- [ ] **QR code** — `qr <link>` shows a scannable QR of any text/URL.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** needs a small pure-Rust QR encoder crate.

---

## 🔎 Not find within your own apps

Element poking into what's open so you don't have to.

- [ ] **Search inside the focused app** — press a hotkey and type what a button says ("print"); Element clicks it for you.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Find a browser tab by what it's about** — type a topic, Enter switches to the tab (no extension).
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Switch to any open window even when minimized** — type a name, Enter brings it to front.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Window actions** — selected window → "left half", "top", "other monitor".
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —

## 📂 Smarter files

- [ ] **Find by file *content*** — "that doc with the ₹ price" finds files whose text matches.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Preview before opening** — Enter on a file shows a snippet/thumbnail of PDF, image, code.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Path flavors** — one key for normal path, forward-slash (for apps), or the folder path.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** nothing — easy, just not prioritized yet.
- [ ] **Zip / rename / move** directly from a file result.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —

## ⌨️ Snippets that type themselves

- [ ] **Type anywhere** — type `/addr` in any app and the address types itself at the cursor.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** needs the low-level keyboard hook to type into the *focused* app (hook exists already); scoped after v1.4.
- [ ] **Grab snippet from current text** — "make a snippet of the selected text".
  - **Status:** Planned | **Built in:** —

## 🤖 AI layer

- [ ] **AI in the bar** — answer a question without opening a browser.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** staging as the top long-term vision.
- [ ] **Fix the selected text** — select text in any app → summarize/translate/shorten → replaces in place.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** same AI layer.
- [ ] **Screen awareness** — "summarize what's on my screen" / selected window context.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** same AI layer.
- [ ] **Local-first AI** — run on our own GPU so nothing leaves the machine (the privacy sell vs the big three).
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** same AI layer.

## 🎮 Quality of life

- [ ] **Game launcher** — every installed game from Steam/Epic/Game Pass in one list type.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Pomodoro / focus** — `pomodoro 25` starts a focus session with silent notifications.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** buffered on the timer (same notification plumbing).
- [ ] **Lorem ipsum** — `lorem` generates a paragraph for design/text.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Emoji with skin tones** — choose a skin-tone variant quickly.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —

## 🧼 Privacy builder of enterprise

- [ ] **Privacy Vault (wipe)** — one command forgets history, clipboard, snippets, cleaner.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —
- [ ] **Auto-clear clipboard** — history rows disappear after a number of minutes.
  - **Status:** Planned | **Built in:** —
  - **Blocked by:** —

---

## Build log

Track each build session here (date → what got built → version).

| Date | Version | What shipped |
|------|---------|--------------|
| 2026-08-08 | — | Tracker created; Everyday commands : volume, screen off, timer, password, screenshot (in progress) |
| 2026-08-08 | v1.4.0 (unreleased) | Everyday quick actions built: `volume`/`mute`, `screen off`, `timer` (tray notification), `password` (BCrypt random), `screenshot` (virtual desktop → clipboard). 96 tests passing |