---
id: asp_epaint-caps-a-rects-radius-at-half-its-side_yagvq3
slug: epaint-caps-a-rects-radius-at-half-its-side
name: >-
  epaint caps a rect's corner radius at half its smallest side: a thin band
  drawn with a card's radius pokes past the card's corners
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T17:04:20.170Z'
updated_at: '2026-11-18T17:04:20.170Z'
last_verified: '2026-11-18T17:04:20.170Z'
file_paths:
  - crates/app/src/tab_bar/title.rs
  - vendor/epaint/src/tessellator.rs
symbols:
  - 'Painter::with_clip_rect'
  - RectShape
content_hashes: {}
tags:
  - egui
  - epaint
  - painting
  - tab bar
feature_id: feat_term-core_nx0g1r
---

## What it does

**A 2-point rect given a card's 8-point top radius does not follow the card's curve.** The
vendored epaint shrinks any corner radius to half the rect's smallest side
(`tessellator.rs:638`), so the band is drawn with ~1-point corners. Laid along the top of a
card whose corners curve 8 points in, its ends stick out past the card on both sides: green
spikes over the title bar on every open tab. Found by review in #112; the headless test that
checked the band's rect and fill passed over it, because the shape on the queue was exactly
the rect it asked for.

**What holds:** paint the card's own rounded shape and clip it to the band —
`painter.with_clip_rect(band).rect_filled(card, radius, colour)`. `with_clip_rect` intersects
with the painter's current clip, so it stays inside the card's painter too. A test that wants
to pin it reads the `ClippedShape`'s `clip_rect` as well as the shape's rect; the rect alone
is the whole card and says nothing about the band.

## Where it lives

crates/app/src/tab_bar/title.rs (the open tab's accent line in `paint`); the cap is in
vendor/epaint/src/tessellator.rs around line 638.
