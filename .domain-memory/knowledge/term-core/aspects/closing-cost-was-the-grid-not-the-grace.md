---
id: asp_closing-cost-was-the-grid-not-the-grace_q84wmv
slug: closing-cost-was-the-grid-not-the-grace
name: >-
  portable-pty's kill cost 50 ms per console because of its GRID, not its grace:
  programs die 1–3 ms after SIGHUP, and a finer look took the close from 50.9 to
  1.2 ms (D55)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T16:08:12.791Z'
updated_at: '2026-09-25T16:08:12.791Z'
last_verified: '2026-09-25T16:08:12.791Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/closing.rs
  - crates/app/src/closing.rs
symbols:
  - hang_up
  - HANGUP_GRACE
  - HANGUP_CHECK_EVERY
  - Drop for Pty
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7d458cd12c3301833284d892b02b1731909b406a21779d771cb71eec244fddb9'
  crates/term-core/examples/closing.rs: 'sha256:86f214f6bd9fca8794efa504f6f028044afaf3bb751c4cc96c3be463ac094e59'
  crates/app/src/closing.rs: 'sha256:a611022a376f88bffac12049a7242c76013f52ae9d91f614439d3be4575eeeb4'
tags:
  - measured
  - closing
  - portable-pty
  - D55
  - D32
feature_id: feat_term-core_nx0g1r
---

## What it does

**The library's name for the call is `kill`, and what it does is sleep.** `portable-pty` 0.9.0, `impl ChildKiller for std::process::Child` (`src/lib.rs:340-373`), on Unix: SIGHUP to the pid, one `try_wait` **immediately**, then **50 ms sleeps** between up to four more, then `std::process::Child::kill` (SIGKILL). On Windows it is `TerminateProcess`, at once, with no grid.

**Every live program misses the immediate look, so every close paid the whole step.** Measured on a real pseudoterminal with the library's own session + controlling-terminal setup, time from SIGHUP to reapable: bash with its rc 1.30 ms, zsh with its rc 1.84 ms (max 2.14), a Node raw-mode program 1.16 ms, sh 0.80 ms; `vim -u NONE -N -X` **through this crate's `Pty`** 3.4 ms. So ~48 ms of each close was sleep, paid on the interface thread and in series per tab.

**D55: this crate hangs up by pid itself** — SIGHUP, `try_wait` every 1 ms up to 200 ms (the library's own grace, kept), SIGKILL. `examples/closing.rs` before → after, three runs after: live program 50.88 → 1.20–1.37 ms; echo-on survivor 50.76 → 1.20–1.31; survivor with echo off 151.01 → 101.35–101.68 (D32's 100 ms deadline is what remains); ignores SIGHUP ~201 both; stopped child ~201 both.

**Signal only a positive pid, and only after a `try_wait` said the child is unreaped** — `kill(0, …)` hits the app's own group, and a reaped pid may already be somebody else's. An error from `try_wait` is treated as gone: with WNOHANG there is no EINTR, and ECHILD means the pid is no longer known to be ours.

**Two traps met on the way.**
- *A harness beside `Pty` is not `Pty`.* A hand-rolled pty harness hung vim up with a bare SIGHUP and measured **103 ms** (vim rewrites ~/.viminfo before re-raising). Through `Pty`, the close first drops the master writer, whose `\n` + `VEOF` reaches vim as end of file, and it leaves in **3.4 ms**. The 103 ms nearly went into the code as «vim uses the grace».
- *Moving the close off the interface pass is not free.* Nothing joins at process exit (a program being closed as the window goes could outlive the app), and `Pane` drops its `Pty` before its panel directory, so a deferred kill lets `PanelDir` delete the directory with the program still alive.

**Still open, reasoned not observed:** if the system refuses the SIGHUP, nothing further is sent and the `wait` is unbounded — as behind the library's `kill`. The kernel allows the signal when the user identities match on the real or saved side. `su` was withdrawn as a candidate (its waiting parent keeps the user's real uid); **`sudo` once authenticated is the plausible one** — widely reported to refuse a plain `kill` from its invoker, and the pane's direct child in `sudo vim …`. **Not measured**: `sudo` needs a password on the machine this was found on. Measure it with a `sudo` pane before trusting either answer.

## Where it lives

`hang_up`, `HANGUP_GRACE`, `HANGUP_CHECK_EVERY` and `Drop for Pty` in `crates/term-core/src/lib.rs`; rows and bands in `crates/term-core/examples/closing.rs` (`cargo run --release --example closing -p term-core`); decision **D55** in `docs/02-decisions.md`; ticket #196.
