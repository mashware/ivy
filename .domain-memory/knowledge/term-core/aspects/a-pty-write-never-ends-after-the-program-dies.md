---
id: asp_a-pty-write-never-ends-after-the-program-dies_wga7mq
slug: a-pty-write-never-ends-after-the-program-dies
name: >-
  On Linux a blocking write to a full pty master never ends when the program
  dies — so the writer thread leaked, and #159 made the master non-blocking (D29
  amended)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T19:31:38.995Z'
updated_at: '2026-09-26T19:31:38.995Z'
last_verified: '2026-09-26T19:31:38.995Z'
file_paths:
  - crates/term-core/src/nonblocking.rs
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/writer_wake.rs
  - crates/term-core/examples/writer_leak.rs
  - docs/02-decisions.md
symbols:
  - 'Pty::spawn_with_env'
  - write_chunk
  - Drop for Pty
  - 'UnixMasterWriter::drop'
  - 'Writable::wait'
  - Readable
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:598307ef482b8847d279ad556b074bb929e86396d7284e83bd995094bd17b1e8'
  docs/02-decisions.md: 'sha256:b003d183812e140541e040f9ee58c03010e9631c6f99decf2f3da9d51cbad7e0'
tags:
  - pty
  - kernel
  - leak
  - D29
  - linux
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured on Linux 7.0.0-34 (26 September 2026, #159), and it was D29's unexplained open question.** A BLOCKING `write()` to a pseudoterminal master whose buffer toward the program is full does **not** end when the program dies and every slave descriptor is closed. The master answers `POLLHUP` and a non-blocking read gives `EIO` — the slave really is gone — yet a non-blocking write gives `EAGAIN`, not `EIO`, and the write parked in the kernel keeps waiting. Nothing a closing process can do wakes it: `tcflush` in every direction, `TIOCSWINSZ`, `TIOCPKT`, `tcflow`, `TIOCSTI`, closing another copy of the master; `vhangup` is `EPERM` unprivileged. A thread-directed signal ends the call with a partial count, and retrying waits again at once. The block is tied to a **full** buffer: a write that fits returns in ~47 µs on a dead slave.

**Most likely mechanism, not traced**: upstream `pty_common_install` counts the master on the slave, so `n_tty_write`'s «the other side is gone» check (`!tty->link->count`) cannot pass while any master descriptor is open — and the parked write holds one, so the condition sustains itself. An earlier guess (a race between the wake in `pty_close` and the count dropping) is contradicted by the `EAGAIN` seen a second after the reap.

**Two blocking sites, not one.** The writer loop's `write_all`, and `portable-pty` 0.9.0's `UnixMasterWriter::drop`, which writes `\n` + VEOF on the blocking master at the thread's end (`stty raw` leaves VEOF = 4). With the buffer exactly full that destructor waits too, so a fix that only reaches the loop still leaks.

**What #159 did, Linux only**: the master's open file description is `O_NONBLOCK` (so the reader's duplicate gets it too — exactly what D29 had refused); the writer waits in `poll` over the master plus a pipe the console drops on close (the «knock»); `POLLHUP` with no room means the program is gone and the writer gives up; the reader retries «would block» after a `poll` for input. Cost, measured with `examples/delivery.rs`: log lines through `OPOST` ~11 % slower (48.2 vs 54.3 MiB/s), off the frame.

**Instrument trap met on the way**: a `std::process::Command` handed `Stdio::from_raw_fd(dup(slave))` keeps the parent's copies open until the `Command` is dropped. That hides the `POLLHUP` and looks exactly like a survivor holding the slave.

## Where it lives

`crates/term-core/src/nonblocking.rs` (the waits), `crates/term-core/src/lib.rs` (`Pty::spawn_with_env`, `write_chunk`, `Drop for Pty`), `crates/term-core/examples/writer_wake.rs` (the below-the-crate probe: `none|flush-out|flush-in|flush-both|winsz|close-master|diag|hold`), `crates/term-core/examples/writer_leak.rs` (thread count after a close), D29's amendment in `docs/02-decisions.md`.
