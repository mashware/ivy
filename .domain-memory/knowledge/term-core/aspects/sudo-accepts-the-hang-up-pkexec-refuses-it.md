---
id: asp_sudo-accepts-the-hang-up-pkexec-refuses-it_26xzn6
slug: sudo-accepts-the-hang-up-pkexec-refuses-it
name: >-
  sudo does NOT refuse a pane's hang-up — pkexec does: measured, and the earlier
  work had named the wrong candidate (D56)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T18:49:01.943Z'
updated_at: '2026-09-25T18:49:01.943Z'
last_verified: '2026-09-25T18:49:01.943Z'
file_paths:
  - crates/term-core/src/lib.rs
  - docs/02-decisions.md
symbols:
  - hang_up
  - send_signal
  - Drop for Pty
  - HangUp
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7d458cd12c3301833284d892b02b1731909b406a21779d771cb71eec244fddb9'
  docs/02-decisions.md: 'sha256:d2dcb9abf55336e4eec456ec7bdf9faf9da3aadc5306c5f699b6fc7bbdf5fa83'
tags:
  - closing
  - signals
  - sudo
  - pkexec
  - measurement
feature_id: feat_term-core_nx0g1r
---

## What it does

**`sudo` does NOT refuse a pane's hang-up; `pkexec` does — measured 2026-09-25, and the #196 work (D55) had named the wrong candidate.** What «sudo refuses a plain kill from its invoker» describes is signalling the *command* sudo runs, not sudo. Both sudo-rs 0.2.13 (Ubuntu's `/usr/bin/sudo`) and classic sudo 1.9.17p2 (`/usr/bin/sudo.ws`) leave the parent that is the pane's child at ruid = the invoker, euid/suid = 0; the kernel allows the invoker's signal (`kill -0` → 0), sudo catches SIGHUP and relays it, and the whole chain (parent, monitor, root command) was gone in 2.65 / 3.55 ms.

`pkexec cmd` execs the command directly with every uid root and no parent left: `kill -0` → EPERM. A pane runs `$SHELL`, so `pkexec cmd` at the prompt is a grandchild and harmless; **`exec pkexec cmd` makes the pane's direct child unsignalable**, and closing it froze the real window (desktop «not responding») because `Drop for Pty` waited for the child with no bound after the refused SIGHUP. D56 now leaves such a child running, reaped by a thread named `left-running`, with one stderr line.

**Testing it:** an unprivileged test cannot create an unsignalable child, so the seam is the refusal itself — `Pty::send_signal` is a `fn` field tests swap for one returning EPERM (all signals, or SIGKILL only), and the rest runs against a real program. **Reproducing by hand needs the user**: sudo-rs's timestamp is per session and pkexec asks through a polkit dialog; a background sampler launched from the agent's shell did not see the user's processes at all.

## Where it lives

crates/term-core/src/lib.rs — `hang_up`, `send_signal`, `Drop for Pty`; docs/02-decisions.md D56 (and D55's struck-through «one hole»)
