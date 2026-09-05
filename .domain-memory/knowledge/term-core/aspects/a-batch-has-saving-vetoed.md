---
id: asp_a-batch-has-saving-vetoed_hwwhag
slug: a-batch-has-saving-vetoed
name: >-
  A measurement batch has layout saving VETOED, so its writes are zero by
  construction: the script's guards count ATTEMPTS
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T22:26:53.883Z'
updated_at: '2026-08-24T22:26:53.883Z'
last_verified: '2026-08-24T22:26:53.883Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/lib.rs
  - crates/app/src/session.rs
  - crates/app/src/measure.rs
symbols:
  - forbid_overwrite
  - 'SaveFailure::WontOverwrite'
  - save
content_hashes:
  scripts/measure-frame.sh: 'sha256:5164b53754c528c121b0dc462bd6b6ded7dd92b97e504c321ea968fdd7d77d2f'
  crates/app/src/lib.rs: 'sha256:8f5d9220dadc886213f3207269ffaf9502f3454547843a94c9aa6e24dedbb2d5'
  crates/app/src/session.rs: 'sha256:e830c75bff3ba304a58835e5647deeb2ef55331849d5092066f7d021d49030c6'
  crates/app/src/measure.rs: 'sha256:b25ea9e9128a5e7699f1b9ac89d4501e7304d44fc5a3c7635d3284cb64e7bc1d'
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

**The fix, and why it is not lowering the bar.** The guard now counts **attempts**. The work that has to
be declared **does happen and does get timed** — copying the layout, validating it, serializing it and
getting as far as the gate —; the only thing that does not happen is the touch to disk. What the guard
watches over is that **that path is walked**, which is what gives the published span its meaning.

> **The general rule: when a safety mechanism of the application switches an effect off, every guard
> that was checking THAT effect becomes impossible to satisfy — and the symptom does not show up until
> the first run that exercises it.** When adding a veto, you have to go and look at who was checking
> what got vetoed. The comment in the code that widened the veto **mentioned the script's guard** and
> even so it was not adjusted: reading that it exists is not the same as checking whether it is still
> satisfiable.

**A note on batch guards in general**, which is where this case fits: a guard that has **never failed**
is not tested. The relayout guard of batch `M` was tested by lowering the gesture's travel until it did
not cross any cell border: the run is injected in full —312 gestures, 936 effects— and **it is
invalidated**. Without that trial the previous guard, which only looked at whether the line existed,
would have passed the run as good — startup already relayouts two or three times on its own.

## Where it lives

The veto is set in `run()` of `crates/app/src/lib.rs`, with `forbid_overwrite`, and `Persistence::save`
in `crates/app/src/session.rs` applies it by returning `SaveFailure::WontOverwrite`. The guard lives in
`scripts/measure-frame.sh`, in the block that runs when `ARRASTRES` is not zero. The line the script
parses is published by `crates/app/src/measure.rs` as «writing the layout to disk (N writes of M
attempts)».
