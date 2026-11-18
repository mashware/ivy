---
id: asp_egui-ids-collide-across-panes_fx4m6k
slug: egui-ids-collide-across-panes
name: >-
  Every console is painted with the same egui `ui`: a widget id built from a
  per-pane index collides across consoles
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T23:35:35.114Z'
updated_at: '2026-11-18T23:35:35.114Z'
last_verified: '2026-11-18T23:35:35.114Z'
file_paths:
  - crates/app/src/search/floating.rs
  - crates/app/src/header/integrated.rs
symbols:
  - 'floating::paint'
content_hashes:
  crates/app/src/header/integrated.rs: 'sha256:e1ec2d740b3e6c85a6e6c58db9e809c4662ecc831767403ff04187135723c5e9'
tags:
  - egui
  - ids
  - tooltips
feature_id: feat_term-core_nx0g1r
---

## What it does

Every console pane is painted with the same egui `ui`, so a widget id built only from an index
inside the pane (`ui.id().with(("search-box", index))`) is the same in every console. In egui 0.36.1
`create_widget` inserts into the pass's widget map and an occupied id is merged: the rect is the
last one, the sense is OR'd (widget_rect.rs:189-195). The second console's hover area replaces the
first's, hover is resolved by id so the first console's tooltip shows when the second is hovered,
and debug builds draw a red «Widget ID clash» overlay.

Salt any per-pane widget id with something that is per pane: the integrated header uses its strip's
corner bits, the search box (#120) the console's corner bits.

Evidence: built-in review finding in #120, confirmed by a skeptic reading egui-0.36.1's
widget_rect.rs and context.rs.

## Where it lives

crates/app/src/search/floating.rs (`paint`, the tooltip ids); crates/app/src/header/integrated.rs
(the header's own salted ids).
