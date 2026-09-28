---
id: asp_a-dated-correction-is-a-claim-too_b43wkb
slug: a-dated-correction-is-a-claim-too
name: >-
  A dated correction in the decisions record is a claim about the present too —
  and three of one review's blockers were the corrections themselves
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T16:49:54.503Z'
updated_at: '2026-09-28T16:49:54.503Z'
last_verified: '2026-09-28T16:49:54.503Z'
file_paths:
  - docs/02-decisions.md
  - crates/app/tests/glossary.rs
  - crates/app/tests/decisions.rs
symbols:
  - NOT_OURS
content_hashes:
  docs/02-decisions.md: 'sha256:b53044393a7694d2c1f56d6f41680d1dd0df011e6f7448de57e3b6d5edc66e4d'
  crates/app/tests/glossary.rs: 'sha256:90d6c32c3639ed72476a08556a5782aedb6529d909f3629eb7791aafedf52ae2'
tags:
  - decisions-record
  - docs
  - review
  - comments-that-lie
feature_id: feat_term-core_nx0g1r
---

## What it does

**The record adds with a date, it does not edit backwards** — so a claim a later milestone made false gets a `> **Corrected on <date>: …**` note beside it. That note is written to *remove* a false sentence, and it is exactly as able to add one. Measured on #147 (28 September 2026): of the first review's three blockers, **all three were the new corrections**, none the old claims:

- D11's note said D29's backlog is «said on screen». It is **one line to standard error**; only D60's refusal paints over the console.
- D44's new paragraph said «the vocabulary guard exempts `split_*` by name». They sit in `glossary.rs`'s `NOT_OURS`, which serves the **citation** guard; no vocabulary rule would flag them.
- D11's note cited **D60** as the ceiling while **D29**, the decision it builds on, still said «the queue carries no ceiling» with no note of its own.

> **A correction is prose about present behaviour**, the kind `CLAUDE.md` calls a test that does not run. Check each clause against the code before writing it, the same as the claim it replaces. **And when a correction cites a later decision, the decision it amends needs its own dated note** — otherwise the record now contradicts itself one section apart.

Two more habits the same delivery paid for:
- **A slip inside a historical annex is not corrected in place** either: the delivery's «23 143 lines» stays, with a dated «checked» note giving the real 23 142 (`git cat-file` over the revisions on either side). A squashed branch can hide an intermediate count, so editing the figure would claim the delivery miscounted.
- **Where a decision sits is part of what it says.** D11 had been spliced into D3's annexes, so D3's closing verdict read as D11's. `crates/app/tests/decisions.rs` now fails on a heading out of numeric order, repeated, missing, or written in any form its walk would not read (`## D61:`, a no-break space, an indent, `D 61`) — the newest decision written that way is otherwise invisible, because no gap follows it.

## Where it lives

`docs/02-decisions.md` (the dated notes in D1, D3's 9 August annex, D9, D11, D29, D44 and the big-file annex of D27); the order guard is `crates/app/tests/decisions.rs`; `NOT_OURS` is in `crates/app/tests/glossary.rs`.
