---
id: asp_windows-turns-insert-and-delete-chords-into-clipboard-events_6yfywh
slug: windows-turns-insert-and-delete-chords-into-clipboard-events
name: >-
  On Windows only, chords over Insert and Delete become copy, cut and paste
  before they are keys — and reach the program as 0x03 and 0x18
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T20:37:54.785Z'
updated_at: '2026-09-22T20:37:54.785Z'
last_verified: '2026-09-22T20:37:54.785Z'
file_paths:
  - vendor/egui-winit/src/lib.rs
  - crates/app/src/keyboard.rs
symbols:
  - is_copy_command
  - is_cut_command
  - is_paste_command
  - bytes
content_hashes:
  vendor/egui-winit/src/lib.rs: 'sha256:17e760455520be79b41f2cbc2524f862c6403768eb785f262d77c9c6cf232f17'
  crates/app/src/keyboard.rs: 'sha256:1299f986e390ceb4006f58cd67d224042e8b13933ba600534b586add3822f91a'
tags:
  - keyboard
  - windows
  - F2.7
  - clipboard
feature_id: feat_term-core_nx0g1r
---

## What it does

**The vendored `egui-winit` has three `cfg!(target_os = "windows")` branches that turn navigation chords into clipboard events and never emit the key**, and each one looks at a single modifier and ignores the rest:
- `is_copy_command`: **any** chord with Ctrl over Insert → `Event::Copy`.
- `is_cut_command`: **any** chord with Shift over Delete → `Event::Cut`.
- `is_paste_command`: **any** chord with Shift over Insert → `Event::Paste` — except over a clipboard with no text, where D33's patch lets it fall through as a plain key.

So on Windows `Ctrl+Shift+Delete`, `Ctrl+Alt+Insert` or `Ctrl+Shift+Insert` are caught too (the last one copies the selection, because `send_keys` sees Shift). The events carry no modifiers, and `keyboard::bytes` writes a bare copy as `0x03` (interrupt) and a cut as `0x18` — `nano` exits on Shift+Delete. On Linux the same chords reach `key_bytes` and go out as xterm's parameter form (`CSI 2;5~`, `CSI 3;2~`, D52).

**Nothing in the suite can see it**: it runs on Linux, and the winit→egui translation is out of reach of any test anyway (see the card on that). Whoever does F2.7 on Windows, or touches the clipboard path, has to decide these chords explicitly; a keyboard table checked on Linux says nothing about them. It predates D52 — it held while these keys had no bytes of their own.

## Where it lives

vendor/egui-winit/src/lib.rs (is_cut_command / is_copy_command / is_paste_command and the branch in on_keyboard_input that uses them); crates/app/src/keyboard.rs (bytes: Copy → 3, Cut → 24). Evidence: read on origin/main eaaf701, 22 Sep 2026, lines 1020-1061 and 1435-1451 at that commit (#274 later moved them and patched the Ctrl+X arm to exclude Shift — the Windows arm this card describes was left as upstream on purpose, D33); found by the two review rounds of #258.
