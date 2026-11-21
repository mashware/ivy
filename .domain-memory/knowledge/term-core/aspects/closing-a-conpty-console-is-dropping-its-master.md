---
id: asp_closing-a-conpty-console-is-dropping-its-master_8zs5m7
slug: closing-a-conpty-console-is-dropping-its-master
name: >-
  On Windows a ConPTY console ends only when its master is dropped, and
  portable-pty's kill hides a refusal — what closing has to do itself (D111)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T22:25:15.793Z'
updated_at: '2026-11-20T22:25:15.793Z'
last_verified: '2026-11-20T22:25:15.793Z'
file_paths:
  - crates/term-core/src/conpty.rs
  - crates/term-core/src/lib.rs
symbols:
  - Pty
  - Slot
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7ee8c68e3c4920c6671a448d0eebf260bbbb8bf6d4e66405f81fcab7e0108a65'
tags:
  - windows
  - conpty
  - portable-pty
  - closing
feature_id: feat_term-core_nx0g1r
---

## What it does

Read in portable-pty 0.9.0 on 20 November 2026 (#163). Not run on Windows yet.

**1. Closing the pseudoconsole is dropping the last holder of portable-pty's shared value.**
The master and the slave share one `Arc<Mutex<Inner>>` whose `PsuedoCon` calls
`ClosePseudoConsole` on drop. `PsuedoCon::new` takes both pipe ends by value and
drops them after the call, so this process keeps no output write end. End of file
reaches the reader only after that close. A `Pty` that keeps the master for resizing
therefore keeps a dead console alive until something drops it.

**2. `ClosePseudoConsole` may never return when nobody reads the output.**
Microsoft documents that its last frame must be drained; before Windows 11 24H2 the
call is synchronous. So the close never runs on the interface pass, the reader
drains to end of file on Windows, and ending the program must not wait behind the
close (a reader that panicked drains nothing).

**3. The pipes are anonymous** (`filedescriptor::Pipe`): no overlapped I/O, so
`CancelIoEx` on a parked writer is not available without replacing `openpty`.
Closing the pseudoconsole breaks the pipe the writer waits on instead.

**4. `WinChild::do_kill` reads `TerminateProcess`'s result the wrong way round** and
`kill()` discards it: a refusal and a success look the same. Call
`TerminateProcess` yourself to see it.

## Where it lives

crates/term-core/src/conpty.rs (Slot, watch_exit, close); portable-pty-0.9.0/src/win/conpty.rs:15-42, psuedocon.rs:69-98, mod.rs:41-58
