---
id: asp_an-integer-state-needs-no-nan-guard-but-does-need-a-ceiling_6m3eyt
slug: an-integer-state-needs-no-nan-guard-but-does-need-a-ceiling
name: >-
  The guard is demanded by the TYPE it is stored in, the ceiling is demanded by
  what can be HONOURED, and the moment it is applied decides whether the state
  remembers what the user asked for
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T15:30:48.346Z'
updated_at: '2026-08-19T15:30:48.346Z'
last_verified: '2026-08-19T15:30:48.346Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/painter.rs
symbols:
  - drag_its_panel_divider
  - drag_the_divider
  - cells_in
content_hashes:
  crates/app/src/lib.rs: 'sha256:67dfdfc9bad7c0d618f018577eecd1de410625cc5cf9347f593360fff275852e'
  crates/app/src/painter.rs: 'sha256:e02cf632e6186cee7d25e81d2f352b8d74ef3e631e3ce675540369378071905a'
tags:
  - egui
  - drag
  - state
  - rust
feature_id: feat_term-core_nx0g1r
---

## What it does

**Three things about storing a gesture's result, and all three were learned by getting them wrong in the
same method.**

**1. The guard against the not-a-number value is demanded by the TYPE, not by where the number came
from.** Verified by **running** with `rustc -O`, not by reading documentation:

| Expression | Result |
|---|---|
| `f32::NAN as usize` | **0** |
| `(f32::NAN / 8.0) as usize` | **0** |
| `-5.0f32 as usize` | **0** |
| `1.0e9f32 as usize` | **1000000000** |

Floating-point to integer conversions in Rust are **saturating**. So a state stored as an integer does
not need the guard that one stored as a fraction does need: the drag of the tree's dividers carries
`if !nueva.is_finite() { return false; }` because **a `clamp` over a `NaN` returns `NaN`**, and once
stored there it never leaves — the window stops painting and not even widening it brings it back.
Copying that guard to a path that stores integers puts in a mechanism with no scenario.

**2. But the ceiling IS needed, and NOT for the reason it looks like.** This project's real case is
neither the not-a-number value nor the negative one: it is that **while the desktop has not yet said how
big the window is, egui answers with its reservation rectangle of ten thousand points**, which in cells
is **more than a thousand** and **passes through** the conversion whole. There is no saturation to save
it.

> **CORRECTION (same branch, one PR later).** This record used to say that that rectangle is «covered»
> by `CellMetrics::cells_in` because it brings its own cap. **That is false and the next PR's review refuted
> it**: its cap is **a thousand columns**, a *grid* limit, not «this rectangle is not from a real
> window». With the reservation rectangle, a ceiling derived from there comes out at almost a thousand
> cells and **lets through** a width no window is ever going to have. Asking that function for the number
> is still right —it avoids repeating the conversion and its floor— but it is **not** the defense that
> was attributed to it here. What really stops that width from being stored is demanding that the pointer
> **has moved** for there to count as a gesture: in the window's first two frames nobody is dragging.
>
> The lesson of method over the technique: this record was written with the confidence of having read
> the code of `cells_in`, and its cap was read as though it bounded **the class of rectangle** when it
> only bounds **the number**. A cap read for what one wants it to mean.

**3. And the ceiling has to be what can be HONOURED, not what the place measures.** With the ceiling set
at «the pane's width», dragging all the way stored a width that the tiling was never going to give
—because the console's floor comes first— and **it appeared out of nowhere on widening the window**: a
width the user never saw. During a gesture, what the user is asking for is what they see.

> **And the moment it is applied is half the decision: the ceiling is applied ON WRITING, never on
> reading.** Without a gesture the clipping is not gone through, so a large width stored with the window
> wide **survives** a narrow one: it is painted clipped and comes back whole on widening. Applying it on
> reading too would destroy the user's intent every time the window is made small.

**And a test trap that goes with this:** asserting over the **geometry** does not pin the ceiling, because
the tiling already clips what it paints. The ceiling protects the **state**, so its test has to assert
the stored value — and the symptom of its absence is not seen by painting, it is seen **on resizing**,
which is when nobody relates it to the gesture any more.

## Where it lives

`Pane::drag_its_panel_divider` (`crates/app/src/lib.rs`) for all three rules; the mould
carrying the guard that does not apply here is `Window::drag_the_divider` in the same file; the
cell cap lives in `CellMetrics::cells_in` (`crates/app/src/painter.rs`), which documents why it exists. The
conversion table was obtained by compiling and running a standalone program, not by reading the
language's reference.
