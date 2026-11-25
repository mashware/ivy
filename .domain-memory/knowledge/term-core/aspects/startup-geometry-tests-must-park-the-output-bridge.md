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
updated_at: '2026-11-25T22:33:13.022Z'
last_verified: '2026-11-25T22:33:13.022Z'
file_paths:
  - crates/app/src/launch.rs
  - crates/app/src/pty_output.rs
  - crates/app/src/launch/tests.rs
  - crates/app/src/viewer/interaction_tests.rs
symbols:
  - prepare
  - 'Process::startup_pending'
  - 'Process::interpreted'
  - 'PtyOutput::seeded'
  - control_click_reveals_markdown_in_files_before_opening_its_viewer
content_hashes:
  crates/app/src/launch.rs: 'sha256:d82ad7a71414f09c368326a30e166db031d7b756f86579d8e752f4eca057f79e'
  crates/app/src/pty_output.rs: 'sha256:aea4d95e850ac3dc311a759d6c1d56deef352d1c235790231f0e2c5a67ab12db'
  crates/app/src/launch/tests.rs: 'sha256:e8b8d3606f2711f39136b58bafc1e360d9bcb9da8024e6ff69eb660555e4942d'
  crates/app/src/viewer/interaction_tests.rs: 'sha256:f5dde8a4d6258434946fda94d75ceaaf7026d6a992edb2728eece069e7db5847'
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

### Synthetic renderer fixtures need a single output producer

A test that writes OSC7 and an absolute path directly through Screen::advance must not leave native startup or resize output attached to that same synthetic screen. The Markdown Ctrl-click fixture in #173 did: native CI painted only a filename suffix. Its is_some local-target assertion could also accept a different relative filename instead of the intended file.

Use the idle-channel boundary already used by the neighboring image fixture, retain the sender for its lifetime, assert the exact full target and deliberately cover a wrapped path. Keep the actual Ctrl-click, Files reveal, file activation and rendered Markdown checks. In a paired deterministic reproduction, the same original-channel queued clear/home/suffix bytes and wake handshake failed without isolation and passed with it; removing Ctrl separately failed the actual Files effect. The crafted replay proves the fixture boundary, not the exact native byte sequence that caused the original CI failure. Both repaired Markdown witnesses passed natively in run45696334411.

This does not replace the parked real bridge in the startup-geometry witness above: that test studies arrival ordering; the synthetic interaction fixture supplies its own screen content and must exclude a second writer.

## Where it lives

crates/app/src/launch.rs — prepare, Process::interpreted/startup_pending, complete and finish_input; crates/app/src/pty_output.rs — seeded output bridge and normal pump; crates/app/src/launch/tests.rs — parked-bridge and finite-prefix regressions.

crates/app/src/viewer/interaction_tests.rs::control_click_reveals_markdown_in_files_before_opening_its_viewer owns synthetic screen content and the idle output channel; The neighboring image interaction fixture is in the same module.
