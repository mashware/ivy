---
id: asp_a-console-does-not-consume-its-keys-from-egui-input_h1xr1q
slug: a-console-does-not-consume-its-keys-from-egui-input
name: >-
  A console holding the keyboard does not remove its keys from egui's input: a
  drawer shortcut must gate on the drawer holding the keys
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T21:05:08.845Z'
updated_at: '2026-11-22T21:05:08.845Z'
last_verified: '2026-11-22T21:05:08.845Z'
file_paths:
  - crates/app/src/viewer.rs
  - crates/app/src/viewer/image/paste.rs
  - crates/app/src/viewer/image.rs
symbols:
  - State
  - take_chord
content_hashes:
  crates/app/src/viewer.rs: 'sha256:e8fb4457f43ffc8cbee1d00981a15f129f3f8b9a331275d26c43ca5f166d6e88'
tags:
  - egui
  - keyboard
  - drawer
feature_id: feat_term-core_nx0g1r
---

## What it does

**A console that holds the keyboard leaves its key events in egui's input**, so a widget painted
later in the same pass (the drawer's image viewer) still sees Ctrl+V as `Event::Key` and acts on
it. In #199 the viewer's paste chord fired while a console had the keys, until it was gated on the
drawer holding them (`viewer::State.keys` = `keyboard || sidebar.maximized`, computed after a
press hands the keys over).

**Rule:** any drawer shortcut gates on the drawer holding the keys, never on being painted.

The viewer's Ctrl+Z comment claims «a console that holds them has taken it by now»; the same
mechanism suggests it is false too. Unverified, recorded as #199 follow-up F2.

Evidence: `a_paste_into_a_console_is_not_taken_by_the_tab` requested an image read with the
console focused (`viewer::blur`) before the gate and passes after it; #199 review round 1.

## Where it lives

crates/app/src/viewer.rs (`State.keys`); crates/app/src/viewer/image/paste.rs (`take_chord`)
