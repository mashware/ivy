---
id: asp_clicking-an-egui-combo-box-in-a-test_kkz5nz
slug: clicking-an-egui-combo-box-in-a-test
name: >-
  A headless test presses an egui ComboBox through AccessKit, and the combo is
  wider than the width asked by twice its padding
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T21:05:09.315Z'
updated_at: '2026-11-22T21:05:09.315Z'
last_verified: '2026-11-22T21:05:09.315Z'
file_paths:
  - crates/app/src/viewer/image/tools.rs
  - crates/app/src/viewer/image.rs
symbols:
  - chooser
  - picker_room
  - accesskit_at
content_hashes: {}
tags:
  - egui
  - testing
  - accesskit
feature_id: feat_term-core_nx0g1r
---

## What it does

**An egui ComboBox exposes no rect, but a headless test can still press it and an entry of its
popup through AccessKit:**

1. call `ctx.enable_accesskit()`;
2. name the combo with
   `response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, NAME))`;
3. read `FullOutput.platform_output.accesskit_update.nodes`, find the node whose `label()` is the
   name, and click the centre of its `bounds()`.

Popup entries (`selectable_label`) are found the same way, by their text.

**Width:** egui 0.36 sizes a combo box as
`max(icon_width + icon_spacing, width_asked - 2 * button_padding.x) + 2 * button_padding.x`, so a
neighbour laid out for `.width(w)` alone overflows by the padding. Reserve the formula's result.

Evidence: `the_picker_beside_send_is_pressed_and_picks` in `viewer/image.rs`, which killed
mutation M7 (the picker never returns its pick); the width overflow was found by #199's built-in
review.

## Where it lives

crates/app/src/viewer/image/tools.rs (`chooser`, `picker_room`); crates/app/src/viewer/image.rs tests (`accesskit_at`)
