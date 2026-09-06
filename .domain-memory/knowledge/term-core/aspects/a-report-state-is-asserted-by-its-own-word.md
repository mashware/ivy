---
id: asp_a-report-state-is-asserted-by-its-own-word_skxfvx
slug: a-report-state-is-asserted-by-its-own-word
name: >-
  A report line with several states is asserted by the word only IT carries:
  written as «it is not the other one», swapping two of the bodies leaves the
  suite green
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T12:42:45.883Z'
updated_at: '2026-09-06T12:42:45.883Z'
last_verified: '2026-09-06T12:42:45.883Z'
file_paths:
  - crates/app/src/measure.rs
  - scripts/measure-frame.sh
symbols:
  - ChoosingTheFont
  - header
content_hashes:
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  scripts/measure-frame.sh: 'sha256:0d3e8ba30e5e7e7750fde4fd84725803c5a98cf5879f8dde30b20974618caef0'
tags:
  - tests
  - mutation
  - measurement
  - guards
feature_id: feat_term-core_nx0g1r
---

## What it does

The measurement report gained a line with four states — no font asked for · found · not installed · installed and unopenable. Its first test asserted, per state, *«the line does not start with «none»»* plus the family name and the elapsed time.

**Swapping the bodies of two of the four arms left the suite green.** Every state carries the family name and every state carries a number, so each assertion was satisfied by any of the other lines. The report could publish *«found»* for a font that was never found, and the guard would not move.

It was caught by a reviewer whose brief was to try the mutation, not by reading.

### The shape that holds

Assert **the word only that state carries**, and assert that **all of them differ**:

```rust
let words = [word_of(&none), word_of(&found), word_of(&missing), word_of(&unusable)];
let distinct: BTreeSet<_> = words.iter().collect();
assert_eq!(distinct.len(), 4, "two states are published under the same word");
```

The set assertion is the load-bearing half: it fails on **any** collapse, including one introduced by a state added later, which a list of four separate `assert!`s does not.

### Why it matters more than a normal green test

This particular line is read by the measurement script to **invalidate a run**. A test that cannot tell the states apart turns the script's guard into a line that is always satisfied — the guard still runs, still passes, and stops catching anything. Same shape as [[the-guard-that-cannot-fail]] wherever it appears: the failure is not the missing assertion, it is a **second** guard downstream quietly losing its subject.

> The sibling rule already in this store: assert **the effect a value has**, never the field where it is recorded. Here the field *is* text, so the equivalent slip is asserting a substring that every variant shares.

## Where it lives

`crates/app/src/measure.rs`, the `chosen font:` line of `header()` and its test `the_four_chosen_font_states_are_distinguished_where_the_script_looks`. The downstream reader is the `chosen font:` guard in `scripts/measure-frame.sh`.
