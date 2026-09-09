---
id: asp_a-measured-run-cannot-be-configured-by-a-file_z5ghva
slug: a-measured-run-cannot-be-configured-by-a-file
name: >-
  A measured run cannot be configured through the settings file, and the
  isolated config directory makes it look as though it can
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:52:52.085Z'
updated_at: '2026-09-09T21:52:52.085Z'
last_verified: '2026-09-09T21:52:52.085Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/settings.rs
  - crates/app/src/lib.rs
symbols:
  - to_use
  - Entry
  - ReadingSettings
content_hashes:
  scripts/measure-frame.sh: 'sha256:ae5f21bf87adf36350d78dad4ef3cc4970d0620f8506024e51a596dc48c8d112'
  crates/app/src/settings.rs: 'sha256:4094267b69277606646a9cfd8696332fa80ffdc67b1b23f5414a628ca6011d3a'
  crates/app/src/lib.rs: 'sha256:e8bd846a3440e53be756ff25a7fc65376922d1ef1119dff8d912594f58492213'
tags:
  - measurement
  - settings
  - D13
  - D26
feature_id: feat_term-core_nx0g1r
---

## What it does

**`scripts/measure-frame.sh` gives every run its own `XDG_CONFIG_HOME` (`CONFIG="$OUTPUT/.config"`) and wipes it, which reads like the natural place to put a setting a batch needs. It is not.**

The **same measurement gate** that isolates the directory also vetoes reading the file at all: any of the gate's environment variables sets `settings::Entry::Measuring`, and `settings::to_use` returns without touching disk (`ReadingSettings::Vetoed`), because whoever measures by hand would otherwise have their own font size change the grid every published figure depends on. So a `config` written into the run's directory is **never applied** and the batch runs with the factory values.

> **The way to give a measured run a value is the gate itself** — return it from the `Measuring` branch, which is the regime that already vetoes the file and forbids saving — **and then declare it on a line of the report**, because a behaviour a run has and nobody names is published as the interface framework's cost (**D13**).

A delivery designed the file route, wrote it into its design document and into its ADR-light table, and only found out when it went to implement it. What made the mistake easy is that the isolation is real and well commented — it just protects the *layout*, not the settings.

**Related trap in the same script, and it is the shape of comment rot this project counts as blocking**: its comment claiming *no code of ours reads `XDG_CONFIG_HOME`* was true when written and became false the day the terminal became configurable. Nothing failed; it went on reassuring readers for two deliveries.

## Where it lives

`crates/app/src/settings.rs` — `to_use`'s `Entry::Measuring` branch is where a measured run's values come from. The gate itself is in `crates/app/src/lib.rs` (`any_measurement_variable`), and the report's line is written in `crates/app/src/measure.rs`.
