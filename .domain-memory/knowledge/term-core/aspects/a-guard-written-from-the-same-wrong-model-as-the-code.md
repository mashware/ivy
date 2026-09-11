---
id: asp_a-guard-written-from-the-same-wrong-model-as-the-code_fm6hss
slug: a-guard-written-from-the-same-wrong-model-as-the-code
name: >-
  A guard written from the same model as the code catches nothing: the rounding
  word three functions satisfy, two mechanisms with no scenario, and a table
  nothing tied to a drawing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T12:51:54.369Z'
updated_at: '2026-09-10T12:51:54.369Z'
last_verified: '2026-09-10T12:51:54.369Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/painter/blocks.rs
  - crates/app/src/fonts.rs
symbols:
  - on_whole_pixels
  - paint_a_block
  - filling_of
  - 'Discoverer::resolve'
content_hashes:
  crates/app/src/painter.rs: 'sha256:8ffdf3a61b18c3a0a9c7c55f78d2b7591595b8c60b3b4259875a66477f3a697d'
  crates/app/src/painter/blocks.rs: 'sha256:6598c8ac747d2b67702b8438c9953f61a756bb20ccf809b070651f6986084717'
  crates/app/src/fonts.rs: 'sha256:6437b6d69ce3d04e8175d68c813e4c4caef651a8e3dbcad219ba51a77ce6fe91'
tags:
  - tests
  - mutation
  - guards
  - painting
  - D30
  - D34
feature_id: feat_term-core_nx0g1r
---

## What it does

**A guard does not catch a mechanism that has no scenario, because the guard is written from the same model the code was.** Three shapes of it came out of one delivery (#207/#208, September 2026), all found by mutation and by an adversarial read, none by the suite.

### 1. The rounding word that three functions satisfy

The rule was «the cell must never be wider than the glyph's advance». The guard asserted **«the cell is a whole number of pixels»**, which `floor`, `round` **and** `ceil` all satisfy. Shipped with `round`, which goes **up** whenever the fraction is at least a half — at 17 points that took an advance of `9.52` px to a cell of `10`, `0.48` of a gap at every column: the reported defect, at a body the settings file can ask for.

> **The fix is to assert the REASON, not the shape**: the cell is never *larger* than what the font measured. That kills `ceil` and `round` for the right cause. And the guard has to assert first that at least one of the bodies it sweeps rounds up under nearest, or on another machine's font it covers nothing.

### 2. Two mechanisms whose scenario does not exist

Both read as the obvious other half and both were removed:

- **Snapping the corner the grid starts from.** The engine rounds each drawing anyway and a fraction shared by every cell survives as one offset — see [[epaint-rounds-each-glyph-origin-on-its-own]]. Its guard demanded whole corners, written from the same wrong model.
- **Skipping font discovery for characters the painter draws itself.** `Discoverer::resolve`'s oracle discards, before touching the machine, anything the loaded faces already show, and the factory faces cover that range behind any family the settings name. The search never fired for those characters in the first place; the skip only added a check on every cell of the grid.

> **The tell for both**: deleting the mechanism left the whole suite green *including the test written for it*. When a test passes with and without its subject, the subject is not what the test is about.
>
> The second test was **kept and turned around**: instead of «the skip works» it now pins **the premise** — a row of blocks walks nothing, a row of Japanese does — so the day a build ships without a factory face covering the range, the reasoning is reopened instead of quietly becoming false. Same remedy as [[a-guard-nobody-can-trip-gets-a-test-of-its-premise]].

### 3. The table and the drawing, asserted separately

One module asserted what a lookup returns; the painter asserted the effect **only for the two characters whose value is the identity** (a full block and a shade, whose part *is* the whole cell). Rebinding every part to the whole cell — so every eighth and every quadrant painted as a full block — was green across 833 tests. **Nothing connected the table to a rectangle.**

Same delivery, same shape: a range test that walks all 32 entries but asserts only `is_some()`, so six of ten quadrants could be in the wrong corner.

## Where it lives

`crates/app/src/painter.rs` (`on_whole_pixels`), `crates/app/src/painter/blocks.rs` (`paint_a_block`, `filling_of`) and `crates/app/src/fonts.rs` (`Discoverer::resolve`). The decisions are **D34** and **D35**; the disguise list this extends is **D30**'s.

It is the neighbour of [[a-test-can-pass-because-the-scenario-did-not-fit]] — there the test's *world* prevented the defect, here the *mechanism* had no world to be wrong in — and one more instance of [[a-mutation-only-tests-what-its-author-imagined]]: the author mutated the painter and never the word the guard was written with.
