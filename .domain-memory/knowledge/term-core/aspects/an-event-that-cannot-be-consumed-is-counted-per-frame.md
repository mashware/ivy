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
updated_at: '2026-09-26T20:49:07.795Z'
last_verified: '2026-09-26T20:49:07.795Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
symbols:
  - whose_button_was_pressed
  - route_the_buttons
  - last_frame_with_buttons
content_hashes:
  crates/app/src/lib.rs: 'sha256:8e58392b7233a1e47075f6bf4e587453eb4c26535fc2c4209c87c3ffc8553a96'
  crates/app/src/pointer.rs: 'sha256:f384df68dd3715e3e27ade9e33ef2e44981f70f478d0876379c3b2307411dd63'
tags:
  - egui
  - mouse
  - events
  - repeated-pass
  - corrected
feature_id: feat_term-core_nx0g1r
---

## What it does

**CORRECTION (26 September 2026, #200 / #290): the premise below is false for egui 0.36.1, and so is the double click it reports.** The loop behind `run_ui` begins every pass with `begin_pass(new_input.take())`, and `RawInput::take` moves the events out. Measured: one primary press plus `request_discard` on the first pass → the passes saw `(events, any_pressed) = [(1, true), (0, false)]`. A repeated pass sees the pointer where it was but **no events**, so neither the keyboard's `retain` nor the button frame count is protecting a case this egui produces. What does run twice is the **closure** (eframe puts `logic` and `ui` inside `run_ui`): guards about pumping, painting or closing the frame once are real. The tests that pin the button guard (`a_repeated_pass_does_not_send_the_click_twice_to_the_program`, `a_repeated_pass_does_not_close_a_second_console_with_the_same_click`, `a_repeated_pass_opens_one_url_for_one_physical_click`, and #200's `a_press_that_answers_the_question_is_not_handed_to_the_program_on_a_repeated_pass`) simulate the repeat by calling `handle_the_mouse` twice inside ONE egui pass — equivalent only because the barrier reads `cumulative_frame_nr`. The tripwire `egui_hands_a_frame_s_events_to_its_first_pass_only` goes red the day an egui upgrade starts replaying events; whether the guards stay as defence is #290's to decide. See [[egui-hands-a-frame-s-events-to-its-first-pass-only]].

The original reasoning, kept as history of why the guards exist:

**The rule, and it comes out of putting together two things this project already had on file separately.** egui was believed to run the interface pass **twice over the same input**, and that is why the keyboard and the wheel **consume** their events with `retain`.

**But there are events that cannot be consumed.** The pointer button ones are what egui needs in order to decide who is being dragged: whoever takes them away leaves the gesture never starting.

**F2.11 angle: the worst consequence would not be a doubled keystroke but a DESTRUCTIVE action done twice.** The close button read the frame's press queue OUTSIDE `route_the_buttons`'s `last_frame_with_buttons != Some(cumulative_frame_nr)` gate; under a (simulated) repeated pass a second console closed with the same click. THREE panes are needed to see it: with two, the survivor of the first close carries no button.

**The transferable rule, still sound as defence**: anything that ACTS on a pointer event goes inside the frame gate, and its test needs three panes. The right number matters — `cumulative_frame_nr` goes up once per frame; `cumulative_pass_nr` per pass and would filter nothing.

| About the event | What was decided |
|---|---|
| Only looking at it | Nothing: reading it twice comes to the same |
| Acting and it can be consumed | Consume it (`retain`), like the keyboard and the wheel |
| Acting and it cannot be consumed | Keep the frame count, not the pass count |

**Side effect worth knowing**: a «do not repeat what was already counted» guard (like not reporting the same cell twice while dragging) covers a repeated datum up without meaning to, which makes a defect show on one path and not the next.

## Where it lives

crates/app/src/pointer.rs (`handle_the_mouse_without_claiming_it`: `last_frame_with_buttons` gate around `open_link_presses`, `whose_button_was_pressed`, `route_the_buttons`); keyboard and wheel `retain` in crates/app/src; tripwire and simulated-repeat tests in crates/app/src/lib.rs; issue #290
