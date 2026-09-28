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
updated_at: '2026-09-06T19:32:33.883Z'
last_verified: '2026-09-06T19:32:33.883Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/src/lib.rs
  - docs/02-decisions.md
symbols:
  - OVER_CEILING_TODAY
content_hashes:
  crates/app/tests/shape.rs: 'sha256:dacd8ab2296ccc7fdae225663020f07a83cc0454f098024665c9be83731910a5'
  crates/app/src/lib.rs: 'sha256:06780a0eaae7013bb6602574e7d50386ede8e464b226f9458287d5071106f017'
  docs/02-decisions.md: 'sha256:96121e6d8650618d166ab688841fe0b2e590d7e6b16a0f4491c8735d188870d7'
tags:
  - guards
  - ratchet
  - mislabelled-number
  - review
feature_id: feat_term-core_nx0g1r
---

## What it does

**D28's ratchet on `crates/app/src/lib.rs` recorded 2 804 production lines while the file was at 2 801.** Three lines of room in which any delivery could grow the file **and pass**, under a rule whose whole point is that the number may only go down.

The delivery that held an over-long paste spent two of them — while its own comments claimed the file *«may only shrink»* and *«ends at the 2 804 it started at»*. **Both were false.** It was found by a comment reviewer who **counted the file on `main`** instead of trusting the record, which is the only way it could have been found: the guard was green throughout, and every human-readable claim in the diff agreed with the guard.

> **A recorded number that is not the measured number is a guard that fires on the fourth offence** — and the three that got through leave no trace, because passing is silent.

### Why it went stale, and why that will happen again

The record is written by hand when a delivery lowers it. A delivery that takes out **more** than it planned, or whose formatter reflows a few lines, leaves the record above the truth and nobody re-measures: the guard is green, which is exactly what it was supposed to be. **Slack accumulates in the direction the rule forbids, invisibly.**

### What closes it

- **The entry now holds what the file actually is**, and its reason says why the number moved.
- **The other two entries of that table have never been checked against the tree.** They are exactly as likely to be stale, for the same reason.
- The general shape: **a guard that records a number needs a test that the number is the measurement**, not only that the measurement is under the number. `shape.rs` has `no_entry_of_the_ratchet_is_stale`, and it only fires **upward**.

This is the same family as [[a-cited-number-is-copied-from-the-docblock-next-door]] — a figure that stops matching what it names, kept alive by everything around it agreeing with it — with the aggravation that here the figure is what a guard enforces.

## Where it lives

`crates/app/tests/shape.rs`, `OVER_CEILING_TODAY`. The entry's own reason carries the autopsy so the next reader meets it there.
