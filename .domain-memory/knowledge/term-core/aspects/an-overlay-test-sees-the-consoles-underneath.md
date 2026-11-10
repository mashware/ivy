---
id: asp_an-overlay-test-sees-the-consoles-underneath_3fs2n2
slug: an-overlay-test-sees-the-consoles-underneath
name: >-
  A test over a full-window overlay is satisfied by the consoles painted
  underneath it: bind the text to the overlay's own rect
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-10T16:09:17.354Z'
updated_at: '2026-11-10T16:09:17.354Z'
last_verified: '2026-11-10T16:09:17.354Z'
file_paths:
  - crates/app/src/tests/board.rs
  - crates/app/src/board/view.rs
symbols:
  - the_text_queued_up
  - card_rect
content_hashes: {}
tags:
  - testing
  - egui
  - mutation
feature_id: feat_term-core_nx0g1r
---

## What it does

The board (formerly the Overview) covers the console area, but the consoles under it are still
painted every frame, and their panels and headers paint the same strings the board does: the claim
phrase («it is waiting for you») in the panel's application row, the project name in the console
header strip. A test that only asks whether a text shape was queued — `the_text_queued_up(&shapes,
"it is waiting for you")` — stays green with the board's own text removed. Measured in #40: deleting
the claim from the card survived the test until it was bound to the card. Bind such assertions to
the overlay: the text must start inside the element's own rect (`window.board.card_rect(pane)`), or
filter by the overlay's font family (board text is monospace, the header strip proportional). And
the rects a test reads back are clipped to the screen, so `screen.contains_rect(card)` is true of a
half-hidden card: compare against a known whole card's height instead.

## Where it lives

crates/app/src/tests/board.rs (a_card_paints_its_claim_and_its_warning,
consoles_are_grouped_by_repository_and_a_folder_joins_the_other_consoles,
the_board_scrolls_only_to_bring_a_card_and_its_header_into_sight)
