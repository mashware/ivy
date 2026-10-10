---
id: asp_a-per-frame-declaration-blinds-the-declaring-tests_7ekd13
slug: a-per-frame-declaration-blinds-the-declaring-tests
name: >-
  Declaring a pass's cost on EVERY frame silently disarms the tests that assert
  another path declared something
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-10-10T22:19:34.047Z'
updated_at: '2026-10-10T22:19:34.047Z'
last_verified: '2026-10-10T22:19:34.047Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - timed_separately
  - send_keys_to_the_focused
  - somebody_typed
content_hashes:
  crates/app/src/lib.rs: 'sha256:84bd2404e27115cb897f05b1804e7d589734c17791469fb29de2598a1e4748ac'
tags:
  - measurement-bench
  - d13
  - test-disguise
  - f2.13
feature_id: feat_term-core_nx0g1r
---

## What it does

Several tests in `crates/app/src/lib.rs` prove a path declares its frame cost by asserting `!window.timed_separately.is_zero()` after one pass, on the premise «in a normal pass nobody else adds to it» (`walking_the_tree_to_paint_is_declared_and_not_given_away_to_the_framework`, and `opening_a_tab_declares_what_launching_its_two_programs_costs` before #177). When #177 closed the declared keyboard-pass hole by timing `send_keys_to_the_focused` on every pass, both stayed green with their own timing deleted: the new contributor satisfied «not zero» for them.

What kept them binding: (1) declare the keyboard pass only on a frame somebody typed in (`keyboard::somebody_typed` — a press, a text or a paste; `before.elapsed() * u32::from(typed)`), so an input-less pass adds nothing; (2) a test that drives a chord compares against a frame that also typed but launched nothing, with a ×5 margin, the shape `splitting_declares_what_launching_the_program_of_the_newborn_costs` already had; (3) the new declaration gets its own two-sided test (`the_keyboard_pass_is_declared_when_somebody_typed_and_only_then`: zero on an empty pass, ≥ the press's own walk on a typed one) — mutated with `*1` and `*0`, both red.

**Rule:** before adding a `timed_separately +=` that can run on every frame, grep the tests for `timed_separately.is_zero()` and `timed_separately` comparisons — each one assumes it is the only contributor on its frame. Family of [[a-frame-cost-is-declared-once]] and [[seeding-can-be-pinned-timing-not-always]].

Evidence: the correctness reviewers of #177 (rounds 1 and 2) reading those tests against lib.rs; fixed in 1de439a and 468100f.

## Where it lives

crates/app/src/lib.rs — `Window::send_keys_to_the_focused` and the tests named above.
