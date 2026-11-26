---
id: asp_colour-tests-that-skip-start-running_mr1smh
slug: colour-tests-that-skip-start-running
name: >-
  Colour emoji tests skip themselves where no colour font paints, so giving a
  platform colour turns them all on at once
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T21:05:25.933Z'
updated_at: '2026-11-21T21:05:25.933Z'
last_verified: '2026-11-21T21:05:25.933Z'
file_paths:
  - crates/app/src/fonts.rs
  - crates/app/src/panel/color.rs
  - scripts/windows-known-failures.json
symbols: []
content_hashes:
  crates/app/src/fonts.rs: 'sha256:1d74c384a8ccdff6d2116aa066f6f24e8204d7fefce313eab7a40b15f0a051c8'
  crates/app/src/panel/color.rs: 'sha256:291e805ed76985547887074d021cafb711a3493cbdbb69a354ceeb88d7650e58'
tags:
  - emoji
  - windows
  - tests
feature_id: feat_term-core_nx0g1r
---

## What it does

About nine colour-emoji tests return early when the machine has no font that paints emoji in
colour (`has_color` guards in `fonts.rs`, early `None` returns, `there_is_color_on_this_machine`
in `panel/color.rs`). On Windows they skipped silently until #176, because Segoe UI Emoji is COLR
and only bitmaps were rasterized, and none of them is in `scripts/windows-known-failures.json`.

A change that gives a platform colour turns them on there in the same commit, and the Windows job
fails on any listed test that starts passing while still listed. So such a change edits the
known-failures list in the same PR, and its Windows CI run must be read for the newly running tests
too. The keycap test expected the face that draws a keycap to also draw a bare digit in colour;
Segoe keeps its digits as plain outlines, so #176 made it accept a face with no colour digit.

Found by the design challenger of #176 reading the guards.

## Where it lives

crates/app/src/fonts.rs (colour tests), crates/app/src/panel/color.rs, scripts/windows-known-failures.json
