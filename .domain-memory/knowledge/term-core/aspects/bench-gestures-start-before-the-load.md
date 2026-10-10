---
id: asp_bench-gestures-start-before-the-load_xeq27r
slug: bench-gestures-start-before-the-load
name: >-
  Bench gestures start at the first frame and the load at the next 30-second
  mark: the first stretch of every gesture batch acts on empty consoles
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-10-10T22:19:34.472Z'
updated_at: '2026-10-10T22:19:34.472Z'
last_verified: '2026-10-10T22:19:34.472Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/host-load/src/main.rs
  - crates/app/src/injected.rs
symbols:
  - host-load run --at
  - 'InjectedDrags::events'
content_hashes:
  scripts/measure-frame.sh: 'sha256:29086f8575e32fb1feef78fdba760b140b2fc5341baa166ef00c6d84670412fa'
  crates/host-load/src/main.rs: 'sha256:0032f450e3fcc425df67cc8bc08c36581eae8d664a7bdb9260363fbfa5dd0a77'
  crates/app/src/injected.rs: 'sha256:14bdd1471a5eca463964760f8b759ce226a20d5456bf33514e4b68558809def9'
tags:
  - measurement-bench
  - f2.13
  - batch-e
  - host-load
feature_id: feat_term-core_nx0g1r
---

## What it does

Every console of `scripts/measure-frame.sh` runs `host-load run --at 30`, which waits for the next multiple of thirty seconds before dumping (host-load main.rs:35), while `InjectedDrags::events` injects from the first frame. So a gesture batch spends its first stretch on consoles with no text.

For a gesture whose work depends on content, that mixes two populations under one mean. In #177's batch E, 80 % and 89 % of the presses found nothing (1 199 presses / 239 found; 2 094 / 238), and the report publishes one mean over both kinds without splitting them — so **neither press mean is the cost of a press during a dump**, and the two runs' means (51.5 and 87.7 µs) cannot be decomposed from the reports. A first draft of D61's annex attributed them anyway («mostly an empty console», «the slowest describes a press during a dump»); the round-2 comment reviewer showed the attribution was not supported (a two-class model would need a negative cost). What the data does support is a bound: no press took more than 409 µs.

**How to apply:** for a content-dependent gesture, either gate it on content being present or publish its counts split by outcome; and in the annex say which part of the run a number comes from.

## Where it lives

scripts/measure-frame.sh (console mode, `--at 30`); D61's annex in docs/02-decisions.md.
