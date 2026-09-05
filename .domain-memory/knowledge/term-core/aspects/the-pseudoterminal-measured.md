---
id: asp_the-pseudoterminal-measured_61rp32
slug: the-pseudoterminal-measured
name: >-
  The Linux pseudoterminal, measured: 20 KiB of headroom before blocking, and
  OPOST as the real culprit of the slowness
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-09T22:15:41.845Z'
updated_at: '2026-08-13T18:42:42.215Z'
last_verified: '2026-08-13T18:42:42.215Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/delivery.rs
symbols:
  - 'Pty::spawn'
  - 'Pty::spawn_with_env'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
  crates/term-core/examples/delivery.rs: 'sha256:3dfd5af97ca821e1a62a34e1f80ab47d4567a47f685d0af62a470488a8686e60'
tags:
  - pseudoterminal
  - termios
  - performance
  - backpressure
  - portability
feature_id: feat_term-core_nx0g1r
---

## What it does

Measured with C programs over a real pseudoterminal (Linux 7.0.0-29, `portable-pty` 0.9.0), **not assumed** — applying the project's rule of reading someone else's API instead of reasoning about it.

> **State (13 August 2026): `OPOST` is still on, and no longer out of oversight but by decision.** Turning it off was attempted, delivered and reverted the same day: putting the carriage return back from outside is impossible to do right. See [[onlcr-does-not-tell-raw-mode-apart]] and **D12**.

**The kernel buffer is small: 20 480 bytes in raw mode, 19 456 in canonical.** Reproducible over three runs. It is the exact headroom between stopping reading the master and the program inside blocking on writing, and it is **six times smaller than the 128 KiB of the per-frame budget**. Whoever designs backpressure has to count on the headroom the kernel gives away being short, not generous.

**Blocking on writing is real, immediate and loses nothing.** 5 MiB written with the master not being read and the reader started 300 ms in: `write()` blocked for those same 300 ms, **0 bytes lost**. It is the premise the terminal's whole brake rests on, and it is verified instead of assumed.

**What costs two orders of magnitude is NOT canonical mode nor the echo: it is `OPOST`/`ONLCR`**, the translation of `\n` to `\r\n` on output. Isolated flag by flag:

| Configuration | Delivery |
|---|---|
| `ICANON` on, `ECHO` on, `OPOST` on — what is used | **3.8–4.6 MiB/s** |
| the same but `OPOST` off | **326–329 MiB/s** |
| fully raw (`cfmakeraw`) | **384 MiB/s** |

**The «×80» needs an adverb, and this was measured afterwards: what is paid is each line break, not each byte.** With the whole `term-core` chain and twelve megabytes dumped with `cat` (`cargo run --release --example delivery -p term-core`):

| Line | Processed in the kernel | Unprocessed | Ratio |
|---|---|---|---|
| log, 70 B | 53.3 MiB/s | 208.2 MiB/s | **4×** |
| short, 2 B | 2.4 MiB/s | 270.0 MiB/s | **112×** |

That is: **the 4 MiB/s of the table above is reproduced with two-byte lines, not with log lines.** Publishing a single number —either of the two— publishes something that looks universal and is not. That also explains why `yes` only delivers ~4 MiB/s: not because it writes two bytes at a time (`strace` shows 16 KiB writes), but because its line is two. An uncomfortable corollary that still stands: **`yes` is no good for testing a backpressure failure**, because it self-limits below what the grid drains.

**And this application's bottleneck, in passing.** Draining the master directly with 8 KiB reads and without the `term-core` chain in between: **46.8 → 358.7 MiB/s**. Through the `term-core` channel ~170 came out in the version that was reverted. If one day the cost of the pseudoterminal really matters, **there is more to gain in the reader thread and in the queues than in `termios`**.

**A trap when measuring this again**: generating the lines with an `sh` `while` and `printf` gives **12 MiB/s with the switch set both ways**, because the bottleneck is the shell loop. The measurement comes out «1×» and buries the finding under a reassuring, false number. A file has to be dumped with `cat`.

**`portable-pty` interposes no buffer of its own**: `try_clone_reader` is a `dup(2)` on the descriptor (`unix.rs:352-355`) and `Read`/`Write` call `libc::read`/`libc::write` directly without a `BufReader` (`filedescriptor-0.8.3/src/unix.rs:208-221`). There is no third place where bytes pile up, which confirms that the queues on the path are exactly two.

**None of this carries over to Windows.** There `portable-pty` uses neither pseudoterminal nor `termios`: it creates two anonymous pipes with `CreatePipe(..., 0)` —the system's default size— and hands them to `CreatePseudoConsole`. Neither the buffer size nor the cost of `ONLCR` means anything on that path, so no fix should lean on these numbers. macOS shares the `unix` code but **was not measured**.

**When it would stop holding**: if the version of `portable-pty` changes, or if it is measured on another kernel or another machine.

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn_with_env`, which opens the pseudoterminal and **does not touch `termios`**: that is where everything above is decided, and since D12 that omission is deliberate. `crates/term-core/examples/delivery.rs` is the re-runnable measurement of the two line lengths. The source of `portable-pty` 0.9.0 and `filedescriptor` 0.8.3 in cargo's local registry is where it was verified that there is no intermediate buffer.
