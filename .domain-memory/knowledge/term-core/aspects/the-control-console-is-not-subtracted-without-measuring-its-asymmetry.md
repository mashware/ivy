---
id: asp_the-control-console-is-not-subtracted-without-measuring-its-asymmetry_2c2aw2
slug: the-control-console-is-not-subtracted-without-measuring-its-asymmetry
name: >-
  Subtracting the control console from the console with the gesture does NOT
  give the cost of the gesture: the two already differ with no gesture at all,
  and that subtraction has already published two false numbers
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T07:36:53.108Z'
updated_at: '2026-08-25T07:36:53.108Z'
last_verified: '2026-08-25T07:36:53.108Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/measure.rs
  - docs/02-decisions.md
symbols: []
content_hashes:
  scripts/measure-frame.sh: 'sha256:96a1e649a130812d99a0d5a88ec31fa3ecb1403745a9d5a00fd270dc962dc043'
  crates/app/src/measure.rs: 'sha256:5c0a49ba86ceeeb28966cab5ea4899a9e7ddfbcd858788896af1912796621d8b'
  docs/02-decisions.md: 'sha256:fc611d2106ed7c61fbf2afacb79dd2ed3c5987f805d981a1861a3d90883c58c3'
tags:
  - measurement
  - instrument
  - batches
  - d22
  - d21
  - f2.6
feature_id: feat_term-core_nx0g1r
---

## What it does

**The instrument's pattern, in one sentence: a control console INSIDE the same run fixes the problem
of comparing between runs, and does not fix the one of comparing between consoles.**

Several batches of the script (`K` the panel, `M` the tree's divider, `N` marking text) do the
gesture in the left console and leave the right one still, as a control. It is a real improvement
over comparing two different runs —see [[two-annexes-from-different-days-are-not-compared]]—: the
binary, the machine and the moment are the same. The temptation that follows is obvious and is where
the trap lies: **subtracting the control one from the gesture one and publishing the difference as
«what the gesture costs»**.

**It is not, because the asymmetry exists with no gesture at all.** The still batch already gives
**32 and 41 µs** between left and right, without anybody touching anything. That baseline difference
did not appear in the annex's table, so the published number swallowed it whole.

**It has already produced two false numbers published in the decisions record:**

1. **D21** — the doubling of the full round attributed to the scrollback. Full autopsy in
   [[two-annexes-from-different-days-are-not-compared]].
2. **D22** — «marking costs ~100 µs», from subtracting 171 from 273 in the same run. Corrected to a
   **range of 50–60 µs declared as NOT isolated**, with **both** possible sums published (62 and 53
   µs) and the fact that **they do not agree** stated in the annex itself.

> **And the cause attributed to it was false too, which is the second half of the finding.** The
> annex said «twenty extra background rectangles when painting», that is, a cost proportional to what
> is marked. What is real is that **with a live selection every cell of the grid is asked** whether
> it goes highlighted: 1 911 in a console of 49×39, whether two are marked or two thousand. The cost
> **does not grow with what is marked**: it grows with the grid and with the number of consoles.
> Arithmetic that contradicted itself inside the annex itself, and even so it was published.

**What it would take to isolate it for real**, and it is written in D22 instead of being faked: run
the batch **twice with the same binary**, once with the gesture and once without it, and compare
**the same console with itself**. Until that is run, the honest thing is to publish the range and to
say that it is not isolated.

**And there is a third bias that stops anyone averaging lightly**: the runs do not share a regime.
The batches with an injected gesture run **many more frames** —1 066 and 2 640 against 429 and 470
of the still one—, because injecting input keeps the window awake just as a human mouse does. Their
means per frame are not compared with those of a still batch.

## Where it lives

The fan-out by subtraction is in `crates/app/src/measure.rs`; the batches with a control console
(`K`, `M`, `N`) and their guards, in `scripts/measure-frame.sh`. The two annexes affected are those
of **D21** and **D22** in `docs/02-decisions.md`, both already rewritten. It is the same family of
error as [[whoever-does-not-declare-their-work-attributes-it-to-someone-else]] and
[[measuring-something-that-changes-the-grid-is-not-a-subtraction]]: the instrument shares out by
subtraction, and every difference that is not declared is kept by the term that is worked out by
subtracting.
