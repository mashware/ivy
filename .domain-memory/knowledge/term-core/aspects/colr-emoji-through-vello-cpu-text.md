---
id: asp_colr-emoji-through-vello-cpu-text_14f9vx
slug: colr-emoji-through-vello-cpu-text
name: >-
  COLR emoji are painted by vello_cpu's glifo, which epaint builds without: the
  lockfile lists it, the build does not
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T21:05:25.725Z'
updated_at: '2026-11-21T21:05:25.725Z'
last_verified: '2026-11-21T21:05:25.725Z'
file_paths:
  - crates/app/src/fonts/atlas.rs
  - crates/app/Cargo.toml
symbols:
  - colr_of
  - rasterize
content_hashes:
  crates/app/src/fonts/atlas.rs: 'sha256:2945ce9ffb87df93d85027bdead1621285168c9986dd573ffd2c71c8466c86ae'
  crates/app/Cargo.toml: 'sha256:3e8a7b0870379c0db6768e7362286f01005ea14c57e546eecfac445c8f63f25f'
tags:
  - emoji
  - colr
  - windows
  - vello_cpu
feature_id: feat_term-core_nx0g1r
---

## What it does

epaint 0.36.1 rasterizes its letters with vello_cpu 0.1.0, but with
`default-features = false` (std, u8_pipeline, f32_pipeline). The `text` feature, which pulls glifo
0.2.0 and its full COLR v0/v1 painter (gradients including sweep, composites, clip boxes), is OFF.
glifo is in `Cargo.lock` anyway, so the lockfile is no evidence it is compiled:
`cargo tree -e features -i glifo --target all` printed nothing before #176.

#176 turns `text` on for app's own `vello_cpu` dependency (Cargo then builds one `vello_cpu` with
the feature for epaint too, idle for its letters). `RenderContext::glyph_run(resources,
&peniko::FontData).fill_glyphs(..)` resolves each glyph COLR > bitmap > OUTLINE, so a caller that
wants colour-or-nothing must gate on skrifa `color_glyphs().get(glyph)`, or a monochrome symbol
face is drawn as a silhouette and wins (the D24 defect). The text-colour layer (palette 0xFFFF) is
the current solid paint, falling back to black, so set it explicitly.

`fontique::Blob` and `peniko::Blob` are the same type (a single `linebender_resource_handle`
0.1.1), so `FontData` from the cached blob is a refcount clone, not a copy of the font.

Discarded: skrifa `ColorPainter` onto tiny-skia, because tiny-skia 0.11 has no sweep gradient and
COLR v1 needs one. fontique already names `Segoe UI Emoji` as Windows' emoji family
(`backend/dwrite.rs:38`); only rasterizing was missing.

## Where it lives

crates/app/src/fonts/atlas.rs (colr_of, rasterize), crates/app/Cargo.toml (vello_cpu), D25 in docs/02-decisions.md
