---
id: asp_a-colour-batch-proves-its-path_2gpdaz
slug: a-colour-batch-proves-its-path
name: >-
  A colour batch must prove which rasterizing path it walked, not only that it
  rasterized
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T21:05:26.016Z'
updated_at: '2026-11-21T21:05:26.016Z'
last_verified: '2026-11-21T21:05:26.016Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/measure/header.rs
  - crates/app/src/measure.rs
symbols: []
content_hashes:
  scripts/measure-frame.sh: 'sha256:2183c098370f198ef0f53ce5281528eb4f1447226ea174656877aca15d4daa6f'
  crates/app/src/measure/header.rs: 'sha256:1a8e44fefe4592fab9d61310b474a095bd0b1f8782e1d8997cb07514af7c9891'
  crates/app/src/measure.rs: 'sha256:4d99a5092370d68364877c7f68f65e4d85c8b4169774ad59717177f9911a8755'
tags:
  - measurement
  - emoji
  - colr
  - D13
feature_id: feat_term-core_nx0g1r
---

## What it does

With two colour-glyph paths (COLR outlines beside bitmaps), the script's colour guard
(«rasterized > 0») stopped proving a batch measured what its name says. Batch Q on a machine whose
only emoji font is COLR (Fedora's Noto-COLRv1) would publish the outlines' cost as the bitmap's.
Since #176 the report writes «N glyphs rasterized, M of them from colour outlines», and the script
reads it both ways: batch `colr` is invalid unless M == N, every other batch unless M == 0.

A fontconfig override for batch `colr` is proved with `fc-list :color=true` under the batch's
`FONTCONFIG_FILE` and `XDG_CONFIG_HOME`, before the window opens. `fc-match emoji` only proves the
best match, while the app walks every face of every emoji family; and `/etc/fonts` includes the
user's config through `XDG_CONFIG_HOME`, which the app runs with set to the batch's directory.

A new report line the script greps must be registered as a keyword in `measure.rs`
`script_patterns`, or `every_pattern_the_script_looks_for_in_the_report_still_extracts_something`
fails: it is the guard that ties each script pattern to a line the report writes.

## Where it lives

scripts/measure-frame.sh (batch colr, outlines guard), crates/app/src/measure/header.rs (Rasterizing), crates/app/src/measure.rs (script_patterns)
