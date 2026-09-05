---
id: asp_the-lnm-mode-does-not-restore-the-carriage-return_j05mbs
slug: the-lnm-mode-does-not-restore-the-carriage-return
name: >-
  The terminal's LNM mode is no good for putting back what the kernel used to
  do: a reset wipes it, and it trades a guarantee for a preference
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T18:29:16.063Z'
updated_at: '2026-08-13T18:43:07.234Z'
last_verified: '2026-08-13T18:43:07.234Z'
file_paths:
  - crates/term-core/src/screen.rs
symbols:
  - 'Screen::advance'
  - 'Screen::new'
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:858f97327f67c34b3921b2beb9f07f3b3ebe6c26c2d6a915d535bb6e1e7a04a0'
tags:
  - alacritty-terminal
  - modes
  - painting
  - discarded
feature_id: feat_term-core_nx0g1r
---

## What it does

**`alacritty_terminal` already knows how to put the carriage return on every line feed —the
`LINE_FEED_NEW_LINE` mode, ANSI's `LNM` (`ESC[20h`)— and even so it is no good for putting back what
`ONLCR` did in the kernel.** It is the obvious solution, it fits in one line, and that is why it is
worth writing down why it was turned down.

> **Context: the whole question ended up answered with a no.** Putting the carriage return back from
> outside turned out to be impossible to do well for a different and deeper reason — see
> [[onlcr-does-not-tell-raw-mode-apart]]. This record still holds as the autopsy of **this**
> particular alternative, which is the first one anybody thinks of.

How it works: `newline()` (`src/term/mod.rs:1470`) calls `linefeed()` and, **only if that mode is
set**, `carriage_return()` as well. `linefeed()` (`:1423`) goes down a line and **does not touch the
column**: that is the staircase.

**Why it is no good:** `reset_state()` (`:1835`) —what `ESC c` runs, that is, `reset` and `tput
reset`— does `self.mode &= TermMode::VI; self.mode.insert(TermMode::default())`, and
`TermMode::default()` (`:113`) is `SHOW_CURSOR | LINE_WRAP | ALTERNATE_SCROLL | URGENCY_HINTS`:
**`LINE_FEED_NEW_LINE` is not in it**. A `reset` typed into the console would wipe the mode and leave
the staircase **for ever** in that pane.

Two more reasons, minor ones: `ESC[20l` turns it off from inside, and the mode **does not act inside
an `OSC`/`DCS` payload** whereas `ONLCR` does —the kernel does not parse—, so the mode is not
faithful either.

> **The rule it leaves:** a terminal mode is state the program inside can stomp on. Putting back
> with a mode something the kernel used to guarantee **trades a guarantee for a preference**, and one
> `reset` is enough to lose it.

## Where it lives

`crates/term-core/src/screen.rs` — `Screen::new` builds the `Term` and **sets no mode at all**,
which is the decision; `Screen::advance` is one line that passes the bytes to the parser without
touching them. The lines cited from `alacritty_terminal` are from 0.26.0 in the local cargo
registry. Written up as a turned-down alternative in **D12** of `docs/02-decisiones.md`.
