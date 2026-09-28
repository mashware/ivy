---
id: asp_whoever-does-not-declare-their-work-attributes-it-to-someone-else_6bbts6
slug: whoever-does-not-declare-their-work-attributes-it-to-someone-else
name: >-
  In an instrument split into parts, whoever does not declare their work
  attributes it to someone else: the failure shows up on going from one to two
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-11T16:28:21.609Z'
updated_at: '2026-08-11T16:28:21.609Z'
last_verified: '2026-08-11T16:28:21.609Z'
file_paths:
  - crates/app/src/measure.rs
  - crates/app/src/lib.rs
symbols:
  - 'Timings::close_frame'
  - 'Timings::record_round'
  - 'RestOfFrame::record'
  - 'Window::close_frame'
content_hashes:
  crates/app/src/measure.rs: 'sha256:b55a4357bfb9ee09793b07a352ac7f74bdcfbbd12c25527fe829182eaadcec3c'
  crates/app/src/lib.rs: 'sha256:bcf3ddb3e7b0c70d3d8b306bd58930520777e0baa28afa408bd8eb0b12ccd2ad'
tags:
  - measurement
  - attribution
  - f1.4
  - review
  - d3
feature_id: feat_term-core_nx0g1r
---

## What it does

**The failure, in one sentence:** when an instrument shares out a total between parts and computes
one of them **by subtraction**, any part that does not declare its work gives it away to the one
that is computed by subtracting. And the symptom does not show up with one part — it shows up with
two.

**The concrete case (F1.4, August 2026).** The cost of a frame is shared out like this: each console
times its own (interpreting the bytes and queueing the painting) and «the rest of the frame»
—tessellating, drawing, the framework's scaffolding— comes out of **subtracting** from the CPU total
what they all declared. The framework publishes the total **only once per pass**, so there is no
other way of obtaining it.

The instrument discarded a console's spans **when no output had reached it** in that frame, with a
correct argument: a frame at rest is not a lightning-fast round, it is a round that did not happen,
and putting it in the mean would say that the application responds instantly precisely because it
did nothing. But in discarding the spans, **the queueing, which had happened**, was discarded too:
the still console is painted just the same every frame. That time ended up published as the
framework's cost.

**Why it was invisible.** With **one** console the failure did not exist: if there were no spans,
the whole frame was skipped and no rest was recorded. With **two** it becomes the normal case — the
usual thing is one working and the other at its prompt. And neither of the two tests that covered
the fan-out caught it, because in both **all** the parts had spans. Two reviewers found it
separately, both reading the complete path of the computation.

**The distinction that fixes it, and which is the transportable lesson:** one has to separate
**«which series to record»** from **«which work to declare»**. They are not the same question. A
statistical series can and must reject samples that represent nothing; the declaration of the work
done **cannot reject anything**, because someone is subtracting it. In the code that is visible in
the wait span having become optional: `None` means «there was no round to measure», and yet the
method **still returns what was timed**.

**The rule, for the next time something is computed by subtraction:** before adding a new part to
the fan-out, ask *can this part do work and not declare it?* If the answer is yes, the number
computed by subtraction is inflated and **nobody is going to notice**, because it still adds up to
the total. It is a failure that breaks no visible invariant: the numbers still square with each
other.

**Why it matters here more than elsewhere.** That number computed by subtraction is precisely the
one that decides **D3** — whether the interface framework holds up. Inflating it with our own work
pushes towards changing framework over a cost that is not its own, which is the most expensive
decision of the phase.

## Where it lives

The fan-out lives in `crates/app/src/measure.rs`: `Timings` (a console's spans, with `waited`
optional), `RestOfFrame` (the part that is computed by subtracting) and `Timings::close_frame`,
which **always** returns what was timed even when it records no series. The one that orchestrates is
`Window::close_frame` in `crates/app/src/lib.rs`, which sums that of all the consoles before
subtracting a single time. The test that pins the behavior is
`a_console_with_no_output_declares_all_the_same_what_it_cost_to_paint_it`; the resulting number is
published in the annex «Measured — 11 August 2026» of D3, in `docs/02-decisions.md`.
