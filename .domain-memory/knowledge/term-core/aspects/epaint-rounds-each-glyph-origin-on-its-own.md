---
id: asp_epaint-rounds-each-glyph-origin-on-its-own_m6j86e
slug: epaint-rounds-each-glyph-origin-on-its-own
name: >-
  epaint rounds every glyph origin on its own, so a fractional cell step opens a
  column of nothing — and the filled rectangles were never the problem
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T12:51:17.542Z'
updated_at: '2026-09-10T12:51:17.542Z'
last_verified: '2026-09-10T12:51:17.542Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/painter/blocks.rs
symbols:
  - on_whole_pixels
  - 'CellMetrics::measure'
  - paint_a_block
  - at_least_a_pixel
content_hashes:
  crates/app/src/painter.rs: 'sha256:8ffdf3a61b18c3a0a9c7c55f78d2b7591595b8c60b3b4259875a66477f3a697d'
  crates/app/src/painter/blocks.rs: 'sha256:6598c8ac747d2b67702b8438c9953f61a756bb20ccf809b070651f6986084717'
tags:
  - egui
  - epaint
  - painting
  - grid
  - measurement
  - D34
feature_id: feat_term-core_nx0g1r
---

## What it does

**epaint 0.36.1 rounds every glyph's origin AND every filled rectangle to the physical pixel grid by itself, and knowing which of the two matters changes what a grid has to fix.**

Read in `tessellator.rs` and then **verified by tessellating**, which is the part that mattered:

- `round_rects_to_pixels` defaults to `true` (`tessellator.rs:736`) and its own doc says it is what «makes filled rectangles tile perfectly (without feathering)». **Two contiguous cells of different background have always shared a rounded boundary.** A seam between backgrounds is not a thing here, however fractional the cell.
- `round_text_to_pixels` defaults to `true` (`tessellator.rs:734`, applied at `:2024`): `galley_pos.round_to_pixels(ppp)`. A painter that queues **one glyph per cell** therefore has **each origin rounded separately**.

**The consequence, measured.** With a cell step of `8.232` px the origins `0, 8.232, 16.464, 24.696…` round to `0, 8, 16, 25, 33, 41` — steps of `8, 8, **9**, 8, 8`. The ninth pixel is a column of nothing between two glyphs about eight wide, and it returns every time the accumulated fraction crosses a half. That is what «a run of `─` opens a gap every few characters» and «the logo drawn out of block characters is crossed by hairlines» actually are.

**And the pane's corner does not matter.** A fraction shared by every cell of the pane survives that rounding as the same offset for all of them: a whole step from a corner at `101.37` gives origins `100, 108, 116, 124, 132, 140`, one step throughout. Snapping the pane's corner was shipped, guarded, and taken back out for having no scenario.

### How to observe it, and the trap

**A test reading `output.shapes` sees the coordinates the painter ASKED FOR**, which is the one place this defect never is. It has to tessellate: `ctx.tessellate(output.shapes.clone(), ppp)`, then `mesh.vertices.chunks_exact(4)` — each glyph is a quad, queued in paint order, so `quad[0].pos.x` is its left edge. The first guard written for this change read `output.shapes` and passed for the wrong reason.

### Two more numbers from the same reading

- **Feathering is 1.0 physical pixel**, centred on the edge. An edge **on** a pixel boundary has coverage 1.0 at the centre of the pixel just inside and 0.0 at the one just outside, so no pixel ends up half-painted. Off the boundary the same ramp straddles a centre — at `x = 8.232` that is about **17.8 %** of what is behind showing through.
- **A rectangle thinner than `0.9` of a feather takes the `thin_line` branch** (`tessellator.rs:1008`), which draws **no end caps** — its own `TODO` says so. Any synthesised sliver has to be at least one whole device pixel to stay out of it.

## Where it lives

`crates/app/src/painter.rs` — `on_whole_pixels` takes the cell **down** to whole device pixels and `CellMetrics::measure` is where the fraction comes from; `crates/app/src/painter/blocks.rs` draws the thirty-two block elements against the cell, with `at_least_a_pixel` keeping a synthesised sliver out of epaint's cap-less `thin_line` branch. The decision is **D34** (the grid on whole pixels) and **D35** (the blocks drawn by the painter). The numbers above come from epaint 0.36.1's `tessellator.rs` in cargo's local registry, and from tessellating in a test rather than reading the shape queue.

This is one more entry for [[someone-elses-api-is-read-not-assumed]]: the mechanism the first version of the record blamed was antialiasing, and the real one is per-glyph rounding.
