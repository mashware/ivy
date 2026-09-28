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
updated_at: '2026-09-09T21:55:19.817Z'
last_verified: '2026-09-09T21:55:19.817Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/lib.rs
symbols:
  - paint_cursor
  - cursor_rectangles
  - the_only_rectangle_stroked_in
  - Stroke
content_hashes:
  crates/app/src/painter.rs: 'sha256:8ffdf3a61b18c3a0a9c7c55f78d2b7591595b8c60b3b4259875a66477f3a697d'
  crates/app/src/lib.rs: 'sha256:e8bd846a3440e53be756ff25a7fc65376922d1ef1119dff8d912594f58492213'
tags:
  - tests
  - mutation
  - painting
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does

**The break, and it survived nine tests written specifically for the feature it breaks.** Change the cursor's outline to `Stroke::new(0.0, color)`. The shape is still queued, still the cursor's colour, still exactly the size of the cell — and **a console that is not being typed into draws no cursor at all**. Found by a coverage auditor in PR #212 (September 2026), not by the delivery's own mutation rounds.

**Why every assertion passed.** Both helpers select the cursor's shape by `rect.fill == ink` or `rect.stroke.color == ink`, then assert its rectangle. A zero-width stroke satisfies the colour, the rectangle and «not filled». `grep -rn 'stroke.width' crates/app/src/` returned **nothing**: in a crate that draws stroked rectangles — the cursor's outline, the focus border and the held-paste notice's box — no test had ever asserted how thick a line is.

> **This is disguise 3 of [[an-expectation-computed-from-what-you-mutate-pins-nothing]] with a new door.** That one says a colour can be in the queue and be invisible **because of what is painted over it**. This is the same invisibility with nothing on top: the shape hides itself. **Asserting a stroked shape's colour and rectangle is not asserting that anything was drawn.**

**The general rule for this crate: a test over a stroked shape asserts colour, rectangle AND width.** For a filled one the fill is enough; a fill has no zero.

**A second, narrower break the same audit found**: give the unfocused outline the underline's own thin bar as its rectangle. It slips past a size assertion that exercises **one** shape — «the outline is the size of a cell» has to be asserted for every shape the program can ask for.

## Two corrections, from the delivery that closed this (#210, PR 1 of 3)

**This card was stale within a day of being written, and that is the reusable half: the delivery that files a card about a hole is usually the delivery that closes part of it.** Re-read it against the tree before quoting it.

- **«Nothing in the crate asserts a thickness» stopped being true immediately.** The cursor's unfocused outline is measured by the very delivery this card describes.
- **The grep returns four distinct marks, not three.** The fourth is `CursorShape::HollowBlock` — and the delivery that went to pin it found it is **unreachable**, not untested: DECSCUSR produces only Block, Underline and Beam, and nothing in this workspace sets alacritty's own default style or enters vi mode. It is dead code kept on purpose — the arm keeps the `match` exhaustive over a public enum, and a `cursor.shape` setting would make it reachable again — not a mark.
- **The focus border and the notice's box are now pinned**, through one shared lookup that asserts colour, corner, exact thickness, stroke kind, clip and uniqueness at once.
- **And the notice's box was diagnosed wrong first.** The first reading said its painting was *unreached*; it is walked by about a dozen tests, all of which read `Shape::Text` and nothing else. The hole was real — the `rect_stroke` could be deleted outright with the suite green — and the diagnosis was not, which matters because the two readings lead to two different repairs. That is now its own card: [[five-ways-a-queued-shape-is-never-drawn]], point 5.

## Where it lives

`crates/app/src/painter.rs`: `cursor_rectangles` returns `(is it filled, its rectangle, how thick its line is)` and `the_console_without_the_keyboard_outlines_its_cursor_whatever_shape_it_asked_for` asserts all three, for block, underline and beam. `crates/app/src/lib.rs`: `the_only_rectangle_stroked_in` does the same for the focus border and the held paste's box, and every caller inherits its six assertions. **Nothing here is uncovered any more**; what is left is `HollowBlock`, which is unreachable.
