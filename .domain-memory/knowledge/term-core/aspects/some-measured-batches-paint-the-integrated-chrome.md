---
id: asp_some-measured-batches-paint-the-integrated-chrome_5ymyq5
slug: some-measured-batches-paint-the-integrated-chrome
name: >-
  Some measured batches paint the integrated chrome: «the measured runs keep the
  historical look» is false for any restyle of it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-16T18:09:53.042Z'
updated_at: '2026-11-16T18:09:53.042Z'
last_verified: '2026-11-16T18:09:53.042Z'
file_paths:
  - crates/app/src/startup.rs
  - scripts/measure-frame.sh
  - docs/02-decisions.md
symbols:
  - MEASURE_CHROME
content_hashes:
  crates/app/src/startup.rs: 'sha256:7ce36e8a3ef0044c79f7f8b0978b2486ee294c693c3dde5af418a76801fb9e81'
  scripts/measure-frame.sh: 'sha256:2183c098370f198ef0f53ce5281528eb4f1447226ea174656877aca15d4daa6f'
  docs/02-decisions.md: 'sha256:26e0f37182cb3eb25578f22cdfa6006c6946c11b3453ec592de5729c143f198d'
tags:
  - measurement
  - restyle
  - comments
feature_id: feat_term-core_nx0g1r
---

## What it does

**The trap.** The integrated chrome (title bar with tabs, console header, restyled status panel —
D94, D96, D97) is opt-in for measurement, so it is easy to write «the measured runs keep the
historical panel, no published series changes». It is false: `crates/app/src/startup.rs` turns the
integrated chrome on whenever `MEASURE_CHROME` is set, and `scripts/measure-frame.sh` sets it for
the `chrome`, `chrome-zoom`, `projects` and `dock-v1-4-*` batches. Their bench panels declare
`attention` and carry marks, so every restyle of the integrated surfaces changes what those batches
paint.

**Measured on PR #3 of #93 (November 2026).** The builder gated all five panel changes behind the
integrated switch and wrote the claim into D97 and eight code comments; two independent reviewers
found it false. The true statement names the batches, says their series are not comparable across
the decision, that nothing invalidates older runs on its own, and what those batches do and do not
exercise (no integrated batch publishes actions, so button frames are never measured).

> **Rule:** before writing what a restyle does to measurement, list the batches that export
> `MEASURE_CHROME` and walk what their panels publish — never infer it from «chrome is opt-in».

## Where it lives

`crates/app/src/startup.rs` (the `MEASURE_CHROME` switch), `scripts/measure-frame.sh` (batches
that export it), D97 «Measurement» in `docs/02-decisions.md`.
