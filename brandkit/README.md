# Element — Brand Kit

Fast local search, notes & command-palette launcher for Windows + Linux.

All artwork here is generated directly from your original logo file — the
three-layer diamond stack (cream / graphite / orange) — pixel-for-pixel.
Nothing was redrawn or regenerated; only resized, cropped, or recolored
where noted below (solid-color glyphs, palette swatch).

## Palette
| Token | Hex | Use |
|---|---|---|
| Top / Cream | `#F5F0EC` | top layer, light text on dark backgrounds |
| Middle / Graphite | `#454444` | middle layer |
| Accent / Orange | `#E8540C` | bottom layer, accent, buttons, links |
| Background / Black | `#000000` | app background, default icon bg, dark surfaces |

See `misc/color-palette.png` for swatches.

## Folders
- `app-icons/` — square icon, 16→1024px, black background (default brand look). Use for exe icon source, About screen, splash.
- `windows/` — `element.ico` (multi-res), `element.rc` (version-info resource script), MSIX/winget tile logos (Square44/150/310, Wide310x150, SmallTile, StoreLogo).
- `linux/` — hicolor icon theme tree (`16x16` → `512x512`, transparent), drop straight into `/usr/share/icons/hicolor/`, plus a starter `element.desktop`.
- `favicon/` — full web favicon set + `site.webmanifest` for the docs/landing site.
- `wordmark/` — full-color mark, solid black/white/orange glyph versions, and horizontal "Element" lockups for dark and light backgrounds.
- `social/` — OG image (1200×630), GitHub social preview (1280×640), Twitter/X header (1500×500).
- `white/` — **exact mirror of `app-icons/`, `windows/`, `favicon/`, and `social/`**, same filenames, background swapped to white for light-theme placements (light docs sites, light app shells, print).

## Usage rules
- `app-icons/`, `windows/`, and `favicon/` ship with a black background baked in — that's the logo as designed. Use the `white/` folder's identical filenames wherever you need the light-background version instead of recoloring anything yourself.
- Solid-color marks (`wordmark/mark-solid-*.png`) are for single-color contexts: tray/menu-bar icon, letterhead, engraved/stamped use, tiny favicons where the 3-color version won't read clearly.
- Keep clear-space around the mark roughly equal to the width of one diamond layer.
- Don't recolor `mark-full-color.png` — recolor only the solid variants.

## Next up
- Vector (SVG) master of the mark for the docs site / README badge (current set is raster).
- App tray icon states (idle / active / notification dot) once the UI shell is in place.
