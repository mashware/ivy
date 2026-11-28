---
id: asp_mouse-report-mode-needs-positive-control_xdj05a
slug: mouse-report-mode-needs-positive-control
name: A mouse mode bit needs a supported-report positive control
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-28T17:39:14.739Z'
updated_at: '2026-11-28T17:39:14.739Z'
last_verified: '2026-11-28T17:39:14.739Z'
file_paths:
  - crates/app/src/mouse.rs
  - crates/app/src/pointer.rs
  - crates/app/src/chrome.rs
  - crates/app/src/usage/chip/summary_tests.rs
symbols:
  - MouseReport
content_hashes:
  crates/app/src/mouse.rs: 'sha256:6c91f031974f81117686dd3dae58ed6bd11587fcecd7d4a498c35f92fcbe77f2'
  crates/app/src/pointer.rs: 'sha256:4bb08fea5c7726ac92cce8dde2d5899d205192fd848c250973689e8435d9c480'
  crates/app/src/chrome.rs: 'sha256:f7776205fbf80d4d3f4c0ee319f71dbaef8743017e24712d483eba45c3bd1c21'
tags:
  - mouse
  - testing
  - positive-control
  - input-ownership
feature_id: feat_term-core_nx0g1r
---

## What it does

A terminal mouse-mode flag is parser state, not evidence that a gesture is encoded or reaches
the program. DECSET 1003 can set a motion flag while the app's MouseReport vocabulary still has
only Press, Release, Drag and Wheel. A negative raw-PTY assertion for bare PointerMoved is
therefore vacuous unless a supported-report positive control proves that the harness reaches the
program. In the verified path, no-button movement produced no bytes even with the summary
closed. Broadly consuming pointer movement to prevent that nonexistent report instead discarded
simultaneous keyboard text; the raw-PTY typing witness rejected that change. Test both the
actual gesture vocabulary and the observable receiver bytes before adding ownership gates.

## Where it lives

crates/app/src/mouse.rs (MouseReport); crates/app/src/pointer.rs; crates/app/src/chrome.rs;
crates/app/src/usage/chip/summary_tests.rs. Verified 2026-11-28 against the reviewed #283
candidate, with supported mouse-report and concurrent typing controls. This records the measured
path, not a promise that every future mouse mode has the same vocabulary.
