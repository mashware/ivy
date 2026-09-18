---
id: asp_two-panes-is-where-window-tests-stop-discriminating_xb5ayt
slug: two-panes-is-where-window-tests-stop-discriminating
name: >-
  Two panes is the pane count at which this project's window tests stop
  discriminating — a fourth test disguise beside D30's three, and three panes is
  the floor
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:06:08.350Z'
updated_at: '2026-09-18T18:06:08.350Z'
last_verified: '2026-09-18T18:06:08.350Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - 'Tab::with_panes'
  - close_a_pane_called
content_hashes:
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
tags:
  - tests
  - test-disguise
  - f2.11
  - d30
feature_id: feat_term-core_nx0g1r
---

## What it does

With two panes: (a) the console that inherits the focus IS the only survivor, so «close moves the keyboard correctly» cannot fail; (b) after one close the survivor is alone in its tab, so anything gated on having a sibling — F2.11's close button, `close_a_pane_called`'s refusal — switches itself off and a second wrong action becomes unreachable; (c) the tiling gives the survivor the whole area whatever the reabsorption rule does. All three were live in PR #3 of F2.11: four new tests written with two panes were green while production stole the keyboard from a third console (see [[a-frame-cost-is-declared-once]]) and a repeated pass closed a second one (see [[an-event-that-cannot-be-consumed-is-counted-per-frame]]). THREE panes is the smallest count that discriminates, and it is cheap to build: `Tab::with_panes((0..3).map(|w| pane_with("sleep 30", w)).collect(), &OpeningLayout::DEFAULT, 0)`. This is the same family as [[what-breaks-is-what-the-new-operation-makes-reachable]] (the report panicking only at three panes) and generalises it into a rule for the whole crate: THREE is the floor for any test about focus, adjacency or reabsorption — two is only enough for tests about ONE pane's own geometry.

## Where it lives

`crates/app/src/lib.rs` (`Tab::with_panes`, the pane-count-dependent tests of F2.11's close button and focus transfer).
