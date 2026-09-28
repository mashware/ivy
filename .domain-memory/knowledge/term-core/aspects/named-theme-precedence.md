---
id: asp_named-theme-precedence_tc34ed
slug: named-theme-precedence
name: >-
  A named theme is the base; explicit colors keep their priority in any line
  order
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T10:27:50.497Z'
updated_at: '2026-09-28T10:27:50.497Z'
last_verified: '2026-09-28T10:27:50.497Z'
file_paths:
  - crates/app/src/settings.rs
  - crates/app/src/theme.rs
  - docs/02-decisiones.md
symbols:
  - 'Settings::resolved_theme'
  - 'NamedTheme::theme'
  - 'Theme::with_colors'
content_hashes:
  crates/app/src/settings.rs: 'sha256:0f5d3514c630adc0cc813d9b0a408717d52b6a6770e45cce09f24d5d3276bb49'
  crates/app/src/theme.rs: 'sha256:6db6707e13762f2ac1656a5bef79c24434e39aa8f8a1e1ff5d54e6ca15d958af'
  docs/02-decisiones.md: 'sha256:d081c660a24d5d7a7ac70e572491483b7c104f4b18af4645ff4d3fe3f5378526'
tags:
  - configuration
  - themes
  - D59
feature_id: feat_term-core_nx0g1r
---

## What it does

D59 makes a named theme the base for all twenty-two colors. Each explicitly configured color overrides only its own slot, regardless of whether that line appears before or after `theme`; this preserves a person's hand-tuned colors when they move or change the theme line. With no theme selected, the existing factory palette remains the base. An unknown theme name warns without erasing the last valid selection.

A theme line does not reset colors written above it: `theme` and each color are distinct keys, so merely reordering the file must not change its appearance.

## Where it lives

`Settings::resolved_theme` composes `NamedTheme::theme` with `Theme::with_colors` in `crates/app/src/settings.rs` and `crates/app/src/theme.rs`; D59 in `docs/02-decisiones.md` records the reason and discarded alternatives.
