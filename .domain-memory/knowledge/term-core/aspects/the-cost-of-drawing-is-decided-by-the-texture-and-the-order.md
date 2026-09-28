---
id: asp_the-cost-of-drawing-is-decided-by-the-texture-and-the-order_8c264j
slug: the-cost-of-drawing-is-decided-by-the-texture-and-the-order
name: >-
  What drawing a frame costs is decided by which texture each shape uses and in
  what order they are queued: putting every emoji into its own texture almost
  tripled the figure egui is judged by
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T17:16:09.795Z'
updated_at: '2026-08-25T17:16:09.795Z'
last_verified: '2026-08-25T17:16:09.795Z'
file_paths:
  - crates/app/src/fonts.rs
  - crates/app/src/painter.rs
  - docs/02-decisions.md
symbols:
  - Glyphs
  - paint
  - paint_in_color
  - ATLAS_SIDE
content_hashes:
  crates/app/src/fonts.rs: 'sha256:d6d2b2e362c00d1b61b89b14f41b98bc0d7c28513c69d568685c156cc1730c74'
  crates/app/src/painter.rs: 'sha256:109c7e330be5065d3b14b62eafcd3fa4c3b54cd36122350c39c71084337b8a87'
  docs/02-decisions.md: 'sha256:d4108883d45707cfa55aee07a21e7083a329aba25d304ecd5269a601f8a84e88'
tags:
  - performance
  - egui
  - epaint
  - emoji
  - atlas
  - measurement
feature_id: feat_term-core_nx0g1r
---

## What it does

**epaint's tessellator cannot put two shapes with a different `texture_id` into the same mesh** —it
says so itself in its source: «Mixing different `texture_id` in the same…»—. So the cost of drawing
a frame does not depend only on how many shapes are queued: it depends on **how many times the
texture changes between consecutive shapes**.

**Measured in F2.9 (August 2026)**, two consoles dumping emojis against their control in ASCII, same
binary and same day. The span is «the rest of the frame», which is the figure **D3** judges the
framework by:

| | Two consoles dumping emojis |
|---|---|
| One texture **per glyph** | **743.4 µs** |
| A single atlas, painted intermixed | 565.9 µs |
| A single atlas **and two painting passes** | **356.3 µs** |
| *Control: the same two consoles in ASCII* | *268.8 µs* |

### The two halves, and why both are needed

**1. One texture for all of them.** With one per emoji, every emoji on the screen splits the batch.
An atlas of its own —512×512, placement by rows, `set_partial` to upload only the new rectangle—
puts them all under the same `texture_id`.

**2. And painting them TOGETHER, which is the half that gets forgotten.** The engine joins
**consecutive** shapes. A terminal painter walks the grid cell by cell —letter, letter, emoji,
letter— so with an atlas the batch went on being split, now between the font atlas and the emoji
one. The solution is one pass per kind: **all the texts and then all the emojis**. That is what
brings it down from 566 to 371.

### The ceiling stopped being a counter, and that is better

The criterion asked for «the memory of the rasterized glyphs has a declared ceiling». With an atlas
the ceiling is **a texture of fixed size**: **1 MiB and it does not change**, whether one emoji
shows up or a thousand. A counter would have given the same limit with memory growing up to it.

### What has to be looked at in the table is not the difference, it is the control

**268.8 µs with the new binary against the 274.3 the previous milestone published.** A screen with
no emojis **pays nothing** for the terminal knowing how to paint them, not even the extra pass. That
number is the one that makes the rest of the table acceptable; without it, the batch with emojis on
its own does not say whether there is a regression or not.

### And a lesson about the numbers themselves

**Two runs of the same batch with the same binary give 280 and 301 µs.** Any difference smaller than
those 21 µs cannot be claimed. One version of this milestone published «28 µs measured» for what the
extra pass cost, and **repeating the run disproved it**: the number was corrected to «below what the
bench can tell apart», with the argument from shape that does hold it up. It is the second time in
the same milestone that a number came out mislabeled, and both times it was caught by repeating the
measurement, not by reading the code.

## Where it lives

The atlas and its placement live in `crates/app/src/fonts.rs` (`Glyphs`, `ATLAS_SIDE`); the two
passes, in the `paint` loop in `crates/app/src/painter.rs`. The whole subtraction, with the four
rows of the table and its control, is in **D25** of `docs/02-decisions.md`. The batch that measures
it is `Q` of `scripts/measure-frame.sh`, whose control is `C`.
