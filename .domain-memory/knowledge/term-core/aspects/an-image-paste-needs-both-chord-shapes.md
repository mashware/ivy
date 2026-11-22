---
id: asp_an-image-paste-needs-both-chord-shapes_17tgkj
slug: an-image-paste-needs-both-chord-shapes
name: >-
  A widget that pastes an image must take Ctrl+V as a Key AND as a Paste, and
  arboard decodes the PNG inside the read
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T21:05:08.080Z'
updated_at: '2026-11-22T21:05:08.080Z'
last_verified: '2026-11-22T21:05:08.080Z'
file_paths:
  - vendor/egui-winit/src/lib.rs
  - crates/app/src/viewer/image/paste.rs
  - crates/app/src/clipboard.rs
symbols:
  - chord
  - released
  - take_chord
  - IMAGE_DEADLINE
content_hashes:
  vendor/egui-winit/src/lib.rs: 'sha256:ad3e91014f9a6f9bbb679bc7eabacbb79ac696a2bfa4c874cc01e240f3c5d1e7'
  crates/app/src/clipboard.rs: 'sha256:ff9678f5ca607b46214f0c5832a660d291acb55c7d00c3f5636a79d93f509f99'
tags:
  - egui-winit
  - clipboard
  - arboard
  - paste
feature_id: feat_term-core_nx0g1r
---

## What it does

**The card on the discarded paste chord describes upstream egui-winit, not this repo.** The
vendored copy (`vendor/egui-winit/src/lib.rs`, the #203 patch) moved the `return` inside the
text branch. So with no text on the clipboard (only a screenshot image) Ctrl+V and Ctrl+Shift+V
arrive as a plain `Event::Key`; with text they still arrive as `Event::Paste(text)` and the key
is swallowed. A design for #199 was nearly built on the stale claim: an advisor proposed dropping
Ctrl+V from the image viewer because of it.

**Rule for any widget that wants the chord for images:** match both `Event::Key` (V with command,
the Paste key, Shift+Insert on Windows) and `Event::Paste`, and only when no `TextEdit` has the
keyboard, or a text paste into a note is stolen. egui-winit reports a held chord's repeats as new
`Event::Paste`s with no repeat flag, but lets the key's release through: end a hold on that
release, on `WindowFocused(false)`, or on a frame the widget did not look in.

**arboard 3.6.1 `get_image()` decodes the PNG inside the call** on X11 and Wayland
(`x11.rs:1024-1034`, `wayland.rs:185-196`), so a deadline on a clipboard image read has to cover
a full decode (10 s in #199), not just the transfer.

## Where it lives

vendor/egui-winit/src/lib.rs (paste chord branch); crates/app/src/viewer/image/paste.rs (`chord`, `released`, `take_chord`); crates/app/src/clipboard.rs (`IMAGE_DEADLINE`)
