---
id: asp_a-ceiling-over-a-floor-recreates-what-the-floor-prevented_469trf
slug: a-ceiling-over-a-floor-recreates-what-the-floor-prevented
name: >-
  A ceiling added over an existing floor makes the floor mean the opposite of
  what its own docblock says, and the numbers are small enough that nobody
  notices by reading
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:25:07.559Z'
updated_at: '2026-09-10T15:25:07.559Z'
last_verified: '2026-09-10T15:25:07.559Z'
file_paths:
  - crates/app/src/painter.rs
symbols:
  - the_question_box
  - paint_the_question_over_it
  - worth_pressing
content_hashes:
  crates/app/src/painter.rs: 'sha256:172632508594099e747d52af8eccc9db1b42e10d13bfc92d976c377d23e90581'
tags:
  - painting
  - geometry
  - review
  - D34
feature_id: feat_term-core_nx0g1r
---

## What it does

**The held-paste notice grows to fit its text and is only clamped to the console. Its floor — «at least one row tall and a few cells wide» — therefore correlates with «there is something readable to press», and its docblock says so in as many words.**

**Cap that box at half the console and the correlation inverts.** At the measured cell height of `17.625` px a three-row console is ≈**52.9 px**; half is ≈**26.4**; minus the two margins the text area is ≈**10 px** — **under one line**. The box passes the floor, gets stroked, swallows the press, and shows nothing. That is precisely the failure the floor was written against, recreated by the ceiling, on a geometry this application allows.

> **A floor and a ceiling on the same dimension have to be evaluated in the order they apply.** Measure the floor **against the capped box**, never against the box the text asked for. Then a console too short for one whole line gets **no box at all**, which is what the floor always meant.

**And the same change has a second victim, one level down.** The `debug_assert!` guarding «the sentence is not taller than the box measured for it» has an escape clause — *the box already fills the console, so a clip is expected* — and **a cap makes that clause unreachable**, so a short console trips the assert in every debug build unless the clause moves to the new bound in the same change.

**And the clip takes the END of the sentence.** The clamp shortens the **box**, never the text; an earlier delivery survived that only because the keys are the **first** thing in the galley. Any new sentence whose actionable half is last loses exactly that half — so «not painted outside its border» is not the criterion to write. **«Every sentence drawn is whole»** is.

**Found before a line was written**, by the design's own challenger, which is the only reason the numbers above are a design note rather than a bug report.

## Where it lives

`crates/app/src/painter.rs` — `the_question_box` (the shared geometry, called by the painter to draw and by the mouse pass to hit-test), its floor, and the `debug_assert!` in `paint_the_question_over_it`. The measurement of the cell comes from **D34**'s whole-pixel grid.
