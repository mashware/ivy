---
id: asp_a-window-bound-request-must-wake-the-frame_2ab1re
slug: a-window-bound-request-must-wake-the-frame
name: >-
  A request only the window can answer is queued during PAINTING, after the
  frame's own drain — without a wake of its own the caller waits out the 5 s
  deadline and is told the window did not answer
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T18:27:42.988Z'
updated_at: '2026-09-16T18:27:42.988Z'
last_verified: '2026-09-16T18:27:42.988Z'
file_paths:
  - crates/app/src/control.rs
  - crates/app/src/lib.rs
  - crates/app/src/panel/pane.rs
symbols:
  - 'PaneControl::apply'
  - 'Window::answer_the_windows_requests'
  - 'Pane::refresh_its_panel'
content_hashes:
  crates/app/src/control.rs: 'sha256:953bd85dc730a278480bdb6e760773a2191539f835c733250f370faf7eb13542'
  crates/app/src/lib.rs: 'sha256:1faa7acf3e537b588bb2ade71bc87583a3362d09288f3078c41f73fc9b756137'
  crates/app/src/panel/pane.rs: 'sha256:74b039dfc25414d32ae832c6023c77953d693d532df3574bc14a7674e8980048'
tags:
  - reactivity
  - frame
  - control-channel
  - deadline
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Any queue that a painting fills for a consumer that drains once per frame needs its own wake, and the deadline makes the missing wake look like a broken window.**

On this codebase the pipeline is: a connection's request is handed to `PaneControl` and the listener calls the wake (`ctx.request_repaint`) as it sends it; the frame's drain (`Window::answer_the_windows_requests`, called in `ui` before the tab bar) picks up what only the tree can answer; but the pane's own `PaneControl::apply` runs **during painting** (`Pane::refresh_its_panel`, reached from `painter::paint_pane` and from `Window::measure_the_ones_out_of_sight`), which is **after** that drain. So the frame that receives a `split_open` cannot answer it: the request is moved to `waiting_on_the_window` once the drain has already run. With the window at rest, no further pass is asked for, and the caller reads the listener's `ANSWER_DEADLINE` message — «the window did not answer within five seconds» — for a request queued, healthy and one frame away. That is a lie about the request, not about the window.

**The fix that shipped:** `PaneControl` keeps the same wake closure it gives the channel (`Arc<dyn Fn() + Send + Sync>`, a field, called as `(self.wake)()`) and calls it whenever a delivery is deferred — the three `split_*` verbs and `What::Closed` alike.

**How it is tested, and the trick that makes the test honest:** the arrival's wake and the deferral's wake land on the same counter, so `control::tests::a_request_that_waits_for_the_window_wakes_it` waits for the arrival's first, records the count, then calls `apply` and asserts it rose again. Mutating the deferral's wake away turns it red.

**Discarded alternative:** draining after painting, which would need the geometry a split uses to be known after the tree walk that consumes it, and would put a new panel between the paint and the press — the window the panel already documents it does not have.

## How it flows

```mermaid
flowchart TD
  A[a connection writes split_open] --> B[listener wakes the window]
  B --> C[ui pass: drain the window-bound queue]
  C --> D[nothing there yet]
  D --> E[painting: refresh_its_panel -> apply defers the delivery]
  E --> F{was the window woken?}
  F -- no --> G[no further pass: the caller reads did not answer within five seconds]
  F -- yes --> H[next ui pass: the drain answers it]
```

## Where it lives

crates/app/src/control.rs — `PaneControl::apply` (the deferral and `(self.wake)()`), `PaneControl::open` (the wake kept as an `Arc`), `Window::answer_the_windows_requests` (the once-per-pass drain). Tests: `control::tests::a_request_that_waits_for_the_window_wakes_it`, `a_split_request_waits_for_the_window_instead_of_being_applied`. Related cards: the-heartbeat-was-armed-a-frame-late, a-press-cannot-see-a-newer-panel-than-the-one-painted.
