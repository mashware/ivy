---
id: asp_egui-hands-a-frame-s-events-to-its-first-pass-only_v3dck6
slug: egui-hands-a-frame-s-events-to-its-first-pass-only
name: >-
  egui 0.36 hands a frame's events to its FIRST pass only: what repeats is the
  closure, not the clicks — and the guards it justified are defence, except two
  that do present work
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T20:49:08.085Z'
updated_at: '2026-09-26T21:29:18.751Z'
last_verified: '2026-09-26T21:29:18.751Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
  - crates/app/src/keyboard.rs
  - crates/app/src/injected.rs
  - docs/02-decisions.md
symbols:
  - last_frame_with_buttons
  - handle_the_mouse
  - send_keys
  - collect_the_wheel
content_hashes:
  crates/app/src/lib.rs: 'sha256:0955cb278e25d124ab132882b82e670f9f54c4579dcbc3627c5326cc35ffafc1'
  crates/app/src/pointer.rs: 'sha256:f384df68dd3715e3e27ade9e33ef2e44981f70f478d0876379c3b2307411dd63'
  crates/app/src/keyboard.rs: 'sha256:8de7e63e699188ed4ad7cddaf074fac66eff1f5376c704896c71e05e26b679d7'
  crates/app/src/injected.rs: 'sha256:d894b21ba9be60046a9431c5adf539d0ba284b482776f6f944e64eba04689351'
  docs/02-decisions.md: 'sha256:7e5600d1c11b0e40126e8873b82fe6f878a2035602c771fcf8a0b3b3f20514b4'
tags:
  - egui
  - events
  - repeated-pass
  - tripwire
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured 26 September 2026 (#200), corrected across the codebase by #290.** egui 0.36.1's loop behind `run_ui` (private, `context.rs:835-874`) begins every pass with `begin_pass(new_input.take())` (`context.rs:854`), and `RawInput::take` (`data/input/raw_input.rs:137`) moves `events` out. A one-off test — one primary press, `request_discard` on pass 1 — saw `(events, any_pressed) = [(1, true), (0, false)]`. The repeated pass gets **no events**; what it carries over is state: pointer position and held buttons, keys held down, modifiers. A drag delta is zero there.

**What does run twice is the closure**: eframe puts `logic` and `ui` inside `run_ui` (`epi_integration.rs:288-303`), so pumping, painting and the frame close really happen twice. Guards about those are real (the frame close keyed by `cumulative_frame_nr`, pinned by `egui_counts_each_frame_once_and_every_pass_separately`).

**Guards against acting on the same EVENT twice** — the button marker `last_frame_with_buttons`, the tree divider's absolute position — protect a case this egui never produces. #290 **kept them as defence** (one frame compare against an upgrade that would turn one click into two consoles closed), recorded in amendments to D31, D46 and D51. Tripwire: `egui_hands_a_frame_s_events_to_its_first_pass_only` in crates/app/src/lib.rs.

**Two guards that looked like the same thing do present work, for other reasons** (found by #290's review, verified in source):
- **The keyboard's consumption is load-bearing**: `Context::end_pass` (`context.rs:2442-2450`) consumes Ctrl+Q (quit) and Ctrl +/-/0 (zoom) from the same queue; a key left there would reach the program AND close or zoom the window. The **wheel** is different: egui turns it into scroll deltas in `InputState::begin_pass` (`input_state/mod.rs:417-469`), before the app reads it, so consuming it stops only the app's own later readers.
- **eframe calls `raw_input_hook` and `App::logic` on logic-only ticks while the window is hidden** (`epi_integration.rs:323-362`), and `run_logic` (`context.rs:912-939`) does not advance `cumulative_frame_nr` — so the injected gesture's frame count and the frame close's guard do real work there.

**Method lesson ([[someone-elses-api-is-read-not-assumed]], case 20)**: a belief repeated in forty comments was never read in the library's source; and when it was found false, two of the guards it justified turned out to be needed for reasons nobody had written down.

## Where it lives

egui-0.36.1 src/context.rs:835-874, :2442-2450, :912-939; src/data/input/raw_input.rs:124-143; src/input_state/mod.rs:417-469; eframe-0.36.1 src/native/epi_integration.rs:288-303, :323-362; tripwire in crates/app/src/lib.rs; amendments to D31, D46, D51 in docs/02-decisions.md
