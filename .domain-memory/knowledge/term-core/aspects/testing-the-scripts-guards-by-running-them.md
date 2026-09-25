---
id: asp_testing-the-scripts-guards-by-running-them_tkrxp4
slug: testing-the-scripts-guards-by-running-them
name: >-
  Testing a measure-frame.sh guard by RUNNING it has three traps, and a batch
  knob the caller exported is judged as the batch's own load
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T15:18:54.876Z'
updated_at: '2026-09-25T15:18:54.876Z'
last_verified: '2026-09-25T15:18:54.876Z'
file_paths:
  - crates/app/tests/batches.rs
  - scripts/measure-frame.sh
symbols:
  - run_the_guards
  - run_the_guards_as
  - a_channel_report_with
  - invalidate
  - LARGE_FLOOR
  - DECLARED
content_hashes:
  crates/app/tests/batches.rs: 'sha256:ed7f6662b7fe6768618501a19661b55b628c6a2eb84dca8e5e130b4f80c5aba8'
  scripts/measure-frame.sh: 'sha256:465f0d9927c9fbebf177895a7c675b43daa5fd1baa5c2ab6e50b91c84020a588'
tags:
  - measurement-bench
  - shell
  - testing
  - d13
feature_id: feat_term-core_nx0g1r
---

## What it does

`crates/app/tests/batches.rs` runs the script's guards — the slice from `invalidate()` to the end of the file — under `sh` with `set -eu` over a fabricated report. That is the only way to test what a guard DOES rather than what its text says, and it has three traps, all met in the two review rounds of #248.

**One: `invalidate()` EXITS.** The first guard that fires ends the script, so a fixture must satisfy every guard above the one it aims at (`buttons:`, `links:`, `control:` lines with parseable numbers) or an earlier one speaks and the aimed guard never runs. And a case that must PASS a guard cannot prove it by the guard's message being absent — a script that died before reaching it gives the same output. **Prove it by the guard AFTER it speaking**: the fixture carries no `header:` line, so the strip guard invalidates next, and its sentence («whether a strip was painted») is the proof the size guards ran and stayed quiet. Three reviewers found the negative-only case independently.

**Two: the slice starts at `invalidate()`, so any top-level variable defined above it** (`LARGE_FLOOR`, defined beside the seeding that also reads it) is unset under `set -u` and the script dies before the guard. The harness prepends that line from the script itself.

**Three: the shell inherits the test process's environment**, so a knob whoever runs the suite has exported changes which guard a fixture reaches. `env_remove`-ing a list goes stale with every new knob (it missed `LARGE` the day it was added); `env_clear()` plus `PATH` and `REPORT` does not. The guards need nothing else — they call `grep`, `sed`, `head`, `mv`.

**And the same leak exists in the real script.** The seeding and the guards read the batch knobs (`PATCHES`, `READS`, `CONTROL`, `LARGE`…) from the script's own shell, so a knob exported by whoever measures reaches every batch AND the guards judge it as the batch's own load: an exported `PATCHES=1` runs X as Y and publishes it valid under X's name. No guard can see it, by construction. Every declared knob is now cleared on one line before `case "$BATCH" in`, held by `no_knob_comes_from_the_callers_environment` over `DECLARED`. A new knob goes on that line the day it is added.

**Also: validate the knobs a fixture passes** against `DECLARED` — a typo (`PATCH`) otherwise stands for a batch that does not exist, and the fixture silently tests another one.

## Where it lives

`crates/app/tests/batches.rs` — `run_the_guards_as`, `a_channel_report_with`, `the_large_pair_is_judged_on_the_size_the_pane_wrote_out`, `no_knob_comes_from_the_callers_environment`; `scripts/measure-frame.sh` — the reset line before the batch table.
