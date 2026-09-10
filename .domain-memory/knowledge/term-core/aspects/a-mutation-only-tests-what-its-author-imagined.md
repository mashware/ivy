---
id: asp_a-mutation-only-tests-what-its-author-imagined_76538a
slug: a-mutation-only-tests-what-its-author-imagined
name: >-
  A mutation only tests what its author thought to mutate — twice in two days
  the hole was found by a reviewer and not by the series, and both times it was
  the harness's own scenario
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T20:24:54.979Z'
updated_at: '2026-09-07T23:04:54.391Z'
last_verified: '2026-09-07T23:04:54.391Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
symbols:
  - closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
  crates/app/src/lib.rs: 'sha256:42e03be5e211dd7bbc84d23fc4f707cdf8ddc91b86d65f56b32f84a8ec71dea0'
tags:
  - tests
  - mutation
  - review
  - method
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

**Mutating a test and an adversarial reader are not substitutes for one another**, and two deliveries in two days say it the same way: the mutation series graded the defences that existed and **a reviewer found the one nobody had built**.

The project's rule (**D30**) is that a test which does not fail when you break what it claims to cover covers nothing. Mutation is how that gets checked. It is necessary and it is **not sufficient**.

## Case one: the hang on closing (#62)

Four mutations against a tree with the fix reverted graded three harness precautions correctly — the echo and the drain load-bearing, the receiver's lifetime not — which is exactly what mutation is for and would not have come from reading.

**What it missed: nothing in the test proved the survivor existed at all.** `{ setsid … & }` exits 0 whatever happens inside it — verified: `sh -c '{ nosuchcmd & } && echo REACHED'` prints REACHED — so on any host without `setsid(1)`, which is every macOS, the script reached its `exec` with nobody holding anything and the test passed over a live bug. Two reviewers found it independently, in minutes.

## Case two: the notice's click (#170), and here the series ran six times

Six rounds of mutation across three review rounds. Every round the series confirmed what it was built to confirm, and every round a reviewer named something the series could not see:

| What the series graded | What it could not see |
|---|---|
| the sentence, the mark, the click answering | that **which button** answers was pinned by nothing — accepting the middle button, the desktop's paste, left the whole suite green and would have destroyed the held paste |
| the answered press being removed | that removing it and **clearing the whole queue** were indistinguishable, because no test carried a second event |
| the box's minimum height | its **width** |
| one click answering one question | that a loop clearing **every** standing question passed, because every test held a paste on a single console |

**The pattern in all four: the harness's own scenario was too small for the defect.** One console, one event, one question. That is [[a-test-can-pass-because-the-scenario-did-not-fit]] arriving through the back door — not the assertion being wrong, the **world** being too small — and a mutation of the *code* cannot reveal it, because the code behaves correctly in that small world.

## The rules that came out of it

> **A mutation series measures the strength of the defences you built. It says nothing about the defence you did not think of.** Mutation grades what is there; an adversarial reader asks what is missing. If only one is affordable, note which question went unasked.

> **Widen the world before mutating.** Two consoles, two events in one batch, two standing questions. Three of #170's four blind spots were «the test only ever has one of the thing».

> **A negative assertion needs an arrival as its clock, never a timer.** «No report reached the program» after a fixed 500 ms passed with the suppression deleted, because a report takes milliseconds to make the round trip and *not yet* is true whether or not it was ever sent. Fixed by aiming two clicks at **different cells** and asserting the absence of one cell's report on a screen that already carries the other's: the arrival proves the round trip works, and the absence is then an absence.

> **A second round is what proves the hardening rather than the hole moving.** After each fix the whole series is re-run; twice a «fix» turned one green mutation into another.

## And two operational notes, both learned the hard way

- **Mutate in a worktree of its own.** The tree is shared with other agents; one mutation left in place has already been observed surviving a whole green suite ([[an-agent-that-dies-can-leave-the-mutation-in-place]]).
- **A reviewer needs the right diff base.** Five reviewers were once briefed with `main...HEAD` where the worktree's local `main` was a **whole merged milestone behind `origin/main`** — about 1 700 lines that belonged to the previous delivery. In a repo with worktrees the base is `origin/<base>`.

## Where it lives

The mutation tables live in the doc comments of the tests themselves: `closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal` in `crates/term-core/src/lib.rs`, and the notice's six tests in `crates/app/src/lib.rs`. Both works' full rounds are recorded in `.claude/work/_archive/`.
