---
id: asp_the-blink-bit-says-nothing-about-what-is-drawn_cxvk2h
slug: the-blink-bit-says-nothing-about-what-is-drawn
name: >-
  The engine answers «did the program ask for a blink», never «is there a
  blinking cursor on screen» — and a timer armed from the first wakes the window
  to paint nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:52:21.886Z'
updated_at: '2026-09-09T21:52:21.886Z'
last_verified: '2026-09-09T21:52:21.886Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/alarms.rs
  - crates/term-core/src/screen.rs
symbols:
  - paint_cursor
  - set_the_alarm
  - RenderableCursor
  - cursor_style
content_hashes:
  crates/app/src/painter.rs: 'sha256:8ffdf3a61b18c3a0a9c7c55f78d2b7591595b8c60b3b4259875a66477f3a697d'
  crates/term-core/src/screen.rs: 'sha256:2d233a7c9bd5ffb14bfd15808bdb8e8ff57cfdc1cf537357b62a5e41f30f9902'
tags:
  - alacritty
  - cursor
  - repaint
  - D13
  - D37
feature_id: feat_term-core_nx0g1r
---

## What it does

**In `alacritty_terminal` 0.26 the blink bit and the visibility of the cursor are two independent facts, and only one of them reaches the renderable snapshot.**

`RenderableCursor::new` (`alacritty_terminal-0.26.0/src/term/mod.rs:2380`, read in the registry) forces `shape = CursorShape::Hidden` when `TermMode::SHOW_CURSOR` is off — but it **never touches `cursor_style()`**, so `Term::cursor_style().blinking` (`:942`) keeps reporting whatever the program last asked for. `CSI ?25l` is what **every** full-screen program sends, so the common case is `blinking: true` sitting next to `shape: Hidden`.

In this application `paint_cursor` leaves at three guards before any ink is chosen: no cursor in the view, a shape of `Hidden`, and no cell where it stands. So a repaint deadline armed from the engine's bit schedules the window to wake — 1.7 Hz at a 600 ms half-period — **to paint nothing at all**. That is the exact cost **D13** exists to bound, arriving through the door that looks like the correct one. And a settings ceiling of «always blink» forces it even for programs that never asked.

> **Anything that has to wake the window arms itself from what the painting actually DREW, because the painting is the only place all three guards have already run.** One frame of lag can only arm a deadline early — egui keeps the shortest it is asked for — and the next frame clears it.

Found by an adversarial reviewer against the design of #210, before the code existed. The mirror trap, on the other side of the same loop, is [[the-logic-half-believes-what-the-painting-stopped-maintaining]].

## Where it lives

`crates/app/src/painter.rs` — `paint_cursor` returns whether what it drew is in a blink; `crates/app/src/alarms.rs` — `BlinkState::painted` is set from that answer and `set_the_alarm` reads it. The decision and its discarded alternatives are **D37** in `docs/02-decisions.md`.
