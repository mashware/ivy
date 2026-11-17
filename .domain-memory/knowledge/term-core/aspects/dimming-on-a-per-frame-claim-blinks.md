---
id: asp_dimming-on-a-per-frame-claim-blinks_qb9g2t
slug: dimming-on-a-per-frame-claim-blinks
name: >-
  Dimming the window on an overlay's per-frame input claim makes the title bar
  blink under the mouse
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T11:00:23.240Z'
updated_at: '2026-11-17T11:00:23.240Z'
last_verified: '2026-11-17T11:00:23.240Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/header/menu.rs
  - crates/app/src/tests/board.rs
symbols:
  - 'tab_bar::partition'
  - 'header::menu::block_pointer'
content_hashes:
  crates/app/src/lib.rs: 'sha256:950e08f0c11dc4e836d4603065c1d76130a1517bdf2b90055eb160d1372ff098'
  crates/app/src/header/menu.rs: 'sha256:e870b08e3af10add9a12102ae1adbaf43f59f9be8701c57effe00758ea759e15'
  crates/app/src/tests/board.rs: 'sha256:5c882103e25da89cf07573950eb2def4529ffba45fb9003f1475f255b2fb82f5'
tags:
  - board
  - egui
  - painting
  - flicker
feature_id: feat_term-core_nx0g1r
---

## What it does

`tab_bar::partition`'s `dim_consoles` argument reaches `header::menu::block_pointer`, which disables the UI and, when the flag is set, leaves it at the disabled opacity — dimming the title bar and everything drawn after. The flag used to include `board_owned`, which is true only on frames where the Board consumed an event: a pointer move over the Board claims that frame, the next frame without events does not. Moving the mouse over the Board therefore alternated dimmed and undimmed frames and the tab blinked. The sidebar's claim has the same shape (a wheel delta over the rail claims the frame). Only steady states (a modal viewer, a pane in rename, the chrome's own dialogs) may drive the dimming; a claim that flips with input may gate input but must not change how things look. Reported by the owner from a real desktop run in #93 PR #4; pre-existing on main.

## Where it lives

crates/app/src/lib.rs (the `tab_bar::partition` call). Evidence: `moving_the_pointer_over_the_board_does_not_blink_the_title_bar` (crates/app/src/tests/board.rs) compares the title bar's shapes on a still frame and a pointer-move frame and fails when `board_owned` is put back into the flag.
