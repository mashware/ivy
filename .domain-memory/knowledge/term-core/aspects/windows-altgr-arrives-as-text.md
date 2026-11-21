---
id: asp_windows-altgr-arrives-as-text_43tw3w
slug: windows-altgr-arrives-as-text
name: >-
  On Windows AltGr already arrives as text; Ctrl+Alt used in its place does not,
  and «text wins» would type the plain letter
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T21:06:14.095Z'
updated_at: '2026-11-21T21:06:14.095Z'
last_verified: '2026-11-21T21:06:14.095Z'
file_paths:
  - vendor/egui-winit/src/lib.rs
symbols:
  - get_agnostic_mods
content_hashes:
  vendor/egui-winit/src/lib.rs: 'sha256:ad3e91014f9a6f9bbb679bc7eabacbb79ac696a2bfa4c874cc01e240f3c5d1e7'
tags:
  - keyboard
  - windows
  - AltGr
  - D115
feature_id: feat_term-core_nx0g1r
---

## What it does

**winit 0.30.13 removes Ctrl and Alt from the modifiers while the right Alt is held on a layout
that has AltGr** (`platform_impl/windows/keyboard_layout.rs`, `get_agnostic_mods`,
`filter_out_altgr`), and fills the key's text with the AltGr character. egui-winit drops text only
while Ctrl/command is held, so `@` and `{` reach the console as `Event::Text`: **AltGr needs no
code.** The fake left Ctrl Windows sends with AltGr can produce a passing modifier change, which
the right Alt's own message clears before the character key.

**Left Ctrl + left Alt used in place of AltGr keeps both modifiers**, so egui-winit drops its
text and nothing is typed. The obvious rule, «on Windows, Ctrl+Alt with a non-empty text means
the text wins», is wrong: for a layout without AltGr winit fills the text with the key minus Ctrl
(`keyboard.rs`, `ctrl_on` branch), so `Ctrl+Alt+A` on a US layout would type `a`. A fix needs to
know whether the layout has AltGr. Left out of #175 and recorded as F1.

**Read in source, not run on a desktop**: D53's Windows rows (es-ES AltGr+2 / AltGr+´, de-DE
AltGr+Q / AltGr+7) are what settles it.

## Where it lives

winit-0.30.13 src/platform_impl/windows/keyboard_layout.rs (get_agnostic_mods) and
keyboard.rs (text with ctrl_on); vendor/egui-winit/src/lib.rs (the text gate in
on_keyboard_input). Evidence: read for #175 (D115), 21 Nov 2026.
