---
id: asp_a-guard-on-a-precondition-cannot-fail_jbgne1
slug: a-guard-on-a-precondition-cannot-fail
name: >-
  A bench guard that reads a count of the precondition instead of the effect
  cannot fail — and narrowing the count breaks the guard of the other direction
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-10-10T22:19:34.169Z'
updated_at: '2026-10-10T22:19:34.169Z'
last_verified: '2026-10-10T22:19:34.169Z'
file_paths:
  - crates/app/src/search.rs
  - crates/app/src/measure/header.rs
  - scripts/measure-frame.sh
  - crates/app/tests/batches.rs
symbols:
  - Searching
  - hand_over
  - mark_matches
  - SEARCH_OPEN
  - SEARCH_MARKED
content_hashes:
  crates/app/src/measure/header.rs: 'sha256:3eac711f75511b682867465a6e191f27260e7a1e3c7c4d43602033c403147da0'
  scripts/measure-frame.sh: 'sha256:29086f8575e32fb1feef78fdba760b140b2fc5341baa166ef00c6d84670412fa'
  crates/app/tests/batches.rs: 'sha256:20d6f96fe0275ccac2acfe69689e8061b4bc8a60d5eac94ebb0b207e1571c78a'
tags:
  - measurement-bench
  - d13
  - f2.13
  - batch-e
feature_id: feat_term-core_nx0g1r
---

## What it does

Two halves of one lesson, both from #177's batch `E` and its `search:` report line.

**1. Guard on the effect, not the precondition.** The first line counted every painting with a search OPEN as «paintings with matches marked», so batch E's guard `SEARCH_MARKED = 0 → invalidate` was satisfied by the empty-query frame after the chord even with `mark_matches` marking nothing — a guard that cannot fail. Fixed by having `Screen::mark_matches` return the cells it marked and publishing two counts: «N paintings with a search open, M with matches marked». Same family as the drags line's «injected · effective».

**2. Then each direction reads its own count.** Once the count was narrowed to the effect, the OTHER direction (`SEARCH != 1` must have done no search) still read the narrowed one, so a non-E run with a search open on a query that matches nothing on screen — painting its row inside the console's span — passed clean. It reads `SEARCH_OPEN` now. And `tests/batches.rs` asserts the exact conditions (`… || … || …` and the reverse), because naming each count separately stays green when `||` becomes `&&`.

Evidence: silent-failure reviewer (round 1) and silent-failure + correctness reviewers (round 2) of #177; fixed in 1de439a and 468100f.

## Where it lives

scripts/measure-frame.sh (the search guard block), crates/app/src/measure/header.rs (`Searching`, the `search:` line), crates/app/src/search.rs (`Cost::hand_over`), crates/app/tests/batches.rs (`batch_e_demands_presses_found_and_marked_and_no_keystroke`).
