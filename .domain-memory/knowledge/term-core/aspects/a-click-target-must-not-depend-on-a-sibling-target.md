---
id: asp_a-click-target-must-not-depend-on-a-sibling-target_a4ps7f
slug: a-click-target-must-not-depend-on-a-sibling-target
name: >-
  A click target whose place depends on a sibling target's state turns one press
  into two answers — and the notice pass runs on every mouse pass
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T21:25:18.400Z'
updated_at: '2026-09-26T20:49:07.959Z'
last_verified: '2026-09-26T20:49:07.959Z'
file_paths:
  - crates/app/src/pointer.rs
  - crates/app/src/painter/notices.rs
symbols:
  - answer_a_clicked_question
  - last_frame_with_buttons
  - the_boxes
  - Notices
content_hashes:
  crates/app/src/pointer.rs: 'sha256:f384df68dd3715e3e27ade9e33ef2e44981f70f478d0876379c3b2307411dd63'
  crates/app/src/painter/notices.rs: 'sha256:00a25ae23301759a442babe1ce0f676edfca63db465d6da1f5f162497ac17746'
tags:
  - egui
  - mouse
  - notice
  - held-paste
  - review-finding
feature_id: feat_term-core_nx0g1r
---

## What it does

**The rule: a clickable box's rectangle may depend on the console and on what the box itself says — never on the state of a sibling target, nor on a live value that can move between the paint and the press.**

**Why it bites here.** `Window::answer_a_clicked_question` (the pass that answers the held-paste notice, its «Send it anyway» button and the «stopped receiving» sentence) runs on **every** mouse pass, before the once-per-frame `last_frame_with_buttons` barrier, and each time it re-reads the first bare press of the queue. That was idempotent while every branch removed its own target (discard → no question, dismiss → no sentence). It stops being idempotent the moment one target's **place** depends on another's **state**.

**What happened in #199.** The first cut hid the send button while «stopped receiving» stood. Dismissing the sentence made the button appear **exactly under the dismissing click**, so a double click **sent the held paste**, the irreversible answer, when the person meant to dismiss a notice. The same cut read `pty.ending()` live, so a reading that ended between the paint and the press grew the question over the button, and a press aimed at Send **discarded**. Found by the built-in code review; the four-reviewer panel missed both.

**Correction (26 September 2026, #290):** this card also named «egui running the same press through its repeated pass» as a trigger. egui 0.36.1 hands a frame's events to its first pass only (measured `[(1, true), (0, false)]`), so that trigger does not occur today; the double click by a person does. The rule stands either way. See [[egui-hands-a-frame-s-events-to-its-first-pass-only]].

**What shipped.** The button's place (`Notices.slot`) is computed only from the measured question and the console rectangle; the sentence goes **under** the button; the only state-dependent part is whether the button is **offered** (`is_deaf`), and a state that only ever turns on can only move a box upwards onto the empty slot, where a press dismisses the sentence — never sends. A test pins it: dismissing the sentence moves neither the question nor the button.

See [[an-event-that-cannot-be-consumed-is-counted-per-frame]] and [[a-click-resolves-against-what-was-painted]].

## Where it lives

crates/app/src/pointer.rs (answer_a_clicked_question, before the last_frame_with_buttons barrier); crates/app/src/painter/notices.rs (the_boxes, Notices.slot); docs/02-decisiones.md D31 «Amended again»
