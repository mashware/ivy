---
id: asp_the-repeated-pass-re-pumps-and-overwrites-the-round_95ka2b
slug: the-repeated-pass-re-pumps-and-overwrites-the-round
name: >-
  eframe runs `logic` AND `ui` inside the closure egui may repeat: in a repeated
  pass the pumping re-arms the round and the good one is lost in silence
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T16:49:53.587Z'
updated_at: '2026-08-16T16:49:53.587Z'
last_verified: '2026-08-16T16:49:53.587Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
symbols:
  - 'Window::logic'
  - 'Window::ui'
  - 'Pane::pump'
  - 'Timings::record_round'
  - 'Spans::and_also'
  - 'Window::close_frame'
content_hashes:
  crates/app/src/lib.rs: 'sha256:b27adaa55359fbc81c3b0a62f6993d4aecce7bd71aeff4f8faee1484d59dbe87'
  crates/app/src/measure.rs: 'sha256:1c09d77a593aaa0cb14679e7508ba91dad93bc864aa84fbb4f4a4378d4f0e069'
tags:
  - measurement
  - egui
  - eframe
  - instrumentation
  - D3
  - F2.1
feature_id: feat_term-core_nx0g1r
---

## What it does

**The fact of the API, read and not assumed:** `eframe` calls `App::logic` and `App::ui` **inside the same closure** that `Context::run_ui` may repeat — `eframe-0.36.1/src/native/epi_integration.rs:288-303`, with both `profiling::scope!` inside the closure. The natural intuition, and the one written down in this project before checking it, is that `logic` runs once per frame and `ui` may run twice. **It is false: both run per pass.** One more case of [[someone-elses-api-is-read-not-assumed]].

**The consequence was a live bias in the measurement, not a curiosity.** In a frame with the pass repeated —egui repeats it when something asks to be laid out again—: (1) `pump_them_all` runs again and `Pane::pump` **re-arms** `pending_measurement` with an interpreting time of ~0; (2) `Timings::record_round` did `self.pending = Some(spans)`, that is, it **overwrote**; (3) the good round of the first pass was lost and what the console really interpreted was published as «rest of the frame», which is the framework's cost — the exact figure with which **D3** judges egui. Measured during the review: **it declared 50 ns when interpreting had cost 20.158 µs**.

The same happened to the **painted** console (`paint_pane` also records once per pass) and it is **older than the tabs**: no PR of F2.1 introduced it, they only made it visible.

**It dragged a companion along:** the guard that keeps the frame from being closed twice used `ctx.cumulative_pass_nr()`, which goes up **per pass** and not per frame, so in a multi-pass frame it filtered nothing. `cumulative_frame_nr` is the per-frame one.

**Fixed in PR #65 (`d9d8fec`), both together:** `record_round` now **adds** instead of assigning, by means of `Spans::and_also`, which accumulates the spans of the two passes and keeps the `waited` of the first —the wait belongs to the frame, not to the pass, and adding it would count the same gap twice—; and the guard moved to `cumulative_frame_nr`.

> **The rule it leaves:** in an instrument whose pending span is **assigned** instead of accumulated, any path that can execute twice per frame loses the good sample in silence. The safe way to add a span to a round is to put it **inside** the one that already exists, never to record a round alongside it.

**And a testing lesson, because the mutation went through three:** two asserted on the field where the cost is recorded instead of on the effect it has —recording it without subtracting it from the total left the suite green— and one compared milliseconds where, in debug mode, queueing the painting already saturates the chosen margin.

## Where it lives

`crates/app/src/measure.rs` — `Timings::record_round`, `Spans::and_also`. `crates/app/src/lib.rs` — `Window::logic`, `Window::ui`, `Pane::pump`, `Window::close_frame`. The fact of the API, in `eframe-0.36.1/src/native/epi_integration.rs:288-303` of cargo's local registry.
