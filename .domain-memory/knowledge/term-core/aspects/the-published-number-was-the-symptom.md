---
id: asp_the-published-number-was-the-symptom_qwc6j2
slug: the-published-number-was-the-symptom
name: >-
  The number that would have shown the hang was published for a month and nobody
  read it as a symptom — and then the fix moved that span without moving its
  name
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T15:16:51.376Z'
updated_at: '2026-09-06T15:16:51.376Z'
last_verified: '2026-09-06T15:16:51.376Z'
file_paths:
  - crates/app/src/measure.rs
  - crates/app/src/lib.rs
  - docs/02-decisiones.md
symbols:
  - record_outbound
  - record_mouse_outbound
content_hashes:
  crates/app/src/measure.rs: 'sha256:898abb44e8a03d68ea655fa4407688345cdcb919c4911a656f706c25ff25c730'
  crates/app/src/lib.rs: 'sha256:16cf7238d33ef681a4275bd08787a1c153e6e8f5ab1c54e2d30cd5157e62f4f4'
  docs/02-decisiones.md: 'sha256:b76cc17fbac2d55b9f81f75cd1d31295e188704e9dc21642114375042b66e8c9'
tags:
  - measurement
  - D3
  - D23
  - D29
  - mislabelled-number
feature_id: feat_term-core_nx0g1r
---

## What it does

**D3 publishes the span *from a key in the frame to the byte in the program*: 36 µs at rest and 3.42 ms with the two consoles dumping.** That second figure existed **because the write waited for the program to accept the bytes**. It was not noise and it was not the framework: it was the hang, at a scale small enough to pass for a cost.

The bug was found by reading an issue. The report had been measuring it since August.

> **A number that is expected to be large is not read as a question.** «It costs more while dumping» sounds like a fact about load; it was a fact about waiting for somebody else. What would have caught it is asking, of a span that grows with someone else's behaviour, **what exactly it is waiting for**.

### And the fix moved the span without moving its name, which is the second half

Once the write is handed to a thread, the same stopwatch stops at **a channel send**: the byte reaches the program afterwards, on a thread nobody times. Same name, shorter stretch — D3's 36 µs and 3.42 ms, and D23's 5.1–5.3 µs per mouse gesture, all describe something narrower than they used to. **Three reviewers found it independently**, and one put it under the project's own hard rule: *a mislabelled number is worse than no number*.

**The trap generalises: when a fix moves *where the waiting happens*, every span whose name mentions the far end has to be re-read.**

### And the comparison of before against after is where it bites hardest

Measuring the mouse path on both sides gave **5.5–6.0 µs against 8.2–9.1** — four runs, alternating the branch and a `main` built minutes earlier, ranges not overlapping. That is real and it is what mattered: **the thread that paints does less work**. But it is **not «the same span falling»**, and publishing it that way is the mislabelled number walking in through the front door. It was published that way for about an hour, in this work's own validation artifact, before the review caught it.

Sibling of [[the-stopwatch-line-goes-where-the-effect-ends]]: that one is about where the line **goes**, this one about what happens to the name when the line **moves**. And of [[a-cited-number-is-copied-from-the-docblock-next-door]], which is the same rule failing on the prose that cites a number rather than on the number.

## Where it lives

`crates/app/src/measure.rs` — `record_outbound`, `record_mouse_outbound` and the two report lines they feed. The published figures are in **D3** and **D23**; **D29** records that they moved. `docs/02-decisiones.md`.
