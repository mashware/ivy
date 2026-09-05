---
id: feat_term-core_nx0g1r
slug: term-core
name: term-core — a pane's PTY and grid
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-07T19:11:09.799Z'
updated_at: '2026-08-07T19:11:09.799Z'
last_verified: '2026-08-07T19:11:09.799Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/shell.rs
symbols:
  - Pty
  - Size
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21f7f713225e6dcff89ef43f9b4945f03c264d3071780325d2969d2661de4d18'
  crates/term-core/examples/shell.rs: 'sha256:316b3b2821edca42c7b5b15633c9ad0743a8db9eee90986d65db3203a22cac3d'
tags:
  - phase-1
  - pty
  - architecture
---

## What it does

The crate that knows how to run and read a program inside a pseudoterminal. It exists apart from the
painting layer for an economic reason, not an aesthetic one: the interface is going to be rewritten
(egui now, GPUI when it is needed, see D3), and when that happens this crate has to keep serving
exactly as it is. That is why it depends on nothing visual, and why the types of its dependencies
(`portable-pty`) do not show through in its public API — whoever uses it should not have to know
them, nor migrate along with them.

It delivers the bytes **raw**: it does not interpret, does not validate UTF-8, does not normalize
line breaks. An escape sequence can arrive split between two messages of the channel, and that is
correct. Whoever interprets is the layer above — up to F1.2, the terminal where it is printed; from
there on, `alacritty_terminal`.

## Where it lives

`crates/term-core/`. The `example` `shell.rs` is the by-hand demonstration that it works.
