---
id: asp_exit-code-259-reads-as-still-running_4depn0
slug: exit-code-259-reads-as-still-running
name: >-
  On Windows a program that exits with code 259 reads as still running through
  portable-pty — ask the handle, then read the code again
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T22:25:15.922Z'
updated_at: '2026-11-20T22:25:15.922Z'
last_verified: '2026-11-20T22:25:15.922Z'
file_paths:
  - crates/term-core/src/termination.rs
  - crates/term-core/src/conpty.rs
symbols:
  - CachedChild
  - Termination
content_hashes: {}
tags:
  - windows
  - portable-pty
  - exit-status
feature_id: feat_term-core_nx0g1r
---

## What it does

Found by the code review of #163 (20 November 2026). Read in source, not run on
Windows.

`WinChild::try_wait` decides «running» from `GetExitCodeProcess == STILL_ACTIVE`
(259). A program that exits with 259 reads as alive for ever: `Pty::termination()`
stays `Pending`, the app keeps the pane open and repaints it every 20 ms, and a
failed resume is never noticed.

Ask the process handle instead: `WaitForSingleObject(h, 0) == WAIT_OBJECT_0` means
it ended. **Then read the exit code again** before falling back to 259: the first
read may have come just before an ordinary exit, and caching 259 then turns a clean
exit into a failure. The exit code is set before the handle is signalled, so the
second read is reliable. The same trap applies to anything that decides «already
exited» from the exit code, such as a check before `TerminateProcess`.

## Where it lives

crates/term-core/src/termination.rs (ended_as_259), crates/term-core/src/conpty.rs (has_ended); portable-pty-0.9.0/src/win/mod.rs:26-40
