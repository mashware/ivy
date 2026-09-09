---
id: asp_the-logic-half-believes-what-the-painting-stopped-maintaining_ctm3wa
slug: the-logic-half-believes-what-the-painting-stopped-maintaining
name: >-
  The `logic` half goes on believing state the `ui` half stopped maintaining —
  and the obvious fix reads the wrong field, because `run_logic` refreshes only
  the raw input
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:52:37.416Z'
updated_at: '2026-09-09T21:52:37.416Z'
last_verified: '2026-09-09T21:52:37.416Z'
file_paths:
  - crates/app/src/alarms.rs
  - crates/app/src/lib.rs
symbols:
  - set_the_alarm
  - BlinkState
  - run_logic
content_hashes:
  crates/app/src/lib.rs: 'sha256:e8bd846a3440e53be756ff25a7fc65376922d1ef1119dff8d912594f58492213'
tags:
  - egui
  - eframe
  - window-loop
  - repaint
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Two staleness traps in eframe 0.36.1's split pass, and the second hides inside the fix for the first.**

`logic` and `ui` run in the same pass, and **`logic` is the only half that runs while the window is minimized or occluded**.

1. **Any per-frame flag written by the painting keeps the last painted frame's value for as long as the window stays hidden.** A `bool` saying «a blinking cursor is on screen» went on rearming a 600 ms repaint in a minimized window — the exact opposite of what the flag was added to bound. (Bounded rather than unbounded, and the first telling of this got it wrong: the keystroke anchor is refreshed by that same half, so a fifteen-second stop ends it. About twenty-five needless frames per minimize.)
2. **Asking egui for the desktop's focus does not work with the obvious field.** `Context::run_logic` (egui-0.36.1 `src/context.rs:912-927`) writes `raw.viewports` and `raw.focused` and carries a comment saying it *leaves the ui input alone*. The processed `InputState::focused` is only refreshed when a **pass begins**, which is the `ui` half — so `ctx.input(|i| i.focused)` inside `logic` answers with the **last painted frame's** focus, and `ctx.input(|i| i.raw.focused)` is the live one. Measured: the failing test stayed failing after the first fix, pointing at the same line, and only the field change killed it.

> **This is the mirror image of [[the-heartbeat-was-armed-a-frame-late]].** That card is about state the `ui` half CREATES and the `logic` half cannot see yet; this is about state the `ui` half STOPS MAINTAINING and the `logic` half goes on believing. Both are the same seam read from opposite ends, and both were only found by mutation.

## Where it lives

`crates/app/src/alarms.rs` — `BlinkState::due` takes the focus as an argument, and `Window::set_the_alarm` reads it from `input.raw.focused`. The test is `a_window_nobody_can_see_stops_flipping_its_cursor` in `crates/app/src/lib.rs`.
