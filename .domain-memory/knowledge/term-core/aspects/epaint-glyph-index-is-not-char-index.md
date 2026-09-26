---
id: asp_epaint-glyph-index-is-not-char-index_6mejyk
slug: epaint-glyph-index-is-not-char-index
name: >-
  In an epaint galley the glyph index is not reliably the character index, and a
  cluster stamps its first character on every glyph it made
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T16:08:28.425Z'
updated_at: '2026-09-26T16:08:28.425Z'
last_verified: '2026-09-26T16:08:28.425Z'
file_paths:
  - crates/app/src/panel/color.rs
symbols:
  - slot
content_hashes: {}
tags:
  - epaint
  - text-layout
feature_id: feat_term-core_nx0g1r
---

## What it does

`Row.glyphs` in epaint 0.36.1 documents «one for each char», and code that maps a string index to a glyph leans on it. **It holds in one direction only**: epaint pads a shaped cluster that produced FEWER glyphs than characters with zero-advance continuation glyphs, but does nothing for a cluster that produced MORE (`text_layout.rs` ~402-425), so past such a cluster the index drifts **while staying in range** — `glyphs.get(i)` returns a glyph, just the wrong one.

And `Glyph::chr` is not the character at that index: **a shaped cluster stamps its first character on every glyph it produced**. Measured by laying out `U+2601 U+FE0F` with `layout_no_wrap`: glyphs `('☁', advance 14.4)` then `('☁', advance 0)` — the selector's glyph carries the cloud.

So the check that catches drift is: the first glyph's `chr` equals the base character, the last one's equals the base **or** its own character (where epaint padded), and a zero advance means a ligature's continuation with nothing of its own to replace. A first attempt that required the last glyph to carry the selector made every emoji with a selector fall back to monochrome — found by a test, not by reading.

## Where it lives

crates/app/src/panel/color.rs `slot` and its tests `a_glyph_that_is_not_the_expected_character_gives_no_slot`, `a_glyph_with_no_advance_gives_no_slot`
