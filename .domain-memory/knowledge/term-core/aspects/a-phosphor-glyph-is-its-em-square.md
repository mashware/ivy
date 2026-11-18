---
id: asp_a-phosphor-glyph-is-its-em-square_kthbxx
slug: a-phosphor-glyph-is-its-em-square
name: >-
  A Phosphor glyph laid out at S points is an S-by-S galley: centre the galley
  and the icon is centred the way its designer drew it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T19:35:25.876Z'
updated_at: '2026-11-18T19:35:25.876Z'
last_verified: '2026-11-18T19:35:25.876Z'
file_paths:
  - crates/app/src/ui/icons.rs
symbols:
  - paint
content_hashes: {}
tags:
  - icons
  - phosphor
  - epaint
  - D103
feature_id: feat_term-core_nx0g1r
---

## What it does

Measured on 2026-11-18 with a probe test over the bundled definitions: every Phosphor Regular glyph
laid out at 16 pt gives `galley.size() == [16, 16]` — advance one em, row height one em, ascent 15 —
with the ink inside (SQUARE inks 2..14, X 3..13, FOLDER and FOLDER_OPEN both from y 2 with 11 rows).
So the galley *is* the em square Phosphor draws every glyph on, and `painter.galley(center -
galley.size() / 2.0, ...)` centres the designer's square: paired glyphs (folder closed and open,
caret right and down) keep the same origin and do not move a row when it expands. Centring each
glyph's ink instead moves pairs and pulls asymmetric glyphs off their optical centre.

Two limits measured in the same work. At a fractional scale the row height is rounded to device
pixels (13 pt at 1.25x lays out 13 x 12.8), so a test of the square allows one device pixel. And the
raster carries about a pixel of antialiasing past the outline; at 8 pt an arrow's tip reaches the
square's edge, so a 'ink inside the box' test needs one device pixel of slack too. epaint snaps the
galley origin itself (`round_text_to_pixels`); a second snap by hand only adds a rounding to keep in
sync.

Evidence: probe test output on branch 114; `every_icon_is_one_inked_glyph_centred_in_its_box` and
`paired_glyphs_share_their_square` in `ui/icons.rs`.

## Where it lives

crates/app/src/ui/icons.rs (paint and its tests)
