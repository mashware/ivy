---
id: asp_selector-emoji-spill-instead-of-widening_3vfy3d
slug: selector-emoji-spill-instead-of-widening
name: >-
  Emoji carrying the selector take one cell for the engine and for every wcwidth
  program, so they are drawn into a blank neighbour rather than widened
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T18:09:44.390Z'
updated_at: '2026-09-26T18:09:44.390Z'
last_verified: '2026-09-26T18:09:44.390Z'
file_paths:
  - crates/app/src/painter/emoji.rs
  - crates/app/src/panel/color.rs
  - docs/02-decisiones.md
symbols:
  - spills
  - slot
content_hashes:
  crates/app/src/painter/emoji.rs: 'sha256:d81556ae5c1f6968c22a2d78f84b3c5869d144a5080a464f58b79c4a6aa40f04'
  crates/app/src/panel/color.rs: 'sha256:98ee668acb004ca1cc56add273ad697d7809512ea322ae96676f78f5c7430637'
  docs/02-decisiones.md: 'sha256:b003d183812e140541e040f9ee58c03010e9631c6f99decf2f3da9d51cbad7e0'
tags:
  - colour-emoji
  - width
  - d25
feature_id: feat_term-core_nx0g1r
---

## What it does

alacritty_terminal 0.26 takes each character's width from unicode-width and pushes the emoji selector onto the previous cell without widening it (term/mod.rs:1062-1085), so the cloud and the keycaps with their selector take one cell - and programs counting with wcwidth (zsh, readline, vim) agree. A colour drawing squeezed into that cell is a speck (about 8x8 px in a 17 px row).

Three options were researched in #283 and chosen with the user: widen the grid always (kitty, WezTerm by default) desynchronises every wcwidth program - line editing drifts, backspace leaves half a drawing; widen only on DECSET 2027 (Ghostty) is correct and today changes nothing visible, since almost no program asks; draw-only spill into a blank neighbour moves nothing anyone counts. The third was taken: dense runs of keycaps with no space stay one cell. In the panel the same set spills - emoji whose base is text by default, turned colour by what follows - while emoji that are colour on their own keep the room they were laid out in. Batch Q measured no change in frame cost (D25 annex); no batch walks the panel's spill. Sources: ghostty.org config reference, kitty issue 3998, wezterm issue 3923, alacritty issue 6144.

## Where it lives

crates/app/src/painter/emoji.rs spills; crates/app/src/panel/color.rs slot; D25
