---
id: asp_a-mutation-only-tests-what-its-author-imagined_76538a
slug: a-mutation-only-tests-what-its-author-imagined
name: >-
  A mutation only tests what its author thought to mutate: two rounds of it
  graded a test's precautions correctly and missed the hole that made the test
  worthless
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T20:24:54.979Z'
updated_at: '2026-09-07T20:24:54.979Z'
last_verified: '2026-09-07T20:24:54.979Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21899fbd18b28515f01ca0c0c025ca2b260981459a259167fbc21b961f6070c7'
tags:
  - tests
  - mutation
  - review
  - method
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

**Mutating a test and an adversarial reader are not substitutes for one another, and this is the case that shows why.** The project's rule (D30) is that a test which does not fail when you break what it claims to cover covers nothing. Mutation is how that gets checked. It is necessary and it is **not sufficient**, because a mutation series can only break what the person writing it thought of.

## What the mutation found, and it was genuinely useful

The regression test for the hang on closing (#62, D32) carried three harness precautions, each with a comment saying it was load-bearing. Four mutations, run in a throwaway worktree against a tree with the fix reverted:

| Mutation | Result |
|---|---|
| none | FAILED, 2.05 s |
| the echo left on | **passed, 0.10 s** |
| the drain to quiet removed | **passed, 0.05 s** |
| the receiver released before the close | FAILED, 2.05 s |

So two of the three precautions were load-bearing, **one was not** — and the test's own comment claimed all three were. That correction is exactly what mutation is for, and it would not have come from reading.

## What it missed, and two reviewers found independently

**Nothing in the test proved the survivor existed at all.** It launched one with `{ setsid … & }`, and a backgrounded list **exits 0 whatever happens inside it** — verified: `sh -c '{ nosuchcmd & } && echo REACHED'` prints REACHED. On any host without `setsid(1)`, which is every macOS and **F2.7 is the next milestone**, the script reached its `exec` with nobody holding anything, the close returned in ~50 ms, and the test passed over a live bug.

The mutation series could not find it **because it never occurred to its author to mutate the premise**: every mutation weakened a precaution, and this was a missing precaution. A reviewer briefed to attack the test found it in minutes, and so did a second one working separately.

## The rule

> **A mutation series measures the strength of the defences you built. It says nothing about the defence you did not think of.** Mutation grades what is there; an adversarial reader asks what is missing. Run both, and if only one is affordable on a given change, note which question went unasked.

The corollary is about **rounds**: after hardening the test, the same five mutations — including the new «make the survivor's command nonexistent» — all went red. A second round is cheap and it is the only thing that proves the hardening worked rather than moving the hole.

## And two operational notes, both learned the hard way here

- **Mutate in a worktree of its own.** The tree is shared with other agents; a mutation left in place destroys their work, and one has already been observed surviving a whole green suite ([[an-agent-that-dies-can-leave-the-mutation-in-place]]).
- **A reviewer needs the right diff base.** Five reviewers on this work were briefed with `main...HEAD`, and the worktree's local `main` was a **whole merged milestone behind `origin/main`** — about 1 700 lines across 18 files that belonged to the previous delivery. Corrected mid-flight; none of them had reported outside the real scope, but the round could have been wasted. In a repo with worktrees, the base is `origin/<base>`.

## Where it lives

The test and its mutation table live in the doc comment of `closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal`, in `mod tests` of `crates/term-core/src/lib.rs`. Both rounds are recorded in the work's artifacts under `.claude/work/62-hangs-on-closing-a-tab/`.
