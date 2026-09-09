---
id: asp_sending-the-keys-empties-the-frames-events_2wffme
slug: sending-the-keys-empties-the-frames-events
name: >-
  Dispatching the frame's keys empties egui's event list, so a question asked of
  the same frame afterwards always answers no — silently
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:53:06.375Z'
updated_at: '2026-09-09T21:53:06.375Z'
last_verified: '2026-09-09T21:53:06.375Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/keyboard.rs
symbols:
  - send_keys_to_the_focused
  - somebody_typed
content_hashes:
  crates/app/src/lib.rs: 'sha256:e8bd846a3440e53be756ff25a7fc65376922d1ef1119dff8d912594f58492213'
  crates/app/src/keyboard.rs: 'sha256:ea5317780ba27064e56b3be190607a40a13034fb7884f50149df3c237e63f76b'
tags:
  - egui
  - keyboard
  - window-loop
feature_id: feat_term-core_nx0g1r
---

## What it does

**`Window::send_keys_to_the_focused` hands the frame's events to the focused pane, and that path consumes them from egui's input state.** A question asked of the same frame *afterwards* — *did somebody type?* — therefore always answers no, with nothing failing to compile and nothing looking wrong.

The cost, measured: a keystroke anchor placed after the hand-over **never moved**, so a blinking cursor would have gone dark and stayed dark however much you typed. The test written for it failed, and the failure looked like the anchor being wrong rather than the read being late. What located it was a probe inside the same closure: `events=[Text("a")]` and `somebody_typed=true` at the **top** of the pass, and the anchor still unchanged after it.

> **Anything the window needs to know about a frame's raw input is read BEFORE the input is dispatched, not after.** The general shape: in an immediate-mode loop, «the events of this frame» are a resource with an owner, and the first consumer empties it for everyone behind them. A second reader is not a second opinion — it is a reader of an empty list.

The same seam has a cost the other way: a pass over the events added at the top of that function lands **inside** the published «key → queue» span, which is a known and declared measurement hole in that function's own docblock. Adding to it is fine; adding to it silently is not (**D13**).

## Where it lives

`crates/app/src/lib.rs` — `send_keys_to_the_focused` asks `keyboard::somebody_typed` as its first act, with a comment saying why it cannot be its last.
