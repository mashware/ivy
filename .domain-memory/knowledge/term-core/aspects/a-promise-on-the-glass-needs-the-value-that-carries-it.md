---
id: asp_a-promise-on-the-glass-needs-the-value-that-carries-it_q3p8kz
slug: a-promise-on-the-glass-needs-the-value-that-carries-it
name: >-
  A console has two directions, two threads and two endings — and the sentence
  written from one of them promised for the other
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T22:24:48.322Z'
updated_at: '2026-09-10T22:24:48.322Z'
last_verified: '2026-09-10T22:24:48.322Z'
file_paths:
  - crates/app/src/painter/notices.rs
  - crates/term-core/src/lib.rs
  - docs/02-decisions.md
symbols:
  - STOPPED_RECEIVING
  - AND_STILL_TAKES_WHAT_YOU_TYPE
  - what_it_says
  - Backlog
  - Ending
content_hashes:
  crates/app/src/painter/notices.rs: 'sha256:ad4f8857c2c5e495f094b96f491b54045f30df3e6b45d3a63c7b09a88ea7cee3'
  crates/term-core/src/lib.rs: 'sha256:5af3288f30591cf1465e35c369b5fd72b46a34799967c3c9dc421abb80bb2aaa'
  docs/02-decisions.md: 'sha256:60927be357e2acc44b11b824c1fb0a8a28fb43f098536d2957a1a9ec72219821'
tags:
  - d29
  - d38
  - d41
  - prose
  - review
feature_id: feat_term-core_nx0g1r
---

## What it does

**The notice over a console that stopped receiving said:** *«This console stopped receiving its program's output. The program may still be running, and what you type still reaches it.»* The first clause is what `Ending` knows (**D38**). **The second is the input side's business**, which lives in `Backlog.gave_up` (**D29**) and which no ending of the *reading* can see.

A console can be quiet **and** deaf. Then standard error says «this console can no longer be typed into» while the glass promises delivery — same window, same moment, opposite claims.

It was written into the design's own literal contract, survived every earlier round, and was caught in the last review of the work by a reviewer asking **which document backed each half of the sentence**.

> **A promise on a surface must be traceable to the value that carries it.** In a feature built around «the reason is a value now», every clause has to name which value says it — and a clause about the other direction has no value at all until somebody reads one.

**The fix has a shape worth reusing**: the clause became a second constant appended only where it is true, and the decision was written over the **fact** rather than over the console — `what_it_says(deaf: bool)` — so both wordings can be asserted. A write failure cannot be produced from outside `term-core` any more than a failed read can, so the reading of `gave_up` itself stays unpinned (it was listed in `docs/DEBT.md`, which #150 deleted; `git log -p -- docs/DEBT.md` recovers it).

**This was the third blocker of one delivery that was a claim about somebody else's half**, which is why it is a card and not a line in a review: a false Windows risk published in the design (the pseudoterminal library maps a broken pipe to end of file on both platforms), and «a clip takes the tail, never the answer» published as the reason a **product** question was decided when nothing clipped at all.

## Where it lives

`crates/app/src/painter/notices.rs` holds both sentences and `what_it_says`; `Backlog` and `Ending` are in `crates/term-core/src/lib.rs`. The product decision is **D41** in `docs/02-decisions.md`, and the ceiling the notice lives under is **D39**.
