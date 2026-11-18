---
id: asp_tabs-marked-is-a-running-maximum_bvd0f6
slug: tabs-marked-is-a-running-maximum
name: >-
  The bench's «tabs marked» is a running maximum: a test that paints a wide
  frame first has already satisfied it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T19:18:42.718Z'
updated_at: '2026-11-18T19:18:42.718Z'
last_verified: '2026-11-18T19:18:42.718Z'
file_paths:
  - crates/app/src/measure/header.rs
  - crates/app/src/tab_bar/title.rs
  - crates/app/src/tests/integrated_chrome.rs
symbols:
  - TabBar
  - saw_marked
  - marked_tabs
content_hashes:
  crates/app/src/measure/header.rs: 'sha256:1a8e44fefe4592fab9d61310b474a095bd0b1f8782e1d8997cb07514af7c9891'
  crates/app/src/tests/integrated_chrome.rs: 'sha256:6afc96f41006ad5fa7a7cdfe24109e5c1b3b246669049edd58ef6aa9b550fb5c'
tags:
  - test-disguise
  - measurement
feature_id: feat_term-core_nx0g1r
---

## What it does

**`Window::tab_bar_cost.marked_tabs` keeps the maximum per frame over the whole run**
(`measure/header.rs` `TabBar::saw_marked`), not the last frame's count. A test that asserts
`marked_tabs == 1` for a narrow-card path is satisfied by any earlier frame that marked a card —
including a wide warm-up frame run only so the fonts exist for measuring the geometry.

**Measured in #113 (review round 2, built-in reviewer).** The summary-dot test warmed up at 1400
points, where both dots fit and were ringed, then narrowed the window. Deleting `marked += 1` from
the summary branch left it green. Fixed by measuring the geometry on a separate probe window and
running the asserted window only at the narrow widths; the same mutation (M11) then failed it.

> **Rule:** a test that asserts the bar's count opens its window at the width under test and
> never paints it wider first. Warm-up frames go to another `Window`.

## Where it lives

`crates/app/src/measure/header.rs` (`TabBar::saw_marked`), and the tests that assert
`tab_bar_cost.marked_tabs` (`crates/app/src/tests/integrated_chrome.rs`, `lib.rs` tests).
