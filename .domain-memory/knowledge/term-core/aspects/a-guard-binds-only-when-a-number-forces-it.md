---
id: asp_a-guard-binds-only-when-a-number-forces-it_h2had9
slug: a-guard-binds-only-when-a-number-forces-it
name: >-
  A guard binds only when a number forces it: the ratchet's tolerance went to
  zero and fired twice the same day, and two more guards were found satisfied by
  nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T17:43:53.647Z'
updated_at: '2026-09-07T17:43:53.647Z'
last_verified: '2026-09-07T17:43:53.647Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/tests/prose.rs
  - crates/app/tests/glossary.rs
  - crates/app/tests/scripts.rs
  - docs/02-decisiones.md
symbols:
  - OVER_CEILING_TODAY
  - no_entry_of_the_ratchet_is_stale
  - production_lines
  - COMMENT_LINES_AT_LEAST
  - GLOSSARY_CLEAN_FILES
content_hashes:
  crates/app/tests/shape.rs: 'sha256:3518e642c9eb392f687f8342964d7f0d35668c7907d2e0e46e0e3cc17615549a'
  crates/app/tests/prose.rs: 'sha256:cd800b209129b690083c67aaea71bb374c1e470b7f24533c1a9ed48c3f00945e'
  crates/app/tests/glossary.rs: 'sha256:e81cee638e6d52574a447c8be39bd0c41afb992e35d9f45e0c2dfbfacb6eb597'
  crates/app/tests/scripts.rs: 'sha256:e79bbe5ce7c7990529def9645a3b75653eb943728eb566968f73d76c6ae954a5'
  docs/02-decisiones.md: 'sha256:72862e89835bb5f95229c18221eaa6bc0f712c24fc943eb2122a6f45d696fa6c'
tags:
  - guards
  - ratchet
  - d28
  - d30
  - method
  - code-move
feature_id: feat_term-core_nx0g1r
---

## What it does

**Five deliveries of one train (#146, September 2026) walked the same defect three times in three different guards.** [[a-ratchet-with-slack-ratchets-late]] records the first one as a problem; this is what closed it, plus the two the same train turned up.

### 1. A tolerance means nobody is forced to re-measure. An equality fires.

`no_entry_of_the_ratchet_is_stale` asserted `at - real <= 200` — two thirds of the `impl` ceiling. Consequences, all observed rather than argued:

- **Nothing forced a delivery to lower the number.** One took 96 production lines out and could have left the record untouched; the guard would have stayed green. It was lowered because somebody measured, not because anything asked.
- **A number stale for weeks passed unnoticed.** `src/lib.rs::impl Pane` recorded **443** against a real **442** and had never been checked against the tree — the entry an issue had been open about since the tolerance was first questioned.
- **Restoring an upstream branch's older figures passed too.** A rebase reinstated «the pointer's router took 296 more» where the merged figure was **430**; with 296 the ratchet's own story accounts for 1 619 instead of 1 434, so **134 lines vanish from the only record of where the file's mass went** — and the guard was green.

The last delivery replaced it with `assert_eq!(*at, real)`. **It fired twice within the hour**, and the second time is the interesting one: a fix meant to *satisfy* the review added a two-line crate-root re-export to the very file the train exists to shrink, and the guard refused it. Moving that import inside `#[cfg(test)] mod tests` cost zero production lines. **The guard chose the design.**

> A recorded number under a tolerance is a claim. Under an equality it is a measurement, because nothing else compiles.

### 2. Production code after `mod tests` is invisible to the size guard

`production_lines` **breaks** at the first line starting `mod tests`. So a block appended to the *end* of a module — after its test module — is measured by nothing: no module ceiling, no `impl` ceiling, no ratchet. The train hit this for real: **fifty production lines** landed there, everything compiled and all six guard lists passed.

**Where a moved block sits inside its destination is not cosmetic.** Production code goes above the tests or it does not exist as far as every ceiling is concerned. (It also means one line — the `#[cfg(test)]` attribute above `mod tests` — *is* counted, which is enough to move a number that an equality now checks.)

### 3. A list of files defends nothing until a number defends the list

Three of this crate's guards are constant arrays of files. Unlisting a file silences the guard **unless a floor holds the length**:

| Guard | State when the train found it |
|---|---|
| `scripts.rs::MODULES` | **no floor at all** — two new files went unwatched, suite green |
| `prose.rs` corpus floor | 12 500 against a corpus of 12 908 — **713 comment lines outside the walk**, green |
| `glossary.rs::GLOSSARY_CLEAN_FILES` | floor of `>= 5` over twenty-three entries — still unfixed, filed |
| `settings.rs`'s sentence walk | `[(&str, &str); 9]` — a **fixed-size array**, so a removal is a *compile error*. The strongest of them all, by accident of type |

By the end every length floor sat exactly at its real count (19/19, 23/23, 28/28), so no file can leave for free. **The strongest form is not a floor at all — it is a type that cannot hold the wrong number of elements.**

### And the shape that has no fix yet

`prose.rs` states, in prose, the list of files carrying fewer comment lines than the floor's slack. The slack is *corpus minus floor*, so **every comment anybody writes anywhere moves it**: the list changed three times during a single delivery and grew from fourteen files to seventeen. A number over a corpus cannot be stated as a list of names; it needs a per-file table. Filed, not fixed.

## Where it lives

`crates/app/tests/shape.rs` holds `OVER_CEILING_TODAY`, the `production_lines` walk that breaks at `mod tests`, and `no_entry_of_the_ratchet_is_stale` — now an `assert_eq!`. The three file lists are `crates/app/tests/prose.rs` (`SOURCES` and `COMMENT_LINES_AT_LEAST`), `crates/app/tests/glossary.rs` (`GLOSSARY_CLEAN_FILES` and `CITATIONS_CHECKED`) and `crates/app/tests/scripts.rs` (`MODULES`); the fixed-size one is the `SOURCES` array inside `crates/app/src/settings.rs`'s test module. The rule the whole thing serves is **D28** in `docs/02-decisiones.md`, whose order table this train closed, and the mutation discipline behind it is **D30**.
