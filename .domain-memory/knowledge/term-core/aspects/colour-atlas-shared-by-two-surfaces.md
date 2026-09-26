---
id: asp_colour-atlas-shared-by-two-surfaces_461n94
slug: colour-atlas-shared-by-two-surfaces
name: >-
  The colour glyph atlas is shared by the console and the panel: keyed by
  character alone, whichever painted first fixed the other's drawing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T16:08:28.350Z'
updated_at: '2026-09-26T16:08:28.350Z'
last_verified: '2026-09-26T16:08:28.350Z'
file_paths:
  - crates/app/src/fonts/atlas.rs
  - crates/app/src/panel/color.rs
symbols:
  - texture
  - remembers
  - slot
content_hashes: {}
tags:
  - colour-emoji
  - atlas
  - d25
feature_id: feat_term-core_nx0g1r
---

## What it does

Until #178 the atlas behind `Discoverer::color_glyph` remembered a glyph by **character**, with the height as a field that empties everything when it changes, and `max_width` out of the key. With one caller that was right. **With two callers asking the same character at different widths — the console at the cells the engine gives it, the status panel at the room its text engine laid out — the first one fixed the drawing for the second**: `☁️` two cells wide in a console that gives it one, or squeezed into one cell in a panel that laid out two.

**The first fix was wrong and the review said so**: making the panel ask at the console's width needed a second table of character widths (`unicode-width`) to agree with the engine's, and let the drawing spill over the letters beside it wherever the panel's face lays the emoji out narrower. **The fix that held: key the atlas by (character, width)**, and let each surface ask at its own size.

Three consequences, each found by review and each tested:
- A **failure** must be remembered once for every width (stored under width 0), or a character no face paints in colour re-walks the emoji families at each new width.
- The panel's width must be the **sum of the glyphs' advances**, not the distance between positions: epaint snaps each position to the pixel grid on its own, so the distance changes by a pixel with the column and each width is one more atlas entry.
- The height stays out of the key on purpose: one cell for everyone, or a panel asking another would empty the atlas on every frame both paint.

The price is atlas room, not memory: the texture stays 1 MiB, but a character can hold two or three of its ~800 places.

## Where it lives

crates/app/src/fonts/atlas.rs `Glyphs::texture`, `Glyphs::remembers`; crates/app/src/panel/color.rs `slot`; D25 in docs/02-decisiones.md
