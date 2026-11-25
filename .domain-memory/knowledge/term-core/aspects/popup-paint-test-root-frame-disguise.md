---
id: asp_popup-paint-test-root-frame-disguise_h9kmxm
slug: popup-paint-test-root-frame-disguise
name: Popup paint assertions can accidentally measure the root frame
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T21:13:35.070Z'
updated_at: '2026-11-25T21:13:35.070Z'
last_verified: '2026-11-25T21:13:35.070Z'
file_paths:
  - crates/app/src/viewer/code_header/tests.rs
  - crates/app/src/viewer/code_header/tests/window.rs
symbols:
  - painted_shapes
  - the_actual_menu_paints_both_wrap_states_and_keeps_comfortable_padding
content_hashes:
  crates/app/src/viewer/code_header/tests.rs: 'sha256:adbc0e7299ff3b1a4c36d35395dc984398264ba3bd173efae136584a3d643d27'
  crates/app/src/viewer/code_header/tests/window.rs: 'sha256:70975c2e79ecbb8237bcfd804261207ebc114f1b032c4185d8b0388a6447ae2b'
tags:
  - tests
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does

egui popup frame drawing may be nested in Shape::Vec. A test inspecting only top-level Rect shapes accidentally selected the 900x600 root frame as the menu, so its text-to-frame padding assertion stayed green when menu padding was deliberately removed. Flatten the shape tree, identify the bounded popup, and compare its frame with actual AccessKit button bounds plus painted label bounds; the same padding mutation then failed.

## Where it lives

crates/app/src/viewer/code_header/tests.rs, crates/app/src/viewer/code_header/tests/window.rs; symbols: painted_shapes, the_actual_menu_paints_both_wrap_states_and_keeps_comfortable_padding. Evidence: issue 245 regression tests and targeted mutation checks (November 2026).
