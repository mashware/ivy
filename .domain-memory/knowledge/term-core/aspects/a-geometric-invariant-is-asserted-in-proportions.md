---
id: asp_a-geometric-invariant-is-asserted-in-proportions_3tthws
slug: a-geometric-invariant-is-asserted-in-proportions
name: >-
  A geometric invariant is asserted in proportions and not in points: the
  version in points failed with the GOOD code, and the margin is chosen between
  two measured figures
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-18T13:59:04.860Z'
updated_at: '2026-08-18T13:59:04.860Z'
last_verified: '2026-08-18T13:59:04.860Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - absorb
  - place
  - DIVIDER_THICKNESS
  - place
content_hashes:
  crates/app/src/lib.rs: 'sha256:d7688143658df12a548b8eb3bd4e0e7fc0ce9966b7a5e14a6e7d7a8e8835ba9d'
tags:
  - tests
  - geometry
  - invariants
  - mutation
feature_id: feat_term-core_nx0g1r
---

## What it does

**A new disguise, and it is the mirror image of the ones this project already had on file.** The known
ones are tests that **pass without pinning anything**
([[a-test-that-pins-the-helper-does-not-pin-the-path]],
[[a-test-can-pass-because-the-scenario-did-not-fit]]). This one is a test that **fails without there
being a defect**, because it asserts something **stronger than what the code promises**.

**The case (F2.3, second PR).** What had to be pinned was that `absorb` —the operation that raises the
granddaughters to siblings when a branch is left with a single child— preserves the geometry. The
formulation the previous session's log had left written down was «the rectangles of the ones that remain
**do not move by even a point**». It was written that way and **it fell with the good code**: a console
went from 599 to 598 points.

> **They do move by one, and it is unavoidable.** Raising the children puts one more divider into the
> outer branch, and dividers are discounted from the area **before** tiling. The divider has to fit
> somewhere. What the invariant promises is that **the tiling between them does not change**, not that
> nothing moves — see [[removing-a-leaf-never-takes-area-from-those-left]].

**The correct form: assert the RATIO between the sizes.** A console twice the size of another is still
twice the size before and after, and that is exactly true, without depending on the divider's thickness
or on the area chosen.

> **And the margin is chosen between two measured figures, not by trial and error.** The divider's point
> shifts the ratio by **0.007**; the defect the test is after —the scaling replaced by a copy of the
> weight— shifts it by **1.0**. With a margin of hundredths the test passed, but **0.007 away from
> failing on its own** the moment anyone changed the scenario's area. It was set at `0.05`: two orders
> of magnitude above the known noise and twenty times below the signal. A margin justified with the two
> figures on either side is the opposite of the disguise this project has on file —choosing a number
> with which the defect and the fix give the same result—, and it is documented **in the test's comment**
> so that the next person does not read it as arbitrary.

## Where it lives

`crates/app/src/lib.rs`, test `absorbing_preserves_the_geometry_of_the_consoles_that_do_not_move`, and the
doc of `Node::absorb` that cites it. The scenario reuses the helper `a_group_that_forces_absorbing`
that the shape invariant's test already had, and takes the console out on a side of the **same
orientation** as the outer branch so that the insertion only tiles the other axis and the widths being
compared do not notice.
