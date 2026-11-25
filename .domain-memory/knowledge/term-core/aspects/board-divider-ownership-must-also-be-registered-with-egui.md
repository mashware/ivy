---
id: asp_board-divider-ownership-must-also-be-registered-with-egui_0k6c7n
slug: board-divider-ownership-must-also-be-registered-with-egui
name: Board divider ownership must also be registered with egui
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T10:44:21.965Z'
updated_at: '2026-11-25T10:44:21.965Z'
last_verified: '2026-11-25T10:44:21.965Z'
file_paths:
  - crates/app/src/board/milestones/columns.rs
  - crates/app/src/board/milestones/tests.rs
symbols: []
content_hashes:
  crates/app/src/board/milestones/columns.rs: 'sha256:a97bbd06bc2b6aefea38ca0159828ec5bbc4b5e989f4ad4d0d3ab23fa7f8e1f7'
  crates/app/src/board/milestones/tests.rs: 'sha256:c1937b25a2cbcc1b251c13d10c961a0d50db865d46b777da521d0344dec65cf3'
tags:
  - egui
  - board
  - pointer
  - touch
feature_id: feat_term-core_nx0g1r
---

## What it does

Manual Board pointer capture does not stop egui ScrollArea content dragging after a touchscreen has been detected. Register the same stable divider identity as a Sense::drag widget, so the resize gesture is not also treated as horizontal/vertical scrolling or release inertia.

## Where it lives

Evidence: egui 0.36.1 RawInput reproduction scrolled 0 -> 50 -> 100 then 133 after release without child ownership. divider_raw_input_never_pans_overflowing_scroll_areas_after_touch_detection passes with the divider interaction and fails at x=-50 instead of 0 when it is removed. Synthetic runtime evidence, not an agent-operated native desktop claim. Issue #250; branch 250-board-section-controls.
