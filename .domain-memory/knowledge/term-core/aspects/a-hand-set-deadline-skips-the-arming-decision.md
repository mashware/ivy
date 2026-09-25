---
id: asp_a-hand-set-deadline-skips-the-arming-decision_48vm42
slug: a-hand-set-deadline-skips-the-arming-decision
name: >-
  The saving tests expire the deadline by hand, which walks past the arming
  decision: a rule that lives in arming is invisible to them
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T11:15:29.643Z'
updated_at: '2026-09-25T11:15:57.443Z'
last_verified: '2026-09-25T11:15:57.443Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/session.rs
symbols:
  - save_due
  - check_whether_the_layout_moved
  - save_if_due
  - 'Persistence::will_never_write'
content_hashes:
  crates/app/src/lib.rs: 'sha256:29d1f78d3bf1f0267b09c1ff0567a34b1e96feae2e534a0216b8eba0e7ff0527'
  crates/app/src/session.rs: 'sha256:28822fa6d65390a16f2578aa7b1ccfae9138d7f1f85083e68887769e9d494310'
tags:
  - tests
  - test-disguise
  - d30
  - d16
feature_id: feat_term-core_nx0g1r
---

## What it does

**The idiom every layout-saving test in `lib.rs` uses to avoid waiting half a second is `window.save_due = Some(Instant::now() - SAVE_DELAY)` followed by `save_if_due()`.** It tests what happens *on expiry* — and by writing the deadline itself it **skips the decision of whether a deadline should exist at all**, which lives in `check_whether_the_layout_moved`.

> **So any rule that belongs to arming — «do not arm when X» — passes a test written in that idiom whether the rule exists or not.** Worse, when the rule is correct the idiom can make the test fail for the wrong reason: the hand-set deadline forces the attempt the rule was preventing.

Measured in #185 (September 2026). The first version of the fix gated in `save_now`; moving that gate to just before the write kept **every** test green while the copy, comparison and validation it claimed to remove were paid again — the D30 disguise of asserting a counter instead of the effect. The review moved the gate to arming, and at that point the attempts tests, still hand-setting the deadline, reported 3 attempts under a correct fix.

**The shape that binds**: expire only a deadline the change actually armed — `if window.save_due.is_some() { window.save_due = Some(past) }` — and, where arming must not happen, assert `save_due` is `None` after a change. With that, removing the arming gate turns both attempts tests red. Any new test about *when* the layout is saved should start from this shape rather than from the neighbouring tests.

## Where it lives

`crates/app/src/lib.rs` test module: helper `attempts_over_three_changes` (the binding shape) beside `when_the_deadline_expires_the_layout_ends_up_on_disk`, `a_write_failure_is_retried_with_the_next_change` and the others that still hand-set it, which is fine for rules about expiry. The arming decision is `check_whether_the_layout_moved` in `crates/app/src/session.rs`.
