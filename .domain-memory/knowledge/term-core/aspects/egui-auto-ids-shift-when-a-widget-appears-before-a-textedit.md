---
id: asp_egui-auto-ids-shift-when-a-widget-appears-before-a-textedit_n7b2dk
slug: egui-auto-ids-shift-when-a-widget-appears-before-a-textedit
name: >-
  egui auto ids shift when a widget appears before a TextEdit: the field loses
  focus after one character
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T16:26:03.939Z'
updated_at: '2026-11-21T16:26:03.939Z'
last_verified: '2026-11-21T16:26:03.939Z'
file_paths:
  - crates/app/src/viewer/image/tools.rs
symbols: []
content_hashes: {}
tags:
  - egui
  - focus
  - ids
feature_id: feat_term-core_nx0g1r
---

## What it does

In egui 0.36.1 a TextEdit with no explicit id takes `ui.next_auto_id()`
(text_edit/builder.rs:511), and every `new_child` or `scope` laid out
before it bumps the parent's auto-id counter. A widget that appears
conditionally BEFORE the fields shifts every field's id. In the image
annotation column the Send button showed once the general note was
non-empty, so the note lost focus after its first character, and again
when it was emptied.

Fix: give each TextEdit an explicit `.id(...)` keyed on something
stable (the document and the marker), or always allocate the
conditional widget's slot.

Evidence: #178 PR 2 review round 4; the test
`the_general_note_keeps_the_keyboard_as_the_send_button_comes_and_goes`
goes red without the general note's explicit id.

## Where it lives

crates/app/src/viewer/image/tools.rs (notes column)
