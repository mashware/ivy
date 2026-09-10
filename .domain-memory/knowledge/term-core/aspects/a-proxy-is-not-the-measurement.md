---
id: asp_a-proxy-is-not-the-measurement_g565y3
slug: a-proxy-is-not-the-measurement
name: >-
  Measuring a proxy instead of the thing: the two sentences were the same LENGTH
  and different HEIGHTS, so the comparison decided nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T23:06:08.762Z'
updated_at: '2026-09-07T23:06:08.762Z'
last_verified: '2026-09-07T23:06:08.762Z'
file_paths:
  - crates/app/src/painter.rs
symbols:
  - the_question_box
content_hashes:
  crates/app/src/painter.rs: 'sha256:7ace3eefa2eb016d1eafeac479942d748c5d09c883675f313c46b4e7a5642a2c'
tags:
  - measured
  - geometry
  - method
  - painter
feature_id: feat_term-core_nx0g1r
---

## What it does

**A box had to be big enough for whichever of two sentences would be shown, so it was measured against the longer one. Both are exactly 58 characters, so the comparison decided nothing — and what sets a box's height is not length, it is wrapping.**

Measured across console widths from 20 to 60 cells: the two wrap differently in **five** of them, and **in both directions**.

| Console width | «Escape or a click discards it · Ctrl+Enter sends it anyway» | «A click discards it · focus this console to send it anyway» |
|---|---|---|
| 20 cells | 368.9 | **384.9** |
| 25 cells | **304.9** | 288.9 |
| 26 cells | **288.9** | 272.9 |
| 27–28 cells | **272.9** | 256.9 |

So at 20 cells the box came out **one line short of the sentence drawn in it** — text painted outside its own border, and the region a click is judged by smaller than the drawing. Twenty cells is this application's floor for a console, so it is not a contrived width.

The cause is the longest unbreakable word: `Ctrl+Enter` is ten characters against `console`'s seven, and a wrap breaks on words.

## The general shape

> **A proxy is not the measurement.** Length stood in for height, and it was a good proxy right up to the width where it was not. Nothing in the code said «proxy»: it said «the longer of the two», which reads like the thing itself.

This repository has the same shape on file twice in other clothes: a width counted in **points** standing in for how many characters fit ([[the-panel-yields-and-its-width-is-in-cells]]), and a test literal whose **length** is the payload, where translating the words moves a wide character off the edge it was placed on ([[what-breaks-without-anything-failing]]).

## And the test that pins it had the same trap one level up

The first version compared **the two boxes to each other** and asserted they were equal. That stayed green under the very mutation it was written for: measuring both with one sentence makes them equal too.

> **Assert the property, not the agreement.** The box is asserted to be at least as tall as **each** sentence laid out at the same wrap width — and at the widths where they measurably differ, which had to be found by walking the range rather than picked.

The fix in the code is the same idea: lay out **both** and take the component-wise maximum. One extra layout, on a path that only runs while a question is standing.

## Where it lives

`painter::the_question_box` in `crates/app/src/painter.rs`, and the test `the_notice_box_does_not_shrink_when_the_keyboard_leaves` in `crates/app/src/lib.rs`, which carries the measured table in its doc comment.
