---
id: asp_egui-singleline-textedit-drops-focus-on-enter_v75mnf
slug: egui-singleline-textedit-drops-focus-on-enter
name: >-
  egui's single-line TextEdit gives up focus on Enter: a list that owns Enter
  must take it back
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T21:02:08.521Z'
updated_at: '2026-11-17T21:02:08.521Z'
last_verified: '2026-11-17T21:02:08.521Z'
file_paths:
  - crates/app/src/explorer/search.rs
symbols:
  - 'egui::TextEdit::singleline'
content_hashes: {}
tags:
  - egui
  - focus
  - keyboard
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36's `TextEdit::singleline` surrenders keyboard focus when Enter is pressed. A floating
list that uses Enter for its own action (pick a match) loses its field whenever Enter picks
nothing, for example with no matches or while still searching. The keys typed next then reach
nobody.

The fix in Files' search: when `field.lost_focus()` and Enter was pressed this frame, call
`request_focus()` again. A pick closes the list, so the request goes nowhere then.

Evidence: the Enter-with-no-match step of
`typing_a_search_keeps_the_tree_loaded_and_the_keys_in_files` fails without it (#93 PR 6).

## Where it lives

crates/app/src/explorer/search.rs (field)
