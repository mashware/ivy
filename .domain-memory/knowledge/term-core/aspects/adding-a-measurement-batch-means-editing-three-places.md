---
id: asp_adding-a-measurement-batch-means-editing-three-places_2xs11n
slug: adding-a-measurement-batch-means-editing-three-places
name: >-
  Adding a batch to measure-frame.sh means editing four places and choosing a
  load that paints nothing new — miss any and the run is invalidated, or worse,
  published
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:06:38.965Z'
updated_at: '2026-09-19T14:30:32.595Z'
last_verified: '2026-09-19T14:30:32.595Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/tests/batches.rs
  - crates/app/src/measure/header.rs
symbols:
  - OPS
  - document_of
  - document_body
  - PATCHED
  - PATCHES_REFUSED
content_hashes:
  scripts/measure-frame.sh: 'sha256:4db2c7bc277a74dc24f341ece091978a1cfe2f69ab4bcfa4074594e4d8ddee30'
  crates/app/tests/batches.rs: 'sha256:2f19f992923c82e0e24d3df1d54c3c0b631dbe41c6de4d5b2593c05e538d81c5'
  crates/app/src/measure/header.rs: 'sha256:10f75f6fe266967fbe060d9c391f47e0b3b5e11f196a86dcc6598c810320944b'
tags:
  - measurement-bench
  - shell
  - f2.11
  - d13
feature_id: feat_term-core_nx0g1r
---

## What it does

## The four places

1. **The batch table** — `case "$BATCH" in ...` with `TABS`, `PANES`, `DUMPING`, `DRAGS`, `GESTURE` and its own knob. The file's own comment says a batch is defined in ONE single place — a previous review merged a second `case` for the drags precisely so that adding a batch would be one edit. It is still not one edit.
2. **The seeding `case`** — `A) : ;; B) seed quiet & ;; C|D|...|S) seed talkative & ;;`. A batch missing here seeds no panel, so no console writes `$WITNESS`, and the run is invalidated with «nobody got to publish a panel» — EVERY run, for ever, whatever the batch was measuring.
3. **The launch block** — the batch's knob has to be exported. And a knob the application reads by PRESENCE (`env::var_os(...).is_some()`) may NOT use the `MEASURE_X="${X:-}"` shape its neighbours use: an empty value switches it ON in every batch (see [[a-veto-needs-a-knob-outside-measurement-variables]]).
4. **The usage prose** — the batch letters in the `${1:?...}` message and the `*)` arm, plus any paragraph claiming «its control is C — same consoles, same load», which is false while the new batch seeds nothing.

**The failure is invisible from the diff and invisible from the suite**, and worse than a plain bug: the script's guard fires, so the run LOOKS correctly rejected. Nobody reads «nobody got to publish a panel» as «this batch was never wired up».

## And two ways a wired-up batch still lies (September 2026, batch `U`)

**One: the LOAD a batch sends must not touch anything painted, and «it is only a field» is how that gets missed.** `U`'s two ops were chosen as one addressed by key and one scalar; the scalar set `phase`. **`phase` is painted** — the identity row is `format!("{id} · {phase} · {source}")` — over a seeded document that declares none, so from the first patch `U` drew ten cells its control `S` does not, every frame, with an identity row that wraps differently at narrow widths. The difference `U − S` would have been published as «what a patch costs». The comment above the ops claimed «the batch differs from its control in the work, not in the picture» and argued it only for the *other* op.

> **Before choosing a value for a load, grep the painter for the field.** The fix was to set the expiry — read and never drawn — to the value the document already declares, so the op does the work and moves no pixel.

**Two: a guard whose number comes out of a `sed` is disarmed by rewording the report, and `${VAR:-0}` is what disarms it.** An extraction that matches nothing yields empty, `:-0` makes it zero, and zero reads as «no patches» — silencing both directions of the pair at once, on every batch, with the report otherwise complete and every other guard quiet. **An empty capture must invalidate the run**, not default. And `crates/app/tests/batches.rs` only held the report LINE's heading (`documents:`), never the words inside it that the patterns anchor on — so a second guard now checks each anchor against what `header.rs` writes.

## One claim to distrust when a new batch makes it

«A refusal costs the parse and the answer and none of the walk, the index or the install» was false in three of four clauses: the parse runs on the listener thread and not in a frame at all, and a refusal *does* pay the clone and the index, because an op list is only known to resolve by resolving it. **A guard's justification is prose and rots like prose**, even while the guard itself stays right.

## Where it lives

`scripts/measure-frame.sh` — the `case "$BATCH"` table, the seeding `case`, the export block, the `${1:?...}` usage message, `document_of`/`document_body` and the `OPS` the patching batch sends. `crates/app/tests/batches.rs` — `DECLARED`, the asymmetric-guard test, and the anchor guard against `crates/app/src/measure/header.rs`. Related: [[a-frame-cost-is-declared-once]], [[a-batch-has-saving-vetoed]].
