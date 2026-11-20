---
id: asp_egui-drag-starts-after-travel-anchor-at-press-origin_ftzphy
slug: egui-drag-starts-after-travel-anchor-at-press-origin
name: >-
  egui reports a drag only after the pointer travels: anchor a selection at
  press_origin, not at the pointer
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T17:25:08.112Z'
updated_at: '2026-11-20T17:25:08.112Z'
last_verified: '2026-11-20T17:25:08.112Z'
file_paths:
  - crates/app/src/viewer/code.rs
symbols:
  - press_origin
  - drag_started
content_hashes: {}
tags:
  - egui
  - selection
  - pointer
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36 sets `Response::drag_started()` only once the pointer has moved past the click
distance, so on that frame `interact_pos()` is already away from where the button went down.
A text selection anchored at `interact_pos()` loses the characters between the press and the
point where egui recognised the drag. Anchor at `InputState::pointer.press_origin()`.

Evidence: the code view's `a_dragged_selection_copies_the_text_without_line_numbers` copied
nothing until the anchor moved to `press_origin` (#159 PR #1); a mutation putting it back on
the pointer turns that test red.

## Where it lives

crates/app/src/viewer/code.rs (show: drag_started branch)
