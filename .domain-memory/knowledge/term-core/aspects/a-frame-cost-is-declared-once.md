---
id: asp_a-frame-cost-is-declared-once_dzpy7b
slug: a-frame-cost-is-declared-once
name: >-
  Declaring a frame cost twice subtracts it twice — which closing door a caller
  uses is a D13 decision, never a taste
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T18:28:16.753Z'
updated_at: '2026-09-18T18:05:58.967Z'
last_verified: '2026-09-18T18:05:58.967Z'
file_paths:
  - crates/app/src/control.rs
  - crates/app/src/closing.rs
  - crates/panel-control/src/wire.rs
  - crates/app/src/pointer.rs
symbols:
  - 'Window::close_a_pane_called'
  - 'Window::answer_the_windows_requests'
  - close_the_focused
  - opening_for_a_connection
  - MAX_PROGRAM
  - MAX_ARGS
content_hashes:
  crates/app/src/control.rs: 'sha256:cae42e29b2622c6cde1a1ba45c4459fb3b7e22b010afa06dcf8c719e678c397c'
  crates/app/src/closing.rs: 'sha256:3287f7455cacb401ef72e4541d23e4876cb82f1e20747e96e83c692fa5481ed6'
  crates/panel-control/src/wire.rs: 'sha256:90675d332e6786a426c0c017bd6ef5a3c219f69534f287ab7ae62001270e9244'
  crates/app/src/pointer.rs: 'sha256:78bdcf4b9e74451cde588a56ecbd693850d038e1adb4420f122caf64b7e3e570'
tags:
  - D13
  - measurement
  - frame-cost
  - ordering
  - execve
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

**Three cases now, same family: which door closes a console, and what it costs to pick the wrong one.**

**One: declaring the same work twice subtracts the same microseconds twice.** `Window::close_a_pane_called` added `its_cost + before.elapsed()` to `timed_separately`, and its caller `Window::answer_the_windows_requests` times the whole drain and adds that too. The work was declared twice, so the framework's figure — the one D3 judges egui by — was understated by it. That is the exact mirror of the failure the rule exists to prevent (undeclared work published as the framework's), and neither the report nor any test shows either direction. **The rule that comes out: declare a piece of work once, at the boundary of the frame path that runs it — and when a helper is called from a frame path that already times itself, the helper must not time itself.** The chord's close keeps its own timing because nothing else times it.

**Two (F2.11, September 2026): THE TWO DOORS THAT CLOSE A CONSOLE CARRY DIFFERENT STOPWATCHES, AND THE CHOICE IS A D13 DECISION, NOT A TASTE.** `closing.rs::close_the_focused` times itself and adds its own elapsed to `Window::timed_separately`; `closing.rs::close_a_pane_called` deliberately does NOT, because the control drain that calls it is already timed (case one, above). So a caller inside an already-timed pass must use the by-name door, or it subtracts the same microseconds twice again — and killing a child costs ~50 ms (#196), so the double subtraction is published as a smaller interface-framework cost, which is exactly the figure D3 judges egui by. **The second difference matters as much: the doors disagree about the keyboard.** The focused door moves the keyboard to the inherited neighbour EVERY time; the by-name one only if the console that leaves was holding it. A button pressed on a console nobody was typing in must use the BY-NAME door, or closing it steals the keyboard from a third pane. Pinned by `closing_a_console_by_its_button_does_not_move_the_keyboard_off_another`, which needs THREE panes to fail (with two, the inherited pane is the only survivor and the wrong rule looks right — see [[an-event-that-cannot-be-consumed-is-counted-per-frame]] for the same three-pane blind spot in the same milestone).

**Three: a name a refusal must be truthful about is acquired BEFORE the thing it names.** The 16-random-byte handle for `split_open` was obtained after `Pane::launching_with_args` and after the graft, so a `getrandom` failure would have answered «nothing was opened» while a live console was on screen. The fix moved the handle to the top of `opening_for_a_connection`, before the name is spent and before the program is launched: failing there spends nothing and opens nothing, and the refusal is true. The same shape applies to any token, name or id whose absence makes an answer a lie.

**Also shipped in the same review round, same family:** `program` and `args` had no bound where `source` had `MAX_SOURCE`, so the transport's 256 KiB ceiling admitted a single argument that reaches `execve` and comes back `E2BIG` — a launch failure the caller cannot read as its own mistake. `MAX_PROGRAM` (1 KiB) and `MAX_ARGS` (16 KiB) are measured on the encoded form and refused as malformed.

## Where it lives

`crates/app/src/closing.rs` (`close_a_pane_called` — no timing of its own; `close_the_focused` — times itself) and `crates/app/src/control.rs::Window::answer_the_windows_requests` (the single declaration). Ordering: `crates/app/src/control.rs::opening_for_a_connection` (the handle before the launch). Bounds: `crates/panel-control/src/wire.rs` (`MAX_PROGRAM`, `MAX_ARGS` in `split_open`). F2.11's button caller: `crates/app/src/pointer.rs` (must call `close_the_focused`, never `close_a_pane_called`, from inside the already-timed mouse pass). Related cards: whoever-does-not-declare-their-work-attributes-it-to-someone-else, seeding-can-be-pinned-timing-not-always, an-event-that-cannot-be-consumed-is-counted-per-frame.
