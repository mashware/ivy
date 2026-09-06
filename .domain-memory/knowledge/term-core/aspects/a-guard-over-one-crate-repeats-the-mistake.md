---
id: asp_a-guard-over-one-crate-repeats-the-mistake_8b3wrt
slug: a-guard-over-one-crate-repeats-the-mistake
name: >-
  A guard that watches one crate repeats the mistake it exists to stop — and a
  guard needs a floor for its REACH, not only for what it finds
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T17:07:24.070Z'
updated_at: '2026-09-06T17:07:24.070Z'
last_verified: '2026-09-06T17:07:24.070Z'
file_paths:
  - crates/app/tests/scripts.rs
  - crates/app/tests/shape.rs
  - crates/term-core/src/lib.rs
symbols:
  - judge
  - MODULES
  - SCRIPTS_AT_LEAST
  - JUDGED_AT_LEAST
  - MARKERS_AT_LEAST
  - LINES_NAMING_A_MODE_COMMAND_AT_LEAST
content_hashes:
  crates/app/tests/shape.rs: 'sha256:dacd8ab2296ccc7fdae225663020f07a83cc0454f098024665c9be83731910a5'
  crates/term-core/src/lib.rs: 'sha256:21899fbd18b28515f01ca0c0c025ca2b260981459a259167fbc21b961f6070c7'
tags:
  - testing
  - guards
  - mutation
  - cross-crate
feature_id: feat_term-core_nx0g1r
---

## What it does

**The defect that cost fifteen false greens in `app` had already been found, fixed and explained in `term-core` weeks earlier** — comment, reason and measured number included — and nobody carried it across. That, and not the `;`, is the reusable finding: [[the-sibling-mould-brings-its-decisions-and-its-gaps]] records that copying a sibling imports its gaps; this is the other half, **not copying it leaves its fixes behind**.

> **So the guard written to stop the recurrence was first written over one crate, which would have repeated the mistake exactly.** It was caught in review. It now reads both, and it was verified by reverting `term-core`'s own script to `;` and watching it name the file and the line.

### Four things that make a source-reading guard actually guard, each found by mutating it

**1. A floor for what it FINDS, and a separate floor for what it REACHES.** A guard that asserts «this list is empty» is satisfied by a walk that examined nothing. Measured: forcing the match to `false` in a test that asserted no line escapes the scanner left it **green**. Counting the lines it looked at is what says the walk happened at all. Three floors of that guard turned out **dominated** by a fourth and are kept only for their error message — which is worth saying out loud, because a floor that cannot fail reads exactly like one that can.

**2. A fixture list must call the rule, not re-implement it.** The first version's mutation test spelled «broken = the list is non-empty» out for itself. Measured: with the rule neutered **and** the original defect restored in the source, **all three tests stayed green over a live bug**. One function, called by the walk and by the fixtures.

**3. A justification written in a comment with nothing under it is the same defect, one level up.** The comment above the rule named the exact script that forces it to take the *last* marker and not the first. Reverting that one word left every test passing. **The claim was made and nothing held it.**

**4. A parser inside a guard fails in ways a normal test does not.** Walking bytes and then slicing the string panics the moment the index lands inside a multibyte character — in a tree that writes `«»` and `—` into its literals, the first script carrying one turns the guard from «reports the defect» into «the test binary aborts».

### And the narrowness is the feature

The rule judges a literal **only** when it both sets a mode and later prints a text some test waits for. Loop syntax (`while :; do stty size; …`) and scripts whose failure already shows red are let through **by construction**, and a test asserts that, so the rule cannot quietly widen into a ban on the character. A guard that fires on correct code gets deleted, and then it guards nothing.

## Where it lives

`crates/app/tests/scripts.rs`, beside `shape.rs` (module size) and `prose.rs`/`glossary.rs` (language). `MODULES` pulls in both crates with `include_str!` so a renamed or deleted file breaks the build instead of being skipped. **D30** carries the decision.
