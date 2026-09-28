---
id: asp_a-rename-reaches-files-no-sweep-sees_f0dt05
slug: a-rename-reaches-files-no-sweep-sees
name: >-
  Renaming a document is not finished when the tracked tree is clean: the
  untracked CLAUDE.md, FLOW.md and staging files cite it too, and no guard sees
  them
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T22:46:14.442Z'
updated_at: '2026-09-28T22:46:14.442Z'
last_verified: '2026-09-28T22:46:14.442Z'
file_paths:
  - crates/app/tests/prose.rs
  - crates/app/tests/common/mod.rs
  - docs/02-decisions.md
symbols:
  - every_document_has_a_row_and_an_english_name
  - names_in_spanish
  - markdown_below
content_hashes:
  crates/app/tests/prose.rs: 'sha256:3ce39d39bbf9d44e6542d61c9a9caf682dc064f5dc965f79c7b711d98407f68c'
tags:
  - d27
  - rename
  - untracked
  - guard-blind-spot
feature_id: feat_migration-to-english_xnx8mc
---

## What it does

**Renaming a document under `docs/` is not finished when `git grep` comes back clean.** Three places cite documents by path and are invisible to `git grep`, to `include_str!` and to every guard in the suite, because none of them is tracked:

- the main checkout's `CLAUDE.md` — its «Before proposing anything, read» table is the first thing every session loads;
- the main checkout's `FLOW.md` — its design note tells an agent to **edit** the decisions record by path;
- `.domain-memory/staging/*.jsonl` — findings waiting to be consolidated, which a later consolidation writes back into tracked cards with the old path.

> **FLOW.md is the dangerous one.** An agent told to edit a file that no longer exists creates it, with no error, and the decisions record splits in two: the guards (`decisions.rs`, `glossary.rs`, `prose.rs`) read only the renamed file through `include_str!`, so whatever lands in the recreated one escapes all of them and the suite stays green.

**The order matters: update them after the merge, not before.** The main checkout stands on `main`, and until the PR merges `main` has no file at the new path, so a `CLAUDE.md` edited early points every session at a file that does not exist yet.

Measured on #150 (September 2026): after renaming seven documents and `prototipos/`, the tracked tree had no old name left except deliberate history, while `CLAUDE.md` still cited five old paths, `FLOW.md` two, and seventeen staging files several. Every reviewer of three rounds flagged it, and none could have been stopped by a test.

**A second trap from the same delivery: a sweep by filename misses citations by description.** Deleting `docs/DEBT.md` and grepping for `DEBT.md` left three sentences saying «the debt log» and one saying «filed rather than fixed» in the present tense — found only by a reviewer reading the prose.

## Where it lives

The untracked files live only in the main checkout (`/CLAUDE.md`, `/FLOW.md`, `/.domain-memory/staging/`). The tracked side is held by `every_document_has_a_row_and_an_english_name`, `names_in_spanish` and `the_names_are_judged_where_the_guard_says` in `crates/app/tests/prose.rs`, with `markdown_below` in `crates/app/tests/common/mod.rs`. The decision and its accepted limits are D27's amendment of 28 September 2026 in `docs/02-decisions.md`.
