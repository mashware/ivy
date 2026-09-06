---
id: asp_the-line-discipline-discards-past-4095-bytes_9xnmyc
slug: the-line-discipline-discards-past-4095-bytes
name: >-
  In canonical mode the line discipline silently discards past 4 095 bytes with
  no newline: 2 MiB written, 4 096 read back, no error anywhere
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T15:17:31.877Z'
updated_at: '2026-09-06T15:17:31.877Z'
last_verified: '2026-09-06T15:17:31.877Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/keyboard.rs
  - crates/term-core/examples/hang.rs
symbols:
  - 'Pty::write'
  - paste
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:36606ed55bf82b06567677e0eac02210dd9c77467ee0ebae732f83244bfae7e8'
  crates/app/src/keyboard.rs: 'sha256:c427780eccb982faa34da9e6de95c43c774cdcbfc1df64cab0596327ca7cb199'
tags:
  - pty
  - termios
  - data-loss
  - measured
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured over a real pseudoterminal:** 2 MiB written to a child in canonical mode with no line breaks; the child read back **4 096 bytes**. **2 093 056 bytes gone**, and **no error on either side** — not a short count, not an errno, nothing. The limit is `MAX_CANON`, and it is the kernel's.

**The product consequence is a data-loss bug of its own:** pasting a long paragraph with no newline into a shell **loses the rest**, quietly. Nobody reports it because nobody can see it happen — the terminal looks like it worked.

### And it is what hid the blocking write for a month

The first reproduction of that hang launched a program in canonical mode and **128 000 bytes went through without blocking**. The explanation offered at the time — the echo returns the bytes, our reader drains them, and that round trip frees the input buffer — **was wrong**, and it was refuted by measuring pairs that differ in one variable:

- Same raw mode, same program, only `echo` versus `-echo`: **blocks at 19 456 bytes in both cases.** The echo does not let one more byte through.
- Canonical **without** echo: 2 MiB go through and **0 bytes come back**. No drain, and no block either.
- The decisive pair: same child, same echo, nobody draining, and the only difference is a `\n` every 64 bytes. **Without line breaks it never blocks; with them it blocks at 19 456.**

**What looked like an accidental drain was a bin.** And what actually discriminates is neither the echo nor raw mode: it is **whether the program reads**. `vim`, `htop`, `less` and a TUI agent read constantly — at rest they do not block. What blocks is a program that is **busy**.

> **The trap for anybody reproducing a pty problem:** a reproduction that gets the terminal's mode wrong publishes «it does not block» and closes the issue. Both mode and payload shape have to be part of the reproduction, and stated in it.

## Where it lives

It is the kernel's line discipline, not this repository's code. It bears on `crates/app/src/keyboard.rs` (`paste`, which filters controls and imposes no length limit) and on `crates/term-core/src/lib.rs` (`Pty::write`). The reproduction that exposed it lives in `crates/term-core/examples/hang.rs`.
