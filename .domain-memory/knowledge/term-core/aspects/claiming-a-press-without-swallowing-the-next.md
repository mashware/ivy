---
id: asp_claiming-a-press-without-swallowing-the-next_71v3jy
slug: claiming-a-press-without-swallowing-the-next
name: >-
  Claiming a press for an overlay: hold the button only while it is down, and
  only where the header pass will release it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T23:35:35.326Z'
updated_at: '2026-11-18T23:35:35.326Z'
last_verified: '2026-11-18T23:35:35.326Z'
file_paths:
  - crates/app/src/header/actions.rs
  - crates/app/src/pointer/search.rs
  - crates/app/src/pointer/clipboard.rs
symbols:
  - 'Input::claim_press'
  - 'Input::hold_context'
content_hashes:
  crates/app/src/header/actions.rs: 'sha256:3f7258d92af4f8c1394b13ff3640bc6271bcb5b4150777b0f331ddd81c28866b'
  crates/app/src/pointer/clipboard.rs: 'sha256:959f3ba06573cf0234a611cc61bc1db32038ab5a4c34fc539f7ce89cdc466a40'
tags:
  - pointer
  - egui
  - overlay
feature_id: feat_term-core_nx0g1r
---

## What it does

An overlay that takes a press (the search box, the context menu) claims it through `header_input` so
the release and any drag after it are nobody else's. Two ways to get that wrong, both met in #120:

1. **Holding a button whose release came in the same batch.** `hold_context(button, frame)` sets
   `held` unconditionally. After a quick click the release is already in the batch, `held` survives
   into the next pass, and `header::actions::pointer` swallows the next press as the tail of the old
   click. Found when a window test's drag right after a click on the search box selected nothing.
   `Input::claim_press(still_down, frame)` holds only a button `ui.input(|i|
   i.pointer.button_down(b))` reports down; the header controls and the search box share it, the
   context-menu path still calls `hold_context`.
2. **Holding where nothing releases it.** `held` is cleared only inside `header::actions::pointer`,
   which returns before its release branch when `window.headers.are_painted()` is false. A hold
   taken there locks the pointer for the rest of the session. The search box therefore holds only
   when headers are painted.

Taking the press out of the queue alone (as the notices do) is not a claim: the console loop still
focuses, clears the selection and starts a drag from it.

Evidence: #120 build (the failing drag control in
`the_search_box_buttons_act_and_nothing_under_the_box_hears_them`) and review round 3 (built-in
finding on headers off).

## Where it lives

crates/app/src/header/actions.rs (`Input::claim_press`, `Input::hold_context`, the release branch of
`pointer`); crates/app/src/pointer/search.rs (`intercept`); crates/app/src/pointer/clipboard.rs.
