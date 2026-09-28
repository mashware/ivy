---
id: asp_the-brake-on-output-was-the-hang-on-input_adbt1z
slug: the-brake-on-output-was-the-hang-on-input
name: >-
  The fact this project uses as its brake going out was its hang coming in: a
  measurement applied in one direction and never read as a risk in the other
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T15:16:14.533Z'
updated_at: '2026-09-06T15:16:14.533Z'
last_verified: '2026-09-06T15:16:14.533Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/pty_output.rs
  - docs/02-decisions.md
symbols:
  - 'Pty::write'
  - 'Pty::spawn'
  - 'Pty::backlog'
  - READS_IN_FLIGHT
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:36606ed55bf82b06567677e0eac02210dd9c77467ee0ebae732f83244bfae7e8'
  crates/app/src/pty_output.rs: 'sha256:deaab103083c3aaf54a7d7eb8a910390aa2087ba44696ad37d605109c80cea73'
  docs/02-decisions.md: 'sha256:b76cc17fbac2d55b9f81f75cd1d31295e188704e9dc21642114375042b66e8c9'
tags:
  - pty
  - concurrency
  - D8
  - D29
  - measured
feature_id: feat_term-core_nx0g1r
---

## What it does

**This project knew that writing to a pseudoterminal blocks when the program is not reading. It measured it, wrote it down twice, and built its only brake on it — in one direction only.**

**D8** rests on that sentence: stop draining the output, the kernel's ~20 KiB fill, and the program inside **blocks on write**. That is correct, deliberate, and the reason a terminal cannot be flooded from inside.

**On the input side the writer is us.** `Pty::write` was `write_all` on a blocking descriptor, called inline from the thread that paints. The same fact that is a feature going out was a hang coming in — measured at **5.71 s** in one shape and, in another, still blocked **54 s after the program had died**.

**The shape of the mistake is worth more than the bug.** Not a missing measurement, not an unread API: a fact used as a tool in one direction, with nobody noticing there was a symmetry to check. It lived thirty days, and it was reachable for twelve — pasting is what made it reachable, and pasting shipped in F2.6, whose own design **wrote this scenario down** and opened an issue to keep it out of that PR.

### What the fix is, and the two things that decided its shape

Writing got a thread of its own, the way reading already had one: `Pty::write` hands the bytes over and returns. **5.71 s to 3.62 µs.**

**A non-blocking descriptor was the first candidate — Alacritty's model — and it was dropped after reading `portable-pty` 0.9 instead of assuming**: `try_clone_reader` and `take_writer` are both `self.fd.try_clone()`, so writer and reader share one open file description. `O_NONBLOCK` on one lands on the other, and the reader's blocking `read` **is** the output chain, D8's brake included.

**And do not copy WezTerm here**: it writes blocking from its GUI thread and has this same bug. Alacritty is the model.

### The asymmetry of *reasons* is real, and it decided the ceiling

D8's ceiling is not copied to the input side, and the reason is volume: output is unbounded because a program dumps hundreds of MiB/s; input is somebody's hands. What **does** transfer is D8's rule — *a ceiling brakes only if the consumer of that queue stops draining* — and D8's refusal to discard, which is why an input ceiling has to mean **backpressure**, never dropping keystrokes.

### And the fix traded a visible failure for an invisible one, which the review caught

With the write on a thread, a program that stops reading for good leaves that thread stuck and everything typed afterwards queued behind it, `Ctrl+C` included, while `write` answers «taken» every time. **The window stopped freezing and a console started going deaf in silence** — worse to diagnose than what it replaced. It says so now: `Pty::backlog` answers what is owed and since when, and the frame says it once.

See [[the-pseudoterminal-measured]] for the 20 KiB and the «blocking loses nothing», and [[closing-hot-hangs-the-window]] for the sibling still open in the same family.

## Where it lives

`crates/term-core/src/lib.rs`: `Pty::write`, the writer thread in `Pty::spawn`, `Pty::backlog`, `Drop for Pty`. The sentence it rests on is in **D8**; the fix and what it discarded are **D29**, both in `docs/02-decisions.md`. The console's own sentence lives in `crates/app/src/pty_output.rs`.
