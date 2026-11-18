---
id: asp_egui-phosphor-add-to-fonts-is-the-f29-trap_6m7pvr
slug: egui-phosphor-add-to-fonts-is-the-f29-trap
name: >-
  egui-phosphor's own installer puts the icon face in the proportional chain:
  register Phosphor as a family of its own, holding nothing else
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T19:35:25.240Z'
updated_at: '2026-11-18T19:35:25.240Z'
last_verified: '2026-11-18T19:35:25.240Z'
file_paths:
  - crates/app/src/fonts/bundled.rs
  - crates/app/src/ui/icons.rs
symbols:
  - base_definitions
  - ui_icon_family
content_hashes: {}
tags:
  - fonts
  - icons
  - phosphor
  - D103
feature_id: feat_term-core_nx0g1r
---

## What it does

egui-phosphor 0.14's `add_to_fonts` (and `add_font_bytes_to_fonts`) inserts the Phosphor face into
`FontFamily::Proportional` at index 1, and `add_font_bytes_as_family` builds a named family that is
the proportional faces with Phosphor second. Both are the F2.9 trap in a new coat: Phosphor maps the
lowercase letters to blank glyphs (the building blocks of its ligatures), and its codepoints sit in
the Unicode private-use area that the terminal's Nerd Font family (`ICON_FAMILY`) serves. A face in
a shared chain that holds letters or PUA codepoints takes them from whatever stands behind it.

What #114 did instead (D103): the face is registered in `fonts::bundled::base_definitions` — the
base every rebuild starts from — as `FontFamily::Name("ivy-icons")` holding exactly `["Phosphor"]`,
no text face behind it, and only the crate's bytes (`variants::bytes::regular::FONT`) and constants
are used. `the_icon_face_lives_in_its_own_family_and_nowhere_else` walks every family and fails if
any other lists the face; mutation "Phosphor in Proportional" turns it red.

Evidence: read `egui-phosphor-0.14.0/src/lib.rs` docblocks (`add_font_bytes_to_fonts`,
`add_font_bytes_as_family`) on 2026-11-18; mutation run on branch 114.

## Where it lives

crates/app/src/fonts/bundled.rs (base_definitions, UI_ICONS, ui_icon_family); egui-phosphor 0.14.0
src/lib.rs
