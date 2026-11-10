---
id: asp_egui-ui-put-centres-its-widget_7smv6z
slug: egui-ui-put-centres-its-widget
name: >-
  egui's `Ui::put` centres the widget in its rect: that was the centred text of
  the old Overview
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-10T16:09:17.446Z'
updated_at: '2026-11-10T16:09:17.446Z'
last_verified: '2026-11-10T16:09:17.446Z'
file_paths:
  - crates/app/src/board/view.rs
symbols:
  - text
  - paint_card
  - bounded
content_hashes: {}
tags:
  - egui
  - layout
feature_id: feat_term-core_nx0g1r
---

## What it does

`Ui::put(rect, widget)` places the widget CENTRED inside the rect, not at its left edge. The
Overview painted every text slot with `put(rect, Label::new(..).truncate())`, which is why every
line of it came out centred on screen (reported from a screenshot). The board paints a one-line slot
as `LayoutJob::simple_singleline` with `wrap = TextWrapping::truncate_at_width(width)`, laid out
through `ui.fonts_mut(|f| f.layout_job(job))` and drawn with `painter.galley` at the slot's left
edge: left-aligned, cut with «…», one row whatever the content. Producer text is still `bounded()`
before layout, because a legal document may fill 256 KiB.

## Where it lives

crates/app/src/board/view.rs (`text`, `paint_card`)
