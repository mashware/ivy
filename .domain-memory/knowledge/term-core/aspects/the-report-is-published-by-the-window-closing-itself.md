---
id: asp_the-report-is-published-by-the-window-closing-itself_9242s5
slug: the-report-is-published-by-the-window-closing-itself
name: >-
  Every measured run publishes its report because the window closes itself, so
  any change to what «finished» means can hang every batch in silence
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:21:57.788Z'
updated_at: '2026-09-10T22:25:16.458Z'
last_verified: '2026-09-10T22:25:16.458Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pty_output.rs
  - scripts/measure-frame.sh
symbols:
  - 'Pane::is_finished'
  - 'PtyOutput::is_alive'
  - all_finished
  - 'Window::some_closed_by_hand'
  - 'pty_output::is_finished'
content_hashes:
  crates/app/src/lib.rs: 'sha256:171c1421de9c4a6d0646eb0c98664b64626350b32415c7e52c7c788d0838ed09'
  crates/app/src/pty_output.rs: 'sha256:e86ddf6c839cd5597c18e2a1a61ee4128cae5d001d589c146bead8f237cf3f2d'
  scripts/measure-frame.sh: 'sha256:44c37660ad8b79f5b2266288c37fb41994343e4d4563b4296607c78626888633'
tags:
  - measurement
  - D13
  - D40
  - instrument
  - guards
feature_id: feat_term-core_nx0g1r
---

## What it does

**The instrument's start button is a boolean, and nothing in the crate says so.**

`lib.rs` — `if all_finished && !self.some_closed_by_hand { send_viewport_cmd(Close) }` — and `all_finished` is `&=`-folded from `Pane::is_finished`. The report is written in `on_exit`, and `scripts/measure-frame.sh` says out loud that killing a run with a signal publishes nothing.

> **So any change to what «finished» means is a change to the instrument.** A console that stops counting as finished gives a window that never closes, an `on_exit` that never runs, and a batch that hangs publishing **nothing** — on every run, for ever, with no error and nothing on screen. Reviewing such a change by reading the diff does not reach the batch script.

**What makes it safe today, checked and not assumed** (`scripts/measure-frame.sh`, console mode): a measurement console **exits normally** — the ones over the quota with a bare `exit 0`, the rest by falling off the end of the script — so its reader reads `Ok(0)`. Its own comment carries the trap that was already found once: *«Finish and do not sleep: a sleeping console is still alive, and the window only closes when all of them have finished — the batch would never end»*.

**The rule: before touching `is_finished`, `is_alive` or `all_finished`, walk to `on_exit` and to the batch script.** They are the instrument's start button and nothing names them as such.

### Amendment (D40, September 2026): the definition changed, and the check was made before it did

`Pane::is_finished` no longer reads `!self.pty_output.is_alive() && !queue_left`. It delegates to `pty_output::is_finished(alive, queue_left, ending)`, which **also demands that the reading ended well** — so a console whose read failed, whose reader died, or whose output had nowhere left to go is **not** finished and the window stays open over it.

**The instrument survives it for one reason, which was verified rather than assumed**: every measurement console exits normally, so its reading ends `EndedWell` and the fold still reaches true. **What is now one step from hanging every batch in silence is anything that makes a measurement console end any other way** — a console killed by the harness, a shell that dies with a signal, a future batch whose program is stopped instead of exited. The dependency the earlier version of this card noted has tightened: publication of every measured report now rests on a value produced in `term-core` **and** on the shape of the ending each console reaches.

## Where it lives

`crates/app/src/lib.rs` (`Pane::is_finished`, the fold in `pump_them_all`, and the auto-close), `crates/app/src/pty_output.rs` (`is_alive` and, since **D40**, `is_finished` itself), and `scripts/measure-frame.sh`'s console mode.
