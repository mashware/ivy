---
id: asp_a-one-turn-scripted-fake-asserts-the-default_nkrnr7
slug: a-one-turn-scripted-fake-asserts-the-default
name: >-
  A sixth test disguise, free with every scripted fake: the fake's own
  exhaustion produces the value the path under test returns, so a one-turn
  script asserts nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:22:57.949Z'
updated_at: '2026-09-10T15:22:57.949Z'
last_verified: '2026-09-10T15:22:57.949Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - feed
  - feed_and_record
  - Scripted
  - never_reached
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
tags:
  - tests
  - mutation
  - D30
  - fakes
feature_id: feat_term-core_nx0g1r
---

## What it does

**The shape.** A loop reads from an `impl Read`; a fake answers scripted turns and then `Ok(0)` — end of file — once its script runs out. A test scripts **one** turn that should make the loop stop, and asserts the ending it expects.

**Three mutations left all eighty tests green** on such a suite: ignoring the delivery's failure entirely, moving a stop-flag check to where it stops binding, and recording a fixed value instead of the computed one. In every case the loop simply **read again**, ran out of script, got end of file, and returned **the very value the assertion was looking for**. The defect and the correct path produce the same answer, which is this project's own second disguise arriving through the fake instead of through a chosen number.

> **The fix is two lines and it generalises: put a turn behind the one under test that no correct loop can reach.** An unmistakable failure after the chunk that should have ended the loop turns «it returned this» into «it **stopped there** and returned this». With it, all three mutations go red, each in exactly the test that claims it.

### A second door in the same family, and it survived the first fix round

**Asserting a pure function's return value and asserting the state it gets written into are different tests, and the wiring between them is covered by neither.** Every failure was asserted on the reading directly; the only tests that read the recorded value expected what a hard-coded write happened to produce. So recording a constant — discarding the loop's entire answer, which is the whole point of the mechanism — **stayed green across eighty-two tests**.

The seam that closes it is one function that does both and can be called with a source that fails, so a test can assert that what was recorded is what the reading answered. Found by the built-in reviewer at `xhigh`, **after** a full four-agent panel had already been through the same code.

**The tell for both**: a test whose assertion would be satisfied by the fake doing nothing at all. Ask of every scripted test — *if the subject ignored its input entirely, would this still pass?* On the suite where this was measured, **three of seven** would.

## Where it lives

`crates/term-core/src/lib.rs`, in its test module: `Scripted` (the fake, which also clamps to the buffer so it can answer a short read), `never_reached()` (the turn no correct loop reaches, with the reasoning in its own docblock), and `feed_and_record` (the seam that made the second door testable).
