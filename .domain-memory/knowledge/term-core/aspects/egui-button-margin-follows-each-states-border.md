---
id: asp_egui-button-margin-follows-each-states-border_keqt8e
slug: egui-button-margin-follows-each-states-border
name: >-
  egui sizes a button's frame per state: a resting border that is never painted
  still costs margin, and the row jumps on hover
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T15:32:58.222Z'
updated_at: '2026-11-18T22:50:00.000Z'
last_verified: '2026-11-17T15:32:58.222Z'
file_paths:
  - crates/app/src/pane_view.rs
  - crates/app/src/tests/context_clipboard.rs
  - crates/app/src/settings/editor/view/dialog.rs
  - crates/app/src/ui/icons.rs
symbols:
  - menu_item
  - action_width
  - close_button
content_hashes:
  crates/app/src/pane_view.rs: 'sha256:9649ff9332590ba19a17242003e2486a2677db6ef3993e4ca65a1b2f6c7eddcc'
  crates/app/src/tests/context_clipboard.rs: 'sha256:0cfabd2869e2ad4c1e746a1c055a08755f765309cc90a308806d4ff7eabd184d'
tags:
  - egui
  - layout
  - menus
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36's `Style::button_style` sets the frame's inner margin to `button_padding + expansion -
bg_stroke.width` of the button's CURRENT state. Since D94 the kit gives `widgets.inactive` a 1-point
border, so a `Button::frame_when_inactive(false)` row (the console context menu's items) loses one
point of margin per side at rest even though that border is never drawn, and gets it back when the
hovered state's border differs: the hovered row grows two points and pushes every row below it. It
shows only with Geist at a fractional desktop scale; with egui's default font the row sits at its
`min_size` and hides it, which is why a plain headless test passed. Fix: give every state of such a
row the same border width (Stroke::NONE in inactive, hovered and active). Reported by the owner from
a real desktop in #93 PR #5.

Two more from #119. A button built with `.frame(false)` is laid out with NO margin at all (egui
button.rs, `Margin::ZERO` without a frame): a width prediction that adds `2 * button_padding.x` to
every label wrapped the rename footer about 70 points too early, so only framed buttons get the
padding. And a glyph given to `ui.button` as its text sits on the text row's baseline, visibly above
the centre of its box, which the owner saw on a real desktop. The close button is now an empty
`Button` square of `interact_size.y` with the icon painted on `response.rect.center()` through
`icons::paint`.

## Where it lives

crates/app/src/pane_view.rs `menu_item`; egui-0.36.1 src/widget_style.rs `button_style`. Evidence:
crates/app/src/tests/context_clipboard.rs `hovering_a_menu_row_does_not_push_the_rows_below`, which
needs `crate::fonts::install_bundled` and `set_zoom_factor(1.25)` to reproduce, red with the resting
border kept. crates/app/src/tests/pane_rename.rs
`a_narrow_window_keeps_the_footer_actions_apart_and_inside_the_dialog` (one row at 320 points with
the bundled fonts; killed the `frameless_padded` mutation) and
`the_close_cross_is_centred_in_its_box_and_on_the_titles_line` (killed `cross_high`).