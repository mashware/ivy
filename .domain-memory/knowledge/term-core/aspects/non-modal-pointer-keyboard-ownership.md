---
id: asp_non-modal-pointer-keyboard-ownership_hwkqsa
slug: non-modal-pointer-keyboard-ownership
name: Non-modal pointer ownership must preserve concurrent console keyboard input
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-28T17:39:29.008Z'
updated_at: '2026-11-28T17:39:29.008Z'
last_verified: '2026-11-28T17:39:29.008Z'
file_paths:
  - crates/app/src/chrome.rs
  - crates/app/src/console_input.rs
  - crates/app/src/usage/chip/summary_tests.rs
symbols: []
content_hashes:
  crates/app/src/chrome.rs: 'sha256:f7776205fbf80d4d3f4c0ee319f71dbaef8743017e24712d483eba45c3bd1c21'
  crates/app/src/console_input.rs: 'sha256:0b31825422d7e5712ab244a71092e46a288a91c19a6bbaed5f9ee2ed58dbdfeb'
tags:
  - non-modal
  - keyboard
  - pointer
  - testing
feature_id: feat_term-core_nx0g1r
---

## What it does

Pointer ownership and keyboard ownership are separate contracts for a non-modal overlay. Adding
its rectangle to a broad chrome-owned predicate can also trigger Text, Key and Paste removal;
secondary presses or wheel events combined with typing then lose console input. A held-button
latch can extend the loss into later frames and releases over the title bar. A test pairing
motion with text does not cover button frames containing no text. Drive simultaneous button or
wheel plus keyboard events through the real dispatcher, include held and release frames and chip
open/close cycles, and assert exact bytes at the raw-mode PTY receiver. Visual opacity and focus
witnesses complement the receiver check; neither substitutes for it.

## Where it lives

crates/app/src/chrome.rs (non-modal pointer frame/held ownership);
crates/app/src/console_input.rs; crates/app/src/usage/chip/summary_tests.rs. Verified
2026-11-28: concurrent input mutations were rejected by raw-PTY witnesses; the owner also
confirmed typing during summary wheel gestures on the native desktop.
