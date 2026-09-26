---
id: asp_keycap-emoji-drawn-as-a-tiny-digit_cvschf
slug: keycap-emoji-drawn-as-a-tiny-digit
name: >-
  Keycap emoji are recognised as colour and drawn as a tiny digit, because the
  glyph is asked for the base character alone
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T16:08:28.594Z'
updated_at: '2026-09-26T16:08:28.594Z'
last_verified: '2026-09-26T16:08:28.594Z'
file_paths:
  - crates/app/src/fonts/atlas.rs
  - crates/app/src/painter.rs
symbols:
  - color_glyph_of
  - paint_in_color
content_hashes:
  crates/app/src/painter.rs: 'sha256:86310d51e47691a817336e50660b43133ea20d2136f7978825b2ab34f3279d28'
tags:
  - colour-emoji
  - keycaps
  - d25
feature_id: feat_term-core_nx0g1r
---

## What it does

A keycap — a digit, `#` or `*` followed by `U+FE0F U+20E3` — passes the presentation rule and goes to the colour path, and there the glyph is asked of the emoji font for **the base character alone** (`1`, `#`), not for the sequence. Noto Color Emoji answers with its small plain digit, so `1️⃣` comes out as a faint tiny `1`, **in the console as well as in the status panel**.

D25's sentence that the twelve ASCII keycap bases «go to color» is true of the rule and not of the drawing. Seen by hand in #178's check, a panel and a console side by side; tracked as #283. The unit tests never saw it because they assert that an image is queued, not what it looks like.

## Where it lives

crates/app/src/fonts/atlas.rs `color_glyph_of` (asks per character); issue #283
