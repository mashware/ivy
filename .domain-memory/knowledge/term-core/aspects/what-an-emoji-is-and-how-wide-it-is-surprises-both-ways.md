---
id: asp_what-an-emoji-is-and-how-wide-it-is-surprises-both-ways_htsvvx
slug: what-an-emoji-is-and-how-wide-it-is-surprises-both-ways
name: >-
  Which characters are emoji and how many cells they take surprises both ways:
  the hash and the digits are, and an emoji with a selector takes one cell while
  one without it takes two
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T17:16:47.025Z'
updated_at: '2026-08-25T17:16:47.025Z'
last_verified: '2026-08-25T17:16:47.025Z'
file_paths:
  - crates/app/src/fonts.rs
  - crates/app/src/painter.rs
symbols:
  - presentation_of
  - goes_to_color_by_default
  - CellWidth
content_hashes:
  crates/app/src/fonts.rs: 'sha256:d6d2b2e362c00d1b61b89b14f41b98bc0d7c28513c69d568685c156cc1730c74'
  crates/app/src/painter.rs: 'sha256:109c7e330be5065d3b14b62eafcd3fa4c3b54cd36122350c39c71084337b8a87'
tags:
  - unicode
  - emoji
  - width
  - term-core
  - alacritty
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two assumptions that look safe and both are false.** They came out of building F2.9 (August 2026) and both were verified **by running**, not by reading the specification.

### 1. ASCII has emojis too: eleven keycaps

Unicode gives **`Emoji=Yes` to the hash, to the asterisk and to the ten digits**. With the emoji selector behind them they are keycaps in their own right: `#️⃣`, `*️⃣`, `0️⃣`…`9️⃣`. **Eleven ASCII characters that go to color.**

Where it bites: a terminal painter walks thousands of cells per frame and the temptation to discard ASCII with a comparison —without consulting the Unicode tables— is enormous, because ASCII is almost everything that goes through a screen. **That shortcut eats the eleven keycaps.** The correct shortcut discards ASCII **with nothing attached**: a bare ASCII never goes to color, one with a selector may.

> It was caught by a test written **to pin the shortcut**, which walks the 128 characters with both selectors behind them. And a test is needed **in each place**: mutating the painter's shortcut left the test of the rule green, because the rule lives in another file.

### 2. An emoji with a selector takes ONE cell; one without it, TWO

Verified by running the engine: `😀` arrives as the head of a **wide** character (two columns), while `☁️`, `❤️` and `✔️` arrive as **`Single`** —one column— with the selector attached. It is the opposite of what common sense suggests: the character that **asks** for emoji presentation is the one that gets the least room.

**The cause:** the width is decided by `wcwidth` over the base character, and `U+2601` is of neutral width. The terminals that are the benchmark —Ghostty, Kitty— force the width of an emoji sequence to two, following UTS #51; `alacritty_terminal` does not.

**The visible consequence, if the declared width is respected:** `☁️` and `❤️` come out at **60 % of the line height**, because the drawing is bounded to the width of **one** cell so as not to invade the neighbouring one. They look small, and that is correct: painting them at the height of the line would cover what the program wrote alongside, and who decides the width of a character is the engine, not whoever paints.

> **And out of this comes a third thing, about the rescaling:** the bitmap of `Noto Color Emoji` is **wider than it is tall** (136×128), so scaling it «to the height of the cell» —which is how the requirement usually gets written— leaves it **wider than the two columns** the engine reserved for it: 18 pixels for a cell 17 tall. The factor has to be **the smaller** of the two, not the one of the height.

## Where it lives

The presentation rule lives in `presentation_of` and `goes_to_color_by_default`, in `crates/app/src/fonts.rs`; the ASCII shortcut, in the loop of `paint` in `crates/app/src/painter.rs`. The width is brought by `CellWidth` in the snapshot of the grid, and it comes out of the third-party engine. The tests that pin the two halves: `an_ascii_with_nothing_attached_never_goes_color` and `an_ascii_key_with_its_selector_does_go_color` in `fonts.rs`, `a_keycap_made_with_a_hash_comes_out_in_color` in `painter.rs` and `a_single_column_emoji_stays_within_it` in `fonts.rs`.
