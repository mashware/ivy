---
id: asp_a-property-defended-twice-fails-one-at-a-time-mutation_jgt9vp
slug: a-property-defended-twice-fails-one-at-a-time-mutation
name: >-
  A one-at-a-time mutation round cannot grade a property production defends
  TWICE — it reports a good test as a weak one
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:06:17.321Z'
updated_at: '2026-09-18T18:06:17.321Z'
last_verified: '2026-09-18T18:06:17.321Z'
file_paths:
  - crates/app/src/pointer.rs
  - crates/app/src/lib.rs
symbols:
  - 'DragIntent::from_the_press'
  - route_the_buttons
  - cell_under
content_hashes:
  crates/app/src/pointer.rs: 'sha256:78bdcf4b9e74451cde588a56ecbd693850d038e1adb4420f122caf64b7e3e570'
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
tags:
  - mutation
  - tests
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

Measured on AC12 of F2.11 («a press on the strip is never reported to a program that asked for the mouse»): deleting the strip's door in `pointer.rs::DragIntent::from_the_press` left the test GREEN, and so did making `route_the_buttons` clamp the cell instead of rejecting it with `cell_under`. Either mutation ALONE reads as «the test covers nothing» — this project's first named disguise (asserting on the field a value is recorded in instead of on its effect) — and the honest conclusion is the OPPOSITE: the press is stopped by the strip's door AND, independently, by the cell resolving to `None` outside the console. Breaking BOTH in the same build turns the test red (the program then receives two reports where it should receive one). The rule to carry: when a mutation survives, ask whether a SECOND mechanism is holding the property up before writing the test off as weak; a test that asserts the OBSERVABLE criterion rather than one particular mechanism is right to survive a single mutation, and the redundancy it reveals is a fact about production worth knowing, not a hole in the test. Evidence: three mutation runs of `a_press_on_the_strip_is_not_reported_to_the_program_that_asked_for_the_mouse` — two green (one mechanism broken), one red (both broken).

## Where it lives

`crates/app/src/pointer.rs` (`DragIntent::from_the_press`, `route_the_buttons`), `crates/app/src/lib.rs` (`cell_under`).
