---
id: asp_a-delayed-fade-in-an-idle-egui-window_ay963s
slug: a-delayed-fade-in-an-idle-egui-window
name: >-
  A delay-then-fade in an idle egui window: start the countdown on the first
  unwanted pass, drop records from unpainted passes, and accept the same frame
  number
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T16:27:00.358Z'
updated_at: '2026-11-20T16:27:00.358Z'
last_verified: '2026-11-20T16:27:00.358Z'
file_paths:
  - crates/app/src/header/linger.rs
  - crates/app/src/header/integrated.rs
  - crates/app/src/tests/integrated_chrome.rs
symbols:
  - controls_opacity
  - revealed
  - painted_id
content_hashes:
  crates/app/src/header/integrated.rs: 'sha256:e1ec2d740b3e6c85a6e6c58db9e809c4662ecc831767403ff04187135723c5e9'
  crates/app/src/tests/integrated_chrome.rs: 'sha256:6afc96f41006ad5fa7a7cdfe24109e5c1b3b246669049edd58ef6aa9b550fb5c'
tags:
  - egui
  - fade
  - repaint
  - frame-number
  - testing
feature_id: feat_term-core_nx0g1r
---

## What it does

Four traps met building the header controls' linger-then-fade (#157, November 2026). Each one
produced a defect a reviewer or a challenger found, or a test that failed before it was right.

1. **Start the countdown on the first pass that finds the thing unwanted**, never from a
   «last wanted» time stamp. A window at rest paints nothing (the blink stops after 15 s of
   keyboard silence, D37; a still pointer sends nothing), so the last wanted pass can be seconds
   old when a click on another console arrives, and the countdown would already be over on the
   pass that should start it. Found by the design challenger before any code existed.
2. **A «was wanted» record survives the passes in which its console is not painted** (a tab in
   the background, a sibling behind a maximized console). Store the frame number with it and
   treat a record older than the previous pass as gone, or a header comes back from a background
   tab lingering for 3 s although nobody saw it leave. Found by the built-in review.
3. **`ctx.cumulative_frame_nr()` is the same for both passes of one frame** (a sizing pass,
   `request_discard`): the closure runs twice with one number, see
   [[the-repeated-pass-re-pumps-and-overwrites-the-round]]. So «painted on the previous pass»
   must be `stored + 1 >= frame`, not `==`, or the second pass reads the first as unpainted. D99's
   own `revealed()` still compares with `==`. Found by the second review round.
4. **egui answers a pointer move with one immediate pass of its own**: `repaint_delay` is 0 on
   the pass after a move, then the scheduled wake shows (measured: 0 ns at 10.1 s after a move
   at 10.0 s, 2.78 s at 10.2 s). A test of a scheduled wake lets that pass go by first.

Evidence: tests `unwanted_header_controls_linger_then_fade_and_come_back_whole`,
`a_header_back_from_a_background_tab_does_not_linger` and unit
`the_second_pass_of_a_frame_starts_the_countdown_too` each fail under the mutation that
reintroduces its trap (checked on branch 157-header-controls-fade-out).

## Where it lives

crates/app/src/header/linger.rs (the countdown), crates/app/src/header/integrated.rs (painting
and the D99 press record), window tests at the end of crates/app/src/tests/integrated_chrome.rs.
