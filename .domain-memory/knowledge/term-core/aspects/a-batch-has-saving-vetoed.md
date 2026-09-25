---
id: asp_a-batch-has-saving-vetoed_hwwhag
slug: a-batch-has-saving-vetoed
name: >-
  A measurement batch has layout saving VETOED, so its writes are zero by
  construction and it makes exactly ONE attempt: the script's guards count
  attempts
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T22:26:53.883Z'
updated_at: '2026-09-25T11:16:17.495Z'
last_verified: '2026-09-25T11:16:17.495Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/lib.rs
  - crates/app/src/session.rs
  - crates/app/src/measure/header.rs
symbols:
  - forbid_overwrite
  - 'SaveFailure::WontOverwrite'
  - 'SaveFailure::lasts_the_session'
  - 'Persistence::will_never_write'
content_hashes:
  scripts/measure-frame.sh: 'sha256:465f0d9927c9fbebf177895a7c675b43daa5fd1baa5c2ab6e50b91c84020a588'
  crates/app/src/lib.rs: 'sha256:29d1f78d3bf1f0267b09c1ff0567a34b1e96feae2e534a0216b8eba0e7ff0527'
  crates/app/src/session.rs: 'sha256:28822fa6d65390a16f2578aa7b1ccfae9138d7f1f85083e68887769e9d494310'
  crates/app/src/measure/header.rs: 'sha256:1de81a58761d6cb65bfddb9b6f0222b4639b5e4b62a99f539af7a7caf69ceb55'
tags:
  - measurement
  - guards
  - d16
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two of the project's mechanisms contradict each other, and neither one is wrong on its own.**

1. **The application vetoes saving as soon as it sees a variable of the measurement gate**
   (`MEASURE_TABS`, `MEASURE_PANES`, `MEASURE_SIZE`, `MEASURE_DRAGS`, `MEASURE_GESTURE`). The reason
   is a good one and it is written down: a batch cannot clobber the layout of whoever is measuring, nor
   open with the consoles they left behind instead of with the ones the batch asks for. The veto was
   widened to **every** variable upon detecting that the batch that restores was the only measurement
   session with writing still alive — two different regimes comparing themselves row by row in an annex.
2. **The script demanded at least one write** in every batch with gestures, with the project's usual
   reasoning: if nobody wrote, the cost of writing gets published as zero and reads as «saving costs
   nothing» instead of as «nothing was saved».

> **Put together, the guard is impossible to satisfy: a batch's writes are zero BY CONSTRUCTION.** Every
> batch with gestures was being invalidated. It did not go off for months because **weeks went by
> between the veto being widened and the next batch with gestures** — and the runs saved under
> `target/medidas/` were older, so at a glance the guard «worked».

**The fix, and why it is not lowering the bar.** The guard counts **attempts**. What the guard watches
over is that the save path is walked up to the gate, which is what gives the published span its meaning.

**Since #185 (September 2026) that is exactly ONE attempt per run, and the guard enforces both ends.** A
refusal the session cannot lift — `WontOverwrite`, which is what the batch veto is, `Vetoed` and `NoPlace`
— ends the attempts after the first: the window arms no further deadline (D16 item 4, amended). So a good
drag batch reports `0 writes of 1 attempts`, and `measure-frame.sh` invalidates **0** (the path was never
walked) **and anything above 1** (a regression back to paying copy + validation per deadline). Two
consequences to know before reading a batch: the write series holds **one sample**, and a drag batch's
**total frame** after #185 is not comparable with one before it — the removed work had been timed
separately and subtracted, so the framework figure did not move. And no batch has ever measured what a
session that **can** write pays: serializing and renaming were always behind the veto.

> **The general rule: when a safety mechanism of the application switches an effect off, every guard
> that was checking THAT effect becomes impossible to satisfy — and the symptom does not show up until
> the first run that exercises it.** When adding a veto, you have to go and look at who was checking
> what got vetoed. #185 walked the same road the other way: changing how often the vetoed path runs
> changed what the guard should demand, and a review had to point out that the comment already said
> «exactly one» while the guard still accepted any number above zero.

**A note on batch guards in general**: a guard that has **never failed** is not tested. The relayout
guard of batch `M` was tested by lowering the gesture's travel until it did not cross any cell border:
the run is injected in full —312 gestures, 936 effects— and **it is invalidated**.

## Where it lives

The veto is set in `run()` of `crates/app/src/lib.rs` with `forbid_overwrite`; `Persistence::save` in `crates/app/src/session.rs` returns `SaveFailure::WontOverwrite`, `SaveFailure::lasts_the_session` classifies it as permanent, and `check_whether_the_layout_moved` stops arming `save_due` once `Persistence::will_never_write` is true. The guard lives in `scripts/measure-frame.sh`, in the block that runs when `DRAGS` is not zero (and the gesture moves the layout). The parsed line «writing the layout to disk (N writes of M attempts)» is published by `crates/app/src/measure/header.rs`.
