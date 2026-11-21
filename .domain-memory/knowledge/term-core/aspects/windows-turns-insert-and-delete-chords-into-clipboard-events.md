---
id: asp_windows-turns-insert-and-delete-chords-into-clipboard-events_6yfywh
slug: windows-turns-insert-and-delete-chords-into-clipboard-events
name: >-
  On Windows the Insert and Delete clipboard chords are keys with a request
  behind them, and a vendored key change reaches every widget
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T20:37:54.785Z'
updated_at: '2026-11-21T21:06:04.284Z'
last_verified: '2026-11-21T21:06:04.284Z'
file_paths:
  - vendor/egui-winit/src/lib.rs
  - crates/app/src/keyboard.rs
  - crates/app/src/console_input.rs
symbols:
  - is_copy_command
  - is_cut_command
  - is_paste_command
  - add_requests_behind
  - request_behind
  - clipboard_chord
content_hashes:
  vendor/egui-winit/src/lib.rs: 'sha256:ad3e91014f9a6f9bbb679bc7eabacbb79ac696a2bfa4c874cc01e240f3c5d1e7'
  crates/app/src/keyboard.rs: 'sha256:1b19ab5d0af9af3f546c6a3cde4466b7d169083284dcf7091e89e7ca4b55a2ca'
  crates/app/src/console_input.rs: 'sha256:2aef42749f1111dea891170f9ffdb541e953a28b3a8a8e68d24acbcaab3877cd'
tags:
  - keyboard
  - windows
  - clipboard
  - D115
feature_id: feat_term-core_nx0g1r
---

## What it does

**Until D115 (#175) upstream's Windows arms turned the chords into clipboard requests before
they were keys**, each reading one modifier and ignoring the rest: any Ctrl+Insert was a copy
(`0x03` in a console), any Shift+Delete a cut (`0x18`, `nano` exits), any Shift+Insert a paste.
The requests carry no modifiers, so a console cannot tell Ctrl+Insert from Ctrl+C.

**Now the vendored copy drops the Ctrl+Insert and Shift+Delete arms and narrows Shift+Insert to
Shift alone.** That alone broke something nobody was looking at: **a change to the vendored
predicates reaches the whole window, not only the consoles.** Text fields (egui `TextEdit`) and
the drawer's code viewer copy and cut only on `Event::Copy` / `Event::Cut`, so they lost both
chords. Found by review, not by use.

**The fix that keeps both:** on Windows, the window's input hook puts the request right behind
an exact Ctrl+Insert or Shift+Delete key (`keyboard::add_requests_behind`). egui's `TextEdit`
ignores Shift+Delete on Windows (egui 0.36.1 `builder.rs:1394`, `Key::Delete if !modifiers.shift
|| os != Windows`) and has nothing for Insert, so it acts on the request alone. The console acts
on the key and drops the request behind it. The drop is armed from the key itself **before** the
keymap, or a bound `ctrl+insert` leaves its request to reach the program as `0x03`. Every reader
of these chords must use the key's own modifiers: the walk's rebuilt chord goes blank on a
focus loss, and then the request is dropped but the copy never happens.

**What fields still lose:** every non-exact variant (Ctrl+Shift+Insert, Ctrl+Alt+Insert,
Ctrl+Shift+Delete, Alt+Shift+Delete, Alt+Shift+Insert). Upstream's copy arm ran before the paste
arm, so Ctrl+Shift+Insert used to copy, never paste.

**The desktop stretch is still untestable (D53);** the pair is this project's translation and
is tested, including through `raw_input_hook`.

## Where it lives

vendor/egui-winit/src/lib.rs (the three predicates, pinned whole by crates/app/tests/vendor_patch.rs);
crates/app/src/keyboard.rs (request_behind, add_requests_behind, Behind, clipboard_chord);
crates/app/src/console_input.rs (before_egui, called from the window's raw_input_hook).
Evidence: #175 review rounds 1-3, D115; tests on_windows_* and
the_window_puts_the_requests_behind_the_keys_only_where_it_was_built_for_windows.
