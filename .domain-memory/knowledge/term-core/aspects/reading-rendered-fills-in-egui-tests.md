---
id: asp_reading-rendered-fills-in-egui-tests_dey3d2
slug: reading-rendered-fills-in-egui-tests
name: >-
  Reading rendered fills in a headless egui test: zero the fade in BOTH styles,
  and skip the transparent rect a frameless button paints
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T15:32:58.101Z'
updated_at: '2026-11-17T15:32:58.101Z'
last_verified: '2026-11-17T15:32:58.101Z'
file_paths:
  - crates/app/src/chrome/updates.rs
  - crates/app/src/settings/editor/view/tests/usability.rs
symbols: []
content_hashes:
  crates/app/src/chrome/updates.rs: 'sha256:b8e178c4338e9dfc16466d5e52014c347eeb6e0f8b515d64bb144252d036ff34'
  crates/app/src/settings/editor/view/tests/usability.rs: 'sha256:c73884a5a7138b17cf0503eac7e42754ba3bbc3ef83b0e898622054852928340'
tags:
  - egui
  - tests
  - painting
feature_id: feat_term-core_nx0g1r
---

## What it does

A headless egui test that reads the fill colours a surface painted has two traps. First, a Modal or
Area fades in over time and egui multiplies the alpha, so the fill is never the literal colour on
the first frames; `ctx.global_style_mut(|s| s.animation_time = 0.0)` is not enough when the test
also pins `ctx.set_theme(Light|Dark)`, because `global_style_mut` writes only the slot in use and
the pinned theme reads the other one. Write it with `ctx.all_styles_mut`. Second, a frameless
`egui::Button::new("")` used as a hit area over a row paints a TRANSPARENT rect the size of the row,
so a 'smallest rect under this label' lookup finds the button instead of the surface behind it;
filter out `Color32::TRANSPARENT` fills. Found in #93 PR #5, where both made correct code look
wrong.

## Where it lives

Evidence: crates/app/src/chrome/updates.rs
`the_updates_dialog_floats_on_raised_at_settings_measures` (all_styles_mut) and
crates/app/src/settings/editor/view/tests/usability.rs
`settings_paints_the_previewed_roles_whatever_the_desktop_prefers` (transparent filter); both failed
on exactly these two until fixed.