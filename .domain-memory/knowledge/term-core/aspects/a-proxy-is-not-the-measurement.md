---
id: asp_a-proxy-is-not-the-measurement_g565y3
slug: a-proxy-is-not-the-measurement
name: >-
  Measuring a proxy instead of the thing: the notice's sentences are compared by
  LAYOUT HEIGHT at every width, never by length or by a simulated wrap — and a
  new one may never be the tallest
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T23:06:08.762Z'
updated_at: '2026-09-26T14:21:34.678Z'
last_verified: '2026-09-26T14:21:34.678Z'
file_paths:
  - crates/app/src/painter/notices.rs
  - crates/app/src/lib.rs
symbols:
  - measure_the_question
  - EVERY_ANSWER
  - ANSWERED_ONCE_BACK
content_hashes:
  crates/app/src/painter/notices.rs: 'sha256:ad4f8857c2c5e495f094b96f491b54045f30df3e6b45d3a63c7b09a88ea7cee3'
  crates/app/src/lib.rs: 'sha256:def487034867d993c3bc3caa084d5e0a5ef05e2cb9e7f6d55ea2195164e51e51'
tags:
  - measured
  - geometry
  - method
  - painter
feature_id: feat_term-core_nx0g1r
---

## What it does

**A box had to be big enough for whichever of two sentences would be shown, so it was measured against the longer one. Both are exactly 58 characters, so the comparison decided nothing. What sets a box's height is not length, it is wrapping.**

Measured across console widths from 20 to 60 cells: the two wrap differently in **five** of them, and **in both directions**.

| Console width | «Escape or a click discards it · Ctrl+Enter sends it anyway» | «A click discards it · focus this console to send it anyway» |
|---|---|---|
| 20 cells | 368.9 | **384.9** |
| 25 cells | **304.9** | 288.9 |
| 26 cells | **288.9** | 272.9 |
| 27–28 cells | **272.9** | 256.9 |

So at 20 cells the box came out **one line short of the sentence drawn in it**. The text was painted outside its own border, and the region a click is judged by was smaller than the drawing. Twenty cells is this application's floor for a console, so it is not a contrived width.

The cause is the longest unbreakable word: `Ctrl+Enter` is ten characters against `console`'s seven, and a wrap breaks on words.

## The general shape

> **A proxy is not the measurement.** Length stood in for height, and it was a good proxy right up to the width where it was not. Nothing in the code said «proxy». It said «the longer of the two», which reads like the thing itself.

This repository has the same shape on file twice in other clothes:
- a width counted in **points** standing in for how many characters fit ([[the-panel-yields-and-its-width-is-in-cells]]);
- a test literal whose **length** is the payload, where translating the words moves a wide character off the edge it was placed on ([[what-breaks-without-anything-failing]]).

## And the test that pins it had the same trap one level up

The first version compared **the two boxes to each other** and asserted they were equal. That stayed green under the very mutation it was written for, because measuring both with one sentence makes them equal too.

> **Assert the property, not the agreement.** The box is asserted to be at least as tall as **each** sentence laid out at the same wrap width. The check runs at the widths where they measurably differ, and those widths had to be found by walking the range rather than picked.

## A third sentence, and why it may never be the tallest (#211, D31 amended)

The box is measured against **every** wording (`EVERY_ANSWER`) so that it keeps its size when the focus moves. The consequence is sharp: **a longer new wording raises the floor of every console**, including those in a focused window where it is never shown. Narrow consoles that used to hold the question then hold the paste in silence.

What was measured in that work:
- **73 chars** («come back to this window to send it with Ctrl+Enter»): did exactly this.
- **57 chars**, shorter than both others («come back, then Ctrl+Enter sends it»): still one line taller at **exactly 20 cells**.
- A **Python word-wrap simulation** said that sentence was fine at every width. Only egui's own layout found the 20-cell case.

> **Candidate wordings are compared with the real engine at every width, never by length and never by a simulated wrap.**

The cheap way to do it was a throwaway test that walks the candidates and prints the failing widths. The kept guard is `the_wording_for_an_absent_window_costs_no_console_its_notice`: 4..=120 cells, built-in font. A user font (D26) is outside that walk, which is why the runtime still lays out all three instead of trusting the test.

## Where it lives

`measure_the_question` and `EVERY_ANSWER` in `crates/app/src/painter/notices.rs`. The tests `the_notice_box_does_not_shrink_when_the_keyboard_leaves` (measured table in its doc comment), `no_console_gets_a_box_that_clips_the_way_out_it_will_show` and `the_wording_for_an_absent_window_costs_no_console_its_notice` are in `crates/app/src/lib.rs`.
