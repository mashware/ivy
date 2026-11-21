---
id: asp_the-viewer-state-clears-its-document-on-a-kind-change_01nnre
slug: the-viewer-state-clears-its-document-on-a-kind-change
name: >-
  The drawer's viewer State holds one document and clears it on a kind change:
  anything carrying user work needs a struct of its own
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T23:24:45.241Z'
updated_at: '2026-11-20T23:24:45.241Z'
last_verified: '2026-11-20T23:24:45.241Z'
file_paths:
  - crates/app/src/viewer.rs
  - crates/app/src/viewer/image.rs
symbols:
  - 'State::open'
  - select_state
  - Kind
content_hashes:
  crates/app/src/viewer.rs: 'sha256:e8fb4457f43ffc8cbee1d00981a15f129f3f8b9a331275d26c43ca5f166d6e88'
tags:
  - viewer
  - drawer
  - trap
feature_id: feat_term-core_nx0g1r
---

## What it does

The drawer's `viewer::State` holds ONE document and clears it whenever its `Kind` changes:
`State::open` drops `requested`, `loaded`, `path` and `message` on a kind change, and clicking the
document tab always asked for `Kind::Markdown`. `open` also re-arms the refresh whenever something
was requested, and `poll` replaced `loaded` for non-Markdown kinds. So a new document kind stored
in that state is erased by switching Changes -> Document and silently re-read from disk on reopen
(an overwritten screenshot would change under the person's marks). `select_state` is a two-way bool
swap between `viewer` and `viewer_other`, so a third slot does not fit it either.

What held in #178: the image lives in its own struct on `Window` (`viewer::image::Viewer`), shown in
the document tab when it was the last document opened (D72), decoded once, never refreshed.

Evidence: read viewer.rs:98-111 and 250-263 on main ea5d7eb; found by the design challenger of
#178 before any code was written.

## Where it lives

crates/app/src/viewer.rs (State::open, activate, select_state); crates/app/src/viewer/image.rs
