---
id: asp_what-resizing-costs-while-dragging_04ss52
slug: what-resizing-costs-while-dragging
name: >-
  What resizing costs while dragging: the grid's reflow is cheap, and what grows
  with the columns is the repaint of the program inside
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-11T19:52:18.687Z'
updated_at: '2026-08-11T19:52:18.687Z'
last_verified: '2026-08-11T19:52:18.687Z'
file_paths:
  - crates/term-core/src/screen.rs
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
symbols:
  - 'Screen::resize'
  - 'Pty::resize'
  - 'Pane::resize'
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:b87919af4bad5a80f53f83baa106bce231d0ab3d35d6284beaf4be188e766002'
  crates/term-core/src/lib.rs: 'sha256:efa66ec48f1e9c4ba5f5d05657492ca563960b543264d613c06cd094b0818c73'
  crates/app/src/lib.rs: 'sha256:c7f2d67be30a71cbe392f9c446013656f0c3443fb6f7d7035e8575837d3f7a1c'
tags:
  - performance
  - measured
  - f1.4
  - f2.2
  - d3
  - panes
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured in `--release`, not estimated**, with a 61×57 grid and real content —color, text and
wrapped lines— so that the reflow has something to move:

| | Cost |
|---|---|
| `Screen::resize`, ±1 column | **11.2 µs** |
| `Screen::resize`, ±20 columns | 11.9 µs |
| `Pty::resize` (the `TIOCSWINSZ` `ioctl`) | **289 ns** |

**The reflow does everything one fears, and even so it does not matter.** `Screen::resize` calls
`Term::resize`, which reflows **both grids** —the active one and the alternate one— and whose
`grow_columns`/`shrink_columns` allocates a new vector of rows, does `take_all()` on the grid and
reassigns row by row. It is proportional to the cells **and** it allocates memory. But at this size
it is 11 µs, and **the jump of twenty columns costs the same as the one of one**: what dominates is
walking the rows, not how much gets moved.

**The real ceiling per frame is two reflows and two `ioctl` = 23 µs**, that is, **0.28 % of the 120
Hz slot** and 0.39 % of the 5.92 ms already spent with the two consoles dumping. A drag that crosses
the window in a second crosses some 114 column borders per console —with the font at 14 points the
cell measures ~8.4— so that is ~1.3 ms of reflow spread over 120 frames. **There is nothing to cache
here**, and a cache would be a mechanism with no scenario to justify it.

What bounds the number of reflows is not the mouse, it is the guard of `Pane::resize`: it is called
once per console and per frame, and it returns without doing anything if the grid already measures
what was asked for. A **sub-cell** drag —three points that do not change the number of columns—
triggers neither of the two halves, and that is correct: the pseudoterminal and the grid are
measured in cells, so both calls would be a no-op.

## The term that DOES grow with the columns, and it is not our own code

Each `Pty::resize` sends **`SIGWINCH`**, and the program inside responds by **repainting its whole
screen**. Those bytes come back through the parser, which is already 3.35 ms of the frame. A 61×57
repaint from `vim` is a few KiB → **~50-90 µs** with the 121 µs per 8 KiB already measured; at most
one per frame, around **1 % of the slot**, and the 128 KiB budget absorbs it without cutting.

**Consequence for F2.2, and it is the reason for writing this down:** when it has to be measured
again with four panes, what has to be looked at is **this** term, not the reflow. The reflow scales
with the rows of one console and the consoles get smaller the more there are; the repaint after the
size notice scales with **how many programs have to be notified**, and a drag that moves a divider
among four can notify two at once.

## An error notice that looked like it could be per frame, and cannot

When `Pty::resize` fails a notice is printed on the error output. The reasonable question is whether
a drag can turn that into an I/O write per frame. **No**, and the reason is the order of the two
halves: the grid is resized **before** the pseudoterminal, so after a failure the grid already
measures what was asked for and the next frame goes out through the guard without trying again. The
ceiling is the **distinct** size changes, not the frames. And the trigger is an `ioctl` that on
Linux does not fail on an open master, not even if the child has died.

## A one-frame lag that is free, so that nobody reads it as a failure

The size is **measured** in the ui pass and **applied** in the logic half of the following frame. So
during a drag each console paints one frame with the old grid inside the new rectangle. The divider
is up to date. At 120 Hz it is not visible, and fixing it would require measuring and applying in
the same half, which is precisely the arrangement that the separation between the two halves of the
frame exists to avoid.

**When these numbers would stop holding**: on another machine, with another version of the VT
engine, or with grids of another order of magnitude. The bench was temporary and was deleted;
redoing it takes a few minutes and what it measures is `Screen::resize` over a grid with content,
never over an empty one — an empty grid has nothing to reflow and would give a number that lies, the
same mistake that already cost twice when feeding the parser with a single repeated character.

## Where it lives

The reflow is `Screen::resize` in `crates/term-core/src/screen.rs`, which delegates to
alacritty_terminal's `Term::resize`. The notice to the program is `Pty::resize` in
`crates/term-core/src/lib.rs`. The two are chained by `Pane::resize` in `crates/app/src/lib.rs`,
with its guard of «if it already measures what was asked for, do nothing» and with the order —grid
first, program afterwards— that this aspect explains. It is called by `Window::pump_them_all`, once
per console and per frame.
