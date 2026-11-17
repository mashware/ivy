---
id: asp_board-presses-reached-the-covered-header_8zqg30
slug: board-presses-reached-the-covered-header
name: >-
  A press on the Board reached the console header beneath it: the header pass
  must be gated on the Board's claim, not on header_input.blocked_frame
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T11:00:23.126Z'
updated_at: '2026-11-17T11:00:23.126Z'
last_verified: '2026-11-17T11:00:23.126Z'
file_paths:
  - crates/app/src/header/actions.rs
  - crates/app/src/board/input.rs
  - crates/app/src/tests/board.rs
symbols:
  - 'header::actions::pointer'
  - 'board::blocked'
content_hashes:
  crates/app/src/header/actions.rs: 'sha256:3f7258d92af4f8c1394b13ff3640bc6271bcb5b4150777b0f331ddd81c28866b'
  crates/app/src/board/input.rs: 'sha256:78d4819bc4373f5a86393005cab45bf8990200b0c5950d53a8191cc9d102a6c1'
  crates/app/src/tests/board.rs: 'sha256:5c882103e25da89cf07573950eb2def4529ffba45fb9003f1475f255b2fb82f5'
tags:
  - board
  - overlay
  - input
  - test
feature_id: feat_term-core_nx0g1r
---

## What it does

The Board is a full-window overlay over the consoles. Its input pass (`board::before_keys`) marks a frame it owns through `board::blocked`, but the console header pass (`header::actions::pointer`, reached from `pointer::header::intercept` in the mouse pass that runs every frame) did its own hit test regardless: `header_input.blocked_frame` was only consulted for double clicks and as a return value, never as a gate before acting. So a press on the Board landed on whatever header button sat beneath it — the Git button swapped the Board for the diff viewer, and the close button closed a console nobody could see. It became visible when #93 PR #4 put the Board's top bar exactly over the console header strip.

Fix: return early from the header pass when `board::blocked(window, ctx)` is true. Gating on `header_input.blocked_frame` instead was tried and is wrong: other overlays (the context menu) set it on frames where the console click must still be routed, and `menu_paste_reply_targets_original_console_after_focus_changes` went red.

Rule for any new overlay over the consoles: gate the header pass on that overlay's own claim, and write a test that presses where the covered controls sit and asserts nothing beneath changed.

## Where it lives

Evidence: `a_press_on_the_board_never_reaches_the_console_header_it_covers` (crates/app/src/tests/board.rs) fails with the guard removed — 'a header close button took the press'. Guard in crates/app/src/header/actions.rs (`pointer`).
