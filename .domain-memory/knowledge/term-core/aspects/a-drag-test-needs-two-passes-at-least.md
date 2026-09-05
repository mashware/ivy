---
id: asp_a-drag-test-needs-two-passes-at-least_p8jmdm
slug: a-drag-test-needs-two-passes-at-least
name: >-
  A drag test in egui needs at least TWO passes: with a single one it passes by
  construction, and it passes the same with the mechanism in place as without it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T18:25:00.505Z'
updated_at: '2026-08-19T18:25:00.505Z'
last_verified: '2026-08-19T18:25:00.505Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - drag_its_panel_divider
content_hashes:
  crates/app/src/lib.rs: 'sha256:33eb0620948d43e72b88537f8c3fcb262d3202ccd13ec8e13f08279329d6867b'
tags:
  - egui
  - tests
  - drag
  - disguise
feature_id: feat_term-core_nx0g1r
---

## What it does

**egui resolves interaction at the start of the frame, with the widgets registered in the PREVIOUS
frame.** So a **freshly registered** widget is never `dragged` on its first pass: it does not matter
that in that very same pass you feed it the press event and the move event, because when egui decided
who is being dragged that widget **did not exist yet**.

**The case, and it is TWICE in the same PR.** Two tests were written to pin down guards that decide
**whether the widget is registered or how its value is computed**. Both of them called the method once,
inside a `run_ui` with the three events together. Both green. And both **green with the guard removed
too**: the new widget could not be dragged with the guard or without it, so the method left through the
«nobody is dragging me» door and **never reached the code that was meant to be tested**.

> **It is a variant of the project's third disguise —the test that passes because the scenario cannot
> occur— with the twist that makes it harder to detect: here the scenario is not prevented by our own
> code nor by the window chosen, it is prevented by a rule of the LIBRARY.** See
> [[a-test-can-pass-because-the-scenario-did-not-fit]], where what was failing was the test's world;
> here what fails is the number of frames.

**And it is not seen by reading the test.** It reads perfectly: it sets the scenario up, injects the
gesture, asserts the right thing. **It is seen by mutating it**, and both times it was the mutation that
said so.

**The form that does work** is the one the gesture's other tests were already using without anyone
having written down why: **one interface pass per event** —move the pointer to the spot, press, move,
release— each one a complete frame. With that the widget already exists when egui decides, and the
mutation falls.

**Practical rule:** if what is being pinned lives **behind** a `dragged`/`dragged_by`, the test has to
reach a frame **after** the widget's registration. A single `run_ui` is worth nothing, and its green
means nothing.

## Where it lives

The tests of the panel border gesture, in the test module of `crates/app/src/lib.rs`: the ones that do
four passes with `one_frame_in` work; the two that were written with a single `run_ui` had to be
redone. egui's rule is in `interaction.rs` of 0.36.1, which works over the widgets of the previous pass.
