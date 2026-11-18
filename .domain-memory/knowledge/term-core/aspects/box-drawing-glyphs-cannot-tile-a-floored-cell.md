---
id: asp_box-drawing-glyphs-cannot-tile-a-floored-cell_wqyrv8
slug: box-drawing-glyphs-cannot-tile-a-floored-cell
name: >-
  A box-drawing line drawn from the font cannot tile under the floored cell: the
  overlap D34 counted on is the defect
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T09:39:42.639Z'
updated_at: '2026-11-18T09:39:42.639Z'
last_verified: '2026-11-18T09:39:42.639Z'
file_paths:
  - crates/app/src/painter/box_drawing.rs
  - crates/app/src/painter.rs
  - crates/app/src/painter/blocks.rs
symbols:
  - 'box_drawing::runs'
  - 'box_drawing::paint'
  - on_whole_pixels
content_hashes:
  crates/app/src/painter.rs: 'sha256:000abc76187b1352113306380e2f361d01fbb5163e4e9b1c4f63ff5d79b51e23'
  crates/app/src/painter/blocks.rs: 'sha256:6598c8ac747d2b67702b8438c9953f61a756bb20ccf809b070651f6986084717'
tags:
  - painter
  - box-drawing
  - pixels
  - D34
  - D35
feature_id: feat_term-core_nx0g1r
---

## What it does

**D34 floors the cell so it is never wider than the glyph's advance, which closed the gap in a
run of `─`. D35 then left the box drawing range to the font, reasoning that consecutive glyphs
«overlap by that fraction instead of parting». The overlap is the defect.** Measured in #103
(November 2026) by rasterizing the tessellated glyph quads from the font atlas, at ppp 1, 1.25,
1.5, 5/3 and 2, with Hack and Ubuntu Sans Mono:

- **A bead at every seam.** Each glyph's raster is 1 px (Ubuntu Sans Mono) or 2 px (Hack) wider
  than its whole-pixel cell — quad 8 vs cell 7, and 10 vs 8, at ppp 1. The overlapping pixels are
  antialiased ends composited twice: `0.24 -> 0.34` in an edge row.
- **A stroke that straddles rows.** epaint rasterizes the stroke at the font's fractional height
  and thickness, so it covers 2-3 partly inked rows. The bitmap itself sits on whole pixels, so the
  blur is inside the raster: no rounding of the origin removes it.

Asserting positions (cell <= advance, origins on whole pixels) cannot see either: the assertions
hold and the line still does not read as one. **What closes it is asserting pixels** — each device
pixel inked once, the same set of rows from the first cell to the last, and a closed frame's ink
and holes counted as connected pieces.

**Three traps the desktop showed and the first tests did not**, all in `painter::box_drawing`:

1. A one-pixel arc made of the pixels its centre line crosses is two pixels touching only at a
   corner, and reads as a gap in the box.
2. Antialiasing that arc leaves its pixels at half ink beside lines at full ink: it reads as a gap
   too. What held: ink whole any pixel whose estimated coverage is at least a third, so every step
   shares an edge, and end the curve a pixel inside the cell.
3. A single line crossing a double (`╪` `╫`) must run through both lines. Stopping at the double's
   near line is right only at a tee (`╟` `╤`); written as one equality between two booleans, the
   rule broke the crossing and no edge-of-cell test saw it. Counting the channels between a
   double's two lines as holes in a grid is what catches it.

## Where it lives

`crates/app/src/painter/box_drawing.rs` (table, mask, cache, `paint`), dispatched from
`paint_character` in `crates/app/src/painter.rs`. Decision: D35 and its November 2026 amendment
in `docs/02-decisions.md`. Tests: `crates/app/src/painter/box_drawing/tests.rs`.
