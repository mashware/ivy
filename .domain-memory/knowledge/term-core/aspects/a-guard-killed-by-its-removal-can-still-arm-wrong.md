---
id: asp_a-guard-killed-by-its-removal-can-still-arm-wrong_0d0vv0
slug: a-guard-killed-by-its-removal-can-still-arm-wrong
name: >-
  A guard whose removal three tests kill can still have an untested ARMING
  condition: mutate when it arms, not only whether it exists
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T20:49:08.195Z'
updated_at: '2026-09-26T20:49:08.195Z'
last_verified: '2026-09-26T20:49:08.195Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
symbols:
  - last_frame_with_buttons
content_hashes:
  crates/app/src/lib.rs: 'sha256:8e58392b7233a1e47075f6bf4e587453eb4c26535fc2c4209c87c3ffc8553a96'
  crates/app/src/pointer.rs: 'sha256:f384df68dd3715e3e27ade9e33ef2e44981f70f478d0876379c3b2307411dd63'
tags:
  - test-disguise
  - mutation
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured on #200 (26 September 2026).** Three sibling tests went red with the once-per-frame button guard disabled (`if true`). A narrower mutation of the SAME guard — arm `last_frame_with_buttons` only when the batch left after answering a notice still carries a press — left all three **green**, because each of them routes a press on its first mouse pass, which arms the marker anyway. Only a scenario whose first pass routes nothing exposes it: a press consumed by the held-paste question, followed by a second mouse pass in the same frame. `a_press_that_answers_the_question_is_not_handed_to_the_program_on_a_repeated_pass` is that test, and it is the only one that falls under the narrow mutation.

**The rule**: a guard reported as «covered» because deleting it fails tests can still have an arming condition nobody pins. List the conditions under which it arms and find a scenario where the arming path differs from the siblings' — then mutate the condition, not the guard.

Same review, two smaller traps worth carrying: a sentinel's comment gave a reason the assertion did not have (it counted only the answered cell, so a doubled sentinel could never fail it); and a comment cited `Context::run`, which egui 0.36.1 does not have — the glossary guard (`tests/glossary.rs`) also rejects citing a foreign private name like `run_dyn`, so egui internals are cited by file and line.

## Where it lives

crates/app/src/lib.rs (tests around the held-paste notice); crates/app/src/pointer.rs (`last_frame_with_buttons` gate)
