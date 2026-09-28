---
id: asp_onlcr-does-not-tell-raw-mode-apart_thhb6n
slug: onlcr-does-not-tell-raw-mode-apart
name: >-
  You cannot turn OPOST off and put the carriage return back from outside: the
  standard raw mode leaves the terminal in the same state, bit for bit
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T18:41:54.127Z'
updated_at: '2026-08-13T18:41:54.127Z'
last_verified: '2026-08-13T18:41:54.127Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/delivery.rs
symbols:
  - 'Pty::spawn_with_env'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
  crates/term-core/examples/delivery.rs: 'sha256:3dfd5af97ca821e1a62a34e1f80ab47d4567a47f685d0af62a470488a8686e60'
tags:
  - pseudoterminal
  - termios
  - performance
  - ncurses
  - discarded
  - reverted
feature_id: feat_term-core_nx0g1r
---

## What it does

**It was tried, delivered and reverted.** Turning `OPOST` off in the pseudoterminal gives between 4× and
112× depending on the line length ([[the-pseudoterminal-measured]]), but it forces the carriage return to
be put back from outside, and **that demands knowing whether the program inside wants it**. The kernel
knew because it translated according to each program's `termios`. From outside **the information is not
there**.

| Terminal state | Does the kernel translate? | Who leaves it that way |
|---|---|---|
| `OPOST` on, `ONLCR` on | yes | nobody has touched it |
| `OPOST` off, `ONLCR` on | **no** | `stty raw`, `cfmakeraw`, `tty.setraw`, crossterm's `enable_raw_mode`, Go's `MakeRaw` |
| `OPOST` off, `ONLCR` on | **no** | **and us too, if we turned it off** |

**The last two rows are the same state, bit for bit** — verified by running it, not reasoned. The
standard raw mode turns **only** `OPOST` off (glibc's `cfmakeraw` does `c_oflag &= ~OPOST` and touches
nothing else in the `c_oflag`) and leaves `ONLCR` set, which is exactly what a switching-off of ours
would leave.

> **The rule it leaves behind, and it is a general one:** the moment you lie to the program about the
> state of its terminal, **you lose the information about who set what**. It is not that a better signal
> is missing: it is that the kernel destroys it upon accepting the write. That is why no terminal
> emulator turns `OPOST` off.

**What was tried and did not suffice:**

- **Conditioning on `ONLCR`.** It is the signal `ncurses` uses —its manual (`curs_outopts`, `nonl`) says
  that it only uses the line feed as a movement after `nonl()`— and it **covers `vim`, `htop` and
  `nano`**, which turn it off on starting up and restore it on exit. It covers nothing that is not
  `ncurses`: any TUI from `ratatui`, from Node or from Go goes into raw mode without touching it. **This
  was the version delivered and reverted.**
- **Comparing the whole `termios` against the one we leave.** It fails just the same: `stty raw` produces
  our very same `c_oflag`.
- **Adding `ICANON` to the condition.** It fixes raw mode and breaks an `ncurses` program in `cbreak`,
  which turns `ICANON` off without turning `ONLCR` off. It trades one hole for another.
- **The terminal engine's `LNM` mode**: see [[the-lnm-mode-does-not-restore-the-carriage-return]].

**And a separate trap, in case anyone tries again:** emulating on **reading** something the kernel did on
**writing** has a window the size of its buffer (~20 KiB), and **with backpressure it is not microseconds
but seconds**, because the reader thread is stopped waiting for room. Measured: 17 KiB of a staircase log
upon opening `vim` behind a large `cat`.

**And how it was discovered, which matters as much as the what:** the design measured `vim`, `htop` and
`nano` —the three that confirm the rule— and took the `ncurses` contract as universal. The corner was
written into the decision itself as «none of the ones tested does this». **Testing only the cases that
confirm the rule is the way to make a declared corner the one that knocks you down.**

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn_with_env` **does not touch `termios`**, and that is now
deliberate. `crates/term-core/examples/delivery.rs` measures the cost that is paid on purpose. The whole
autopsy is in **D12** of `docs/02-decisions.md`.
