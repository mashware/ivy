---
id: asp_what-is-stuck-to-a-cell-takes-no-column-but-is-text_w5ycas
slug: what-is-stuck-to-a-cell-takes-no-column-but-is-text
name: >-
  The engine keeps the zero-width characters stuck to each cell, and this crate
  was throwing them away: that is why the symbol and its emoji version arrived
  identical
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:58:32.624Z'
updated_at: '2026-08-25T13:58:32.624Z'
last_verified: '2026-08-25T13:58:32.624Z'
file_paths:
  - crates/term-core/src/view.rs
  - crates/term-core/src/screen.rs
  - docs/02-decisions.md
symbols:
  - CombiningSpan
  - ViewCell
  - 'View::combining_of'
  - cell_from
  - to_text
content_hashes:
  crates/term-core/src/view.rs: 'sha256:8bb69d1e65b99d0bc3c8a4c29da0701affe1d675cbb02b5b782636ae76a99cc9'
  crates/term-core/src/screen.rs: 'sha256:6af1919efa54066bdf18665edd3cf6316c9ff4c01f3cfbd0b9a94a029dd24ef9'
  docs/02-decisions.md: 'sha256:0de8ae72f3ac308cb5cc9ba4d96b4b7d71443f38da0b0ca87bafa69318fc44ce'
tags:
  - term-core
  - unicode
  - emoji
  - F2.9
  - D8
  - D22
feature_id: feat_term-core_nx0g1r
---

## What it does

**`alacritty_terminal 0.26` keeps the zero-width characters stuck to their cell** —`Cell::zerowidth()`, a `Vec<char>`— **and until August 2026 this crate was throwing them away**: `ViewCell` only carried its character. **Consequence: `☁` and `☁️` arrived at whoever paints IDENTICAL**, because the only thing separating them is the `U+FE0F` selector, which takes no column.

### Three things learned on exposing it, and none of them is obvious

**1. They do not fit inside the cell.** `ViewCell` is `Copy` and the grid is rebuilt **whole every frame** (`vec![ViewCell::default(); rows × columns]`): a `Vec` per cell would be a memory allocation **per cell and per frame** on the hottest path of the project, and it would take `Copy` away from a type that has it for that very reason. They go in a **flat vector of the view** and the cell carries a **span**.

> **And the span is packed into ONE integer, which is four bytes per cell.** In two fields of a nested type the cell goes from 24 to **32** bytes —the type brings its own padding and the `u8` does not fit in the gap the cell already had—; packed into a `u32`, **28**. It was caught by a review compiling both forms.

**2. The engine has NO ceiling.** Its `push_zerowidth` is a `Vec::push` with no cap, that is, **an unbounded input from outside** —the pattern **D8** already punished with the output queue—. The ceiling is set by this crate. **And that number preserves no particular sequence**: it was believed that six were needed so as not to split an emoji family (`👨‍👩‍👧‍👦`) and **that is false** — verified by running: its members are **wide**, so they arrive as **four cells with one joiner each**, not as one with six. The ceiling **bounds**, it does not preserve.

**3. The head of a wide character carries it and its companion does not** —and it is the **normal** case, because CJK and emojis are wide, not a corner case—. If the companion brought a copy, the right-hand half of an emoji would be painted with a different presentation from the left. **And a loose selector sticks to the EMPTY cell in front of it**: verified by running, and it is what makes a blank space able to end up in the color font.

### The defect it uncovered

**`to_text` returned `☁` where the program wrote `☁️`.** It is not a clipped character: it is **another** character. **Copying with the mouse never had it**, because that is done by the engine (`term/mod.rs:610`). The defect was old, but **until this milestone the crate had no way of knowing it**, so it was neither detectable nor fixable. **D22** gained its fifth rule of what «the text» is.

### A trade-off that has to be known

**Comparing two `ViewCell` only means something within the SAME view.** The span says **where** the characters are stored, not which they are: two cells showing the same thing in two different views may come out different. Today nobody compares them; the day it is needed, what one wants to compare is the character and whatever `View::combining_of` returns.

## Where it lives

The `CombiningSpan` type and the `View::combining_of` reader live in `crates/term-core/src/view.rs`; the construction of the snapshot and the dump to text, in `crates/term-core/src/screen.rs`. The rules of what «the text» is are in **D22** of `docs/02-decisions.md`.
