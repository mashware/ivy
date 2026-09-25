---
id: asp_no-eprintln-on-the-developer-channel_zh0ynj
slug: no-eprintln-on-the-developer-channel
name: >-
  A developer-channel line never uses eprintln! — it panics on a closed stderr
  pipe, and inside a Drop that is the interface pass or an abort
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T18:49:02.087Z'
updated_at: '2026-09-25T18:49:02.087Z'
last_verified: '2026-09-25T18:49:02.087Z'
file_paths:
  - crates/app/src/pty_output.rs
  - crates/term-core/src/lib.rs
symbols:
  - say
  - Drop for Pty
content_hashes:
  crates/app/src/pty_output.rs: 'sha256:e86ddf6c839cd5597c18e2a1a61ee4128cae5d001d589c146bead8f237cf3f2d'
  crates/term-core/src/lib.rs: 'sha256:7d458cd12c3301833284d892b02b1731909b406a21779d771cb71eec244fddb9'
tags:
  - stderr
  - panic
  - drop
  - review-lesson
feature_id: feat_term-core_nx0g1r
---

## What it does

**`eprintln!` must not be used for this project's developer-channel lines, and least of all inside a
`Drop`**: it panics when standard error is a closed pipe (`agent-terminal 2>&1 | head` once `head`
exits).

The app learned it in `pty_output.rs` — its `say` is `writeln!(stderr().lock())` with the error
ignored — and **the rule did not travel across crates**. D56's first version of the «program left
running» line used `eprintln!` inside `Drop for Pty`, which runs on the interface pass, so a panic
there takes the window down; and if the drop runs during an unwind, the process aborts.

Caught by review round 1 of #271, not by the suite: nothing in it closes standard error. term-core
has no `say` of its own, so the line there is `let _ = writeln!(std::io::stderr().lock(), …)` with
the reason beside it. Before adding a line to the developer channel anywhere, search the crate for
`eprintln!` — a new one is the defect, whatever the message says.

## Where it lives

crates/app/src/pty_output.rs (`say`), crates/term-core/src/lib.rs (`Drop for Pty`, the `HangUp::Refused` arm)
