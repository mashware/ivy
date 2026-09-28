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
updated_at: '2026-09-26T21:29:18.591Z'
last_verified: '2026-09-26T21:29:18.591Z'
file_paths:
  - crates/app/src/links/console.rs
  - crates/app/src/pointer/links.rs
  - crates/app/src/panel/links.rs
  - crates/term-core/src/view.rs
  - docs/02-decisions.md
symbols:
  - PaintedConsole
  - LinkGesture
  - PaintedLink
  - 'View::link_at'
content_hashes:
  crates/app/src/links/console.rs: 'sha256:4f1b8ddb99c68deb0df51cb964403462d48c3e7ae797e531f9047c4d55e12a90'
  crates/app/src/pointer/links.rs: 'sha256:7e8a2830de64475ec210f446f7b372e783b46bafd435c61c78b179fd5ac9db1e'
  crates/app/src/panel/links.rs: 'sha256:9148b2f640bd2526f7691cb854e643281f7cfafe608700c1d4bd8f4310f26863'
  crates/term-core/src/view.rs: 'sha256:555d89ad879f7983354d1a0d6868ca13d479c31e1adb21e1dcd08481f7a83d7a'
  docs/02-decisions.md: 'sha256:7e5600d1c11b0e40126e8873b82fe6f878a2035602c771fcf8a0b3b3f20514b4'
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

- **Bound the point by the painted AREA, not only by the view's grid.** When a panel first appears or grows, the console rectangle shrinks in that frame but the grid is resized one frame later, so for one frame the view is wider than the area and a Ctrl+click over the panel strip resolved a console link nobody could see. Skeptic-confirmed on #257.
- **Freshness is `frame` or `frame + 1`**: the preceding frame is what is on the glass when a new one starts, and egui's repeated pass runs the same frame again. **Corrected 26 September 2026 (#290):** in egui 0.36.1 that repeated pass carries no button events, so it cannot open the same press twice; the once-per-frame barrier (`last_frame_with_buttons`) is kept as defence against an egui that replayed them. See [[egui-hands-a-frame-s-events-to-its-first-pass-only]].
- **A press consumed by a link still leaves egui's own pointer state behind**: `response.clicked()`/`drag_started()` fire from it on later frames, so the link gesture has to hold the pane until the frame after its release, or a Ctrl+click with a small movement becomes a selection, a focus change or a pane drag. And a frame's queue can carry an EARLIER click's release before the link press — only a release after the consumed press ends the gesture.

Plain URLs are parsed only for the clicked logical line (wrapped rows joined by the engine's wrap flag), so `ViewCell` never grows; OSC 8 spans are stored sparse beside the cells.

## Where it lives

`crates/app/src/links/console.rs` (`PaintedConsole`), `crates/app/src/pointer/links.rs` (`open_link_presses`, `LinkGesture`), `crates/app/src/panel/links.rs` (`PaintedLink`, `reachable`), `crates/term-core/src/view.rs` (`link_at`); decision D51 in `docs/02-decisions.md`.
