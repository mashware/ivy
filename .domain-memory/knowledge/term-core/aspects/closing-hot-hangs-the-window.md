---
id: asp_closing-hot-hangs-the-window_s1ca6d
slug: closing-hot-hangs-the-window
name: >-
  Closing a pseudoterminal hot froze the window for ever, and TWO accidents were
  hiding it — one of them ours. Fixed with a deadline (D32); the ~101 ms is
  still paid
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:06.689Z'
updated_at: '2026-09-07T20:23:28.317Z'
last_verified: '2026-09-07T20:23:28.317Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/closing.rs
  - crates/app/src/lib.rs
  - docs/02-decisiones.md
symbols:
  - Pty
  - Drop
  - READER_GIVE_UP
  - reader_ended
  - stop
  - try_clone_reader
  - UnixMasterWriter
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21899fbd18b28515f01ca0c0c025ca2b260981459a259167fbc21b961f6070c7'
  crates/app/src/lib.rs: 'sha256:a0c2bb67292fa890b3b484429675a8747e96cdbaaa36ef25625f7e2c98fb897b'
  docs/02-decisiones.md: 'sha256:72862e89835bb5f95229c18221eaa6bc0f712c24fc943eb2122a6f45d696fa6c'
tags:
  - measured
  - closing
  - threads
  - pseudoterminal
  - F2.1
  - D32
  - resolved
feature_id: feat_term-core_nx0g1r
---

## What it does

**Fixed on 7 September 2026 (D32). The hang is gone; the ~101 ms is not.** Read the two halves separately — they have different causes and only one of them is closed.

## The half that is closed: it could hang for ever

`Drop for Pty` joined the reader thread with **no deadline**, and that thread sits inside `read()` on a descriptor cloned from the master. When the program inside leaves behind anything that outlived it, that survivor inherited the **slave** end, so there is no end of file, the `read` never returns and neither did the join — **inside the interface pass**, so no repaint and no keyboard for as long as the survivor lived. Measured: a `Drop` still going after eight seconds.

Three facts, each fine alone: the signal goes to the child's **pid**, never its group; a survivor **inherits the slave's descriptors**; and the `stop` flag is consulted **only inside `deliver`**, which the reader reaches after a read returns — parked in `read`, it never looks.

**The fix is a deadline, because the thread cannot be woken.** It is blocked on a descriptor a survivor holds by design: nothing signals it, nothing closes it, nothing interrupts it into giving the descriptor back. The reader now owns the sending half of a channel it never writes to, so the channel goes quiet exactly when the thread ends; `Drop` waits `READER_GIVE_UP` = 100 ms and abandons it. Measured, seven rows, before → after: the two that never returned come back at **150.65** and **150.86 ms**, and the five that already returned **did not get more expensive** — with nobody holding the slave end the thread has gone before the deadline is consulted, and that is **asserted**, not read off a table.

**It is per console, not per gesture.** Closing a tab drops every pane it had, in series, in the same pass (`app/src/lib.rs`, `self.tabs.remove`). Four consoles each over a survivor is four times the deadline plus four of the library's polls.

**Abandoning costs a thread and a descriptor while the survivor stays silent** — not for the life of the process, which is what the code said until a review corrected it: the stop flag is already raised, so the survivor's **next byte** ends the thread. Same trade D29 records for the writer thread (#159); audited with it.

## TWO accidents were hiding it, and the second one is ours

This is the part worth remembering. The hang was **not reproducible on `main`** in four attempts, and the reason is a pair:

1. **The terminal's echo.** `UnixMasterWriter::drop` writes `\n` + `VEOF` into the master; while the line discipline echoes, that comes back out as exactly `"\r\n"` — two bytes, logged from an instrumented reader — the parked `read` returns, and only *then* does the thread see its flag and leave.
2. **#157 moved that writer.** Before it, `Pty` held the writer as a **field**, and a struct's fields drop **after** `Drop::drop` returns — so the write happened after the join and rescued nothing. #157 (D29, writing on its own thread) moved it into the write thread, where it drops as the **first** thing the destructor does. See [[moving-a-value-into-a-thread-moves-its-drop]].

So from 6 September a month-old hang was unreachable **unless the echo was off** — and `stty -echo` is the first thing `vim`, `htop` or a TUI agent does. Measured on both trees: at `03803eb^` all four grandchild rows hang, echo on included, which is what finally explains the issue's August measurement.

## Two things this entry used to get wrong about WHO survives

Both were in the code as comments and a review knocked them down:

- **`nohup` is not an example of this at all.** It replaces the terminal on all three descriptors with a file, so a `nohup`ed survivor holds **no** slave end.
- **A plain `sleep &` does hold one — and stays in the child's process group**, so a group signal *would* reach it. What escapes a group signal is a process that made a session of its own. So «signalling the group would not close it» is false; **«would not be enough»** is the true claim, and sending one would also kill the background jobs of whoever closed the pane, which is an undecided product question.

## The half that is still open

**~101 ms per tab, untouched.** `portable-pty`'s `child.kill()` sends **SIGHUP, not SIGKILL**, then grants grace by polling with 50 ms waits — and the **first poll is immediate**, so 50 ms is the interval and not «what was already spent», another sentence a review corrected. Ten samples with a live shell: median **50.72 ms**, deviation under a millisecond — the fixed `sleep`, not noise. Already dead: **0.23 ms**. Two consoles per tab in series → ~101 ms, some 17 frames of D3's budget. A program that ignores SIGHUP: 404 ms in August, **201–203 ms** re-measured — same mechanism, a number nobody chased.

It needs a different change (SIGHUP then an immediate SIGKILL, or closing off the interface pass), so it is a fix of its own.

**And the sibling wait is bounded, which was the adjacent risk**: a *stopped* child never dies of a signal it can hold, so `child.wait()` has the same shape. It does not happen — after its grace the library's `kill` falls through to a real `SIGKILL`, which nobody holds. Measured as a row: **202.52 ms**, which is that grace period and not the cost of the wait.

**No zombies:** `child.wait()` reaps the direct child and `init` adopts the orphans.

## What F2.7 inherits

Every mechanism here is Linux-specific — the kernel hangup, `EIO`, the `VEOF` echo, `setsid(1)` itself. The regression test is written so a host without `setsid(1)`, which is every macOS, **fails loudly instead of passing quietly**. That is the right failure and it gives the first run on another platform a named thing to fix.

## Where it lives

`Drop for Pty` and `READER_GIVE_UP` in `crates/term-core/src/lib.rs`; the reproduction is `crates/term-core/examples/closing.rs` (seven rows, run by hand, exits non-zero if a row costs something its own explanation does not cover). The two regression tests are in that crate's own `mod tests`: `closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal` and `closing_an_ordinary_console_does_not_spend_the_abandon_deadline`. The destructor is walked from `Window::close_the_active_tab` in `crates/app/src/lib.rs`, inside `eframe::App::ui`. The decision, with seven discarded alternatives, is **D32** in `docs/02-decisiones.md`. Ticket: https://github.com/mashware/agent-terminal/issues/62
