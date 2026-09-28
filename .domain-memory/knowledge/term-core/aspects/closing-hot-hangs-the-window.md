---
id: asp_closing-hot-hangs-the-window_s1ca6d
slug: closing-hot-hangs-the-window
name: >-
  Closing a pseudoterminal hot froze the window for ever, and TWO accidents were
  hiding it — one of them ours. Fixed with a deadline (D32); the ~101 ms per tab
  was closed by D55
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:06.689Z'
updated_at: '2026-09-25T16:07:49.068Z'
last_verified: '2026-09-25T16:07:49.068Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/closing.rs
  - crates/app/src/closing.rs
  - docs/02-decisions.md
symbols:
  - Pty
  - Drop
  - READER_GIVE_UP
  - reader_ended
  - stop
  - try_clone_reader
  - UnixMasterWriter
  - hang_up
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7d458cd12c3301833284d892b02b1731909b406a21779d771cb71eec244fddb9'
  crates/term-core/examples/closing.rs: 'sha256:86f214f6bd9fca8794efa504f6f028044afaf3bb751c4cc96c3be463ac094e59'
  crates/app/src/closing.rs: 'sha256:a611022a376f88bffac12049a7242c76013f52ae9d91f614439d3be4575eeeb4'
  docs/02-decisions.md: 'sha256:d2dcb9abf55336e4eec456ec7bdf9faf9da3aadc5306c5f699b6fc7bbdf5fa83'
tags:
  - measured
  - closing
  - threads
  - pseudoterminal
  - F2.1
  - D32
  - D55
  - resolved
feature_id: feat_term-core_nx0g1r
---

## What it does

**Both halves are closed now: the hang on 7 September 2026 (D32), the ~50 ms per console on 25 September (D55).** They had different causes; read them separately.

## The half closed by D32: it could hang for ever

`Drop for Pty` joined the reader thread with **no deadline**, and that thread sits inside `read()` on a descriptor cloned from the master. When the program inside leaves behind anything that outlived it, that survivor inherited the **slave** end, so there is no end of file, the `read` never returns and neither did the join — **inside the interface pass**, so no repaint and no keyboard for as long as the survivor lived. Measured: a `Drop` still going after eight seconds.

Three facts, each fine alone: the signal goes to the child's **pid**, never its group; a survivor **inherits the slave's descriptors**; and the `stop` flag is consulted **only inside `deliver`**, which the reader reaches after a read returns — parked in `read`, it never looks.

**The fix is a deadline, because the thread cannot be woken.** It is blocked on a descriptor a survivor holds by design: nothing signals it, nothing closes it, nothing interrupts it into giving the descriptor back. The reader owns the sending half of a channel it never writes to, so the channel goes quiet exactly when the thread ends; `Drop` waits `READER_GIVE_UP` = 100 ms and abandons it. With nobody holding the slave end the thread has gone before the deadline is consulted, and that is **asserted**, not read off a table.

**It is per console, not per gesture.** Closing a tab drops every pane it had, in series, in the same pass (`app/src/closing.rs`, `self.tabs.remove`).

**Abandoning costs a thread and a descriptor while the survivor stays silent** — not for the life of the process: the stop flag is already raised, so the survivor's **next byte** ends the thread. Same trade D29 records for the writer thread (#159).

## TWO accidents were hiding it, and the second one is ours

The hang was **not reproducible on `main`** in four attempts, and the reason is a pair:

1. **The terminal's echo.** `UnixMasterWriter::drop` writes `\n` + `VEOF` into the master; while the line discipline echoes, that comes back out as exactly `"\r\n"`, the parked `read` returns, and only *then* does the thread see its flag and leave.
2. **#157 moved that writer.** Before it, `Pty` held the writer as a **field**, and a struct's fields drop **after** `Drop::drop` returns — so the write happened after the join and rescued nothing. #157 moved it into the write thread, where it drops as the **first** thing the destructor does. See [[moving-a-value-into-a-thread-moves-its-drop]].

So from 6 September a month-old hang was unreachable **unless the echo was off** — and `stty -echo` is the first thing `vim`, `htop` or a TUI agent does. Measured on both trees: at `03803eb^` all four grandchild rows hang, **echo on included**, which is what finally explains the issue's August measurement that `main` could not reproduce.

## Two things this entry used to get wrong about WHO survives

- **`nohup` is not an example of this at all.** It replaces the terminal on all three descriptors with a file, so a `nohup`ed survivor holds **no** slave end.
- **A plain `sleep &` does hold one — and stays in the child's process group**, so a group signal *would* reach it. What escapes a group signal is a process that made a session of its own. So «signalling the group would not be enough» is the true claim, and sending one would also kill the background jobs of whoever closed the pane (#193, open).

## The half closed by D55: the ~50 ms per console

`portable-pty`'s Unix `kill` checked for the child's death on a **50 ms grid** after its SIGHUP, and every live program needs a millisecond or two to die, so every close paid the whole step (median 50.72 ms; a tab of two ~101 ms). Before D55, ten samples with a live program: median **50.72 ms**, deviation under a millisecond — the fixed `sleep`, not noise; already dead **0.23 ms**; a program ignoring SIGHUP **404 ms** in August, **201–203 ms** re-measured in September (same mechanism, a number nobody chased). D55 hangs up by pid from this crate and looks every 1 ms, keeping the library's 200 ms grace and its SIGKILL. Over three runs: an ordinary close **1.20–1.37 ms** (was 50.88); a survivor with echo off **101.35–101.68 ms** (was 151.01 — D32's deadline, untouched); a program ignoring SIGHUP **201.54–201.99 ms** either way. The mechanism and the measurements are in [[closing-cost-was-the-grid-not-the-grace]].

**And the sibling wait is bounded**: a *stopped* child never dies of a signal it can hold, so `child.wait()` has the same shape — but after the grace the hang-up falls through to a real `SIGKILL`, which nobody holds. Measured as a row: ~201 ms, which is the grace and not the cost of the wait. **No zombies:** `child.wait()` reaps the direct child and `init` adopts the orphans.

## What F2.7 inherits

Every mechanism here is Linux-specific — the kernel hangup, `EIO`, the `VEOF` echo, `setsid(1)` itself. The regression test is written so a host without `setsid(1)`, which is every macOS, **fails loudly instead of passing quietly**.

## Where it lives

`Drop for Pty`, `hang_up` and `READER_GIVE_UP` in `crates/term-core/src/lib.rs`; the reproduction is `crates/term-core/examples/closing.rs` (seven rows, run by hand, exits non-zero if a row costs something its own explanation does not cover). Regression tests in that crate's `mod tests`: `closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal`, `closing_an_ordinary_console_does_not_spend_the_abandon_deadline`, and D55's three. The destructor is walked from `crates/app/src/closing.rs`. Decisions: **D32** and **D55** in `docs/02-decisions.md`. Tickets: #62, #196.
