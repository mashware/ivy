---
id: asp_epaint-rounds-each-glyph-origin-on-its-own_m6j86e
slug: epaint-rounds-each-glyph-origin-on-its-own
name: >-
  epaint rounds glyph origins and rectangles separately; shared background edges
  must use identical coordinates
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T12:51:17.542Z'
updated_at: '2026-11-13T14:01:39.609Z'
last_verified: '2026-11-13T14:01:39.609Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/painter/blocks.rs
  - crates/app/src/painter/background_tests.rs
symbols:
  - on_whole_pixels
  - 'CellMetrics::measure'
  - 'CellMetrics::cell'
  - paint_a_block
  - at_least_a_pixel
content_hashes:
  crates/app/src/painter.rs: 'sha256:000abc76187b1352113306380e2f361d01fbb5163e4e9b1c4f63ff5d79b51e23'
  crates/app/src/painter/blocks.rs: 'sha256:6598c8ac747d2b67702b8438c9953f61a756bb20ccf809b070651f6986084717'
  crates/app/src/painter/background_tests.rs: 'sha256:0db809fb49363e5bfacdf07985025131dd9d5c412b0ef71cfe12573afd973df6'
tags:
  - egui
  - epaint
  - painting
  - grid
  - measurement
  - D34
  - selection
feature_id: feat_term-core_nx0g1r
---

## What it does

**epaint 0.36.1 rounds every glyph's origin AND every filled rectangle to the physical pixel grid by itself, and knowing which of the two matters changes what a grid has to fix.**

Read in `tessellator.rs` and then **verified by tessellating**, which is the part that mattered:

- `round_rects_to_pixels` defaults to `true` (`tessellator.rs:736`) and its own doc says it is what «makes filled rectangles tile perfectly (without feathering)». **Corrected in November 2026:** pixel snapping tiles adjacent backgrounds only when their supplied shared edges agree. Independently calculated f32 edges can straddle a rounding threshold and leave a whole pixel uncovered.
- `round_text_to_pixels` defaults to `true` (`tessellator.rs:734`, applied at `:2024`): `galley_pos.round_to_pixels(ppp)`. A painter that queues **one glyph per cell** therefore has **each origin rounded separately**.

**The consequence, measured.** With a cell step of `8.232` px the origins `0, 8.232, 16.464, 24.696…` round to `0, 8, 16, 25, 33, 41` — steps of `8, 8, **9**, 8, 8`. The ninth pixel is a column of nothing between two glyphs about eight wide, and it returns every time the accumulated fraction crosses a half. That is what «a run of `─` opens a gap every few characters» and «the logo drawn out of block characters is crossed by hairlines» actually are.

**The pane origin need not be snapped for glyph placement.** The original whole-step example from a corner at `101.37` still gives evenly spaced glyph origins. Rectangle tiling has an additional finite-precision requirement: both sides of a shared boundary must use the same expression, even when the physical cell step is whole.

### How to observe it, and the trap

**A test reading `output.shapes` sees the coordinates the painter ASKED FOR**, which is the one place this defect never is. It has to tessellate: `ctx.tessellate(output.shapes.clone(), ppp)`, then `mesh.vertices.chunks_exact(4)` — each glyph is a quad, queued in paint order, so `quad[0].pos.x` is its left edge. The first guard written for this change read `output.shapes` and passed for the wrong reason.

### Two more numbers from the same reading

- **Feathering is 1.0 physical pixel**, centred on the edge. An edge **on** a pixel boundary has coverage 1.0 at the centre of the pixel just inside and 0.0 at the one just outside, so no pixel ends up half-painted. Off the boundary the same ramp straddles a centre — at `x = 8.232` that is about **17.8 %** of what is behind showing through.
- **A rectangle thinner than `0.9` of a feather takes the `thin_line` branch** (`tessellator.rs:1008`), which draws **no end caps** — its own `TODO` says so. Any synthesised sliver has to be at least one whole device pixel to stay out of it.

### Corrected by the reopened selection-background investigation (November 2026)

`(origin + column * width) + width` and `origin + (column + 1) * width` can differ in f32 even with a whole-device-pixel width. At a half-pixel tie epaint rounds them to different device pixels. Actual egui tessellation of two identical blank fills reproduces a 1px gap at ppp 1.5 (width 7/1.5, origin 1, columns 5/6: right 43px versus next-left 44px) and ppp 1.75 (width 9/1.75, origin 2, columns 11/12: 111px versus 112px). Both cases fail with antialiasing on and off; integral-scale and off-tie-origin controls tile. Glyph shaping, ANSI color differences and font choice are unnecessary to this reproduction.

The production-painter regression observes final background meshes, not copied arithmetic. It is red before matching shared edge expressions and green after; separately restoring the old horizontal or vertical expression turns it red again. Selection colors and glyph near origins stay unchanged. The earlier glyph repair in PR 64 left this background arithmetic intact. Exact attribution of every original screenshot crack requires its effective scale/origin; no such desktop match was claimed by the geometry proof.

Evidence: reopened Ivy issue 58, D34 amendment, `crates/app/src/painter/background_tests.rs` (production tessellation regression and both-axis mutation evidence).

## Where it lives

`crates/app/src/painter.rs` — whole-pixel cell step and shared endpoint geometry. `crates/app/src/painter/background_tests.rs` — actual production painting/final tessellation at fractional scales. `crates/app/src/painter/blocks.rs` — synthesized block ranges. Decisions D34 and D35. The November correction comes from real egui0.36.1 tessellation and independent x/y regression mutations, not shape-queue inspection.

This remains an example of [[someone-elses-api-is-read-not-assumed]]: verify the final tessellated geometry rather than treating pixel-snapping documentation as proof that independently calculated inputs agree.
