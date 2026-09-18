---
id: asp_the-pointer-offsets-from-the-pane-not-the-console_fz4nb6
slug: the-pointer-offsets-from-the-pane-not-the-console
name: >-
  A pointer position becomes a cell by offsetting from the PANE's rectangle, not
  the console's — anything that cuts chrome off the top shifts every selection
  and mouse report by a row
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:04:19.861Z'
updated_at: '2026-09-18T18:04:19.861Z'
last_verified: '2026-09-18T18:04:19.861Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
  - crates/app/src/painter.rs
symbols:
  - cell_under
  - cell_to_extend_to
  - clamped_cell
  - route_the_buttons
  - select_while_dragging
content_hashes:
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
  crates/app/src/pointer.rs: 'sha256:78bdcf4b9e74451cde588a56ecbd693850d038e1adb4420f122caf64b7e3e570'
  crates/app/src/painter.rs: 'sha256:408ca127169047c52bada60076ee7c23c1ec511857cf45ffa5bc38a40649f0ef'
tags:
  - pointer
  - geometry
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

`Window::cell_under` does `point - pane_area.min` (`crates/app/src/lib.rs`), and so do `cell_to_extend_to` and `clamped_cell`, all three called with the pane rect by `route_the_buttons`, `select_while_dragging` and the wheel path in `pointer.rs`. Only the grid's SIZE comes from the console rect (`painter.rs`, `cells_in(panel_split.console.size())`). This works today only because the panel is cut from the RIGHT and the extra columns are clamped away on purpose. Anything that cuts chrome off the TOP of a pane (a header, a title bar) shifts every selection and every mouse report by a row, and makes a press on that chrome resolve to console row 0, reaching a program that asked for the mouse before the chrome's own intent is decided. Found by a design challenger during F2.11's header milestone, not by the three approach advisors who all named the painting path and none the pointer path — fixed there by feeding the pointer helpers the CONSOLE's rectangle instead of the pane's.

## Where it lives

`crates/app/src/lib.rs` (`cell_under`, `cell_to_extend_to`, `clamped_cell`), `crates/app/src/pointer.rs` (`route_the_buttons`, `select_while_dragging`), `crates/app/src/painter.rs` (`cells_in`).
