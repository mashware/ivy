---
id: asp_the-bridge-owns-the-receiver-in-its-own-thread_2e3604
slug: the-bridge-owns-the-receiver-in-its-own-thread
name: >-
  A refutation is a claim too: «the receiver cannot be dropped first» was
  refuted twice, and the exit that decided it is the one only reachable while
  the queue is jammed
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:24:29.841Z'
updated_at: '2026-09-10T16:07:59.735Z'
last_verified: '2026-09-10T16:07:59.735Z'
file_paths:
  - crates/app/src/pty_output.rs
  - crates/app/src/lib.rs
  - crates/term-core/src/lib.rs
symbols:
  - 'PtyOutput::new'
  - 'PtyOutput::drop'
  - deliver
  - Undelivered
  - Pane
content_hashes:
  crates/app/src/pty_output.rs: 'sha256:59875e0e20d664da06e7fc7eea8a1ee54071c54b2d01f31066da9da43987f610'
  crates/app/src/lib.rs: 'sha256:6e40d885aa0d84af380a9a90dbfd6902f6a9faac6931dd349cd81f06ba69ed84'
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
tags:
  - threads
  - drop-order
  - ownership
  - review
  - method
feature_id: feat_term-core_nx0g1r
---

## What it does

**This card said the opposite for a few hours, and the arc is the lesson. Three rounds on one claim:**

1. **A reviewer said** that `term-core`'s ability to tell «somebody is closing this console» from «the far end of my output vanished on its own» depends on `struct Pane` declaring `pty` before `pty_output`, because Rust drops fields in declaration order and only that makes the stop flag go up first.
2. **A skeptic refuted it**: `PtyOutput::new` **moves** the `Receiver` into its own thread, whose loop is a `for` over that channel, so the thread parks in a receive **holding** the receiver and cannot let go until the reader's sender closes. Its `Drop` sets a flag and notifies a condvar and does not join. Conclusion: the receiver cannot disappear first, whatever the field order.
3. **A second reviewer refuted the refutation**, and it is right. **The bridge has another exit.** Its pick-up loop does `break 'pickup` when its own stop flag is up — the flag its destructor sets — and that arm sits **inside the wait for room**, reachable **only while the queue is at its ceiling**. Which is precisely the jam that also parks the reader inside a delivery. The skeptic read the destructor and the happy path and missed the arm in the wait; and there is already a test in that module demonstrating the receiver being dropped with the console alive.

> **So the dependency is real, in a narrower and checkable form: reverse those two field declarations and every ordinary close of a JAMMED pane is recorded as «nobody was listening»** — the terminal telling somebody a console went silent on the pane they just closed. Not a conjunction nobody has seen: a jammed queue is the state a pane closed mid-dump is in.

**Two rules, and the second is the expensive one.**

- **Before writing down that behaviour depends on drop order, find out who owns the value.** A struct field and a value moved into a thread are indistinguishable at the call site and are dropped at completely different times. This project learned the same from the other side in [[moving-a-value-into-a-thread-moves-its-drop]].
- **A refutation needs the same exhaustive read the claim needed.** «It cannot happen» is a claim about every path, and one unexamined exit is enough to make it false — here the one guarded by a condition («the queue is full») that reads like an edge case and is in fact the ordinary case for the failure under discussion. The cheap check is to enumerate the loop's `break`s and `return`s before believing any statement about when a thread ends.

**Where the truth is written**: `deliver`'s own comment says the crate cannot enforce the order and asks for a test in whoever first reads the distinction. That comment was right through all three rounds; **D38**'s open list was wrong twice.

## Where it lives

`crates/app/src/pty_output.rs` — `PtyOutput::new` moving the receiver into its thread, the `break 'pickup` inside the wait for room, and `Drop for PtyOutput`; the test in that module that ends the bridge's thread with the queue full. `crates/app/src/lib.rs` — `struct Pane`'s field order, which **is** the mechanism. `crates/term-core/src/lib.rs` — `deliver`'s two failures. The decision is **D38**.
