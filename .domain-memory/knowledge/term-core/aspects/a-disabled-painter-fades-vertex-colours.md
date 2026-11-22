---
id: asp_a-disabled-painter-fades-vertex-colours_t7nx91
slug: a-disabled-painter-fades-vertex-colours
name: >-
  A disabled or faded egui painter fades a text galley's vertex colours, not
  TextShape::opacity_factor
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T22:13:40.598Z'
updated_at: '2026-11-21T22:13:40.598Z'
last_verified: '2026-11-21T22:13:40.598Z'
file_paths:
  - crates/app/src/viewer/image/tools.rs
symbols: []
content_hashes: {}
tags:
  - egui
  - test-disguise
  - opacity
feature_id: feat_term-core_nx0g1r
---

## What it does

In egui 0.36.1, Painter::transform_shape (painter.rs:202-206) applies a painter's reduced opacity,
including the one ui.disable() sets, through multiply_opacity, which calls epaint's
shape_transform::adjust_colors. For a Shape::Text that function rewrites underline.color,
fallback_color, override_text_color and every vertex colour in the galley's row meshes, and
explicitly leaves opacity_factor untouched (shape_transform.rs:77-100).

So a test checking that text was drawn greyed out must read the alpha of the galley's vertex colours
(or the fallback/override colour). opacity_factor stays 1.0 under a disabled painter. In #197 the
greyed line under the notes column was proven this way, and removing its ui.disable() turned the
vertex alpha back to 255.

Correction: the test-disguise paragraph of [[only-modal-surfaces-dim-the-consoles]] says a dimmed
pass changes only a TextShape's opacity_factor. The egui source says the opposite. Its practical
advice, to compare Rect fills, still holds.

## Where it lives

egui-0.36.1 src/painter.rs:202-206, 563; epaint-0.36.1 src/shape_transform.rs:77-100;
crates/app/src/viewer/image/tools.rs
