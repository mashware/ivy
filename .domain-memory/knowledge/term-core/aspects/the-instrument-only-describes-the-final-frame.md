---
id: asp_the-instrument-only-describes-the-final-frame_czdwv1
slug: the-instrument-only-describes-the-final-frame
name: >-
  The report describes the FINAL frame: a window that changes size halfway
  through and comes back leaves a batch identical to a clean one — declared
  without being fixed
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T16:09:26.353Z'
updated_at: '2026-08-17T16:09:26.353Z'
last_verified: '2026-08-17T16:09:26.353Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
  - scripts/measure-frame.sh
symbols:
  - window_size
  - insist_on_the_size
  - header
content_hashes:
  crates/app/src/lib.rs: 'sha256:5303a764d73851d1199719a27ca85c6efac8142650b896676e8be63ee15d187e'
  crates/app/src/measure.rs: 'sha256:c66a2a454bba2d7e6e289ede1b8a1d6e060fcdf40111022e0d4b234291d66daa'
  scripts/measure-frame.sh: 'sha256:3827b1223e7110cde05841466d25a38ccbc6d8fe0ceace88acd29fabf84c4e5f'
tags:
  - measurement
  - F2.2
  - risk
  - declared-without-fixing
feature_id: feat_term-core_nx0g1r
---

## What it does

**A blind spot of the instrument, and it is declared without being fixed, with two measurements
behind it.** The report's header publishes the **final** grid and window size: `window_size` is
overwritten on every frame and the report publishes the last one. So a window that changes size
halfway through a batch and **goes back to where it was** leaves a report **identical** to that of a
clean run, with the spans of the frames of the other grid inside the series. A notice from the
desktop, a monitor being plugged in or a compositor policy are enough. The measurement script
documented it in its header from F2.1 on **without covering it up**.

**Covering it up was tried twice, and both rules fired on the first REAL batch:**

1. **«It had the size it asked for and lost it afterwards»** — a false positive. On GNOME/Wayland
   the first two frames **already have** what was asked for and the compositor sends its own on the
   third, so the normal startup was published as an anomaly.
2. **«It only counts if the frame contributes samples to the report»** — it fired too, and that
   teaches the datum that was missing: **the compositor sends its configuration when the consoles
   are already dumping**. So **every clean run has one frame with another size and its samples
   inside** — one out of about 358.

**Conclusion, and hence the decision not to build it:** telling the normal startup apart from a real
anomaly demands **counting** contaminated frames and setting a **threshold**, and that number cannot
be justified while the scenario —something that resizes the window halfway through— **has never been
observed**. One frame out of 358 is noise; what would have to be caught is a change that **lasts**,
and for that it has to have been seen once.

> **Two general rules it leaves, and they hold outside this case:** when a new guard **fires on the
> first real run**, the fault is usually in the rule and not in the world. And a signal that **does
> not tell the startup apart from the defect** is not a signal.

It is written down in the header of `scripts/measure-frame.sh` under «WHAT IS STILL NOT CHECKED»,
with the two measurements, so that whoever comes back does not try it a third time without new data.

## Where it lives

`crates/app/src/lib.rs` — `window_size`, which is overwritten on every logical pass, and
`insist_on_the_size`, which is where the signal would live; `crates/app/src/measure.rs` —
`header`, which publishes the last value; `scripts/measure-frame.sh` — the declaration. Found by
the integrated review pass of PR #75 of F2.2.
