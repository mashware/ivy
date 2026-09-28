---
id: asp_measuring-something-that-changes-the-grid-is-not-a-subtraction_6sfcd7
slug: measuring-something-that-changes-the-grid-is-not-a-subtraction
name: >-
  Measuring a piece that keeps part of the room for itself is not a subtraction:
  the frame with panels comes out cheaper and that is not an improvement
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T15:24:41.224Z'
updated_at: '2026-08-13T15:24:41.224Z'
last_verified: '2026-08-13T15:24:41.224Z'
file_paths:
  - docs/02-decisions.md
  - scripts/measure-frame.sh
  - crates/app/src/measure.rs
symbols:
  - 'Timings::publish_spans'
  - header
content_hashes:
  docs/02-decisions.md: 'sha256:315a02dd674468d2ff9049b2b9750cd655d842d9e9b347b4cb6de82ece55c25f'
  scripts/measure-frame.sh: 'sha256:3da4da4657cf7b2b100640bc4feb8e2606a9b52079895401e847cdab18618b78'
  crates/app/src/measure.rs: 'sha256:2c620cfa14e9ac593b3293628869a8ddb5d48b5ba3de54e08eb72f775d3ebe17'
tags:
  - measurement
  - performance
  - D3
  - methodology
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The number that comes out is a trap, and the trap is the good news of having seen it.** With both panels painting, the frame costs **5.78 ms**; without them, **7.01**. Cheaper with panels. And it is false to read it as an improvement: **the panel takes 36 of its console's 57 columns**, and the console goes on to paint **63 % fewer cells**. What goes down is `enqueue` —1.80 → 0.75 ms— because there is less to stack up.

> **When the piece you are measuring keeps part of everything else's room, the before/after comparison is not a subtraction.** The total can go down while the cost goes up. The only subtractable thing is that piece's **own span**, and only if its floor does not change — here it does: the batch with no panel still measures 0.77 µs (picking up the flag, asking for the time, tiling the width), and **B − A** leaves clean what reading and painting cost.

**Without the control batch this gets published the wrong way round.** The annex would have said «the panels lower the frame's cost by 18 %». Batch A is not a methodological luxury: it is the only thing that prevents the conclusion opposite to the truth.

**The figures that stand**, two consoles dumping 44 MiB each, 357-358 frames with output per console: the two panels cost **13 µs per frame**, 0.2 %; the interface framework is left with **0.50 ms demonstrable out of 8.33**, 6 % of the gap; and what the frame spends is **interpreting**, 4.5 ms.

**Two things that came out the opposite of what was expected:**

1. **Wrapping does not show.** At 21 columns the ~50-character lines wrap into three rows and at 57 they do not wrap; interpreting could be expected to get more expensive, because the wrapping width is what it depends on. **It does not change**: 4.35 / 4.51 / 4.35 ms. What dominates is the quantity of bytes, not how they are spread across rows.
2. **A panel that publishes without stopping does not add work: it adds frames.** Each console produces in the same 357-358, but the window goes from 368 to 472 — less overlap, the same work spread across 28 % more frames, each one cheaper. **Hard consequence: the «frame cost» of two batches with different window counts is not comparable**, because they are frames that carry different quantities of work inside.

## Where it lives

Annex «Measured — 13 August 2026» of D3 in `docs/02-decisions.md`, taken with `scripts/measure-frame.sh` (batches A with no panel, B with the panel still, C publishing, D control for the temporary one).
