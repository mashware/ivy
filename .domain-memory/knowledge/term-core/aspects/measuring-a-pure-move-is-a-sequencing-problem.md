---
id: asp_measuring-a-pure-move-is-a-sequencing-problem_9h6c5r
slug: measuring-a-pure-move-is-a-sequencing-problem
name: >-
  Measuring a pure move is a sequencing problem before it is a numbers problem:
  interleave the two trees, and know what else is in the reports directory
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T15:52:45.998Z'
updated_at: '2026-09-07T15:52:45.998Z'
last_verified: '2026-09-07T15:52:45.998Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/report.rs
  - docs/02-decisions.md
symbols:
  - publish_the_report
  - close_the_unpainted_rounds
content_hashes:
  scripts/measure-frame.sh: 'sha256:ae5f21bf87adf36350d78dad4ef3cc4970d0620f8506024e51a596dc48c8d112'
  docs/02-decisions.md: 'sha256:72862e89835bb5f95229c18221eaa6bc0f712c24fc943eb2122a6f45d696fa6c'
tags:
  - measurement
  - method
  - frame-cost
  - code-move
  - d28
  - d3
feature_id: feat_term-core_nx0g1r
---

## What it does

**A code move has no expected effect, so the whole measurement is about not publishing the machine's noise as a result.** Three things came out of measuring the third delivery of the `lib.rs` split (September 2026, batch `C`, four runs a side), and all three are about *how the runs are sequenced and read*, not about the figures.

### 1. Interleave the two trees. Do not run a batch per side.

Run branch, control, branch, control — never four of one then four of the other. **A machine that gets busy halfway through lands entirely on one side**, and then the contamination is indistinguishable from a real difference.

This is not hypothetical: the delivery before this one ran a tanda per side and **four of its seven control runs came back contaminated with no branch twin** — frame cost at 643–726 µs against a normal 360–441 — and its own validation had to say the comparison rested on three runs instead of seven, and that the asymmetry could not be explained from the timestamps. Interleaving is what makes that asymmetry impossible by construction.

[[two-annexes-from-different-days-are-not-compared]] gives the rule one level up — never compare two annexes from different days, alternate the variable over the same binary. This is the next step down: **alternate the *runs*, not just the binary.**

### 2. The reports directory accumulates, and a glob mixes sessions in silence

`target/measurements/` keeps every report ever taken in that tree. Globbing `batch-C-*.txt` to build the comparison **silently picks up somebody else's session**.

Measured, and it is not a small effect: on the same day, in the same tree, on the same branch, four `batch-C` reports from 13:20 sat at **406–430 µs** while the four taken at 17:12 sat at **283–409**. Averaging all eight compares *machine states*, not trees. **Select the runs by the timestamp of the session that took them**, not by the batch's name.

### 3. «It touches no measured span» is read from the call sites, never declared

The design of this same train wrote off its first delivery as a non-measurement on that reason and **the reason was false** — `Node::place` calls `tile` on every frame. It was about to be false a second time here: of the two methods that moved, `publish_the_report` runs once in `on_exit` and is inside no span, but **`close_the_unpainted_rounds` runs at the end of the interface pass, on every frame that paints** — so the delivery does touch «the rest of the frame», the figure **D3** judges the framework by.

One method out of two was enough to make the delivery measurable. Reading the two call sites is all it took to find out.

### The corollary, which is what a table cannot say about itself

**A batch can walk the moved code and give it nothing to do.** Batch `C` has one tab, so no pane is ever unpainted: the moved sweep iterates its two panes and takes nothing. It measures the per-frame *walk*, which is what a boundary crossing can change — and it measures **none of the closing**. That has to be declared next to the table, or the next reader takes the green as coverage of the whole method. Here the batch with real subjects is `G` (two tabs, the background one dumping), which was the *next* delivery's stated batch, so the train covers it; saying so is the difference between a gap and a schedule.

### And the verdict that keeps these tables believable

Four runs a side gave a median of 305.1 µs on `main` against 281.5 on the branch — and each side's own spread (97 and 126 µs) is four to five times the 23.6 µs gap. **No measurable change**, no improvement claimed, and the one contaminated control run left *in* the table rather than dropped: dropping the single run that disagrees is how a measurement stops being one.

## Where it lives

`scripts/measure-frame.sh` takes the batches and writes each report into `<tree>/target/measurements/`; its `compare` mode only says whether two reports describe the same window, so **it does not protect against either mistake above** — picking the runs and ordering them is the caller's job. The moved code is `crates/app/src/report.rs` (`publish_the_report`, `close_the_unpainted_rounds`). The measurement of the train's first delivery, with the same «the reason was false» finding, is the annex of **D28** in `docs/02-decisions.md`; the span the verdict is about is the one **D3** judges egui by.
