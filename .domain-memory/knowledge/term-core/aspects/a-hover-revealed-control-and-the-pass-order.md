---
id: asp_a-hover-revealed-control-and-the-pass-order_p3w8gk
slug: a-hover-revealed-control-and-the-pass-order
name: >-
  A control revealed on hover: the pointer is handled before the panes paint, a
  hidden press focuses, and egui counts a double click from release to release
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T10:20:36.179Z'
updated_at: '2026-11-18T10:20:36.179Z'
last_verified: '2026-11-18T10:20:36.179Z'
file_paths:
  - crates/app/src/header/actions.rs
  - crates/app/src/header/integrated.rs
  - crates/app/src/tests/integrated_chrome.rs
symbols:
  - integrated_hit
  - revealed
  - Offer
  - max_double_click_delay
content_hashes:
  crates/app/src/header/actions.rs: 'sha256:3f7258d92af4f8c1394b13ff3640bc6271bcb5b4150777b0f331ddd81c28866b'
  crates/app/src/header/integrated.rs: 'sha256:e1ec2d740b3e6c85a6e6c58db9e809c4662ecc831767403ff04187135723c5e9'
  crates/app/src/tests/integrated_chrome.rs: 'sha256:6afc96f41006ad5fa7a7cdfe24109e5c1b3b246669049edd58ef6aa9b550fb5c'
tags:
  - egui
  - pointer
  - double-click
  - tests
  - D99
feature_id: feat_term-core_nx0g1r
---

## What it does

Four facts met building D99's hover-revealed header controls (#104, November 2026); each one
produced a defect or a test that could not fail.

1. In a pass, the window handles the pointer (`handle_the_mouse`) before it paints the panes
   (`paint_pane`). A record written while painting pass N is what the press in pass N+1 finds, and a
   pass whose own input brings the pointer onto a header has not painted that header's controls yet
   when its press arrives. That is what lets 'pressable only if painted in the previous pass'
   (`painted + 1 == now`) refuse a move and a press in one batch; reversing the order would make
   every reveal pressable in its own pass.

2. A press on a header's title focuses that console, and a focused console's controls are always
   painted. So the first click of a double click on a hidden control cell reveals the controls under
   the pointer, and the second click lands on whatever appeared there, a close included. Two review
   rounds found this: first the window counted from the first press, then the reviewer showed egui
   counts a double click from release to release (`max_double_click_delay`, 0.3 s), so a held first
   press escaped. The window has to start at the release of the unseen press.

3. `max_double_click_delay` lives in `Context::options().input_options`; `InputState::options` is
   private and does not compile.

4. Two test traps. The `frame` helper in the integrated chrome tests leaves `RawInput.time` unset,
   so egui advances 1/60 s per pass and no time window ever expires: a 'the swallow ends' test needs
   `ui_pass` with explicit `time`. And `request_repaint()` shows as `repaint_delay == 0` in the pass
   that asked and in the next one, so a 'reveal asks for one pass, staying shown asks for none' test
   must skip one pass before asserting rest.

## Where it lives

crates/app/src/header/actions.rs (integrated_hit, Input::unseen),
crates/app/src/header/integrated.rs (revealed, paint), crates/app/src/tests/integrated_chrome.rs
