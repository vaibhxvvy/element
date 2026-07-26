# Element — Brand Kit

Rust-cast-style launcher / notepad / mind-map / local search for Windows + Linux.

## Palette
| Token | Hex | Use |
|---|---|---|
| Ink / BG | `#08060D` | app background, icon bg, dark surfaces |
| Primary | `#6D4AFA` | accent, buttons, links, tray icon |
| Deep Indigo | `#4D2AC4` | gradients, secondary accent |
| Core White | `#FDFAFE` | glow highlight, primary text on dark |
| Text Grey | `#A7A0BD` | secondary text, taglines |

See `misc/color-palette.png` for swatches.

## Folders
- `app-icons/` — square glow icon, 16→1024px. Use for exe icon source, About screen, splash.
- `windows/` — `element.ico` (multi-res), MSIX/winget tile logos (Square44/150/310, Wide310x150, StoreLogo).
- `linux/` — hicolor icon theme tree (`16x16` → `512x512`) drop straight into `/usr/share/icons/hicolor/`, plus a starter `.desktop` file.
- `favicon/` — full web favicon set + `site.webmanifest` for the docs/landing site.
- `wordmark/` — mark + "element" lockups (dark bg + light bg), and standalone mark in glow / solid-white / solid-black / solid-purple.
- `social/` — OG image (1200×630), GitHub social preview (1280×640), Twitter/X header (1500×500).

## Usage rules
- **Glow mark** (`mark-glow.png`) only ever sits on the ink-black `#08060D` (or pure black) background — it's an additive-light effect, it will look wrong on white.
- **Solid-black / solid-white marks** are for anywhere you need a flat, single-color glyph: tray/menu-bar icon, letterhead, stamped/engraved use, small favicons.
- Keep clear-space around the mark equal to the width of one "spike" of the star.
- Don't recolor the glow version — recolor only the solid variants.
- Wordmark type is Space Grotesk (SemiBold), lowercase, as used in every lockup — keep it lowercase, don't caps-lock the name.

## Next up
- Vector (SVG) master of the mark for the docs site / README badge (current set is raster, generated from the source render).
- App tray icon states (idle / active / notification dot) once UI shell is in place.
