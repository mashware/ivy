---
id: asp_a-frame-cost-is-declared-once_dzpy7b
slug: a-frame-cost-is-declared-once
name: >-
  Declare a frame cost once, at the boundary that runs it — and publish every
  span the work TOUCHES, not the one it runs in
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T18:28:16.753Z'
updated_at: '2026-09-19T14:31:10.465Z'
last_verified: '2026-09-19T14:31:10.465Z'
file_paths:
  - crates/app/src/closing.rs
  - crates/app/src/control.rs
  - crates/panel-control/src/wire.rs
  - crates/app/src/panel/driving.rs
  - crates/app/src/measure/header.rs
symbols:
  - 'PanelState::patch'
  - 'PaneControl::apply'
  - close_a_pane_called
  - close_the_focused
content_hashes:
  crates/app/src/closing.rs: 'sha256:3287f7455cacb401ef72e4541d23e4876cb82f1e20747e96e83c692fa5481ed6'
  crates/app/src/control.rs: 'sha256:d1ba1204d05372a43943bd4dbba8108fa8df97d28faaf31910cfc8e41fba1421'
  crates/panel-control/src/wire.rs: 'sha256:d13326fcea3c48d140e2b92d8ab60bd05016e1d1b791a31da2e7581501c948f8'
  crates/app/src/panel/driving.rs: 'sha256:6feab70171dcc55afb3b52234ca53fc645b0deba7d8775271b9479cb137af709'
  crates/app/src/measure/header.rs: 'sha256:10f75f6fe266967fbe060d9c391f47e0b3b5e11f196a86dcc6598c810320944b'
tags:
  - d13
  - measurement
  - frame-cost
feature_id: feat_term-core_nx0g1r
---

## What it does

**Four cases now, same family: a number that is subtracted twice, a number that is subtracted from the wrong place, a name acquired too late — and a cost read from one span when it lands in two.**

**One: declaring the same work twice subtracts the same microseconds twice.** `Window::close_a_pane_called` added `its_cost + before.elapsed()` to `timed_separately`, and its caller `Window::answer_the_windows_requests` times the whole drain and adds that too. The work was declared twice, so the framework's figure — the one D3 judges egui by — was understated by it. That is the exact mirror of the failure the rule exists to prevent (undeclared work published as the framework's), and neither the report nor any test shows either direction. **The rule that comes out: declare a piece of work once, at the boundary of the frame path that runs it — and when a helper is called from a frame path that already times itself, the helper must not time itself.** The chord's close keeps its own timing because nothing else times it.

**Two (F2.11, September 2026): THE TWO DOORS THAT CLOSE A CONSOLE CARRY DIFFERENT STOPWATCHES, AND THE CHOICE IS A D13 DECISION, NOT A TASTE.** `closing.rs::close_the_focused` times itself and adds its own elapsed to `Window::timed_separately`; `closing.rs::close_a_pane_called` deliberately does NOT, because the control drain that calls it is already timed (case one, above). So a caller inside an already-timed pass must use the by-name door, or it subtracts the same microseconds twice again — and killing a child costs ~50 ms (#196), so the double subtraction is published as a smaller interface-framework cost, which is exactly the figure D3 judges egui by. **The second difference matters as much: the doors disagree about the keyboard.** The focused door moves the keyboard to the inherited neighbour EVERY time; the by-name one only if the console that leaves was holding it. A button pressed on a console nobody was typing in must use the BY-NAME door, or closing it steals the keyboard from a third pane. Pinned by `closing_a_console_by_its_button_does_not_move_the_keyboard_off_another`, which needs THREE panes to fail (with two, the inherited pane is the only survivor and the wrong rule looks right — see [[an-event-that-cannot-be-consumed-is-counted-per-frame]] for the same three-pane blind spot in the same milestone).

**Three: a name a refusal must be truthful about is acquired BEFORE the thing it names.** The 16-random-byte handle for `split_open` was obtained after `Pane::launching_with_args` and after the graft, so a `getrandom` failure would have answered «nothing was opened» while a live console was on screen. The fix moved the handle to the top of `opening_for_a_connection`, before the name is spent and before the program is launched: failing there spends nothing and opens nothing, and the refusal is true. The same shape applies to any token, name or id whose absence makes an answer a lie.

**Four (243, September 2026): a new verb's cost is not one span, and reading the span it is obviously in publishes a fraction of it.** `panel_patch` is drained where the control channel is drained, so the walk, the key index and the write-out that weighs the result land in `the control channel` — which is where anyone measuring the new verb looks. But installing the result sets `layout = None`, so **the re-layout it forces is paid in the NEXT frame, in `the status panel` span**. Measured, batch `U` against its control `S`, two runs each interleaved:

| span, per frame (left pane) | `S` | `U` | difference |
|---|---|---|---|
| the control channel | 722–729 ns | 1.076–1.107 µs | +347 to +385 ns |
| the status panel | 10.28–10.60 µs | 11.37 µs | +773 ns to +1.09 µs |

**Two thirds of what the verb costs is in the span that does not carry its name**, and a first draft of the validation reported only the channel's.

> **Before publishing a difference, ask which spans the new work TOUCHES, not which span it RUNS IN.** Anything that invalidates a cache, wakes a pass, or changes what the next frame has to do spends itself somewhere else. **D13**'s subtraction cannot catch this: every span is declared, so nothing is mislabelled, and the reader still walks away with the wrong number because they read one row.

**And the framework's own figure is the control for whether anything moved at all**: 508.9–516.6 µs on `S` against 516.2–532.7 µs on `U`, two overlapping spreads, so *no measurable change* — not «slightly worse», which two runs of one batch varying by 8 µs would also say.

**Also shipped in an earlier review round, same family:** `program` and `args` had no bound where `source` had `MAX_SOURCE`, so the transport's 256 KiB ceiling admitted a single argument that reaches `execve` and comes back `E2BIG` — a launch failure the caller cannot read as its own mistake. `MAX_PROGRAM` (1 KiB) and `MAX_ARGS` (16 KiB) are measured on the encoded form and refused as malformed. The same gap reappeared with `ops`: see [[a-bound-counts-the-encoded-width-not-the-decoded-value]].

## Where it lives

`crates/app/src/closing.rs` (`close_a_pane_called` — no timing of its own; `close_the_focused` — times itself) and `crates/app/src/control.rs::Window::answer_the_windows_requests` (the single declaration). Ordering: `crates/app/src/control.rs::opening_for_a_connection` (the handle before the launch). Bounds: `crates/panel-control/src/wire.rs`. F2.11's button caller: `crates/app/src/pointer.rs`. The two-span case: `crates/app/src/panel/driving.rs::PanelState::patch` (the `layout = None` that moves the cost) against the `the control channel` and `the status panel` spans in `crates/app/src/measure/header.rs`. Related cards: whoever-does-not-declare-their-work-attributes-it-to-someone-else, seeding-can-be-pinned-timing-not-always, an-event-that-cannot-be-consumed-is-counted-per-frame, [[adding-a-measurement-batch-means-editing-three-places]].
