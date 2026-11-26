---
id: asp_prove-output-reply-noninterference-through-send_bvtx4s
slug: prove-output-reply-noninterference-through-send
name: >-
  Draft tracking noninterference tests must establish the draft before pumping
  real output or terminal replies
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T08:50:13.650Z'
updated_at: '2026-11-26T08:50:13.650Z'
last_verified: '2026-11-26T08:50:13.650Z'
file_paths:
  - crates/app/src/viewer/image.rs
  - crates/app/src/pty_output.rs
symbols:
  - >-
    program_output_and_terminal_replies_neither_create_nor_clear_a_draft_for_send
  - pump_pane
content_hashes:
  crates/app/src/viewer/image.rs: 'sha256:6876a565a4d988ec94fc0582d9ee15e7291b67c4785d0d1280515352a95958a3'
  crates/app/src/pty_output.rs: 'sha256:aea4d95e850ac3dc311a759d6c1d56deef352d1c235790231f0e2c5a67ab12db'
tags:
  - testing
  - pty
  - draft-tracking
  - mutations
feature_id: feat_term-core_nx0g1r
---

## What it does

To prove program output and automatic terminal replies do not change a person's draft tracking, establish the person's draft before the first console pump. A helper that paints immediately can consume the output before the test seeds its draft, bypassing the transition under test. Use a raw PTY receiver to witness the actual CSI 5 n -> ESC[0n reply, then assert Send's painted confirmation and a no-write sentinel, or the actual copy, line and later Enter, instead of only the stored tracking bit. During #196 validation, separate deliberate mutations that recorded ordinary output or reply bytes as person input failed this observable test; restoring each mutation passed.

## Where it lives

crates/app/src/viewer/image.rs: program_output_and_terminal_replies_neither_create_nor_clear_a_draft_for_send constructs the window before its first pump and uses the raw Received fixture. crates/app/src/pty_output.rs: pump_pane is the output/reply boundary. Evidence is recorded in the #196 validation artifact and expected-red mutation logs070/080.
