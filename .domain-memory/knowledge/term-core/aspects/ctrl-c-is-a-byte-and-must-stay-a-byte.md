---
id: asp_ctrl-c-is-a-byte-and-must-stay-a-byte_034fvj
slug: ctrl-c-is-a-byte-and-must-stay-a-byte
name: >-
  `Ctrl+C` must keep being written as a byte: in raw mode `0x03` is not a
  signal, and signalling would kill the very TUI agents this terminal hosts
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T15:17:03.535Z'
updated_at: '2026-09-06T15:17:03.535Z'
last_verified: '2026-09-06T15:17:03.535Z'
file_paths:
  - crates/app/src/keyboard.rs
  - crates/term-core/src/lib.rs
  - docs/02-decisions.md
symbols:
  - 'Pty::write'
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:c427780eccb982faa34da9e6de95c43c774cdcbfc1df64cab0596327ca7cb199'
  crates/term-core/src/lib.rs: 'sha256:36606ed55bf82b06567677e0eac02210dd9c77467ee0ebae732f83244bfae7e8'
  docs/02-decisions.md: 'sha256:b76cc17fbac2d55b9f81f75cd1d31295e188704e9dc21642114375042b66e8c9'
tags:
  - pty
  - termios
  - D29
  - measured
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured over a real pseudoterminal in three termios states, while investigating the blocking write:**

| state | queued ahead | the `0x03` write | did SIGINT arrive? |
|---|---|---|---|
| canonical (a shell at its prompt) | 4 MiB, **never refused a byte** | accepted | **yes, instantly** |
| cbreak (`ISIG` on, `ICANON` off) | jammed at 14 336 | **refused** | no |
| raw (`ISIG` off) — vim, htop, a TUI agent | jammed at 11 776 | accepted | **no, not even with the queue empty** |

**In raw mode `0x03` is not a signal.** The program asked to read that byte, and `ISIG` is off precisely so it can. A `kill(-pgrp, SIGINT)` from the terminal would interrupt a TUI agent that was deliberately reading its own `Ctrl+C` — **the very thing this terminal exists to host.**

**And in canonical mode the interrupt was never broken**, which corrects the issue that reported it: the queue cannot jam at a shell prompt, because the line discipline **discards** instead of blocking (see [[the-line-discipline-discards-past-4095-bytes]]). So «there is no way to cancel» is true only in cbreak and raw.

**What real emulators do, checked rather than assumed:** WezTerm, Alacritty, Kitty and xterm all **write the byte**; none signals on the key. Kitty offers `signal_child` as an explicit action, with no shortcut bound by default.

### The narrow case no byte can reach, declared rather than papered over

**cbreak with the buffer jammed.** There, no byte-based path delivers the interrupt — **not even a pending queue of our own**, because the queue's job is to hold bytes until the kernel takes them, and the kernel is refusing. Only a signal would, and that is a deliberate, separate feature.

> **The general shape:** the escape hatch of a queue cannot be a message in that same queue. It is why an interrupt that queues behind the payload it is meant to interrupt is not an interrupt — and why, after the write got a thread of its own, a console whose program stopped reading had to be made to **say so**: the byte that would unstick it is stuck behind the paste.

## Where it lives

`crates/app/src/keyboard.rs` turns the chord into `[3]`; `crates/term-core/src/lib.rs` `Pty::write` carries it. The reasoning and what was discarded are in **D29**, `docs/02-decisions.md`.
