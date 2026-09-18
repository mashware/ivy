---
id: asp_a-cleared-drag-intent-is-not-the-same-as-none_cqym5m
slug: a-cleared-drag-intent-is-not-the-same-as-none
name: >-
  A gesture whose intent is CLEARED mid-flight is not a gesture with no intent —
  the pointer loop's fall-through arm used to treat them the same, moving the
  pane as the price
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:05:20.843Z'
updated_at: '2026-09-18T18:05:20.843Z'
last_verified: '2026-09-18T18:05:20.843Z'
file_paths:
  - crates/app/src/pointer.rs
symbols:
  - DragIntent
  - handle_the_mouse_without_claiming_it
  - select_while_dragging
  - drop_a_pane
content_hashes:
  crates/app/src/pointer.rs: 'sha256:78bdcf4b9e74451cde588a56ecbd693850d038e1adb4420f122caf64b7e3e570'
tags:
  - pointer
  - mutation
  - state-machine
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

The pointer loop branches on `self.drag_in_progress`: one arm for `Some(Select)`, one for `Some(ToTheProgram)`, and everything else fell through to the arm that previews a destination and moves the pane on release. «Everything else» is `Some(MovePane)` AND `None` — and `None` is reachable IN FLIGHT: `select_while_dragging` cancels a selection whose press did not land on the grid by setting `drag_in_progress = None` (the fix for «pressing on the panel and dragging changed what was selected»). From the next frame that gesture had no intent, fell through, and moved the console — so pressing on the status panel's columns and dragging onto another console reordered the tree with no chord held. Predates F2.11 by two milestones. Fix, one line: the branch that transforms the tree runs ONLY for `Some(DragIntent::MovePane)`; anything else resets on release and continues. An enum whose `None` means «not decided yet» should never be the default arm of a match that acts. Found not by review — five reviewers read that loop — but by MUTATING the new mechanism: the delivery that gave the strip a door into `MovePane` removed that door, and the test for it stayed green, because the press on the strip was cancelling a selection and then being moved by the fall-through. Lesson: a mutation that stays green does not always mean the test is weak — sometimes it means TWO paths reach the same effect, and the second one is a defect nobody had named.

## Where it lives

`crates/app/src/pointer.rs` (`DragIntent`, `handle_the_mouse_without_claiming_it`, `select_while_dragging`, `drop_a_pane`).
