---
id: asp_an-event-that-cannot-be-consumed-is-counted-per-frame_e1crwv
slug: an-event-that-cannot-be-consumed-is-counted-per-frame
name: >-
  An egui event that is ACTED upon but cannot be consumed is counted per frame —
  a defence: egui 0.36 hands a frame's events to its first pass only (corrected,
  #290)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T09:23:01.083Z'
updated_at: '2026-09-26T21:29:37.167Z'
last_verified: '2026-09-26T21:29:37.167Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
  - crates/app/src/keyboard.rs
symbols:
  - whose_button_was_pressed
  - route_the_buttons
  - last_frame_with_buttons
content_hashes:
  crates/app/src/lib.rs: 'sha256:0955cb278e25d124ab132882b82e670f9f54c4579dcbc3627c5326cc35ffafc1'
  crates/app/src/pointer.rs: 'sha256:f384df68dd3715e3e27ade9e33ef2e44981f70f478d0876379c3b2307411dd63'
  crates/app/src/keyboard.rs: 'sha256:8de7e63e699188ed4ad7cddaf074fac66eff1f5376c704896c71e05e26b679d7'
tags:
  - egui
  - mouse
  - events
  - repeated-pass
  - corrected
feature_id: feat_term-core_nx0g1r
---

## What it does

**CORRECTION (26 September 2026, #200 / #290): the premise below is false for egui 0.36.1, and so is the double click it reports.** The loop behind `run_ui` begins every pass with `begin_pass(new_input.take())`, and `RawInput::take` moves the events out. Measured: one primary press plus `request_discard` on the first pass → `(events, any_pressed) = [(1, true), (0, false)]`. A repeated pass carries state (pointer, held buttons and keys, modifiers) but **no events**, so the button frame count protects a case this egui does not produce — #290 kept it **as defence** (amendments to D31, D46, D51). **The keyboard's `retain` is different: it is load-bearing today**, because egui's `end_pass` (`context.rs:2442-2450`) consumes Ctrl+Q and the zoom keys from the same queue; left there, a key would reach the program and close or zoom the window. The wheel's consumption only stops the app's own later readers (egui reads the wheel in `begin_pass`). What does run twice is the **closure** (eframe puts `logic` and `ui` inside `run_ui`), and eframe also runs `logic` on logic-only ticks while hidden without advancing the frame number: guards about pumping, painting or closing the frame once are real. The tests that pin the button guard simulate the repeat by calling `handle_the_mouse` twice inside ONE egui pass — equivalent only because the barrier reads `cumulative_frame_nr`. Tripwire: `egui_hands_a_frame_s_events_to_its_first_pass_only`. See [[egui-hands-a-frame-s-events-to-its-first-pass-only]].

The original reasoning, kept as history of why the guards exist:

**The rule, and it comes out of putting together two things this project already had on file separately.** egui was believed to run the interface pass **twice over the same input**, and that is why the keyboard and the wheel **consume** their events with `retain`.

**But there are events that cannot be consumed.** The pointer button ones are what egui needs in order to decide who is being dragged: whoever takes them away leaves the gesture never starting.

**F2.11 angle: the worst consequence would not be a doubled keystroke but a DESTRUCTIVE action done twice.** The close button read the frame's press queue OUTSIDE `route_the_buttons`'s `last_frame_with_buttons != Some(cumulative_frame_nr)` gate; under a (hand-made) repeated pass a second console closed with the same click. THREE panes are needed to see it: with two, the survivor of the first close carries no button.

**The transferable rule, still sound as defence**: anything that ACTS on a pointer event goes inside the frame gate, and its test needs three panes. The right number matters — `cumulative_frame_nr` goes up once per frame; `cumulative_pass_nr` per pass and would filter nothing.

| About the event | What was decided |
|---|---|
| Only looking at it | Nothing: reading it twice comes to the same |
| Acting and it can be consumed | Consume it (`retain`), like the keyboard and the wheel |
| Acting and it cannot be consumed | Keep the frame count, not the pass count |

**Side effect worth knowing**: a «do not repeat what was already counted» guard (like not reporting the same cell twice while dragging) covers a repeated datum up without meaning to, which makes a defect show on one path and not the next.

## Where it lives

crates/app/src/pointer.rs (`handle_the_mouse_without_claiming_it`: `last_frame_with_buttons` gate around `open_link_presses`, `whose_button_was_pressed`, `route_the_buttons`); keyboard `retain` in crates/app/src/keyboard.rs (`send_keys`); wheel in `collect_the_wheel`; tripwire and simulated-repeat tests in crates/app/src/lib.rs; amendments to D31, D46, D51
