---
id: asp_claude-title-spinner-geometry_q3rmdh
slug: claude-title-spinner-geometry
name: Claude's two title frames can select glyphs with different sizes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T14:25:35.568Z'
updated_at: '2026-11-26T14:25:35.568Z'
last_verified: '2026-11-26T14:25:35.568Z'
file_paths:
  - crates/app/src/header/title_text.rs
  - crates/app/src/fonts/bundled.rs
symbols: []
content_hashes:
  crates/app/src/header/title_text.rs: 'sha256:d485f3328c23cb6d9762a3bb560103f4c6f73df60d485fb0e9a06aef548d259d'
  crates/app/src/fonts/bundled.rs: 'sha256:32409f3c349bffd1fb90f4c0ed753430a5ffc99ed028188e947d9ca69a9cf172'
tags:
  - claude
  - rendering
  - fonts
  - title
feature_id: feat_term-core_nx0g1r
---

## What it does

Claude Code 2.1.332 alternates the two leading title characters U+25D0 and U+25D1. In Ivy's bundled proportional fallback chain at 14 points, those symbols selected Hack and egui's symbol face and painted 9 × 10 and 13 × 13 pixel rasters. The following text moved by four pixels. Restoring one missing glyph can leave this visual regression because coverage and matching geometry are separate properties.

Render the two leading frames with the same centered outline and advance, with opposite half fills. Leave ordinary circle glyphs in the font fallback paths. Compare both rendered frames and the location of the first following text at several title sizes. A test that checks only glyph coverage cannot detect this defect. See D95.

## Where it lives

The title painter is shared by integrated pane headers and tab labels. The regression checks outline diameter, center, opposite fills and the following text location.
