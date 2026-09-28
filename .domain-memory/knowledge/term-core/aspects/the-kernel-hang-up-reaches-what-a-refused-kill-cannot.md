---
id: asp_the-kernel-hang-up-reaches-what-a-refused-kill-cannot_9wmpx6
slug: the-kernel-hang-up-reaches-what-a-refused-kill-cannot
name: >-
  A refused kill does not keep a console's program alive: the last master close
  makes the kernel SIGHUP the session leader with no identity check (D56
  amended, #288)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T20:14:28.672Z'
updated_at: '2026-09-26T20:14:28.672Z'
last_verified: '2026-09-26T20:14:28.672Z'
file_paths:
  - crates/term-core/src/lib.rs
  - docs/02-decisions.md
symbols:
  - Drop for Pty
  - hang_up
  - close_one_that_refuses
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:598307ef482b8847d279ad556b074bb929e86396d7284e83bd995094bd17b1e8'
  docs/02-decisions.md: 'sha256:b003d183812e140541e040f9ee58c03010e9631c6f99decf2f3da9d51cbad7e0'
tags:
  - pty
  - sighup
  - kernel
  - D56
  - test-disguise
feature_id: feat_term-core_nx0g1r
---

## What it does

**D56 «left running» is true at the moment of the close and usually false a millisecond later.** Measured on Linux 7.0.0-34, 26 September 2026 (#288).

Closing a console drops every copy of the master: the `Pty`'s own field, the reader's clone (the writer thread's `\n`+VEOF on its way out is echoed back and wakes the reader, which sees the stop flag and ends), and the **never-joined writer thread's duplicates — which can be the last, after `drop(pty)` has returned**. The last close makes the kernel hang the terminal up and send SIGHUP+SIGCONT to the **session leader only** (not the foreground group) with `SEND_SIG_PRIV`: no credential check. portable-pty's `pre_exec` does `setsid()` + `TIOCSCTTY` and resets SIGHUP to `SIG_DFL`, so the console's child — or whatever it `exec`'d — is that leader.

**So a program whose `kill(2)` is refused still dies**, unless it ignores or handles SIGHUP, or some master copy stays open — an abandoned reader (D32: echo off + a survivor holding the slave) keeps one until the survivor's next byte.

**Evidence**: with every signal of this crate forged as `EPERM`, a script trapping HUP logged `HUP` after the close; holding a `dup` of the master across the close kept the program alive, and closing the dup killed it within 300 ms. The D56 regression test (`exec sleep 30`) passed only because its `kill(pid, 0)` usually ran before `left-running` reaped the SIGHUPed program — it failed once in a loaded full run.

**Unmeasured**: that real `exec pkexec cmd` keeps the pid and the controlling terminal (so is hung up the same way) — needs an authenticated prompt. The stderr line therefore says only «It may keep running».

**Rule for tests that forge a refusal**: the forgery covers only signals this crate sends; the program must `trap '' HUP` (it survives `exec`) or the kernel's hang-up ends it behind the test's back.

## Where it lives

`Drop for Pty` and `hang_up` in crates/term-core/src/lib.rs; tests `close_one_that_refuses` and the two refusal tests; D56 in docs/02-decisions.md (amendment of 26 September 2026)
