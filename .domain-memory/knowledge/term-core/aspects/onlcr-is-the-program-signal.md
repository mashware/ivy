---
id: asp_onlcr-is-the-program-signal_x7zqk9
slug: onlcr-is-the-program-signal
name: >-
  Taking the line-break translation away from the kernel forces you to ask
  ONLCR: it is the signal by which a program says whether it wants it
type: aspect
status: superseded
superseded_by: asp_onlcr-does-not-tell-raw-mode-apart_thhb6n
confidence: 80
created_at: '2026-08-13T18:28:50.597Z'
updated_at: '2026-08-13T18:42:02.621Z'
last_verified: '2026-08-13T18:28:50.597Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - ProcesadoDeSalida
  - 'Pty::spawn_with_env'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:2f7280fbd9a155e9d432ba2db3b11f67fbec767928a454bf2643cb0cd575d2e7'
tags:
  - pseudoterminal
  - termios
  - performance
  - ncurses
  - painting
feature_id: feat_term-core_nx0g1r
---

## What it does

**Turning `OPOST` off forces you to take charge of the line break, and the part you do not see coming is
that the translation CANNOT be unconditional.**

The kernel translated `\n` into `\r\n` **according to each program's `termios`**, and full-screen
programs count on being able to turn it off. Measured by reading the **master**'s `c_oflag` while the
program runs:

| Program | While it runs | On exit |
|---|---|---|
| `vim` | `ONLCR` **off** | `ONLCR` on |
| `htop` | `ONLCR` **off** | `ONLCR` on |
| `nano` | `ONLCR` **off** | — |
| `less` | **turns `OPOST` back on** | restores `OPOST` off |

And `infocmp xterm-256color` —the terminal type that `FIXED_ENV` declares to the child— gives `cud1=\n`
and `ind=\n`: for those programs a bare line feed means **go down without moving the column**.
Translating for them throws off their repainting.

> **The correct rule is «translate if and only if `ONLCR` is still set», and it is not a heuristic of
> this application: it is the contract the programs already honour.** The `ncurses` manual
> (`curs_outopts`, `nonl`) says that it only uses the line feed as a movement **after `nonl()`** — which
> is exactly what turns `ONLCR` off. As long as that bit is set, no curses program sends a bare line feed
> expecting to stay in its column.

**Two properties make the rule work by itself**, and they are the ones that avoid having to keep track of
anything: **`ONLCR` survives intact when only `OPOST` is turned off** —in `stty -a` it still shows
without a dash, an inert subflag—, so it stands as a declaration of what the program wants; and **the
programs restore it on exit**, so translating stops on entering `vim` and starts again on leaving it.

**And translating too much is harmless**, which is what stops the two halves from getting out of sync: if
something turns `OPOST` back on, `\r\n` arrives, `\r\r\n` goes out, and two carriage returns leave the
cursor where one would. That is why turning it off can be best-effort.

**What asking costs, measured** with a C program over a real pseudoterminal (200 000 rounds, after
warming up): **`tcgetattr` is 282 ns**. At 8 KiB per read and 170 MiB/s that is ~21 800 calls per second,
**0.61 % of a core** at full throughput; and it is only asked when the chunk carries some line break, so a
binary dump pays none.

**The irreducible window, and why it is not closed.** The kernel translated **on writing**; emulating it
from outside forces translating **on reading**, and between those two things the program may have changed
its settings. If it turns the signal off right after writing, the chunk that was already in the kernel's
buffer is delivered under the new rule. Bound: the ~20 KiB of the buffer, and only at the instant of the
transition — right before the program repaints the whole screen.

> **The general rule it leaves behind:** emulating from outside something the kernel did at the moment of
> writing has a window the size of its buffer. It is bounded and declared; any remedy —remembering the
> previous state, delaying the translation— is a mechanism with no demonstrable scenario.

**The corner that remains**: a program that turns `OPOST` off while leaving `ONLCR` set (`cfmakeraw`
only touches the first) would get translation where the kernel did not translate. None of the ones tested
does this, and by the `ncurses` rule it would be harmless.

**And what bounds the headline gain**: `less` turns `OPOST` back on, so any pager —`git log`, `man`— does
not see it while it is open. It is for line-oriented programs launched from the shell, which is the
project's case: an agent spitting out output. See [[the-pseudoterminal-measured]] and
[[the-lnm-mode-does-not-restore-the-carriage-return]].

## Where it lives

**Nothing, and that is the point of this record's `superseded` status.** The names below are the
ones the reverted code carried, in Spanish, because it never existed in English: the type was
`ProcesadoDeSalida`, `quitarselo_al_nucleo` turned `OPOST` off over a duplicate of the master's
descriptor **before** launching the child (a shell reads the settings on starting up, and `less`
photographs them so as to restore them on exit), and `apply` translated in the reader thread by
consulting `lo_quiere_el_programa`. Six tests at the end of the file went through it with a real
`sh`. All of it was reverted the same day it was delivered — the autopsy is **D12** in
`docs/02-decisions.md`, and `crates/term-core/src/lib.rs` carries none of it today.
