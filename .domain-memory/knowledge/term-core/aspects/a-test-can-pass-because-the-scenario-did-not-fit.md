---
id: asp_a-test-can-pass-because-the-scenario-did-not-fit_j56vtg
slug: a-test-can-pass-because-the-scenario-did-not-fit
name: >-
  A new disguise of a test that pins nothing: it passes because the scenario
  chosen did not let the defect occur, not because the guard works
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T23:13:30.250Z'
updated_at: '2026-08-16T23:13:30.250Z'
last_verified: '2026-08-16T23:13:30.250Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - can_split
  - 'Pane::send_keys'
content_hashes:
  crates/app/src/lib.rs: 'sha256:01adf2fc83535e392e34a240d6ed26b7af67123641006bbb510b9f94b9c87f5a'
tags:
  - tests
  - mutation
  - F2.2
feature_id: feat_term-core_nx0g1r
---

## What it does

**The case.** Splitting a pane launches a program, so the chord carries a guard against the repeat of a
held key: without it, a second held down is thirty consoles. The test sent the chord with three repeats
and asserted that there was only one new pane. **Green.** With the guard removed: **green as well**.

**Why.** The test's window measured 160 columns. After the first split, the focused pane was left at 40,
and the geometric floor demands 40 **plus the divider's thickness** to split again. In other words: what
stopped the second split **was not the guard, it was that it no longer fitted**. The test was measuring
something else and it said everything right.

> **The disguise, in one sentence: the test's scenario prevented, all by itself, the defect the test
> claimed to cover.** It is not asserting over the wrong field nor choosing a number where defect and
> fix coincide —the two disguises this project already had on file—: here the assertion is the right one
> and so is the number; what is wrong is **the world** in which it is asserted.

**How it is fixed, and it is cheap:** widen the scenario until the defect **can** occur, and add the
half that was missing —the one that proves the scenario was not the limit—. Here: 200 columns, and after
the held chord, the same chord **released and pressed again** does split. Without that second half, the
test can pass for the wrong reason again as soon as anyone touches the floor.

**How it is detected:** only by mutating. No reading of the test's code gives it away —it reads
perfectly—, and the suite in green is exactly what the defect produces.

## Where it lives

`crates/app/src/lib.rs` — the `!repeat` guard in `Pane::send_keys` and the floor `can_split`; the
test `holding_down_the_chord_for_splitting_opens_no_more_than_one_console`. Found in F2.2, PR #73.
