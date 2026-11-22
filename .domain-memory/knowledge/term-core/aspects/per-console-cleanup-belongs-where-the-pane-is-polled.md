---
id: asp_per-console-cleanup-belongs-where-the-pane-is-polled_0d12d3
slug: per-console-cleanup-belongs-where-the-pane-is-polled
name: >-
  Cleanup that must run however a console closes belongs where its owner polls
  for the pane, not in each close path
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T18:37:21.581Z'
updated_at: '2026-11-22T18:37:21.581Z'
last_verified: '2026-11-22T18:37:21.581Z'
file_paths:
  - crates/app/src/closing.rs
  - crates/app/src/viewer/image/send.rs
symbols:
  - 'send::poll'
  - close_the_active_tab
  - close_the_focused
  - close_a_pane_called
  - detach_panes_called
content_hashes:
  crates/app/src/closing.rs: 'sha256:02b40f5507b087bb356a15591688de4cfb11039bcc943ab670772f690e34f211'
tags:
  - console-close
  - send
  - method
feature_id: feat_term-core_nx0g1r
---

## What it does

A console closes by many roads: closing.rs alone has four (`close_the_active_tab`,
`close_the_focused`, `close_a_pane_called`, `detach_panes_called`), plus the tree's own
closes. Work that must follow every one of them, such as cancelling a Send whose console
is gone, is placed where its owner already polls once a frame and can ask whether the
pane still exists (for a Send: the top of `send::poll`). Editing each close path misses
one the next time a path is added.

Evidence: #198 PR #1 review fix 54f81c0; the test closes the console mid-write and the
other consoles' Send stops waiting on it.

## Where it lives

crates/app/src/viewer/image/send.rs (send::poll); crates/app/src/closing.rs (the close paths)
