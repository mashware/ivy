---
id: asp_a-veto-needs-a-knob-outside-measurement-variables_tkw9hk
slug: a-veto-needs-a-knob-outside-measurement-variables
name: >-
  The measurement gate is ON in every batch, so a load that must be OFF while
  measuring but ON in a batch of its own needs an opt-in knob outside
  MEASUREMENT_VARIABLES
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:04:11.570Z'
updated_at: '2026-09-18T18:04:11.570Z'
last_verified: '2026-09-18T18:04:11.570Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/settings.rs
  - scripts/measure-frame.sh
  - crates/app/tests/batches.rs
symbols:
  - any_measurement_variable
  - MEASUREMENT_VARIABLES
  - 'settings::to_use'
  - DECLARED
content_hashes:
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
  crates/app/src/settings.rs: 'sha256:0bccb6d9ae638866607d3f2974f288f05be3923f54aee4b4283e482cfa7ba763'
  scripts/measure-frame.sh: 'sha256:180b8517595b278aa0eb55476f7fdeaab7a027bf5da0ea0dcd80803c2a95bd15'
  crates/app/tests/batches.rs: 'sha256:e7236d7aafdb21e69cb4435981784470bd64fadf38f967d5bce2597cf0f9460e'
tags:
  - measurement-bench
  - d13
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

`scripts/measure-frame.sh` exports `MEASURE_SIZE` (the window size) on every single batch, and `SIZE_VARIABLE` is one of the five entries in `MEASUREMENT_VARIABLES` (`crates/app/src/lib.rs`), so `any_measurement_variable` is true in every measured run and `settings::to_use` always takes the `Entry::Measuring` branch. Anything that must be switched OFF under the gate but ON in exactly one batch cannot be reached through that list — the batch that exists to measure it could never paint its own subject. The fix already in the codebase (`MEASURE_ACTIONS`, `MEASURE_CJK`, `MEASURE_EMOJI`, `CONTROL`, and now the strip's `MEASURE_HEADER`) is a knob declared OUTSIDE `MEASUREMENT_VARIABLES`, read as an opt-in on top of the gate rather than folded into it, with both directions of the invalidation enforced by `crates/app/tests/batches.rs::DECLARED`.

## Where it lives

`crates/app/src/lib.rs` (`MEASUREMENT_VARIABLES`, `any_measurement_variable`), `crates/app/src/settings.rs` (`settings::to_use`), `scripts/measure-frame.sh` (the per-batch knobs), `crates/app/tests/batches.rs` (`DECLARED`).
