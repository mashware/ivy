---
id: asp_a-drag-that-lasts-several-frames-in-egui_a1f1w9
slug: a-drag-that-lasts-several-frames-in-egui
name: >-
  A drag lasting several frames in egui: the id carries the NAME, Escape ends it
  without a release, no button qualifies, it is marked dragged WITHOUT MOVING,
  and the tie is won by the LAST one
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:07:30.328Z'
updated_at: '2026-08-24T22:29:27.895Z'
last_verified: '2026-08-24T22:29:27.895Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - handle_the_mouse
  - drag_its_panel_divider
  - drag_the_divider
  - move_a_divider
content_hashes:
  crates/app/src/lib.rs: 'sha256:8f5d9220dadc886213f3207269ffaf9502f3454547843a94c9aa6e24dedbb2d5'
tags:
  - egui
  - input
  - drag
  - identity
feature_id: feat_term-core_nx0g1r
---

## What it does

**Five things about egui 0.36 that its naming does not announce. The first three cost a real defect in
F2.3; the fourth, in F2.4; the fifth, in F2.6 — and the third cost one TWICE, see the warning at the
end.**

**1. The widget's id has to carry the NAME of what is being dragged, never its PLACE.** egui holds the
drag across frames by that id. With the position inside it, anything that renumbers **while the button
is still held down** makes the in-flight drag point at something else — and in this application the
chords for changing tab, splitting and closing **are applied at the start of that same interface pass**,
long before the mouse is attended to. The case that slipped through: grab a console, press
`Ctrl+Shift+→` without releasing, and release — two consoles of **the destination tab** got reordered.

> **This answers the case that D14 left named and referred on to F2.3, and the answer was NOT to
> refactor the identity of the borders.** The leaves already had a stable name; what was missing was
> using it in the widget's id as well. The tree's dividers still go by position because their drag
> **does not survive anything**: it is resolved within the same frame. **That reasoning is not
> inherited**: the panel border, which does last as long as the button does, carries the name of its
> pane.

**2. Escape aborts the drag and that raises `drag_stopped()` — with the button STILL HELD DOWN.**
`interaction.rs` sets `dragged = None` on receiving Escape, and `context.rs` includes
`res.drag_stopped()` in its «an interaction is happening» condition, so `interact_pointer_pos()` keeps
returning a position. A gesture that confuses «the drag ended» with «the user released» **completes the
operation at the very moment of cancelling it**. What tells them apart is
`drag_stopped_by(PointerButton::Primary)`.

**3. No button is qualified.** `interaction.rs:162` matches `PointerEvent::Pressed { .. }` **without
looking at which button it was**, so any press sets `potential_drag_id`. In a terminal this is not
theoretical: the middle button is X11's paste and the right one is the selection in almost all of them.

**4. A widget that ONLY senses drags is marked as dragged ON THE PRESS ITSELF, with no movement.**
`interaction.rs:219-220`, with its own comment: *«This widget is just sensitive to drags, so we can mark
it as dragged right away»*. There is no threshold to travel.

> **And `dragged_by` only covers point 3, not point 4.** `response.rs:438-440` is
> `dragged() && button_down(button)`: it filters the button and does **not** demand movement. A drag
> widget that **writes state** needs both conditions: the right button **and** the pointer having moved.

**Why number 4 is the dangerous one, with its concrete case.** The handle of the panel border is drawn
where the tiling **clips** the panel, which in a narrow pane is not the width the user asked for.
Without demanding movement, **a dry click read that clipping as intent** and clobbered forever the width
asked for with the window wide. Demanding movement is what separates «I pressed here» from «I want this
width».

**5. When several widgets touch each other, the tie is won by the LAST one registered — and egui's hit
test is NOT exact containment.** Its `interact_radius` (5 points by default) puts into the candidate set
everything that is nearby, not only what contains the point. A tree divider measures **two points** and
both consoles touch it: all three are candidates, and the consoles register **after** the divider, so
they win.

> **The case, in F2.6, and it is one of those that gets published as another number.** The gesture a
> measurement batch injects into itself carried the pointer to the divider's line and pressed **in the
> same frame**. Result: the tie went to a console, the gesture **dragged that one**, releasing it
> relaid it out and the tiling went back to half and half. The batch was measuring the **D15** gesture
> —moving a console— under the label of the border gesture, with the report declaring effects all the
> same, so no guard of the script caught it.

> **What fixes it is doing what a hand does: arrive, stop, and press afterwards.** Bring the pointer
> closer in one frame and press in the **next** — verified that a human drag always wins, and that the
> injected one only won the first of the run. The underlying cause is the same as
> [[a-drag-test-needs-two-passes-at-least]]: egui resolves interaction at the start of the frame with
> what was registered in the **previous** one. **It does not happen to whoever uses the application**,
> and that is measured, not reasoned.

**And one that does come free:** `interact_pointer_pos()` **is still valid on the frame in which the
release happens**, on purpose and with a comment from the authors. That is why a gesture like this
**needs no state of its own** across frames; a field with «what am I dragging» would be a second copy of
something already stored, and the two could disagree.

**Bonus:** **`Sense::CLICK_AND_DRAG` does not exist**. Only the method `click_and_drag()`, and that one
adds `FOCUSABLE` — the trap already paid for with the click and with the border. The correct thing is
`Sense::CLICK | Sense::DRAG`.

> **THE WARNING WORTH MOST IN THIS RECORD, and it is about the record itself.** Point 3 was written here
> **before** F2.4, with its X11 paste example and everything — and in F2.4 it was committed again: the
> panel border was moving with the right button. Having the knowledge stored **is not enough** if the
> new path is written by copying a sibling piece instead of going back over this list. See
> [[the-sibling-mould-brings-its-decisions-and-its-gaps]]. Every new drag widget is reviewed against the
> five points, one by one.

## Where it lives

Read in the source of egui 0.36.1 from the local cargo registry (`sense.rs`, `response.rs`,
`interaction.rs`, `context.rs`, `hit_test.rs`), not assumed. Implemented in `Window::handle_the_mouse`,
`Window::drag_the_divider` and `Pane::drag_its_panel_divider`
(`crates/app/src/lib.rs`); the injected gesture of point 5 lives in
`InjectedDrags::move_a_divider`, in the same file. Each point pinned by a test that falls when
it is reverted — verified by mutating them one by one; the one for point 5 is
`the_divider_gesture_cycle_returns_the_tiling_to_the_starting_one`, which was **the one that found
it**.
