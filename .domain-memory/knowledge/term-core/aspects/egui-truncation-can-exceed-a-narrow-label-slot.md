---
id: asp_egui-truncation-can-exceed-a-narrow-label-slot_34766e
slug: egui-truncation-can-exceed-a-narrow-label-slot
name: Egui truncation can exceed a narrow label slot
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T21:53:04.084Z'
updated_at: '2026-11-22T21:53:04.084Z'
last_verified: '2026-11-22T21:53:04.084Z'
file_paths:
  - crates/app/src/viewer/drawer.rs
symbols:
  - tab
  - text
  - painted_tab_content_stays_before_its_close_target
content_hashes: {}
tags:
  - egui
  - painting
  - clipping
  - testing
feature_id: feat_term-core_nx0g1r
---

## What it does

TextWrapping::truncate_at_width is not a hard painting boundary when a slot is narrower than
the ellipsis. In an egui 0.36 probe using default proportional fonts at 12.5 points, requested
widths 0, 2, 6 and 10 all returned a 12.4375-point ellipsis. Using that galley's natural right
edge to place a neighboring count can push it onto a reserved close target. Clip label painting
to its allocated rectangle and place adjacent content from the visible clipped edge. A
geometry-only test cannot see this: inspect actual text/rectangle shapes together with their
clip rectangles.

## Where it lives

Verified on 2026-11-22 while fixing Ivy issue #215. crates/app/src/viewer/drawer.rs, tab(): a
140-point drawer produced a 68-point Changes tab; the count chip overlapped close by 2.4375
points before clipping. painted_tab_content_stays_before_its_close_target fails before the
paint correction at drawer widths 100, 140 and 142 and passes afterward; it also witnesses
normal label/count painting at width 300. Reviewed production revision:
52f3fab512ff14cb0ecc2b2dd67533b07616760b.
