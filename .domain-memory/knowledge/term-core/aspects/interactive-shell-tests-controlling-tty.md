---
id: asp_interactive-shell-tests-controlling-tty_mfgy3r
slug: interactive-shell-tests-controlling-tty
name: Pipe-driven interactive shell tests must not borrow the caller's terminal
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T21:13:30.992Z'
updated_at: '2026-11-25T21:13:30.992Z'
last_verified: '2026-11-25T21:13:30.992Z'
file_paths:
  - crates/app/src/shell_integration.rs
symbols:
  - pipe_driven_shell
content_hashes:
  crates/app/src/shell_integration.rs: 'sha256:52a09e2d1acfa00249d13a7228e9a88e038180b6b6ff19d2e6808b3ef8ff663d'
tags:
  - tests
  - egui
  - shell
feature_id: feat_term-core_nx0g1r
---

## What it does

Pipe-driven interactive shell fixtures can hang when cargo itself runs inside a real terminal: five zsh children were observed with pipe stdin fd0, /dev/tty fd10 and n_tty_read, while over2300 other tests completed. Starting those test shell children in a fresh session before exec prevents borrowing cargo's controlling terminal; the same unchanged shell-input/assertion fixtures then complete in the actual Ivy auxiliary console. A no-TTY CI or agent-shell run cannot reproduce this failure mode.

## Where it lives

crates/app/src/shell_integration.rs; symbols: pipe_driven_shell. Evidence: issue 245 regression tests and targeted mutation checks (November 2026).
