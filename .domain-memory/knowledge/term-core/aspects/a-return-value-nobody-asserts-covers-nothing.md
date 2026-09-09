---
id: asp_a-return-value-nobody-asserts-covers-nothing_ws9gm8
slug: a-return-value-nobody-asserts-covers-nothing
name: >-
  A fifth test disguise: the test calls the function for its return value and
  asserts only its side effects — when a function grows an answer, the tests
  that already call it stop covering it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:53:43.709Z'
updated_at: '2026-09-09T21:53:43.709Z'
last_verified: '2026-09-09T21:53:43.709Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/alarms.rs
  - crates/app/src/lib.rs
  - crates/app/src/settings.rs
symbols:
  - paint_cursor
  - BlinkState
  - CursorInk
content_hashes:
  crates/app/src/painter.rs: 'sha256:8ffdf3a61b18c3a0a9c7c55f78d2b7591595b8c60b3b4259875a66477f3a697d'
  crates/app/src/lib.rs: 'sha256:e8bd846a3440e53be756ff25a7fc65376922d1ef1119dff8d912594f58492213'
  crates/app/src/settings.rs: 'sha256:4094267b69277606646a9cfd8696332fa80ffdc67b1b23f5414a628ca6011d3a'
tags:
  - D30
  - testing
  - mutation
feature_id: feat_term-core_nx0g1r
---

## What it does

**A painting function was changed to *answer* something — whether what it drew is in a blink — and that answer is what arms a repaint deadline. The test called it and counted the rectangles it queued. Flipping the answer left all 819 tests green**, while shipping a cursor that goes dark once and never comes back until you type.

> **When a function grows a return value, the tests that already call it keep passing and STOP covering it — they were written when it returned nothing.** Nothing in the compiler or the suite marks the moment they went blind.

One mutation round on one delivery found **six** breaks that left the suite green, and five are the same family — state whose only observable effect is one frame later, or one layer away:

| The break | What it would have shipped |
|---|---|
| the dark half answers «nothing is blinking» | the cursor goes dark once, for ever |
| the flag that answer feeds is never **cleared** | a console that stops blinking keeps waking the window — **a single-frame test cannot see it**: it takes two painting passes with the answer changing between them |
| the settings key never reaches the window | the key parses, warns, is published in the manual — and is wired to nothing; **no test walked that one assignment** |
| a counter a shell script invalidates runs on returns a constant zero | the guard stays satisfied for ever while the report publishes a number nobody produced |
| the program's own request is never consulted | the one test named for «the program asks for a steady cursor» set the **settings ceiling** instead — disguise 3, the scenario prevented the defect |

**Where to look:** callers that discard a return value in a test; any state whose effect appears one frame later; and any assignment that is the only bridge between two layers. See also [[a-test-can-pass-because-the-scenario-did-not-fit]], [[a-test-that-pins-the-helper-does-not-pin-the-path]] and [[an-expectation-computed-from-what-you-mutate-pins-nothing]] — this is the fifth member of that family.

## Where it lives

`crates/app/src/painter.rs` — `what_paint_cursor_answered` is the helper written for this, and its docblock says why counting rectangles was not enough. The five window-level tests are in `crates/app/src/lib.rs`, each naming the mutation it was written against.
