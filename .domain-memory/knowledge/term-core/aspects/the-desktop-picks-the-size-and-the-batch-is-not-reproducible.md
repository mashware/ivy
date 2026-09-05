---
id: asp_the-desktop-picks-the-size-and-the-batch-is-not-reproducible_cgyn1y
slug: the-desktop-picks-the-size-and-the-batch-is-not-reproducible
name: >-
  The size of the window has to be asked for AND insisted on: the compositor
  sends its own on the third frame — SOLVED in F2.2
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T12:02:19.548Z'
updated_at: '2026-08-17T16:07:58.498Z'
last_verified: '2026-08-17T16:07:58.498Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
  - scripts/measure-frame.sh
symbols:
  - run
  - window_size
  - RequestedSize
  - insist_on_the_size
  - SIZE_TOLERANCE
content_hashes:
  crates/app/src/lib.rs: 'sha256:5303a764d73851d1199719a27ca85c6efac8142650b896676e8be63ee15d187e'
  crates/app/src/measure.rs: 'sha256:c66a2a454bba2d7e6e289ede1b8a1d6e060fcdf40111022e0d4b234291d66daa'
  scripts/measure-frame.sh: 'sha256:3827b1223e7110cde05841466d25a38ccbc6d8fe0ceace88acd29fabf84c4e5f'
tags:
  - measurement
  - F2.2
  - reproducibility
  - solved
  - wayland
feature_id: feat_term-core_nx0g1r
---

## What it does

> **Status (17 August 2026): SOLVED** in PR #75 of F2.2. It is kept whole because the cause was not
> the one it looked like and the mechanism of the fix **is not obvious**: asking for the size once is
> not enough.

**Every figure in D3's «Measured» annex depends on a number the project did not control: the grid.**
The cost of a frame is above all **parsing** and **queueing the painting**, and both things are
proportional to the cells there are. The grid comes out of the window's size, and `run()`
opened with `eframe::NativeOptions::default()`, which **never asks for a size**: the desktop chose
it.

> **The symptom, measured on 17 August 2026 on the same machine and the same day: three runs of
> batch A gave three grids.** 61×57 in the early hours, **36×47** on a working branch and **40×66**
> on `main`, measured four minutes later in a separate worktree with its own release binary. The last
> one is the one that closes it: **it cannot be the code's**, because it did not have inside it a
> single line of the change that was being validated.

### The cause, probed frame by frame

**Asking for the size when BUILDING the window does get applied — and it lasts two frames.** With
`MEASURE_SIZE=800x600` on GNOME/Wayland:

| Frame | Size of the window |
|---|---|
| 0 and 1 | **800×600** — what was asked for, applied |
| 2 | **1126×661** — the compositor sends its own configuration |
| 3 and onwards | **800×600** — after asking for it again with `ViewportCommand::InnerSize` |

That is where the two numbers of the symptom come from: **36×47 was a window of 800×600** (winit's
default size) and **40×66 the compositor's**. There was nothing random about it; there were two
different sizes depending on who got the last word.

**Asking for it again afterwards IS respected**, and it stays. That is the whole fix: `RequestedSize`
keeps what was asked for and `insist_on_the_size` asks for it again on every frame in which the
window does not have it.

### Three things about the fix that are not obvious

1. **The insisting runs out, and the cap counts FRAMES, not answers.** A resize on Wayland needs a
   configuration round from the compositor, that is, at least one frame, and at startup the frames
   come one after another: with a cap of four, a compositor that takes five to answer —a slower
   machine, a busier session— **would use up the attempts before the first answer** and the window
   would not ask for anything again, throwing away every batch from that machine. It is **32**. And
   the cap has to exist: without it, a compositor that does not give in turns this into a fight per
   frame, and every attempt asks for another frame — the measurement would be measuring itself
   fighting.
2. **The frames that already have the right size do not spend an attempt.** The first two already
   have it, and spending them there leaves the window with nothing to answer with when the
   compositor's configuration arrives.
3. **The compositor hands the size over with a fraction and NOT always the same one**: with the same
   1280×800 asked for, two runs gave 1280×800 and a third **1279.8125×799.8125**. So all the
   comparisons —the application's, so as not to get into a fight, and the measurement script's two—
   carry **half a point of margin**, with `<=` in both places. An exact comparison throws away
   identical runs, and since the fraction is not deterministic, taking them again may fail once more.

### And the report publishes the size it HAD

It is the **only line of the header that can be asked for again**: the grid in cells comes out of
the size **and** of the font's metric, so reading it does not say what window the batch was taken
with. The annexes before this one were measured with windows whose size nobody wrote down, and that
is why **they can neither be reproduced nor compared with the ones after**.

> **The rule, which still stands even though the case is closed: an instrument whose scale is set by
> a third party is not reproducible, even if it publishes its number.** And the variant this case
> adds: **asking the desktop for something is not the same as having it** — you have to check that it
> was met, and publish what there was.

**A field datum worth keeping:** if more is asked for than fits, this desktop grants up to
**5120×1357 points** — checked by asking for 4000×3000 and seeing what it hands over.

## Where it lives

`crates/app/src/lib.rs` — `SIZE_VARIABLE` (`MEASURE_SIZE`), `window_size`,
`RequestedSize`, `FRAMES_INSISTING_ON_SIZE`, `SIZE_TOLERANCE` and `insist_on_the_size`;
`crates/app/src/measure.rs` — the `ventana:` line of the header; `scripts/measure-frame.sh` — the
reference size, the per-run guard and the `comparar` mode. Discovered while validating PR #74 and
solved in #75, both of F2.2.
