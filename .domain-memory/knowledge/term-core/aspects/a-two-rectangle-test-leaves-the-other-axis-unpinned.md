---
id: asp_a-two-rectangle-test-leaves-the-other-axis-unpinned_tcef69
slug: a-two-rectangle-test-leaves-the-other-axis-unpinned
name: >-
  A helper test that varies rectangles on ONE axis pins that axis only — 899
  tests green while a horizontal split's right-hand console marked nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:05:00.906Z'
updated_at: '2026-09-18T18:05:00.906Z'
last_verified: '2026-09-18T18:05:00.906Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - 'Pane::cell_under'
  - 'Pane::cell_to_extend_to'
  - 'Pane::clamped_cell'
content_hashes:
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
tags:
  - tests
  - mutation
  - pointer
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

Pinning «the helper counts from the rectangle it is given» with two rectangles that differ on ONE axis leaves the OTHER axis completely unpinned, and the rest of the suite does not cover it either. The delivery's test `a_cell_is_counted_from_the_rectangle_the_helper_is_given` hands the same point to two rectangles one cell apart VERTICALLY and requires the rows to come out one apart. It catches `point - Pos2::ZERO` in place of `point - console.min` (the whole offset) and the row half on its own — but mutating only `let column = offset.x / metrics.width()` to `point.x / metrics.width()` in `cell_under` and `cell_to_extend_to` left ALL 899 tests of the crate green, while the right-hand console of a horizontal split marked nothing at all: `two_wide_panes()` puts it 402 points in, so every press there resolves to a column its grid does not have and `cell_under` returns `None`. Why the suite was blind: every pointer test in the crate drives `panes()[0]`, whose rectangle starts at the window's origin, so `point.x` and `point.x - console.min.x` are the same number — a control case degenerate by construction. Closed by a test that drives the SECOND pane through `drag_to_select`, marking the left one first as a control inside the same run.

## Where it lives

`crates/app/src/lib.rs` (`Pane::cell_under`, `Pane::cell_to_extend_to`, `Pane::clamped_cell`); the closing test `the_console_that_is_not_at_the_windows_edge_marks_from_its_own_left`.
