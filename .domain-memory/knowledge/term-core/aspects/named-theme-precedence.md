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
updated_at: '2026-11-15T19:14:38.264Z'
last_verified: '2026-11-15T19:14:38.264Z'
file_paths:
  - crates/app/src/theme.rs
  - crates/app/src/settings.rs
symbols: []
content_hashes:
  crates/app/src/theme.rs: 'sha256:ac839d79a999f104e16c41147c83070eebb8db531a49325d51ae00dfe94d51d8'
  crates/app/src/settings.rs: 'sha256:850f221a17b6a2731950bde335fe43b0d8b68b0739a533031434bda0ea0a08bb'
tags:
  - theme
  - D59
  - D94
feature_id: feat_term-core_nx0g1r
---

## What it does

D59 makes a named theme the base for all twenty-two colors. Each explicitly configured color
overrides only its own slot, regardless of whether that line appears before or after `theme`; this
preserves a person's hand-tuned colors when they move or change the theme line. With no theme
selected, the base is Ivy Night since D94 (#93, November 2026) — no longer the old gray factory
palette; Settings names that choice «Ivy Night (default)». An unknown theme name warns without
erasing the last valid selection. Since D94 every theme's focus is a green (converted palettes:
normal green, else bright, else moved until 3:1), so the interface accent never equals the `done`
claim's ANSI blue; an explicit `focus` is used verbatim.

## Where it lives

crates/app/src/theme.rs (Default for Theme, NamedTheme, green_focus); crates/app/src/settings.rs (theme parsing)
