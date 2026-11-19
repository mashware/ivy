---
id: asp_startup-geometry-tests-must-park-the-output-bridge_9t7gef
slug: startup-geometry-tests-must-park-the-output-bridge
name: >-
  Startup geometry tests must park the output bridge: waiting for its completion
  hides the race
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-19T00:15:58.058Z'
updated_at: '2026-11-19T00:15:58.058Z'
last_verified: '2026-11-19T00:15:58.058Z'
file_paths:
  - crates/app/src/launch.rs
  - crates/app/src/pty_output.rs
  - crates/app/src/launch/tests.rs
symbols:
  - prepare
  - 'Process::startup_pending'
  - 'Process::interpreted'
  - 'PtyOutput::seeded'
content_hashes:
  crates/app/src/pty_output.rs: 'sha256:a4d23c4dc759b0fffd4494985ed440832c81a0fe1d3f8018767c00bf4037c7fa'
tags:
  - startup
  - terminal-geometry
  - async
  - testing
  - regression-oracles
feature_id: feat_term-core_nx0g1r
---

## What it does

An empty application output queue does not mean the program produced no startup output: the raw receiver may already hold bytes while its bridge thread has not started. Resizing on that empty queue parses later-enqueued startup bytes at a geometry the program did not use.

The launch worker captures a finite already-queued FIFO prefix and transfers its byte watermark. The normal frame budget consumes that prefix at the original geometry; resize and both queued and newly arriving input wait until the watermark is drained. Capturing on the worker avoids unbounded draining or prefix copying in the frame.

The regression must park the bridge before its first enqueue rather than wait for bridge termination. Include a prefix larger than the frame budget to prove that resize and both input paths remain held over multiple frames. Compare complete visible contents: reflow can make a cursor marker alone agree even when initial parsing used the wrong grid.

Evidence: #124 startup_prefix_is_interpreted_before_resize_even_with_the_bridge_parked and startup_snapshot_has_a_finite_chunk_bound_and_preserves_the_remaining_stream. Deliberately removing capture, resize gating, either input gate or the finite snapshot bound failed the corresponding assertions; source restored afterward. Verified 2026-11-19, candidate6cb89bb.

## Where it lives

crates/app/src/launch.rs — prepare, Process::interpreted/startup_pending, complete and finish_input; crates/app/src/pty_output.rs — seeded output bridge and normal pump; crates/app/src/launch/tests.rs — parked-bridge and finite-prefix regressions.
