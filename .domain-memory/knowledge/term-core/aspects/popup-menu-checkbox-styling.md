---
id: asp_popup-menu-checkbox-styling_sxk6cn
slug: popup-menu-checkbox-styling
name: Popup menu styling can hide an unchecked checkbox
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T21:13:21.502Z'
updated_at: '2026-11-25T21:13:21.502Z'
last_verified: '2026-11-25T21:13:21.502Z'
file_paths:
  - crates/app/src/viewer/code_header.rs
  - crates/app/src/ui/floating.rs
  - crates/app/src/ui/style.rs
symbols:
  - menu
  - menu_frame
content_hashes:
  crates/app/src/viewer/code_header.rs: 'sha256:825e2eb475c7408cf49d23d4e83918e643dbf94c9152b27d34f528dfeb20ea4e'
  crates/app/src/ui/floating.rs: 'sha256:a3e58700137e7d4a135a0361aed57af9885057492f00e0d234ce6a8b5e8bc81d'
  crates/app/src/ui/style.rs: 'sha256:5d6f44f1c29ed1f565b9aeef4433419e3076f029ddaea38a253ea78bc07faef1'
tags:
  - tests
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36 Popup::menu applies menu_style after a custom frame: it overwrites button padding to (2,0) and clears widget strokes. In Ivy a standard unchecked Checkbox then has the same roles.raised fill as the popup and no border, so it disappears at rest while the checked tick remains. Verify actual popup output with pointer away from the checkbox; testing only toggle behavior misses this.

## Where it lives

crates/app/src/viewer/code_header.rs, crates/app/src/ui/floating.rs, crates/app/src/ui/style.rs; symbols: menu, menu_frame. Evidence: issue 245 regression tests and targeted mutation checks (November 2026).
