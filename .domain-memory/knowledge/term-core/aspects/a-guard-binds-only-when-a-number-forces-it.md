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
updated_at: '2026-11-28T17:49:36.594Z'
last_verified: '2026-11-28T17:49:36.594Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/tests/prose.rs
  - crates/app/tests/glossary.rs
  - crates/app/tests/scripts.rs
  - docs/02-decisions.md
symbols:
  - OVER_CEILING_TODAY
  - no_entry_of_the_ratchet_is_stale
  - production_lines
  - COMMENT_LINES_AT_LEAST
  - GLOSSARY_CLEAN_FILES
  - measured
  - ceiling_of
content_hashes:
  crates/app/tests/shape.rs: 'sha256:ac0d279da8cc37dfc910fec710eea84978f410fd6367df4b69185db166196dfe'
  crates/app/tests/prose.rs: 'sha256:6f02c546d10b931caef8be226773d3374ecfc2adc511b33782e1ae8fbc5f2ae1'
  crates/app/tests/glossary.rs: 'sha256:e38f751aff77f31376eb0a5697d9597f1a551971a234affa292acd625dfdba68'
  crates/app/tests/scripts.rs: 'sha256:c4a66721886bf58f9f0c1f87f10c888a7aa158313f32a1328ba22dde1a4f78ac'
  docs/02-decisions.md: 'sha256:c96630f7c02a4c7f1812d7c81ca30f189cad5213725739e9a31b23e97cc7cbeb'
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

**Five deliveries of one train (#146, September 2026) walked the same defect three times in
three different guards.** [[a-ratchet-with-slack-ratchets-late]] records the first one as a
problem; this is what closed it, plus the two the same train turned up.

### 1. A tolerance means nobody is forced to re-measure. An equality fires.

`no_entry_of_the_ratchet_is_stale` asserted `at - real <= 200` — two thirds of the `impl`
ceiling. Consequences, all observed rather than argued:

- **Nothing forced a delivery to lower the number.** One took 96 production lines out and could have left the record untouched; the guard would have stayed green. It was lowered because somebody measured, not because anything asked.
- **A number stale for weeks passed unnoticed.** `src/lib.rs::impl Pane` recorded **443** against a real **442** and had never been checked against the tree — the entry an issue had been open about since the tolerance was first questioned.
- **Restoring an upstream branch's older figures passed too.** A rebase reinstated «the pointer's router took 296 more» where the merged figure was **430**; with 296 the ratchet's own story accounts for 1 619 instead of 1 434, so **134 lines vanish from the only record of where the file's mass went** — and the guard was green.

The last delivery replaced it with `assert_eq!(*at, real)`. **It fired twice within the hour**,
and the second time is the interesting one: a fix meant to *satisfy* the review added a two-line
crate-root re-export to the very file the train exists to shrink, and the guard refused it.
Moving that import inside `#[cfg(test)] mod tests` cost zero production lines. **The guard chose
the design.**

> A recorded number under a tolerance is a claim. Under an equality it is a measurement, because nothing else compiles.

### 2. Production code after `mod tests` is invisible to the size guard

`production_lines` **breaks** at the first line starting `mod tests`. So a block appended to the
*end* of a module — after its test module — is measured by nothing: no module ceiling, no `impl`
ceiling, no ratchet. The train hit this for real: **fifty production lines** landed there,
everything compiled and all six guard lists passed.

**Where a moved block sits inside its destination is not cosmetic.** Production code goes above
the tests or it does not exist as far as every ceiling is concerned. (It also means one line —
the `#[cfg(test)]` attribute above `mod tests` — *is* counted, which is enough to move a number
that an equality now checks.)

### 3. A list of files defends nothing until a number defends the list

Three of this crate's guards are constant arrays of files. Unlisting a file silences the guard
**unless a floor holds the length**:

| Guard | State when the train found it |
|---|---|
| `scripts.rs::MODULES` | **no floor at all** — two new files went unwatched, suite green |
| `prose.rs` corpus floor | 12 500 against a corpus of 12 908 — **713 comment lines outside the walk**, green |
| `glossary.rs::GLOSSARY_CLEAN_FILES` | floor of `>= 5` over twenty-three entries — still unfixed, filed |
| `settings.rs`'s sentence walk | `[(&str, &str); 9]` — a **fixed-size array**, so a removal is a *compile error*. The strongest of them all, by accident of type |

By the end every length floor sat exactly at its real count (19/19, 23/23, 28/28), so no file
can leave for free. **The strongest form is not a floor at all — it is a type that cannot hold
the wrong number of elements.**

**But an exact floor is only right over a list somebody edits by hand.** Over a *count the walk
finds* — impl blocks (121), types charged lines (68) — an exact floor fails the change the
ratchet wants: merging two blocks of a type. #187 set those floors to today's exact numbers and
the review caught it; they sit below (100, 55) with today's number in the comment.

### 4. A refusal the real tree never meets cannot be mutated

**Measured on #187**: two checks written inline inside `no_entry_of_the_ratchet_is_stale` —
refuse a type entry whose name is defined twice, and hold a module entry to 600 rather than 300
— stayed **GREEN when deleted**, because no ratchet entry today is ambiguous and no module entry
sits between 300 and 600. A guard over the real crate proves only the states the crate is in.
The fix: the logic goes into a pure function over `&[(&str, &str)]` modules (`measured`,
`ceiling_of`, `unreadable_headers`, `the_one_block`, `type_sizes`) with a test over synthetic
sources, and the real-crate guard just calls it. After that, all fourteen mutations of the
delivery went red.

### The per-file floor closes the remaining shape

The prose guard stores a floor per source file as the third field of `SOURCES`; it checks each
file before the corpus backstop. Its declared paths are also compared with files on disk. Global
corpus slack therefore no longer silently excuses ordinary per-file losses. The earlier changing
list of files below the total floor's slack remains a historical example of why the per-file
table is needed.

## Where it lives

`crates/app/tests/shape.rs` holds `OVER_CEILING_TODAY`, the `production_lines` walk that breaks
at `mod tests`, `no_entry_of_the_ratchet_is_stale` — an `assert_eq!` — and, since #187, the pure
functions `measured`, `ceiling_of`, `type_sizes` and `unreadable_headers` with their synthetic
tests. The three file lists are `crates/app/tests/prose.rs` (`SOURCES` and
`COMMENT_LINES_AT_LEAST`), `crates/app/tests/glossary.rs` (`GLOSSARY_CLEAN_FILES` and
`CITATIONS_CHECKED`) and `crates/app/tests/scripts.rs` (`MODULES`); the fixed-size one is the
`SOURCES` array inside `crates/app/src/settings.rs`'s test module. The rule the whole thing
serves is **D28** in `docs/02-decisions.md`, and the mutation discipline behind it is **D30**.
