---
id: asp_splitting-in-parallel-fails-between-files_tekr4a
slug: splitting-in-parallel-fails-between-files
name: >-
  Splitting a translation across parallel agents does not fail inside a file:
  it fails BETWEEN files, and the glossary guard cannot see it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:07:08.972Z'
updated_at: '2026-09-03T16:07:08.972Z'
last_verified: '2026-09-03T16:07:08.972Z'
file_paths:
  - crates/app/src/panel.rs
  - crates/app/src/measure.rs
  - crates/app/src/session.rs
  - crates/app/tests/glossary.rs
  - crates/app/tests/prose.rs
  - docs/02-decisiones.md
symbols:
  - set_aside
  - PanelTiling
  - Tiling
  - 'Action::SplitRight'
  - GLOSSARY_CLEAN_FILES
content_hashes:
  crates/app/src/panel.rs: 'sha256:0cd7f08563db761ba4ba5eaa9a622b5f190737996094a503ebb2f0914785c966'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  crates/app/src/session.rs: 'sha256:b7824b96a3e438e7695ed87ccecbe9d65ed9dd16258b5e74ba3c39c8008f09e3'
  crates/app/tests/glossary.rs: 'sha256:bb3266feceb40eb1c3c4847deb16921f3432c141ca48f81cb8d8ab367e00fa32'
  crates/app/tests/prose.rs: 'sha256:b3b194d61079eaeef5626f13b7b1d59f308fe619c376a967181913fc0c47b441'
  docs/02-decisiones.md: 'sha256:4eb23bb4f11a5f60d8d236644be773469b8f5a617d1657294e0b9d39d4c9872a'
tags:
  - d27
  - f2.12
  - translation
  - glossary
  - guards
  - fan-out
feature_id: feat_migration-to-english_xnx8mc
---

## What it does

**Every translator was consistent with itself and none of them was consistent with the others.** Delivery #12 (prose of `app` minus `lib.rs`, September 2026), seven translators in parallel. D27 decides that **the thing is a `Pane` and `split` is the gesture**, and `split` was already spent in the project (`Action::SplitRight`, `key.split-right`, `close-pane`):

| File | «pane» (correct) | «split» as a noun (violation) |
|---|---|---|
| `measure.rs` | 34 | **0** |
| `panel.rs` | **0** | **27** |
| the other nine | mixed | 14 |

A reviewer who opens `measure.rs` finds it flawless. One who opens `panel.rs` finds it broken from top to bottom. **Checking inside a file proves nothing about the fan-out.**

Same with `reparto`, which the glossary maps to `Tiling`: it came out as «split» in 5 places and as **«division»** in 6 — exactly the word D27 retired for the thing. **The fan-out resurrected the retired term with another meaning**, which is worse than leaving it in Spanish: in Spanish the guard would have seen it.

### It gives itself away in adjacent lines, and that is the only signal to the naked eye

- `panel.rs:85` — *«Splits a split's rectangle»*, above `pub fn set_aside(pane: Rect, …)`. **The parameter was already called `pane`.**
- `panel.rs:42` — *«the floor that decides whether a split can be split in two»*: the same word for the thing and for the gesture, in a single sentence.
- `panel.rs:53` — *«It is the floor of the **gesture**, not of the split»*: a sentence whose only purpose is to separate gesture from thing, calling the thing by the name of the gesture.

### Why no guard catches it, and this is the half that matters

`glossary.rs` catches **retired Spanish names**. A **badly chosen English word** is invisible to it by construction: `split` is English, it is in the glossary and it is a live identifier. **The guard was green with 41 violations inside it.** And the prose was invisible to it by declaration as well — its header said that comments, rustdoc and literals are out of its scope — so until `prose.rs` was written nothing stopped Spanish from being put back into an already translated file.

> **The operational consequence: after a parallel fan-out, the sweep goes BY TERM AND ACROSS EVERY FILE, counting and not reading.**

### And counting has a trap of its own: the noun is not the verb

The first count said **60** places because it was counting «splits the batch», which is the **verb** and is correct. There were **41**. There are ~65 legitimate uses of the verb or the gerund in those same files — «splits the text», «can be split in two», «splitting requires a human gesture» — so **a `sed` of `split`→`pane` corrupts more than it fixes**. Checked: a sweep anchored on the determiner slipped in all the same at «the effects are panes and layouts», where the original said `repartos`, that is «tilings».

### Method corollary, from the same case

What a translation delivery **claims about itself** is verified by counting, not by reading its brief. #12 declared «the identifiers were already English» and had renamed four test functions; it declared **three** islands of Spanish and there were **five** — the two missing ones were correct and useful, and precisely because they were not declared the next delivery would have broken them without warning.

## Where it lives

**The glossary that decides**: `docs/02-decisiones.md`, annex of D27 — `:3457` and `:3470` («the thing is a `Pane`, and the gesture is `split`», with Ghostty cited as the terminal that uses `split` for the thing, that is, the option the decision weighed and discarded), `:3494` and `:3504` (`Reparto`→`Tiling`), `:3553` (`Division`→`Pane`), `:3524` (the batch letters are not translated).

**The places that give themselves away**: `crates/app/src/panel.rs:42`, `:53`, `:85`.

**The guards and their scope**: `crates/app/tests/glossary.rs` (identifiers; its header declares prose out of scope) and `crates/app/tests/prose.rs` (the prose, since #12).

**The two islands that were not declared**: `crates/app/src/keyboard.rs:1255-1258`, the retired Spanish action names that a guard claims the terminal **rejects**; and `crates/app/src/settings.rs:1668-1670`, fixtures of the comment parser.
