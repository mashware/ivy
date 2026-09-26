---
id: asp_keycap-emoji-drawn-as-a-tiny-digit_cvschf
slug: keycap-emoji-drawn-as-a-tiny-digit
name: >-
  A keycap is a ligature of its whole sequence in the emoji font: asked for the
  digit alone it came out as a thin, faint digit
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T16:08:28.594Z'
updated_at: '2026-09-26T18:09:57.926Z'
last_verified: '2026-09-26T18:09:57.926Z'
file_paths:
  - crates/app/src/fonts/atlas.rs
  - crates/app/src/fonts.rs
symbols:
  - glyph_of
  - ColorEmoji
content_hashes:
  crates/app/src/fonts/atlas.rs: 'sha256:e2ec8192eb557889c972b3337b71d11b174dd2826335579fec6dfad8aa18fd76'
  crates/app/src/fonts.rs: 'sha256:ac2b422a7f09b02fae4bb27b8226e42846ad22b735c12d063e6e6aba83c8ec47'
tags:
  - colour-emoji
  - keycaps
  - d25
feature_id: feat_term-core_nx0g1r
---

## What it does

In Noto Color Emoji a keycap is a GSUB ligature of the whole sequence, not a cmap entry: charmap('1') gives glyph 7, a thin 1 in a 32x78 ink box (1 216 inked pixels) on its 136x128 canvas, while shaping 1 U+FE0F U+20E3 with harfrust 0.12 gives glyph 1485, the boxed keycap (121x121, 14 582 pixels). Until #283 the colour path asked by the base character alone, so keycaps came out as a faint small digit in the console and the panel - invisible to tests that only asserted an image was queued; the regression test now counts ink.

The fix (#283): ColorEmoji { base, keycap }, keycap only for the twelve bases [0-9#*] carrying U+20E3 - a mark behind anything else has no keycap, and asking for one lost the character's own colour drawing; the atlas keys by it; shaping runs only on an atlas miss, after a cheap charmap check that the face maps both the base and U+20E3 (most faces the system calls emoji are monochrome symbol fonts); zero-advance glyphs are dropped before counting, since a font may keep the selector as its own empty glyph. Found by probe tests measuring the strike's alpha box.

## Where it lives

crates/app/src/fonts/atlas.rs glyph_of; crates/app/src/fonts.rs ColorEmoji; D25
