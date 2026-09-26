---
id: asp_egui-hands-a-frame-s-events-to-its-first-pass-only_v3dck6
slug: egui-hands-a-frame-s-events-to-its-first-pass-only
name: >-
  egui 0.36 hands a frame's events to its FIRST pass only: what repeats is the
  closure, not the clicks — and ~40 comments say otherwise (#290)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T20:49:08.085Z'
updated_at: '2026-09-26T20:49:08.085Z'
last_verified: '2026-09-26T20:49:08.085Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
symbols:
  - last_frame_with_buttons
  - handle_the_mouse
content_hashes:
  crates/app/src/lib.rs: 'sha256:8e58392b7233a1e47075f6bf4e587453eb4c26535fc2c4209c87c3ffc8553a96'
  crates/app/src/pointer.rs: 'sha256:f384df68dd3715e3e27ade9e33ef2e44981f70f478d0876379c3b2307411dd63'
tags:
  - egui
  - events
  - repeated-pass
  - tripwire
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured 26 September 2026 (#200).** egui 0.36.1's loop behind `run_ui` (private `Context::run_dyn`, `context.rs:823-874`) begins every pass with `begin_pass(new_input.take())`, and `RawInput::take` (`data/input/raw_input.rs:137`) moves `events` out. A one-off test — one primary press, `request_discard` on pass 1 — saw `(events, any_pressed) = [(1, true), (0, false)]`. The repeated pass keeps the pointer position and modifiers, but gets **no events**.

**What does run twice is the closure**: eframe puts `logic` and `ui` inside `run_ui` (`epi_integration.rs:288-303`), so pumping, painting and the frame close really happen twice. Guards about those are real (e.g. the frame close keyed by `cumulative_frame_nr`, pinned by `egui_counts_each_frame_once_and_every_pass_separately`). Guards against acting on the same **event** twice — the button marker `last_frame_with_buttons`, the keyboard and wheel `retain`, the tree divider's absolute position — protect a case this egui never produces.

**The project believed the opposite in ~40 comments and two D-record passages** (docs/02-decisiones.md around 5124 and 6564), and one card reported a measured double press that this egui cannot produce. Issue #290 carries the correction and the decision whether the event guards stay as defence. Tests that pin them simulate the repeat by calling `handle_the_mouse` twice inside ONE egui pass (valid only because the barrier reads the frame number). Tripwire: `egui_hands_a_frame_s_events_to_its_first_pass_only` in crates/app/src/lib.rs — red the day an upgrade replays events.

**Method lesson, again ([[someone-elses-api-is-read-not-assumed]])**: a belief repeated in forty comments was never read in the library's source; one `sed` of `raw_input.rs` settled it.

## Where it lives

egui-0.36.1 src/context.rs:823-874 and src/data/input/raw_input.rs:124-143; tripwire test in crates/app/src/lib.rs; issue #290
