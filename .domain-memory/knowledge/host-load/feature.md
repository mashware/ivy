---
id: feat_host-load_t5sdjw
slug: host-load
name: host-load — measuring how much a terminal drains
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-08T21:25:53.276Z'
updated_at: '2026-08-08T21:25:53.276Z'
last_verified: '2026-08-08T21:25:53.276Z'
file_paths:
  - crates/host-load/src/lib.rs
  - crates/host-load/src/main.rs
symbols:
  - build_load
  - start_instant
  - pace_delay
  - aggregate
  - Run
content_hashes:
  crates/host-load/src/lib.rs: 'sha256:cb1d4bed2f3afd7e12c233f07ec78cbb3a75875b3776adc4bd988c8958722e7d'
  crates/host-load/src/main.rs: 'sha256:c48abf553ba48a5cdd59d228bc128248579c86bbb7e438565339aeb2721f53fc'
tags:
  - measurement
  - performance
  - terminal
  - D3
---

## What it does

The tool that turns into something repeatable the measurement that holds **D3** up: how long a terminal takes to **drain** a fixed load dumped at it through standard output. It is launched by hand inside N visible panes at a time and `report` aggregates the batch.

It exists because the criterion of D3's «Measured» annex —*with N visible panes producing at once, the aggregate has to go up with N, not down*— had no way of being checked from the repo, and because two «Concrete risks» of Phase 1 rest the most expensive decision of the phase (changing the interface framework) on a measurement.

Three things define it, and all three come from mistakes made measuring by hand before it existed:

1. **The load is deterministic** — the content of line `i` depends only on `i`, with no clock and no randomness. It is what makes two measurements from different machines or different days comparable. And it carries color, bold, dim and a repaint with cursor movement **on purpose**: a single repeated character runs down the parser's fast path and gives a number that lies (the same lesson as `throughput.rs` in `term-core`).
2. **The start is aligned to the wall clock** — N processes that do not even know each other start together without coordinating. With a 10 s window one pane fell behind into the next cycle and falsified the batch; that is why the default value is 30 s.
3. **`report` refuses to aggregate what did not compete** — it excludes the measurements whose output was not a terminal (a pipe swallowed 22 MiB at 3 010 MiB/s, three orders of magnitude above any terminal) and it gives no aggregate if the execution windows did not overlap. The aggregate divides by the **maximum** and not by the mean: the batch occupies the terminal until the last pane finishes.

**The instrument has a structural limit that decides what it is good for** — it is in the associated aspect, and it has to be read before using this tool to close any criterion.

It does not depend on `term-core` or on any other crate: it is a writer to `stdout`, and the subject being measured is the terminal that receives.

## Where it lives

`crates/host-load/`. The checkable logic in `src/lib.rs` (`build_load`, `start_instant`, `pace_delay`, `Run`, `aggregate`); the binary and the argument parsing in `src/main.rs`. The measurement protocol and the reference numbers of WezTerm and Orca are in the `//!` of `lib.rs`. The decision that originates it, in `docs/02-decisiones.md`, D3, section «Measured».
