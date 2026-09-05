---
id: asp_the-layout-cache-invalidates-by-cells-not-by-points_0eqcww
slug: the-layout-cache-invalidates-by-cells-not-by-points
name: >-
  The cache of the panel's layout invalidates by WHOLE CELLS and not by points:
  a drag of 300 points costs 36 layouts, not 300
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T15:31:13.202Z'
updated_at: '2026-08-19T15:31:13.202Z'
last_verified: '2026-08-19T15:31:13.202Z'
file_paths:
  - crates/app/src/panel.rs
symbols:
  - paint
  - lay_out_if_needed
  - PanelLayout
content_hashes:
  crates/app/src/panel.rs: 'sha256:33a221357d004d8d72e2406f45898b7bcf0384b4637af8a486cb30a2eeda0668'
tags:
  - panel
  - performance
  - cache
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The key of the cache of the panel's layout is in whole cells, not in points, and that changes the size of the risk of any gesture that moves its width.**

`panel::paint` computes the width it lays out with like this: `((rect.width() / metrics.width()) as usize).saturating_sub(2 * MARGIN as usize)`, and that is the `usize` that `lay_out_if_needed` compares with the stored layout.

> **The natural intuition is false, and it is the one that has to be corrected**: it looks as though a continuous drag —which changes the width in points on every frame— would invalidate the cache on **every** frame. It does not: it only invalidates it **when crossing a cell border**. A drag of 300 points with a cell of ~8.4 costs some **36 layouts, not 300**.

**Why it matters more than it looks.** The worst case of laying out is measured and it is brutal —a line of 260 000 characters costs **65.8 ms**, against a budget of 16.7 ms per frame; see [[the-worst-case-for-painting-is-the-inverse-of-reading]]—, so «the cache is invalidated on every frame of the gesture» and «it is invalidated every eight points» are two very different conclusions about whether a gesture that narrows the panel is viable. With the second, it is, without bounding anything.

**And there is a consequence for the SCOPE, not only for the cost.** The scenario of «two different widths in points inside the same cell» **is not produced by the gesture**: the width is stored in whole cells and the tiling multiplies them, so the panel's rectangle is always an exact multiple of the cell. It **is** produced by resizing the window in a pane where the tiling clips the panel, because there its width stops being a multiple — a path that existed long before the border could be dragged.

> **The consequence for the tests is the one that bites.** A criterion along the lines of «the gesture does not redo the layout more than once per frame» **is satisfied by construction** —the panel is painted once per pass, so there is a single width and a single query to the cache— and its test would pass without pinning anything down. What can be planted is the real property: **two different widths in points inside the same cell do not redo the layout**.

## Where it lives

`panel::paint` computes the key and `PanelState::lay_out_if_needed` compares it, both in `crates/app/src/panel.rs`. The width that goes in there is produced by `panel::set_aside` out of the width in cells that each pane stores.
