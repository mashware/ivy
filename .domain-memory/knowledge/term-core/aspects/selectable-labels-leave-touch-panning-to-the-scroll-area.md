---
id: asp_selectable-labels-leave-touch-panning-to-the-scroll-area_nqv6q6
slug: selectable-labels-leave-touch-panning-to-the-scroll-area
name: Selectable labels leave touch panning to the scroll area
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T11:35:21.327Z'
updated_at: '2026-11-26T11:35:21.327Z'
last_verified: '2026-11-26T11:35:21.327Z'
file_paths:
  - crates/app/src/viewer/markdown/selection.rs
  - crates/app/src/viewer/markdown/selection_tests.rs
symbols:
  - 'Frame::new'
content_hashes:
  crates/app/src/viewer/markdown/selection.rs: 'sha256:b373d35e373ea47682529673540ef783207f97b5ad39af26780b229f91f4a7dd'
  crates/app/src/viewer/markdown/selection_tests.rs: 'sha256:47d8e17bb28e54fdeca08b87f2077940b2c71f239a722feee570603328365271'
tags:
  - egui
  - markdown
  - selection
feature_id: feat_term-core_nx0g1r
---

## What it does

On a detected touch screen egui 0.36.1 gives a selectable label click sense. A custom selection
surface that claims drag unconditionally prevents the scroll area from receiving the finger pan.
The Markdown viewer needs click sense on touch screens and click-and-drag sense elsewhere. In a
headless test, send both touch and emulated pointer events, then verify the text actually moves
and copying yields no selection from the pan.

## Where it lives

The Markdown selection surface in crates/app/src/viewer/markdown/selection.rs registers its
interaction with egui. The touch-pan test is in
crates/app/src/viewer/markdown/selection_tests.rs.
