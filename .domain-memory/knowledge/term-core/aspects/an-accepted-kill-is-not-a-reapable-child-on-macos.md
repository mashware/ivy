---
id: asp_an-accepted-kill-is-not-a-reapable-child-on-macos_4c9sqx
slug: an-accepted-kill-is-not-a-reapable-child-on-macos
name: >-
  On macOS a kill the system accepts does not make the console's child reapable
  — every closing wait needs a bound after SIGKILL too (D56 amended)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-19T14:50:29.130Z'
updated_at: '2026-11-19T14:50:29.130Z'
last_verified: '2026-11-19T14:50:29.130Z'
file_paths:
  - crates/term-core/src/lib.rs
  - docs/02-decisions.md
symbols:
  - Drop for Pty
  - hang_up
  - reaped_within
  - leave_running
  - REAP_GIVE_UP
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7ee8c68e3c4920c6671a448d0eebf260bbbb8bf6d4e66405f81fcab7e0108a65'
  docs/02-decisions.md: 'sha256:26e0f37182cb3eb25578f22cdfa6006c6946c11b3453ec592de5729c143f198d'
tags:
  - macos
  - closing
  - pty
  - wait
feature_id: feat_term-core_nx0g1r
---

## What it does

**A signal the system accepts is not a program gone, on macOS.**
Every closing wait in `Drop for Pty` therefore needs a bound, even after SIGKILL.

**Observed on 19 November 2026** (#138, macOS 27.2.1 ARM64, 0.1.0-alpha.12).
Closing a console froze the app for good.
A `sample` put 100 % of the main thread in `wait4` under `Drop for Pty`,
after `hang_up` returned `Waitable`.
The waited pid was a live, unreaped child of ours:
`wait4` on anything else fails at once with `ECHILD`.
So the signals had reached it.

**Most probable mechanism, NOT reproduced.**
XNU's `proc_exit` calls `ttywait(tp)` on a session leader's controlling terminal
(`bsd/kern/kern_exit.c`).
`ttywait` sleeps with no timeout while `t_outq` is not empty
and `TS_CONNECTED` holds (`bsd/kern/tty.c`).
`Drop for Pty` raises the reader's stop flag before the hang-up,
so nobody reads the master afterwards, and the master stays open during the wait.
A kill sent to an already exiting process cannot shorten that sleep.
Linux has no such drain at exit,
which is why every closing measurement (D32, D55, D56) missed it.

**What settles it next time:**
`ps -o pid,stat,wchan,command -p <pid>` on the left-over pid that the
«had not finished after being told to end» stderr line names.
State `E` / wchan `ttywai` confirms it.

**The fix does not rest on the mechanism.**
The close waits at most `REAP_GIVE_UP` = 100 ms through `reaped_within`,
then hands the child to the detached `left-running` reaper.
On Linux a killed child is reapable about 1 ms later, so the bound is never reached there.
A unit test with a fake child pins that; a bound of 0 passed every other test.

## Where it lives

`crates/term-core/src/lib.rs` — `Drop for Pty`, `hang_up`, `reaped_within`, `leave_running`,
`REAP_GIVE_UP`; `docs/02-decisions.md` D56 (amended 19 November 2026).
