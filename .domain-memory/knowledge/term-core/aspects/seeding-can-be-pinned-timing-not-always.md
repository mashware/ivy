---
id: asp_seeding-can-be-pinned-timing-not-always_38pyam
slug: seeding-can-be-pinned-timing-not-always
name: >-
  If the work can be SEEDED it is pinned with a test; if it can only be timed,
  what decides is WHERE the stopwatch goes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T16:09:00.137Z'
updated_at: '2026-08-17T20:08:23.772Z'
last_verified: '2026-08-17T20:08:23.772Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - rearm_the_watcher
  - timed_separately
  - take_what_this_frame_left
  - handle_the_mouse
  - handle_the_mouse_without_claiming_it
content_hashes:
  crates/app/src/lib.rs: 'sha256:bd1d056ad4bcd72e1edfbea41ecd1ed51b925068d136aabb02b79da1032b45e3'
tags:
  - measurement
  - tests
  - mutation
  - F2.2
  - F2.3
feature_id: feat_term-core_nx0g1r
---

## What it does

**The project's rule is that every undeclared piece of work gets published as the framework's cost**
—the frame is shared out by subtraction— and the usual fix is to move the stopwatch line so that it
encloses the work. What this case teaches is **when that fix can be tested and how**, and it is
worth knowing before writing the test.

**To test it you have to compare what is declared against a wall clock taken from OUTSIDE the
operation**, and a scheduler preemption fits between the two clocks. Measured on 17 August 2026 with
`rearm_the_watcher` on opening and closing a tab: in the suite one rearm costs **microseconds**,
that is, **the same as the gap between the two clocks**. Signal and noise of the same order cannot
be told apart. Three formulations —a margin of half a rearm, calibrating with the minimum of five
measurements, accepting one attempt out of eight— and all three turned down: either fragile, or with
both mutations alive.

**The test was removed.** A test that does not fail when what it claims to cover is broken on
purpose covers nothing, and leaving it there is worse than not having it. It is the second time in
the same milestone —see [[the-measurement-round-closes-on-painting]]— that a test that **looked
stronger was weaker**.

> **Practical rule: if the work can be SEEDED, it is pinned with a test; if it can only be TIMED and
> costs the same as the noise, what holds it up is structural.**

**What is pinned:** `closing_a_tab_does_not_take_away_what_its_consoles_had_already_declared` **seeds** three
large and distinct durations —five, three and two seconds— in the three places where a console keeps
undeclared work, and asserts that what is declared exceeds the sum. Large **and distinct** on
purpose: with the same number in all three, dropping one would still leave plenty.

---

## Extension (F2.3, issue 76): it is not only WHETHER it can be tested, it is WHERE the stopwatch goes

There are **three places** and only the third is pinned in full:

| Where | What a test catches |
|---|---|
| **In the caller** (the style that was already there) | **Nothing.** A test that calls the function does not touch the counter. Checked by mutating it: removing the line leaves the whole suite green |
| **Inside, with the body in the same function** | That the line **disappears**, yes. That the region **shrinks**, no: moving the first clock reading up a few lines publishes almost all the work as the framework's cost and the test stays green, because «greater than zero» is met by any pair of consecutive readings. It is the second disguise on file —a zero with which the defect and the fix give the same result— and it was caught by the code review, not by whoever wrote the test |
| **Inside, and the work in ANOTHER function** | Everything. The timed region is the whole call, so shrinking it stops being moving a line and becomes taking code out of a function — visible in any review |

> **The test pins that the declaration EXISTS; the structure pins that it COVERS EVERYTHING. Neither
> of the two on its own is enough.**

**And watch out for the wrapping:** splitting the function creates exactly the shape
[[a-test-that-pins-the-helper-does-not-pin-the-path]] warns about. Here it does not bite because
**the tests come in through the outer function**, which is the same door production uses. If any of
them came in through the inner one, the hole would be back.

**A corollary of the same family:** a test that takes its number **from the very constant it tests**
is a mirror, not a test. And **a warning about method that cost redoing work**: a mutation script
that reverts with `git checkout --` takes with it whatever is not committed, and **a patch that
fails to apply reads exactly like a mutation that survives** — the script has to check that the file
changes before taking the mutation as good. Both things happened.

## Where it lives

`crates/app/src/lib.rs` — `timed_separately`; the four places that call `rearm_the_watcher`;
`Pane::take_what_this_frame_left`, which is the half that can be seeded; and since F2.3,
`Window::handle_the_mouse` (the stopwatch) over `handle_the_mouse_without_claiming_it` (the work).
Learned in PR #75 of F2.2 and extended in PR #77 of F2.3.
