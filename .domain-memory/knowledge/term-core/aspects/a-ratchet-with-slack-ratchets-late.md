---
id: asp_a-ratchet-with-slack-ratchets-late_hhrtnw
slug: a-ratchet-with-slack-ratchets-late
name: >-
  A ratchet whose recorded number is not the measured number fires on the fourth
  offence — and stays green while a delivery grows the file it forbids growing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T19:32:33.883Z'
updated_at: '2026-11-28T17:49:33.978Z'
last_verified: '2026-11-28T17:49:33.978Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/src/lib.rs
  - docs/02-decisions.md
symbols:
  - OVER_CEILING_TODAY
content_hashes:
  crates/app/tests/shape.rs: 'sha256:ac0d279da8cc37dfc910fec710eea84978f410fd6367df4b69185db166196dfe'
  crates/app/src/lib.rs: 'sha256:71f680d1eb63bce74513252e59d4cbb370074d074fab378619d8242ab298cb68'
  docs/02-decisions.md: 'sha256:c96630f7c02a4c7f1812d7c81ca30f189cad5213725739e9a31b23e97cc7cbeb'
tags:
  - guards
  - ratchet
  - mislabelled-number
  - review
feature_id: feat_term-core_nx0g1r
---

## What it does

**D28's ratchet on `crates/app/src/lib.rs` recorded 2 804 production lines while the file was at
2 801.** Three lines of room in which any delivery could grow the file **and pass**, under a
rule whose whole point is that the number may only go down.

The delivery that held an over-long paste spent two of them — while its own comments claimed the
file *«may only shrink»* and *«ends at the 2 804 it started at»*. **Both were false.** It was
found by a comment reviewer who **counted the file on `main`** instead of trusting the record,
which is the only way it could have been found: the guard was green throughout, and every
human-readable claim in the diff agreed with the guard.

> **A recorded number that is not the measured number is a guard that fires on the fourth offence** — and the three that got through leave no trace, because passing is silent.

### Why it went stale, and why that will happen again

The record is written by hand when a delivery lowers it. A delivery that takes out **more** than
it planned, or whose formatter reflows a few lines, leaves the record above the truth and nobody
re-measures: the guard is green, which is exactly what it was supposed to be. **Slack
accumulates in the direction the rule forbids, invisibly.**

### What closes it

- **The entry now holds what the file actually is**, and its reason says why the number moved.
- The follow-up replaced the stale-entry tolerance with exact equality for every ratchet entry. `no_entry_of_the_ratchet_is_stale` checks that the recorded size equals the measured size; see [[a-guard-binds-only-when-a-number-forces-it]] for that closure.
- The general shape remains: a guard that records a number needs a test that the number is the measurement, not only that the measurement is under the number.

This is the same family as [[a-cited-number-is-copied-from-the-docblock-next-door]] — a figure
that stops matching what it names, kept alive by everything around it agreeing with it — with
the aggravation that here the figure is what a guard enforces.

## Where it lives

`crates/app/tests/shape.rs`, `OVER_CEILING_TODAY`. The entry's own reason carries the autopsy so
the next reader meets it there.
