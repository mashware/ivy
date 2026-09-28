---
id: asp_the-worst-case-for-painting-is-the-inverse-of-reading_v28vha
slug: the-worst-case-for-painting-is-the-inverse-of-reading
name: >-
  The worst case for painting a panel is the inverse of the one for interpreting
  it, and the contract's ceiling does not bound the painting one
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T15:18:25.660Z'
updated_at: '2026-08-12T15:18:25.660Z'
last_verified: '2026-08-12T15:18:25.660Z'
file_paths:
  - crates/app/src/panel.rs
  - crates/panel-contract/src/panel.rs
  - docs/04-panel-contract.md
symbols:
  - MAX_CHARACTERS
  - MAX_BYTES
  - sanitize
  - paint
content_hashes:
  crates/app/src/panel.rs: 'sha256:b229abf065c777de9460a87101cb6343d6f7e56eddac128b0f4f261f5d440247'
  crates/panel-contract/src/panel.rs: 'sha256:024c1d104e63a6ddc80b1dc2596686d28bff05a93cc3f44ef57dc86b59e4ff0a'
  docs/04-panel-contract.md: 'sha256:3db1e8ddfe5ca04a53e88131e5eb07c1b1600c8d00d81b5df051fb5baee44edd'
tags:
  - measurement
  - frame
  - untrusted-input
  - egui
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Two different costs, with opposite worst cases, and a single ceiling that bounds only one of them.**

The contract's ceiling of 256 KiB bounds what a file makes it reserve **when it is interpreted**, and its worst case is thirty thousand tiny objects ([[the-peak-depends-on-the-shape-not-the-size]]). **Painting has the worst case the other way round**: measured in `--release`, a single line of 260 000 characters —254 KiB, comfortably under the ceiling, and one of the **cheap** ones to interpret (0.1 ms)— costs **65.8 ms of layout the first time and 14.6 ms of tessellation on EVERY frame** for as long as it stays published, with **1.04 million vertices per frame** going to the card. The frame's budget is 16.7 ms and the two consoles already spend 5.92. With four panes, times four.

> **And clipping to the rectangle does not save you.** epaint does its discarding **per row, not per glyph** (`tessellator.rs:2050`, with its own comment explaining why). A row that starts inside the clip is copied whole into the mesh even if it runs off to the right. Clipping the painter gives visual correctness, not bounded cost.

**The rule it leaves behind:** every time a new consumption of an outside input is put into service —reading, interpreting, laying out, tessellating— you have to ask **what its worst case is**, because it does not have to be the previous consumption's. An inherited ceiling gives a false sense of coverage: the file the contract calls harmless was exactly the one that froze the window.

**And the other control character that bites, also measured in the source:** `break_on_newline` comes set by default in the layout jobs that `Painter::text` uses (`text_layout_types.rs:107`). A single string with a `\n` **paints several rows**, and a loop that advances one row per element writes over what comes after. A tab is the twin case: it advances the width of several cells while the width measurement counts it as zero.

**How it is bounded, and where.** At the border, before calling the painter: replace the control characters with a space —do not delete them, or two words separated by a break end up read as one— and **cut by number of characters**, generously so as not to pre-empt the wrapping to the width. Cutting by characters is cheaper than laying out with a limit and does not depend on measuring glyphs.

## Where it lives

`crates/app/src/panel.rs`: the cap constant and the function that sanitizes, with its test `an_absurdly_long_line_is_cut_before_being_painted`. The warning for whoever publishes, in `docs/04-panel-contract.md`.
