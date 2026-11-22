---
id: asp_a-parked-word-is-invisible-to-a-test-on-an-empty-slot_x2n1ys
slug: a-parked-word-is-invisible-to-a-test-on-an-empty-slot
name: >-
  A test disguise: a word parked in an image slot reaches its notice only when
  that slot's image is laid out, so asserting the notice of a console with no
  image sees nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T18:37:21.433Z'
updated_at: '2026-11-22T18:37:21.433Z'
last_verified: '2026-11-22T18:37:21.433Z'
file_paths:
  - crates/app/src/viewer/image.rs
  - crates/app/src/viewer/image/send.rs
symbols:
  - 'Slot::unshown'
  - 'Slot::notice'
  - forsake
content_hashes: {}
tags:
  - test-disguise
  - image
  - send
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

A Send's last word that arrives while its console's image is not on screen is parked in
that slot's `unshown`, and only reaches `notice` when the slot's image is laid out
(`show` drains it). A test that asserts only `notice` on a console with no image open
cannot see a word mis-routed into that console: the mutation that routed a closed
console's Send word to the focused slot stayed green.

Rule: assert on painted text, after the surviving console opens an image and again after
a reopen, never on the field the word is recorded in.

Evidence: #198 PR #1 review, pr-test-analyzer at viewer/image.rs:4332, confirmed by a
skeptic; fixed in f4bdb3b with that mutation going red.

## Where it lives

crates/app/src/viewer/image.rs (Slot, Viewer::show and the Send tests); crates/app/src/viewer/image/send.rs (forsake)
