---
id: asp_egui-keeps-two-styles-and-follows-the-desktop_e4ad65
slug: egui-keeps-two-styles-and-follows-the-desktop
name: >-
  egui 0.36 keeps a dark and a light style and follows the desktop unless
  pinned: writing one style lets the stock one back on a light/dark switch
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-15T19:14:37.814Z'
updated_at: '2026-11-15T19:14:37.814Z'
last_verified: '2026-11-15T19:14:37.814Z'
file_paths:
  - crates/app/src/ui/style.rs
  - crates/app/src/project_view.rs
symbols:
  - install
  - apply
  - all_styles_mut
  - set_theme
content_hashes:
  crates/app/src/project_view.rs: 'sha256:0985ddb992699577352eab392bfa8cc705b30ae19bf793609f0721acf774d1fc'
tags:
  - egui
  - theme
  - style
  - D94
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36 holds two `Style`s, one per `egui::Theme`, and picks between them by `ThemePreference`, which defaults to following the system. Nothing in this app set the preference before D94. Writing the theme's look into «the» style (`global_style_mut`) changes only the slot in use; when the desktop flips to light, egui swaps in its untouched stock light style and the person's theme vanishes from every stock widget.

The shape that holds (D94, `ui::style::install`): write the roles into **both** slots with `ctx.all_styles_mut`, then pin `ctx.set_theme(ThemePreference::Dark|Light)` from the theme's own darkness. Install only when the effective `Theme` changes (compared in `ctx.data` temp memory), so the per-frame cost is one comparison.

Two traps around it: `Context::style()` does not exist in 0.36 (`global_style()` / `style_of(theme)` do); and surfaces that assign `Visuals::dark()` in their own scope (Settings, the viewer) are not reached by the global install until those resets go.

Evidence: egui-0.36.1 `src/context.rs` (`all_styles_mut` 2210, `style_of` 2218, `set_theme` 2167); test `ui::style::tests::both_styles_carry_the_themes_roles_whatever_the_desktop_prefers` asserts both slots and the pinned preference; mutation «write one style only» killed in review of #93.

## Where it lives

crates/app/src/ui/style.rs (install, apply); crates/app/src/project_view.rs (refresh calls it every frame, before the enabled early return)
