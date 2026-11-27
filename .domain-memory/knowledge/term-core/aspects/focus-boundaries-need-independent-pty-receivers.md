---
id: asp_focus-boundaries-need-independent-pty-receivers_f29ncg
slug: focus-boundaries-need-independent-pty-receivers
name: Verify same-batch keyboard focus with independent PTY receivers
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-27T15:54:59.603Z'
updated_at: '2026-11-27T15:54:59.603Z'
last_verified: '2026-11-27T15:54:59.603Z'
file_paths:
  - crates/app/src/console_input.rs
  - crates/app/src/console_input/navigation.rs
symbols: []
content_hashes:
  crates/app/src/console_input.rs: 'sha256:0b31825422d7e5712ab244a71092e46a288a91c19a6bbaed5f9ee2ed58dbdfeb'
tags: []
feature_id: feat_term-core_nx0g1r
---

## What it does

The final focused console cannot prove where intermediate keystrokes went.
In #288, typing before and after a tab switch sent both text events to the same PTY when the switch was applied only at the next navigation boundary.

Give each child its own output file and check the exact contents after the entire input batch.
Checking only the screen can produce a false positive because the terminal driver echoes input.

## Where it lives

The test bytes_follow_each_boundary_including_a_tab_switch in crates/app/src/console_input/navigation.rs exercises keyboard routing in crates/app/src/console_input.rs.
