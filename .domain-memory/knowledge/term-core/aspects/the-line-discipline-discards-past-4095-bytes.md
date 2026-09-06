---
id: asp_the-line-discipline-discards-past-4095-bytes_9xnmyc
slug: the-line-discipline-discards-past-4095-bytes
name: >-
  In canonical mode the line discipline stops keeping a line past 4 095 bytes —
  and the constant is NOT `MAX_CANON`, which is 255
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T15:17:31.877Z'
updated_at: '2026-09-06T19:29:54.192Z'
last_verified: '2026-09-06T19:29:54.192Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/keyboard.rs
symbols: []
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21899fbd18b28515f01ca0c0c025ca2b260981459a259167fbc21b961f6070c7'
  crates/app/src/keyboard.rs: 'sha256:c427780eccb982faa34da9e6de95c43c774cdcbfc1df64cab0596327ca7cb199'
tags:
  - termios
  - paste
  - kernel
  - portability
  - mislabelled-number
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured over a real pseudoterminal:** 2 MiB written to a child in canonical mode with no line breaks; the child read back **4 096 bytes**. **2 093 056 bytes gone**, and **no error on either side** — not a short count, not an errno, nothing.

> ### ⚠ CORRECTION (6 September 2026): this entry named the constant wrong, and so did the ticket, the code and the issue tracker
>
> It said *«the limit is `MAX_CANON`, and it is the kernel's»*. **`MAX_CANON` is 255.** Measured: `getconf MAX_CANON /` returns 255, and `/usr/include/linux/limits.h:10` defines it as 255.
>
> What actually bounds a canonical line on Linux is **`N_TTY_BUF_SIZE`**, a **compile-time constant of the kernel's `n_tty` line discipline** — not a POSIX limit, not reachable through `termios`, and **not portable**: Darwin documents a different figure and nobody has run it on a Mac.
>
> **The number was right and its name was wrong, which is this project's own rule about mislabelled figures applied to a constant.** The practical consequence, and it is not academic: **4 095 must never be written into code or a test as if it were a contract**, because it is one platform's number and **F2.7 goes to a platform with another**. Anything that hard-codes it will lie there, silently, on the milestone whose whole purpose is to find out what is different.

### The product consequence is not data loss, and that is a separate record

Pasting a long paragraph with no newline does not lose a command: the line discipline **closes** the line instead of refusing it, so the program receives something syntactically complete and **runs it**. See [[an-over-long-pasted-line-is-executed-truncated]].

### And it is what hid the blocking write for a month

The first reproduction of that hang launched a program in canonical mode and **128 000 bytes went through without blocking**. The explanation offered at the time — the echo returns the bytes, our reader drains them, and that round trip frees the input buffer — **was wrong**, and it was refuted by measuring pairs that differ in one variable:

- Same raw mode, same program, only `echo` versus `-echo`: **blocks at 19 456 bytes in both cases.** The echo does not let one more byte through.
- Canonical **without** echo: 2 MiB go through and **0 bytes come back**. No drain, and no block either.
- The decisive pair: same child, same echo, nobody draining, and the only difference is a `\n` every 64 bytes. **Without line breaks it never blocks; with them it blocks at 19 456.**

### Two regimes, not one rule

Past the boundary **with the line still open**, the excess is dropped. But with a **full line already pending unread**, the master's writer **blocks** instead — measured: 4 099 written, 4 099 received, reader asleep 1.5 s. «Past the boundary → loss» is only half of it.

## Where it lives

`crates/term-core/src/lib.rs` for the measurements; `crates/app/src/keyboard.rs`'s `the_longest_line_a_reader_keeps` is where the number is declared per platform, by its real name, and documented as a threshold for asking rather than a contract. **D31** carries the decision.
