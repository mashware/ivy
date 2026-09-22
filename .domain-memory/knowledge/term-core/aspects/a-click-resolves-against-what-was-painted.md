---
id: asp_a-click-resolves-against-what-was-painted_x6pyc7
slug: a-click-resolves-against-what-was-painted
name: >-
  A click that opens something resolves against the snapshot the last paint
  used, never the live grid or the current panel document
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T19:51:41.208Z'
updated_at: '2026-09-22T19:51:41.208Z'
last_verified: '2026-09-22T19:51:41.208Z'
file_paths:
  - crates/app/src/links/console.rs
  - crates/app/src/pointer/links.rs
  - crates/app/src/panel/links.rs
  - crates/term-core/src/view.rs
  - docs/02-decisiones.md
symbols:
  - PaintedConsole
  - LinkGesture
  - PaintedLink
  - 'View::link_at'
content_hashes:
  crates/term-core/src/view.rs: 'sha256:00f538e3d2bb8852962d02df4b33e7877c41bdc86cd4730fd9ade4d9a0aa4519'
  docs/02-decisiones.md: 'sha256:ecc41dc2084fbb08906e9695aecb329047cf089cc71722a9e293a06f8a26b2b9'
tags:
  - links
  - pointer
  - D51
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does

**The window pumps program output and can commit a panel document before it routes the mouse**, so a click resolved against the live `Screen` or the panel's current document can open something the person never saw — output that arrived after the paint, or a new document whose lines moved under the old pixels. That is why D51 makes a pane retain the exact `View` and console rectangle its painter just used (`links::PaintedConsole`, moved in, never cloned) and makes a painted panel region own its target string instead of looking it up by line index in `last_good`.

Three details that decide whether that holds:

- **Bound the point by the painted AREA, not only by the view's grid.** When a panel first appears or grows, the console rectangle shrinks in that frame but the grid is resized one frame later (`resize` runs at the start of the next frame's pump), so for one frame the view is wider than the area and a Ctrl+click over the panel strip resolved a console link nobody could see. Skeptic-confirmed on #257.
- **Freshness is `frame` or `frame + 1`**: the preceding frame is what is on the glass when a new one starts, and egui's repeated pass runs the same frame again — the once-per-frame barrier (`last_frame_with_buttons`) is what stops the second pass opening the same press twice.
- **A press consumed by a link still leaves egui's own pointer state behind**: `response.clicked()`/`drag_started()` fire from it on later frames, so the link gesture has to hold the pane until the frame after its release, or a Ctrl+click with a small movement becomes a selection, a focus change or a pane drag. And a frame's queue can carry an EARLIER click's release before the link press — only a release after the consumed press ends the gesture.

Plain URLs are parsed only for the clicked logical line (wrapped rows joined by the engine's wrap flag), so `ViewCell` never grows; OSC 8 spans are stored sparse beside the cells.

## Where it lives

`crates/app/src/links/console.rs` (`PaintedConsole`), `crates/app/src/pointer/links.rs` (`open_link_presses`, `LinkGesture`), `crates/app/src/panel/links.rs` (`PaintedLink`, `reachable`), `crates/term-core/src/view.rs` (`link_at`); decision D51 in `docs/02-decisiones.md`.
