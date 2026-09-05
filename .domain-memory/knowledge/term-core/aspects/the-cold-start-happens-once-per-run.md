---
id: asp_the-cold-start-happens-once-per-run_gjzzwd
slug: the-cold-start-happens-once-per-run
name: >-
  A «cold» number is not averaged within a run: there is ONE sample, and its
  spread is only seen by repeating the run
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:58:59.656Z'
updated_at: '2026-08-25T13:58:59.656Z'
last_verified: '2026-08-25T13:58:59.656Z'
file_paths:
  - crates/app/examples/fonts.rs
  - docs/02-decisiones.md
  - scripts/measure-frame.sh
symbols:
  - summary
  - SAMPLES
content_hashes:
  docs/02-decisiones.md: 'sha256:0de8ae72f3ac308cb5cc9ba4d96b4b7d71443f38da0b0ca87bafa69318fc44ce'
  scripts/measure-frame.sh: 'sha256:b41b8d91a5fb47b233a78a5da664f8d0563c96031a5d9556d93510866c9663ab'
tags:
  - measurement
  - benches
  - F2.9
  - D24
feature_id: feat_term-core_nx0g1r
---

## What it does

**A bench that takes N samples per point does NOT bound its cold number, and publishing it as if it
did is the error this project punishes.** The cold **happens once per run**: sample 1 fills the
caches —of files, of lookup by script— and the rest already find them full. So within a run **there
is nothing cold to average**: there is **one** sample, with the whole system's variance on top of it.

**And it tends to be precisely the one that decides.** In **D24** the number that holds the decision
up —registering a face with the window already running— is **always paid cold**, once per new script.

### Measured

Three runs of the same binary **with the machine at rest** gave **3.4 / 4.8 / 4.5 ms** for that
point. The first version of D24 published **3.2 ms** as *the* value: **below all three**. Another
number —enumerating hot, 13.7 ms— **has not been seen again in four runs** (15.07 · 15.10 · 15.17 ·
15.12).

**The contrast is the datum:** the **hot** column varies by less than **1 %** between runs and the
**cold** one by as much as **40 %**. What varies is **opening files**, not walking lists.

### The rule it leaves

1. **The cold column is published as a RANGE of runs**, not as a value. And how many runs hold it up
   is stated.
2. **Raising the number of samples does not improve the cold** — the second sample already finds the
   caches full. The only thing that improves it is **repeating the run**.
3. **The bench's header has to say what each column is.** The font bench's said «median of 5
   samples, with the extremes in parentheses» when the cold one was not a median and no extreme was
   printed. It is the first line whoever repeats the measurement reads.
4. **And it has to be said whether the decision holds over the whole range.** In D24 it does: the
   worst case of the lazy route (5.7 ms) against the **best** one of the startup route (23.5 ms) is
   still more than four times, so no combination of the extremes turns it around.

### Corollary for any measurement in this repository

**A run with the machine compiling in parallel is no good.** The same point gave **5.08 ms under
load** against 3.4–4.8 at rest. You have to wait until neither `cargo` nor `rustc` is left alive
before timing anything.

## Where it lives

The bench that uncovered it is `crates/app/examples/fonts.rs` —its `summary` function is the one
that separates the cold sample from the median of the hot ones, and it has its two tests—. The
published ranges are in **D24** of `docs/02-decisiones.md`. For the frame batches, the script is
`scripts/measure-frame.sh`.
