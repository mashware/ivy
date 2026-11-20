---
id: asp_testing-an-egui-fade_waba1z
slug: testing-an-egui-fade
name: >-
  Testing an egui fade: the first step happens on the change pass, the header
  asks two more passes, and the blink never lets repaint_delay reach MAX
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T15:02:22.325Z'
updated_at: '2026-11-20T15:02:22.325Z'
last_verified: '2026-11-20T15:02:22.325Z'
file_paths:
  - crates/app/src/tests/integrated_chrome.rs
  - crates/app/src/pane_view.rs
symbols:
  - focus_mix
  - animate_value_with_time
content_hashes:
  crates/app/src/tests/integrated_chrome.rs: 'sha256:6afc96f41006ad5fa7a7cdfe24109e5c1b3b246669049edd58ef6aa9b550fb5c'
  crates/app/src/pane_view.rs: 'sha256:9649ff9332590ba19a17242003e2486a2677db6ef3993e4ca65a1b2f6c7eddcc'
tags:
  - egui
  - testing
  - animation
  - repaint
feature_id: feat_term-core_nx0g1r
---

## What it does

Four traps met writing the focus-green fade test (#141, November 2026); each one failed a naive
assertion before the test was written as it is now.

1. On the pass where the target changes, egui 0.36's `animate_value_with_time` already steps one
   predicted frame (~1/60 s). Asserting the old value on that pass fails; assert that neither end
   has been reached.
2. In the integrated chrome the two passes after a focus move request another pass anyway: the
   focused header's controls appear. A «no repaint requested» check on those passes is decided by
   the header, not by the animation. Check the third pass.
3. The cursor blink keeps `repaint_delay` near 570 ms, never `Duration::MAX`, in any window with a
   console. A «nothing wakes» assertion needs a threshold (more than 100 ms), not `MAX`.
4. Calling `animate_value_with_time` at all requests a repaint, even with an animation time of 0.
   A gate that must wake nothing (D13's measured runs) has to skip the egui call entirely.

Evidence: `moving_focus_fades_the_green_and_a_measured_run_does_not` failed on traps 1-3 in that
order on branch 141 before being rewritten; trap 4 is egui-0.36.1 `animation_manager.rs`.

## Where it lives

crates/app/src/tests/integrated_chrome.rs (`moving_focus_fades_the_green_and_a_measured_run_does_not`), crates/app/src/pane_view.rs (`focus_mix`)
