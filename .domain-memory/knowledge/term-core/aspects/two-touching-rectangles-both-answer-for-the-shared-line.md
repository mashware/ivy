---
id: asp_two-touching-rectangles-both-answer-for-the-shared-line_rm5xvn
slug: two-touching-rectangles-both-answer-for-the-shared-line
name: >-
  Any cut that leaves one rectangle's bottom equal to the next one's top gives
  that line TWO owners — egui::Rect::contains is inclusive on both edges, so
  both answer yes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:05:11.849Z'
updated_at: '2026-09-18T18:05:11.849Z'
last_verified: '2026-09-18T18:05:11.849Z'
file_paths:
  - crates/app/src/panel/pane.rs
  - crates/app/src/header.rs
  - crates/app/src/pointer.rs
symbols:
  - 'PaneChrome::strip_holds'
  - set_the_header_aside
  - cell_under
  - 'DragIntent::from_the_press'
content_hashes:
  crates/app/src/panel/pane.rs: 'sha256:0b26824e52f24226148312b2bfca49e2a3739c7c7992ac9849e907e610349b72'
  crates/app/src/header.rs: 'sha256:7f9288133cd629b55437e55a3e5440f7f15aae05998ba669a91ee583ed4818fb'
  crates/app/src/pointer.rs: 'sha256:78bdcf4b9e74451cde588a56ecbd693850d038e1adb4420f122caf64b7e3e570'
tags:
  - geometry
  - egui
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

A cut with `split_top_bottom_at_y` makes `header.bottom() == console.top()`, contiguity the module's own unit test asserts as «neither overlaps the other». But `egui::Rect::contains` is inclusive on BOTH edges, so a press at exactly that y is BOTH `on_the_strip` (`strip.contains(at)`) and row 0 of the grid (`cell_under`, whose guard is `row >= 0.0`). Whichever branch is checked first wins twice over: on F2.11's strip, a press aimed at the console's first row of text moved the pane instead, and a program that had asked for the mouse never learned about a click on its own first row. This is NOT the interaction-tie already on file (`a-drag-that-lasts-several-frames-in-egui`, point 5: `interact_radius` widens the candidate set and the LAST registered widget wins) — it needs no widget and no radius: it is plain containment arithmetic over two rectangles produced by one split, and it bites wherever a point is classified by asking each of them in turn. The rule: when two rectangles of one split classify a point, exactly ONE of them tests the shared edge, and that test is written in ONE place, not at each call site — here `PaneChrome::strip_holds` (`strip.contains(at) && at.y < strip.bottom()`), read by both the press routing and the drag's door. Pinned by an effect (press exactly on the shared line marks text, moves nothing) with its own premise asserted (the point really is `console.top()` == `strip.bottom()`), not by the arithmetic — found by asking «what does each rectangle answer for the pixels on its border» before asking anything about the gesture, which is how a silent-failure reviewer caught it after three other reviewers and twelve mutations missed it.

## Where it lives

`crates/app/src/panel/pane.rs` (`PaneChrome::strip_holds`), `crates/app/src/header.rs` (`set_the_header_aside`), `crates/app/src/pointer.rs` (`cell_under`, `DragIntent::from_the_press`).
