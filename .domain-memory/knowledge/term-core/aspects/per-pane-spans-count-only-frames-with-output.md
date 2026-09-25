---
id: asp_per-pane-spans-count-only-frames-with-output_q25qc8
slug: per-pane-spans-count-only-frames-with-output
name: >-
  The report's per-pane series count only frames with console output, so a
  per-event cost cannot be derived from them
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T15:18:55.282Z'
updated_at: '2026-09-25T15:18:55.282Z'
last_verified: '2026-09-25T15:18:55.282Z'
file_paths:
  - crates/app/src/measure.rs
  - docs/02-decisiones.md
symbols:
  - close_frame
  - record_round
content_hashes:
  crates/app/src/measure.rs: 'sha256:6feea1edd4c721d24de2ef333587ace2fe1d7a16caa4870041e873ce4b7e645c'
  docs/02-decisiones.md: 'sha256:d2dcb9abf55336e4eec456ec7bdf9faf9da3aadc5306c5f699b6fc7bbdf5fa83'
tags:
  - measurement-bench
  - d13
  - d47
  - d49
feature_id: feat_term-core_nx0g1r
---

## What it does

The per-pane series of the frame report — waiting, interpreting, enqueueing, the status panel, the buttons and **the control channel** — are recorded only in frames that carried console output: `Timings::close_frame` records them inside `if let Some(waited) = spans.waited`, and returns the declarable time either way so the subtraction from «the rest of the frame» stays right. That is correct for a latency series (a frame at rest is a round trip that did not happen). It also means **work done in a frame without output is subtracted and averaged nowhere**.

So a per-event cost — a publication, a patch, a read — cannot be derived as `(mean × frames with output) ÷ events`: any event that landed in a quiet frame is missing and the report does not say how many did. Measured in #248: batch Y ran about 510 frames for 358 with output. The 95th percentile does not help either: at ~60 fps with one event per second per console, it lands in frames that drained nothing.

**And two runs of a channel batch do not publish the same number of documents** (X applied 28 and 44 in two runs, Y 56 and 56), so the per-frame means of a pair carry different loads even when `compare` says the windows match.

**What to do instead:** quote the bench per frame, and time the verb itself in a release-mode ignored test with the bench's own documents and ops — D49 did it for reads (`measure_off_frame_document_reads`), D47 for patches (`measure_a_patch_at_the_ceiling`: 4.1 µs at 456 B, 1.82 ms at 249 KB). Say what the test includes that the frame pays elsewhere (it drops the replaced panel inside the loop) and what it leaves out (the re-layout, which lands in the panel's span).

## Where it lives

`crates/app/src/measure.rs` — `close_frame`; the D47 block «Measured at the ceiling» in `docs/02-decisiones.md`.
