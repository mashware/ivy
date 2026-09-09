---
id: asp_a-stroke-of-zero-is-queued-and-invisible_4mew7m
slug: a-stroke-of-zero-is-queued-and-invisible
name: >-
  A stroke of zero is queued, carries the right colour, covers the right
  rectangle — and cannot be seen: nothing in this crate asserted a line's
  thickness
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T17:13:38.712Z'
updated_at: '2026-09-09T17:13:38.712Z'
last_verified: '2026-09-09T17:13:38.712Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/lib.rs
symbols:
  - paint_cursor
  - cursor_rectangles
  - Stroke
content_hashes:
  crates/app/src/painter.rs: 'sha256:c055c3b589c2fda1a7af6cb97592995f697585d7ab2f89935cc1e3fc357f1e31'
  crates/app/src/lib.rs: 'sha256:60da7c8ba51c1fc4bcb6f7b7c8ae54bccc71aa8d350d4bd1e40d40ac53c693ab'
tags:
  - tests
  - mutation
  - painting
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does

**The break, and it survived nine tests written specifically for the feature it breaks.** Change the cursor's outline to `Stroke::new(0.0, color)`. The shape is still queued, still the cursor's colour, still exactly the size of the cell — and **a console that is not being typed into draws no cursor at all**. Found by a coverage auditor in PR #212 (September 2026), not by the delivery's own mutation rounds.

**Why every assertion passed.** Both helpers select the cursor's shape by `rect.fill == ink` or `rect.stroke.color == ink`, then assert its rectangle. A zero-width stroke satisfies the colour, the rectangle and «not filled». `grep -rn 'stroke.width' crates/app/src/` returned **nothing**: in a crate that draws three stroked rectangles — the cursor's outline, the focus border and the held-paste notice's box — no test had ever asserted how thick a line is.

> **This is disguise 3 of [[an-expectation-computed-from-what-you-mutate-pins-nothing]] with a new door.** That one says a colour can be in the queue and be invisible **because of what is painted over it**. This is the same invisibility with nothing on top: the shape hides itself. **Asserting a stroked shape's colour and rectangle is not asserting that anything was drawn** — the width is the third of the three, and it is the one nobody writes down because `Stroke::new(1.0, …)` reads like boilerplate.

**The general rule for this crate: a test over a stroked shape asserts colour, rectangle AND width.** For a filled one the fill is enough; a fill has no zero. And the same hole is open, today, for the two other stroked shapes here — the focus border (`lib.rs`, `theme.focus()`) and the notice's box (`painter.rs`): both are found by colour in their tests and neither has its thickness pinned, so both can be made invisible with the suite green.

**A second, narrower break the same audit found**: give the unfocused outline the underline's own thin bar as its rectangle. It slips past a size assertion that exercises **one** shape — the lesson being that «the outline is the size of a cell» has to be asserted for every shape the program can ask for, not for whichever one the test happened to pick.

## Where it lives

`crates/app/src/painter.rs`: `cursor_rectangles` returns `(is it filled, its rectangle, how thick its line is)` and `the_console_without_the_keyboard_outlines_its_cursor_whatever_shape_it_asked_for` asserts all three, for block, underline and beam. The zero-stroke and thin-bar breaks both fall it.

Uncovered: the focus border and the held-paste notice's box, both stroked, both pinned by colour alone.
