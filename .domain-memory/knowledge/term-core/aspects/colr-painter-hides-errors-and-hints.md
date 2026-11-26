---
id: asp_colr-painter-hides-errors-and-hints_3mmb41
slug: colr-painter-hides-errors-and-hints
name: >-
  vello_cpu's glyph painter hides COLR paint errors behind a blank pixmap, and
  hints by default: two defaults a COLR caller must override
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T21:05:52.344Z'
updated_at: '2026-11-21T21:05:52.344Z'
last_verified: '2026-11-21T21:05:52.344Z'
file_paths:
  - crates/app/src/fonts/atlas.rs
symbols:
  - colr_of
  - WalkOnly
content_hashes:
  crates/app/src/fonts/atlas.rs: 'sha256:2945ce9ffb87df93d85027bdead1621285168c9986dd573ffd2c71c8466c86ae'
tags:
  - emoji
  - colr
  - vello_cpu
  - measurement
feature_id: feat_term-core_nx0g1r
---

## What it does

(1) The glyph painter vello_cpu 0.1 uses for its `text` feature (the `glifo` 0.2.0 crate)
discards the COLR paint graph's error (`let _ = color_glyph.paint(..)`, its `src/colr.rs:100`)
and sizes its intermediate image from the clip box or a computed box that falls back to an empty
rect. An unwalkable graph (the colr_1.ttf fixture's cyclic glyph 179) comes back as a blank pixmap
with no error; cached in the atlas as a success it would hide that emoji for the session. Ivy
walks the graph first with a no-op skrifa `ColorPainter` (`WalkOnly`) and also refuses an
all-transparent pixmap. On the fixture each check masks the other: mutating either alone keeps the
test green, removing both turns it red. The walk alone covers a half-drawn graph, which no fixture
here reproduces.

(2) `GlyphRunBuilder` hints by default. With a fresh `Resources` every run builds a whole-font
hinting instance (autohinter style classification, or TrueType fpgm/prep) that a COLR glyph never
uses. Call `.hint(false)`, or that set-up is timed as drawing on every atlas miss. Drawing at the
final size without resampling also relies on `glyph_run`'s default `atlas_cache = false`.

Found by the #176 review (built-in code-review, type-design and silent-failure reviewers).

## Where it lives

crates/app/src/fonts/atlas.rs (colr_of, WalkOnly, test a_colour_graph_that_cannot_be_walked_is_not_drawn)
