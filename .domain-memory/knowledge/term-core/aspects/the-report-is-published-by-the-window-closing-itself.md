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
updated_at: '2026-09-10T15:21:57.788Z'
last_verified: '2026-09-10T15:21:57.788Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pty_output.rs
  - scripts/measure-frame.sh
symbols:
  - 'Pane::is_finished'
  - 'PtyOutput::is_alive'
  - all_finished
  - 'Window::some_closed_by_hand'
content_hashes:
  crates/app/src/lib.rs: 'sha256:6e40d885aa0d84af380a9a90dbfd6902f6a9faac6931dd349cd81f06ba69ed84'
  crates/app/src/pty_output.rs: 'sha256:59875e0e20d664da06e7fc7eea8a1ee54071c54b2d01f31066da9da43987f610'
  scripts/measure-frame.sh: 'sha256:44c37660ad8b79f5b2266288c37fb41994343e4d4563b4296607c78626888633'
tags:
  - measurement
  - D13
  - instrument
  - guards
feature_id: feat_term-core_nx0g1r
---

## What it does

**The instrument's start button is a boolean, and nothing in the crate says so.**

`lib.rs` — `if all_finished && !self.some_closed_by_hand { send_viewport_cmd(Close) }` — and `all_finished` is `&=`-folded from `Pane::is_finished`, whose only definition is `!self.pty_output.is_alive() && !queue_left`. The report is written in `on_exit`, and `scripts/measure-frame.sh` says out loud that killing a run with a signal publishes nothing.

> **So any change to what «finished» means is a change to the instrument.** A console that stops counting as finished gives a window that never closes, an `on_exit` that never runs, and a batch that hangs publishing **nothing** — on every run, for ever, with no error and nothing on screen. Reviewing such a change by reading the diff does not reach the batch script.

**What makes it safe today, checked and not assumed** (`scripts/measure-frame.sh`, console mode): a measurement console **exits normally** — the ones over the quota with a bare `exit 0`, the rest by falling off the end of the script — so its reader reads `Ok(0)`. Its own comment carries the trap that was already found once: *«Finish and do not sleep: a sleeping console is still alive, and the window only closes when all of them have finished — the batch would never end»*.

**The rule: before touching `is_finished`, `is_alive` or `all_finished`, walk to `on_exit` and to the batch script.** They are the instrument's start button and nothing names them as such.

**Where it was nearly paid.** The delivery that made «finished» mean «ended well» (D38, September 2026) checked this before writing a line, precisely because the failure would have been a whole day of runs producing no report and no error. It is the cheapest check in that phase and the most expensive one to skip.

## Where it lives

`crates/app/src/lib.rs` (`Pane::is_finished`, the fold in `pump_them_all`, and the auto-close), `crates/app/src/pty_output.rs` (`is_alive`), and `scripts/measure-frame.sh`'s console mode. The trigger's dependency widened with **D38**: from that delivery on, the publication of every measured report depends on a value produced in `term-core`.
