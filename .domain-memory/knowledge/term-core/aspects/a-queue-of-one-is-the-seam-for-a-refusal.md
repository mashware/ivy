---
id: asp_a-queue-of-one-is-the-seam-for-a-refusal_s6es0t
slug: a-queue-of-one-is-the-seam-for-a-refusal
name: >-
  A bounded queue's refusal is tested by shrinking the queue to one and calling
  the real function — reaching it through the front door costs eight deadlines,
  which is why it had no test
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T19:00:20.850Z'
updated_at: '2026-09-16T19:00:20.850Z'
last_verified: '2026-09-16T19:00:20.850Z'
file_paths:
  - crates/panel-control/src/listener.rs
  - crates/app/src/control.rs
symbols:
  - publish
  - MAX_PENDING
  - ANSWER_DEADLINE
  - try_send
  - sync_channel
content_hashes:
  crates/panel-control/src/listener.rs: 'sha256:2af6d0bfb151c93e2fc56a245fa3a08b7264d1237e385787c3b65fe30d8779d6'
  crates/app/src/control.rs: 'sha256:80972ca7068a8864fc08f786251bf39b773b76a3e8aaabdaabdd0c4ecf2bbeed'
tags:
  - tests
  - backpressure
  - bounded-queue
  - seam
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

**A refusal that only happens at a queue's ceiling is cheap to test and expensive to reach, and the expensive route is why these refusals ship untested.**

The control channel hands each request to the window over a `sync_channel(MAX_PENDING)` — **eight** deliveries. When the window is minimised or occluded it does not drain, so `publish` answers `Failed("not draining")` instead of growing. Reaching that state **through the socket** means opening nine connections, and each one that is not answered burns `ANSWER_DEADLINE` (**5 s**): about **40 s** for one assertion, in a suite that runs on every commit. That cost is the whole reason the refusal had no test.

> **Shrink the queue, not the function.** Build a `sync_channel::<Delivery>(1)`, fill it with one throwaway delivery via `try_send` — a buffered send does not wait — and call `publish(…, &to_the_window, &wake)` directly. Its `try_send` hits `Full` on the first attempt and the refusal is reached in **milliseconds**, through the same function a real connection reaches.

**Why that is not the disguise it resembles.** This project's own rule is that a test which calls a setter instead of driving the door covers nothing ([[a-revocation-test-must-drive-the-door-not-the-setter]]). The distinction is what the test replaces: **the capacity is a parameter of the queue, the door is still the production function**. Deleting the `try_send` guard, or changing which reply it produces, turns this test red. Had it instead set a `refused` flag on the pane and asserted the flag, it would have covered nothing.

**It generalises to any backpressure refusal whose production capacity is large**: the number that makes the path expensive to reach is the capacity, and the capacity is the one thing a test is allowed to choose. The technique's sibling for the other direction — a wait that must be bounded — is [[a-test-for-a-hang-needs-a-shorter-deadline]]: there the deadline is chosen shorter than the failure's patience, here the queue is chosen shorter than production's.

**One trap while filling it:** the throwaway delivery has to be a legal `Delivery` with its own answer channel (`What::Closed` serves), and the receiving end must be **kept alive** — bound to `_inbox` and not to `_`, or the channel is closed and `try_send` fails as disconnected rather than full, which is a different refusal with a different message.

## Where it lives

`crates/panel-control/src/listener.rs` — `publish` (the `try_send` and its `Reply::Failed`), `ANSWER_DEADLINE`, and the test `listener::tests::a_publication_is_refused_when_the_window_is_not_draining`, whose docblock records the 8 × 5 s alternative. `crates/app/src/control.rs` — `MAX_PENDING` (8), the production capacity of that same queue. Related cards: [[a-revocation-test-must-drive-the-door-not-the-setter]], [[a-test-for-a-hang-needs-a-shorter-deadline]], [[the-per-frame-budget-does-not-bound-the-memory]].
