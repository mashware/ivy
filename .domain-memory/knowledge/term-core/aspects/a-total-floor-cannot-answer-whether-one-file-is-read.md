---
id: asp_a-total-floor-cannot-answer-whether-one-file-is-read_z4b4cv
slug: a-total-floor-cannot-answer-whether-one-file-is-read
name: >-
  A guard that walks a list of files needs two floors: the total one cannot
  catch the failure it was written for, because its slack is wider than most
  single files
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:22:54.158Z'
updated_at: '2026-09-11T20:22:54.158Z'
last_verified: '2026-09-11T20:22:54.158Z'
file_paths:
  - crates/app/src/settings.rs
  - crates/app/tests/prose.rs
symbols:
  - COMMENT_LINES_AT_LEAST
  - COMMENT_LINES_PER_FILE_AT_LEAST
  - SOURCES
content_hashes:
  crates/app/src/settings.rs: 'sha256:b4bcf5132ae7c60dc9cccb4fedae1c93d77362d46527808e07d10c98d53a1040'
  crates/app/tests/prose.rs: 'sha256:6a3dff196aac01ef74b59d6dc3d077e0ddd4c211cae097434f47b9b3a542f533'
tags:
  - guards
  - floors
  - d27
  - d28
feature_id: feat_term-core_nx0g1r
---

## What it does

**A floor over the *total* answers «the walk still works». It cannot answer «this file is still being read»**, because its slack is by construction wider than most single files in the list — so a split that moves one file's contribution into a module nobody added drops the total by less than the slack and the guard stays green.

**Measured on the sentence walk inside `crates/app/src/settings.rs`** — the sixth of the lists a module split breaks, see [[the-sixth-list-lives-in-production-code]]. With eleven files carrying 27, 15, 10, 6, 5, 5, 4, 3, 2, 1 and 1 literals and a total floor five below the corpus, **four of the eleven sit inside that slack**. Dropping `mouse.rs` from the list left the guard green at 77 against 74; dropping two files together turned it red. `tests/prose.rs` had already paid for this lesson and carries both floors; this walk had only half of it.

> **The second floor is per file and it is `>= 1`, not a table**: two of the files carry a single literal each, so any higher number stops being a floor and becomes a maintained list of counts — which is the flaw these guards keep rediscovering, and is [issue #183](https://github.com/mashware/agent-terminal/issues/183).

**And the same delivery measured the other half of the trap.** `tests/prose.rs`'s **total** floor had drifted **2 392 lines** behind the corpus — 13 300 against a real 15 692 — so the guard whose entire argument is that its slack is what it defends had none left to defend. Nothing had failed; the number is written by hand and only the delivery that lowers it re-measures.

> **A floor is re-measured by the delivery that touches the list, not by the one that happens to read the comment.** Same family as [[a-ratchet-with-slack-ratchets-late]], applied to a corpus instead of a line count.

## Where it lives

`crates/app/src/settings.rs`, inside its `mod tests`: the `SOURCES` array, the total `sentences.len() >= N` and the per-file loop beside it. Its sibling with both floors is `crates/app/tests/prose.rs` (`COMMENT_LINES_AT_LEAST`, `COMMENT_LINES_PER_FILE_AT_LEAST`).
