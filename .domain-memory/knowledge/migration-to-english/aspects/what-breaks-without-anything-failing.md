---
id: asp_what-breaks-without-anything-failing_2ne2h4
slug: what-breaks-without-anything-failing
name: >-
  Three breakages that neither the compiler nor the suite sees: length as the
  test payload, the keys that match by text, and the two-halved `assert` whose
  halves are edited together
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:13:22.483Z'
updated_at: '2026-09-03T16:13:22.483Z'
last_verified: '2026-09-03T16:13:22.483Z'
file_paths:
  - crates/app/examples/screen_demo.rs
  - crates/app/src/fonts.rs
  - crates/app/src/session.rs
  - crates/app/src/measure.rs
symbols:
  - ICON_FAMILY
  - SYSTEM_PREFIX
  - FILE
content_hashes:
  crates/app/examples/screen_demo.rs: 'sha256:49fe72a81b94223134f9eb25135cbc3032e3bde5ddaee2415b6c0ed57673b422'
  crates/app/src/fonts.rs: 'sha256:e64e2947f2bc20575da8a4a16877b532af56a559637f28262753778f7df63c98'
  crates/app/src/session.rs: 'sha256:b7824b96a3e438e7695ed87ccecbe9d65ed9dd16258b5e74ba3c39c8008f09e3'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
tags:
  - d27
  - f2.12
  - guards
  - mutation
  - runtime-keys
feature_id: feat_migration-to-english_xnx8mc
---

## What it does

**All three came out of looking for what a rename could break, and none of them is a rename.**

### 1. Literals whose LENGTH is the test payload

`crates/app/examples/screen_demo.rs` builds the screen the painter is eyeballed with, and some of its rows are calculated so that **a wide character falls exactly on the edge of column 80**. Its own comment says so: without that row, nobody looks at the branch of the painter that deals with that gap. **That file has zero `#[test]`.**

> Translating that text —or simply rewriting it— moves the wide character somewhere else and **disables the branch silently**: nothing fails, there is no test to fall, and the gap is only seen by looking at the screen. A test text like that **is measured in columns, not in words**.

This is how it was handled: the delivery that translated `app` declared that file an **island**, translated its comments and **did not touch a single literal**. Verified on reviewing it: the literals arrived identical and there are 78 cells before the wide character. What still does not exist is the guard: **nothing pins that width down**, and the cheapest thing would be a `#[cfg(test)]` inside the example itself.

### 2. Constants that are runtime keys

`ICON_FAMILY = "iconos-del-sistema"` and `SYSTEM_PREFIX = "sistema:"` are registered as **text** in the font engine and are looked up by that same text. Renaming the constant without the value —or the other way round— breaks the matching **at run time, and compiles perfectly**. There is no type in between.

> **And here there is a correction that had better be written down, because the milestone's record came to claim the opposite.** It was noted that renaming them «could stop painting the icons without anything catching it», and that **is false**: every consumer goes through the symbol, so a rename is checked by the compiler, and `fonts.rs:1609` pins `"sistema:Hack"` by hand while production builds it from `SYSTEM_PREFIX` — renaming it goes **red immediately**. The guard exists. Deferring them out of scope discipline is reasonable; deferring them for that reason misleads whoever picks the work up.

### 3. A test that looks like the net and is a disguise

There was an `assert_eq!(FILE, "layout.json")`. It looks like it ties the name of the file down and **it ties down nothing that matters**: it asserts that the constant equals a literal written **right beside it**, not that it equals what the measurement script seeds on disk. Both halves of the `assert` get renamed, the suite stays green, and the script goes on writing the old name.

**Closed, and the fix is the pattern to copy**: `measure.rs:2879` asserts `SCRIPT.contains(crate::session::FILE)` over the text of the script read with `include_str!` — one half **derived** from the other instead of two halves written together. Its comment explains the disguise it replaces.

### The rule the three of them leave together

> When looking for «what breaks without anything failing» in this project, looking at types and literals is not enough. One also has to ask **who matches two strings at run time** and **which text has a significant length** — and to distrust every test that compares two halves the same editing pass can change at once.

## Where it lives

`crates/app/examples/screen_demo.rs` — the rows whose length is the payload, without a single test in the file.

`crates/app/src/fonts.rs` — `ICON_FAMILY`, `SYSTEM_PREFIX`, the color texture, and at `:1609` the assertion that pins the prefix by hand and acts as a guard without declaring itself one.

`crates/app/src/measure.rs:2879` — the pattern to copy: one half derived from the other.

`docs/02-decisions.md` — **AC14** of milestone F2.12 (the runtime keys still match after the rename), which is still not closed.
