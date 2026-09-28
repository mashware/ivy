---
id: asp_covering-a-character-is-not-painting-it-with-its-presentation_cgxbdq
slug: covering-a-character-is-not-painting-it-with-its-presentation
name: >-
  A font that «covers» an emoji can paint it in grey and win by alphabetical
  order: the rule is not «the first one that covers it», it is «the first one
  that covers it with the presentation it should have»
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:57:29.445Z'
updated_at: '2026-08-25T13:57:29.445Z'
last_verified: '2026-08-25T13:57:29.445Z'
file_paths:
  - crates/app/examples/fonts.rs
  - crates/app/src/fonts.rs
  - docs/02-decisions.md
symbols:
  - presentation_of
  - goes_to_color_by_default
  - who_covers
  - first_in_color
content_hashes:
  docs/02-decisions.md: 'sha256:0de8ae72f3ac308cb5cc9ba4d96b4b7d71443f38da0b0ca87bafa69318fc44ce'
tags:
  - fonts
  - emoji
  - unicode
  - F2.9
  - D24
feature_id: feat_term-core_nx0g1r
---

## What it does

**A font that «covers» an emoji can paint it in black and white, and win by alphabetical order.**
Measured with `cargo run --release --example fuentes -p app`: walking the 259 installed families in
order, the one that takes `😀` on this machine is **DejaVu Sans**, in **monochrome**, with
`Noto Color Emoji` installed. It is the defect Alacritty has had documented for years — it takes DejaVu's
glyph and only falls back to Noto Emoji if that one does not carry it.

**The correct rule is not «the first face that covers the character», it is «the first one that covers it
WITH THE PRESENTATION IT SHOULD HAVE».** Asking for the generic emoji family fixes it and is on top of
that the cheapest thing in the whole bench: **7.2–10.8 µs cold** against the 94 µs–18.2 ms of walking the
families in order. Its first candidate **is** `Noto Color Emoji`.

> **A bench that only asks «does this face cover this character?» publishes «the emoji is covered» and
> hides the defect.** The delivery that brings the color would discover it with the code already
> written. The first version of the F2.9 bench did exactly that, and that is where the finding was
> hidden.

### The presentation rule has a half that gets forgotten

**«In color if `U+FE0F` follows it», read literally, sends to the color font any character with the
selector behind it** — an `a`, and **a blank space**. And the space one is not far-fetched: verified by
running the engine, **a loose selector attaches itself to whatever empty cell it has in front of it**,
and a loose selector comes out of cutting a string through the middle of an emoji.

Unicode only defines variation sequences for characters with `Emoji=Yes`. The complete rule, which is
Ghostty's:

> In color **if and only if**: `U+FE0F` follows it **and the character has `Emoji=Yes`**, **or** it has
> `Emoji_Presentation=Yes` and `U+FE0E` does **not** follow it.

**And when adding that condition it has to be pinned from both sides**: removing it sends the `a` and the
space to color; overshooting into strictness —demanding `Emoji_Presentation` instead of `Emoji`— leaves
`☁️` in monochrome, which is the case that gives the whole milestone its point.

### What the rule does not reach

**The skin tone modifiers (`U+1F3FB`–`U+1F3FF`) are wide**, so the engine gives them **a cell of their
own** instead of attaching them: in `☝🏽` the base arrives with nothing attached and comes out as text
while the modifier would go to color next to it. Unicode and Ghostty treat the whole sequence as an
emoji. **Resolving it demands looking at the next cell**, which is information a per-cell rule does not
have.

### And the Unicode properties crate does not make it easy

`unicode-properties 0.1.4` **has no method** for `Emoji_Presentation`: it exposes `Emoji` and
`Emoji_Component`, and that property only appears **inside the names** of its enum. You have to enumerate
its four variants by hand, checking them against its table —**two of which the name misrepresents**,
`EmojiModifierBase` and `EmojiOtherAndEmojiComponent`, which look like candidates and are not—. And its
enum is `#[non_exhaustive]`: a future category would fall into the default arm and come out monochrome
**with no warning from the compiler**.

## Where it lives

The rule lives in `crates/app/src/fonts.rs` (`presentation_of`, `goes_to_color_by_default`). The bench that
uncovered the alphabetical-order business is `crates/app/examples/fonts.rs`. The numbers are published in
**D24** of `docs/02-decisions.md`.
