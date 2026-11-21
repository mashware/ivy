---
id: asp_zoom-needs-an-own-viewport-not-a-scroll-area_8xnj3d
slug: zoom-needs-an-own-viewport-not-a-scroll-area
name: >-
  Zoom around the pointer needs an own viewport: egui's ScrollArea clamps the
  offset, and a zoom test must start smaller than the view
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T23:24:45.563Z'
updated_at: '2026-11-20T23:24:45.563Z'
last_verified: '2026-11-20T23:24:45.563Z'
file_paths:
  - crates/app/src/viewer/image.rs
  - crates/app/src/viewer/image/viewport.rs
symbols:
  - zoom_around
  - clamp
content_hashes: {}
tags:
  - egui
  - viewer
  - test-disguise
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36 `ScrollArea` clamps its offset to `[0, content - view]` per axis on the current frame's
size (scroll_area.rs:742-743, 1082, 1437-1438, 1527-1529). A zoom that keeps the point under the
pointer by adjusting the offset therefore pins to the top-left on any axis where the zoomed content
does not yet fill the view. The first headless test used a 2400x1600 fixture larger than the view on
both axes and stayed green while the owner saw the image grow from its corner on the desktop.

Forcing the ScrollArea to hold out-of-range offsets (blank space laid before and after) broke it
again three ways: a stale trailing blank after the view grows, scroll-bar drags remapped against
content that shrinks every frame (the thumb runs ahead and jumps), and guessing where egui puts its
bars. What held (#178, D72): the viewer keeps its own scale and offset, paints the image clipped,
has no scroll bars, and clamps per axis (a larger axis never opens a gap; a zoom never clamps a
smaller axis; a pan only brings it back inside).

Test rule: a zoom test starts from fit with at least one axis smaller than the view, and asserts the
point under the pointer, not a field.

## Where it lives

crates/app/src/viewer/image.rs; crates/app/src/viewer/image/viewport.rs
