---
id: asp_the-store-had-the-answer-and-the-search-returned-it_82v1jg
slug: the-store-had-the-answer-and-the-search-returned-it
name: >-
  The store had the answer, the search returned it, and it was skipped:
  searching by the symptom instead of by the mechanism about to be written
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T23:05:59.046Z'
updated_at: '2026-09-07T23:05:59.046Z'
last_verified: '2026-09-07T23:05:59.046Z'
file_paths:
  - crates/app/src/pointer.rs
  - crates/app/src/painter.rs
symbols:
  - handle_the_mouse
  - answer_a_clicked_question
content_hashes:
  crates/app/src/pointer.rs: 'sha256:530c56215a28b476d066ba3b55f0e0dd1cea5aa8ee3cb8fbb8268b60d68830d5'
  crates/app/src/painter.rs: 'sha256:7ace3eefa2eb016d1eafeac479942d748c5d09c883675f313c46b4e7a5642a2c'
tags:
  - method
  - retrieval
  - review
  - knowledge
feature_id: feat_term-core_nx0g1r
---

## What it does

**Three of one delivery's seventeen blockers were already written down — one in the very module being edited, twice, six lines from the new code; another in a card this store already held; and that card was returned by the delivery's own search.** (#170, September 2026.)

This is the retrieval half of [[the-sibling-mould-brings-its-decisions-and-its-gaps]], which records that copying a sibling imports its gaps *and that two of its findings were already filed*. Same disease, one step earlier: nothing was copied. The knowledge was **fetched and not read**.

## What was there, and where

| The blocker | Where the answer already was |
|---|---|
| The press was judged at egui's **latest pointer position** instead of where the button went down | `pointer.rs` documents that trap **twice**, in `DragIntent::from_chord` and in `chord_of_the_press`, six lines below the new code |
| **Which button** answers was pinned by nothing, so the middle button — X11's paste — would have destroyed a held paste | [[a-drag-that-lasts-several-frames-in-egui]], point 3, with the X11 paste as its example. **That card was in the results of this delivery's own first search** |
| The whole button queue was skipped, dropping the **release** of a live press | `route_the_buttons`' own release branch, whose comment says a program left without its release keeps the button down «for ever» |

## Why the search did not help, which is the transferable part

The queries were about **the symptom the ticket described** — «the notice painted over a console», «which pane answers it». Good queries for the *bug*. Not one was about **the mechanism about to be written**: who owns a mouse gesture, how a press is identified, what a drag latches.

> **Search by what you are about to write, not by what is broken.** The ticket's vocabulary retrieves the ticket's neighbours; the code you are about to add has different neighbours, and those are the ones holding the traps.

And the card that *was* returned was skipped **because its title named a different gesture** — a drag, where the work was about a click. A card about how egui reports one pointer gesture is about all of them.

> **A returned card is read, not triaged by its title.** The cost of reading one is a minute; skipping it was measured here at three blockers and a mechanism replaced.

## And the module's prose is specification, not commentary

`pointer.rs` carries four hundred lines of decisions about who keeps each gesture — the record **D23** made code. It was skimmed as accompaniment and it is the contract.

> **Before adding a case to a module, read what that module already decided about the thing you are adding.** In this repository that is not optional advice: the project's own rule is that **no decision lives only in the code**, which means the decisions are in the prose, which means not reading the prose is not reading the code.

## Where it lives

The traps live in `crates/app/src/pointer.rs` — `DragIntent::from_chord`, `chord_of_the_press`, and the release branch of `route_the_buttons`. The delivery that walked past them is #170 (PR #197); its three review rounds are recorded under `.claude/work/`.
