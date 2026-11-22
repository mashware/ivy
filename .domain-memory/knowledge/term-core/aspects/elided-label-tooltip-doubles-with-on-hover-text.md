---
id: asp_elided-label-tooltip-doubles-with-on-hover-text_mqqfz3
slug: elided-label-tooltip-doubles-with-on-hover-text
name: >-
  An elided egui Label already has its own tooltip: adding on_hover_text shows
  two
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T22:13:40.693Z'
updated_at: '2026-11-21T22:13:40.693Z'
last_verified: '2026-11-21T22:13:40.693Z'
file_paths:
  - crates/app/src/viewer/image/tools.rs
symbols: []
content_hashes: {}
tags:
  - egui
  - tooltip
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36 Label::truncate() with show_tooltip_when_elided(true), which is the default, shows the
full text on hover whenever the label was cut. Adding .on_hover_text(full) on the same response
stacks a second tooltip under egui's own, so hovering the cut notice in the image drawer (#197)
showed the text twice.

To say more than the full text (in #197: «Enlarge the drawer to send» when the Send button was
dropped), keep a single tooltip. Either turn show_tooltip_when_elided off and write one
on_hover_text with everything, or keep egui's own and add nothing. Test: hover the cut label and
count the tooltip areas, or the times the text was queued. Turning show_tooltip_when_elided off
without a replacement turned the #197 test red.

## Where it lives

crates/app/src/viewer/image/tools.rs (cut notice); egui-0.36.1 widgets/label.rs
