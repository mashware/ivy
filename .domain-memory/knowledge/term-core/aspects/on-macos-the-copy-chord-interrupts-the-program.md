---
id: asp_on-macos-the-copy-chord-interrupts-the-program_mpcg03
slug: on-macos-the-copy-chord-interrupts-the-program
name: >-
  On macOS the terminal's copy chord `Ctrl+Shift+C` will reach the program as
  `0x03` — an interrupt instead of a copy
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T12:47:36.212Z'
updated_at: '2026-09-25T12:47:36.212Z'
last_verified: '2026-09-25T12:47:36.212Z'
file_paths:
  - vendor/egui-winit/src/lib.rs
  - crates/app/src/keyboard.rs
  - docs/02-decisions.md
symbols:
  - is_copy_command
  - key_bytes
content_hashes:
  vendor/egui-winit/src/lib.rs: 'sha256:17e760455520be79b41f2cbc2524f862c6403768eb785f262d77c9c6cf232f17'
  crates/app/src/keyboard.rs: 'sha256:7e963bf8423cdd2068c9fa24d2bd5506c3191d8e7bae74949bac8037eea89286'
  docs/02-decisions.md: 'sha256:d2dcb9abf55336e4eec456ec7bdf9faf9da3aadc5306c5f699b6fc7bbdf5fa83'
tags:
  - macos
  - f2.7
  - keyboard
  - egui-winit
feature_id: feat_term-core_nx0g1r
---

## What it does

**The same mechanism that moves the paste chords on macOS (D11) moves the copy chord, and nobody had written it down.** Found in the review of #206, by reading, not by running on a Mac.

- The vendored `egui-winit`'s `is_copy_command` is `keycode == Copy || (modifiers.command && keycode == C) || (windows && ctrl && Insert)`. `command` is ⌘ on macOS and Ctrl elsewhere, so on macOS `Ctrl+Shift+C` is **not** turned into `Event::Copy`: it arrives as an ordinary `Event::Key { key: C }` with `ctrl` and `shift` set.
- The copy door in `keyboard.rs` reacts only to `Event::Copy`, so it never sees that key.
- The only arm in `keyboard::bytes` that silences a Ctrl+Shift chord is for `Key::V` (the D33 paste asymmetry) — there is none for `C`.
- `key_bytes` lets Shift through with Ctrl **on purpose** (Shift does not change a control code, which is what `xterm` does), so the C maps to `0x03`.

Result: on macOS the terminal's copy shortcut sends SIGINT to whatever runs in the pane — an agent mid-task, a build — instead of copying. **Nothing on Linux can show it**, and the desktop→framework translation is out of reach of any test anyway (see the card on that). It is written in D53's list as a row F2.7 must rewrite for macOS, not read as a failure.

**The general lesson**: every chord this terminal binds through `command` (copy, paste) changes owner on macOS, and a silence arm written for one letter does not cover its sibling. Whoever does F2.7 on macOS should check every application chord, not only the two D11 names.

## Where it lives

`vendor/egui-winit/src/lib.rs` (`is_copy_command`), `crates/app/src/keyboard.rs` (the `Key::V` silence arm in `bytes`, `key_bytes`, the `Event::Copy` door), `docs/02-decisions.md` D53 (macOS bullet).
