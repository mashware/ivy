---
id: asp_adding-a-measurement-batch-means-editing-three-places_2xs11n
slug: adding-a-measurement-batch-means-editing-three-places
name: >-
  Adding a batch to measure-frame.sh means editing three places, and only one of
  them is where a reader looks — miss any and the run is invalidated with no
  clue why
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:06:38.965Z'
updated_at: '2026-09-18T18:06:38.965Z'
last_verified: '2026-09-18T18:06:38.965Z'
file_paths:
  - scripts/measure-frame.sh
symbols: []
content_hashes:
  scripts/measure-frame.sh: 'sha256:180b8517595b278aa0eb55476f7fdeaab7a027bf5da0ea0dcd80803c2a95bd15'
tags:
  - measurement-bench
  - shell
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

1. **The batch table** — `case "$BATCH" in ...` with `TABS`, `PANES`, `DUMPING`, `DRAGS`, `GESTURE` and its own knob. The file's own comment says a batch is defined in ONE single place — a previous review merged a second `case` for the drags precisely so that adding a batch would be one edit. It is still not one edit.
2. **The seeding `case`** — `A) : ;; B) seed quiet & ;; C|D|...|S) seed talkative & ;;`. A batch missing here seeds no panel, so no console writes `$WITNESS`, and the run is invalidated with «nobody got to publish a panel» — EVERY run, for ever, whatever the batch was measuring.
3. **The launch block** — the batch's knob has to be exported. And a knob the application reads by PRESENCE (`env::var_os(...).is_some()`) may NOT use the `MEASURE_X="${X:-}"` shape its neighbours use: an empty value switches it ON in every batch (see [[a-veto-needs-a-knob-outside-measurement-variables]]).

**The failure is invisible from the diff and invisible from the suite**, and worse than a plain bug: the script's guard fires, so the run LOOKS correctly rejected. Nobody reads «nobody got to publish a panel» as «this batch was never wired up».

**A fourth place, and it lies quietly**: the usage prose — the batch letters in the `${1:?...}` message and the `*)` arm, plus any paragraph claiming «its control is C — same consoles, same load», which is false while the new batch seeds nothing.

## Where it lives

`scripts/measure-frame.sh` (the `case "$BATCH"` table, the seeding `case`, the export block, and the `${1:?...}` usage message).
