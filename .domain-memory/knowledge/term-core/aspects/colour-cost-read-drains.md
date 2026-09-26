---
id: asp_colour-cost-read-drains_ah9d6a
slug: colour-cost-read-drains
name: >-
  Reading the colour cost empties it, so a second surface that rasterizes needs
  a read of its own right after its work
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T16:08:28.270Z'
updated_at: '2026-09-26T16:08:28.270Z'
last_verified: '2026-09-26T16:08:28.270Z'
file_paths:
  - crates/app/src/fonts.rs
  - crates/app/src/painter.rs
symbols:
  - color_cost
  - paint_pane
content_hashes:
  crates/app/src/fonts.rs: 'sha256:6437b6d69ce3d04e8175d68c813e4c4caef651a8e3dbcad219ba51a77ce6fe91'
  crates/app/src/painter.rs: 'sha256:86310d51e47691a817336e50660b43133ea20d2136f7978825b2ab34f3279d28'
tags:
  - measurement
  - d13
  - colour-emoji
feature_id: feat_term-core_nx0g1r
---

## What it does

`Discoverer::color_cost()` DRAINS its counter (`std::mem::take`), so **where it is read decides which span the rasterizing is subtracted from**. The console's read sits right after the console is queued and before the panel is painted. When the status panel started rasterizing colour glyphs (#178) it needed a **second** read right after `panel::paint`, taken off the panel's span and summed into what the pane declares.

> **Moving the single read later looks equivalent and is not.** It subtracts the panel's rasterizing from the console's «enqueueing» span, which never did that work — the console under-reports, to zero on a first emoji — and leaves it inside the panel's span as well, which `lib.rs` adds to `timed_separately`: counted twice.

It was caught by the design challenger, before any code, from reading `fonts.rs:422` and `painter.rs:1032-1068`. No test can see which span a few microseconds landed in: the read-and-sum is tested (the panel's glyph appears in the pane's declared cost, and the counter is empty afterwards), and the placement of the two reads is a declared guard. It belongs to the family of [[a-frame-cost-is-declared-once]]: a cost read from one span when it lands in two.

## Where it lives

crates/app/src/painter.rs `paint_pane` (the two `color_cost()` reads around `panel::paint`); crates/app/src/fonts.rs `Discoverer::color_cost`
