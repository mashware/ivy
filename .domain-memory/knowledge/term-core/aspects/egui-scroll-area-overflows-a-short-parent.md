---
id: asp_egui-scroll-area-overflows-a-short-parent_axs9kn
slug: egui-scroll-area-overflows-a-short-parent
name: >-
  An egui ScrollArea is never laid out shorter than its minimum height: in a
  short parent it paints past the bottom unless clipped
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T22:13:40.508Z'
updated_at: '2026-11-21T22:13:40.508Z'
last_verified: '2026-11-21T22:13:40.508Z'
file_paths:
  - crates/app/src/viewer/image/tools.rs
symbols: []
content_hashes: {}
tags:
  - egui
  - layout
  - clip
  - test-disguise
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36 lays a ScrollArea out at least at its minimum height (min_scrolled_height, 64 pt by
default) whatever space the parent has left. In the image annotation drawer (#197) the line band
under the notes column is a ScrollArea; on a drawer shorter than that minimum it painted past the
drawer's bottom edge, over whatever sits below. The parent's max_rect does not stop it: the shapes
are queued with the ui's clip rect, which is the parent's clip, not its rect.

What held: give the band its own clip (ui.set_clip_rect to the space actually left) before laying
the ScrollArea out. Test: lay the drawer at a height under the minimum and assert that no shape
painted by the band starts or ends below the drawer's rect. Removing the clip turned that test red
in #197.

## Where it lives

crates/app/src/viewer/image/tools.rs (footer band clip); egui-0.36.1 containers/scroll_area.rs
