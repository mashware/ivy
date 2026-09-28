---
id: asp_the-drain-does-not-see-the-painting_tefp5s
slug: the-drain-does-not-see-the-painting
name: Draining the PTY is no use as a criterion for our own terminal
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-08T21:26:20.297Z'
updated_at: '2026-08-08T21:26:20.297Z'
last_verified: '2026-08-08T21:26:20.297Z'
file_paths:
  - docs/05-roadmap.md
  - docs/02-decisions.md
  - crates/host-load/src/lib.rs
symbols: []
content_hashes:
  docs/05-roadmap.md: 'sha256:fb6628f3b9309687a4d815b927ef4c9ae979a2c270414ca49dbbfb0f06cfa6e1'
  docs/02-decisions.md: 'sha256:755c58ddfca86a83e18f70795c4477ede05615e490fd9f1e807cd210776d6ded'
  crates/host-load/src/lib.rs: 'sha256:cb1d4bed2f3afd7e12c233f07ec78cbb3a75875b3776adc4bd988c8958722e7d'
tags:
  - D3
  - F1.3
  - measurement
  - instrument-limit
feature_id: feat_host-load_t5sdjw
---

## What it does

Measuring how long a terminal takes to **drain the PTY** is a valid instrument for **comparing other
people's terminals**, but **it is no use as an acceptance criterion for our own app**. The reason is
not a nuance: it is structural, and whoever comes back here to «close D3's criterion» with this tool
would be fooling themselves.

**The instrument does not see the painting.** A terminal that swallows fast and then discards frames
comes out well in the number and badly to the eye. And **the mitigation the roadmap already plans
for F1.3 is exactly that**: «repaint only when the PTY produces changes», that is, coalescing
frames. Our app would pass the test **by construction**, measuring well precisely because it does
what the test does not look at.

**The painting is unobservable from the process that produces the load.** No terminal protocol
reports a presented frame; the round trip of `ESC[6n` (DSR) only proves *consumed and parsed* —it
takes out the slack of the PTY buffer— and not that anything was drawn. The only place where it can
be seen is **inside `app`**: bytes processed per frame, frames presented, the lag of the last frame
with respect to the last input. That is F1.3/F1.4 and it cannot be built before the painting loop.

**Who it does decide against.** Against third parties, and with room to spare: Orca failed at the
draining already — 8.6 MiB/s with one pane and 7.1 aggregated with four, that is, **negative
scaling**, while WezTerm went from 28.4 to 85.7. There was no need to see the painting in order to
discard that stack.

**A second limit, smaller but easy to step on:** the tool's two regimes do not compare with each
other. Without `--rate` the MiB/s answers «how much does it give»; with `--rate` the time includes
the waits, so it answers «did it hold the throughput?» — measured, `--rate 10` with a fast consumer
returns 10.0 MiB/s dead on and with a slow one drops to 0.3.

## Where it lives

Written in three places on purpose, because it is what prevents the wrong conclusion: the `//!` of
`crates/host-load/src/lib.rs`, D3's «Measured» annex in `docs/02-decisions.md`, and the «Concrete
risks» of Phase 1 in `docs/05-roadmap.md`, which is where the sentence about repainting only when
the PTY produces changes comes from.
