---
id: asp_stands-apart-is-a-surface-threshold-not-ink-contrast_cgtf3b
slug: stands-apart-is-a-surface-threshold-not-ink-contrast
name: >-
  Roles::stands_apart separates two surfaces, it does not say ink reads: an icon
  or a tint needs a real contrast ratio, and translucent fills blended first
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T19:35:25.429Z'
updated_at: '2026-11-18T19:35:25.429Z'
last_verified: '2026-11-18T19:35:25.429Z'
file_paths:
  - crates/app/src/theme/roles.rs
  - crates/app/src/ui/icons/file_kind.rs
symbols:
  - stands_apart
  - apart
  - panel_or
  - row_grounds
  - legible_or
content_hashes: {}
tags:
  - theme
  - contrast
  - D94
  - D98
  - D103
feature_id: feat_term-core_nx0g1r
---

## What it does

`Roles::stands_apart` and `Roles::apart` use `INVISIBLE = 1.03`: the threshold below which two fills
read as one surface. It is not an ink check — a 1.2:1 yellow on a light panel passes it. Ink needs
`project_ink_contrast` against a real figure: text 4.5 (`TEXT_CONTRAST`, what `panel_or` uses),
non-text marks such as icons 3.0 (WCAG 1.4.11, what `green_focus` uses for the focus border and
`FileKind::color` for Files' type tints).

Second trap in the same place: `list_hover` and `list_sel` are translucent (`[16,16,16,18]` and
`[13,43,29,56]` in Ivy Night), so a contrast taken against them as stored measures nothing. Blend
them onto the surface they land on first (`surface.blend(fill)`, as `ui::icons::row_grounds` does).
And the surface is not always `panel`: Files paints `panel_or(tx, base)` (D98), so a check against
`panel` alone was wrong on the themes where Files falls back to `base`. The design of #114 first
used `stands_apart` for the tint check; the design challenger caught it, and round 1 of the review
caught the wrong ground.

Evidence: `theme/roles.rs` (INVISIBLE, panel_or, list_hover/list_sel) and `explorer/view.rs`
(surface) read on branch 114, base c52a7ed.

## Where it lives

crates/app/src/theme/roles.rs; crates/app/src/ui/icons/file_kind.rs (MARK_CONTRAST, legible_or,
row_grounds)
