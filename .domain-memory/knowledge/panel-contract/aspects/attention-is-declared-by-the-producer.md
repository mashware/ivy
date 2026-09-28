---
id: asp_attention-is-declared-by-the-producer_89mxc6
slug: attention-is-declared-by-the-producer
name: >-
  Knowing whether a pane claims you needs a declared field: deducing it from a
  `mark: block` in the body breaks D6 from the inside
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T08:26:45.811Z'
updated_at: '2026-08-24T08:26:45.811Z'
last_verified: '2026-08-24T08:26:45.811Z'
file_paths:
  - docs/02-decisions.md
  - docs/04-panel-contract.md
  - docs/05-roadmap.md
symbols: []
content_hashes:
  docs/02-decisions.md: 'sha256:97753287aa018f84f87473e2872f7347a4aacb199a69cc2007289f3c34ee1475'
  docs/04-panel-contract.md: 'sha256:95629fb310672544479a213c3275547f03e137cc6cdad1da3920c8076c83eba9'
  docs/05-roadmap.md: 'sha256:d6b3727577809ebf4b11c0171d1beba0910ec2b86610b88e3ae5a294cb66b583'
tags:
  - contract
  - attention
  - d18
  - d6
  - navigation
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The gap nobody had seen in three documents at once.** The contract has known how to express «waiting» and «blocked» from the start — `mark: wait`, `mark: block` — but those are **lines of the body**, and the body is free by design. To paint a tab's indicator or fire a notification out of that, the app would have to **read the body and decide what it means**: that a line with `block` in the middle of a summary means that the whole pane claims something.

> **That is D6 broken from the inside, which is the worst way to break it: without any document saying it was broken.** And it would be fragile in the silliest way — a historical line («I waited for approval on Tuesday», with `mark: wait`) would mark the tab forever.

**The decision (D18):** optional field `attention` with three values, written by the producer. `wait` = **it is waiting for you** (a question, a permission, a decision); `block` = stuck on something else, not you; `done` = it finished and nobody looked. Absent = it claims nothing. **It accumulates pane → tab**, with `wait` beating `block` and `block` beating `done`: `wait` goes first because it is the only thing that gets unstuck with you in front of it right now.

**Why it does not contradict the free-body rule.** It names neither a metric nor domain vocabulary: it names **the only question the app needs in order to navigate** — do I have to go there? There are still no fields for cost, context or progress, and `phase` is still free text.

**And keeping quiet is keeping quiet.** Without the field the pane is not marked, and the app does **not** deduce it from the body, nor from `phase`, nor from whether the console writes. There is no deduced default value.

**What was discarded, and the first one is the important one:** an enum of work state along the lines of `idle`/`working`/`blocked` —what Herdr, Xirp, Superset, Warp and Orca have— describes **what the program is doing**, and that is either deduced by the app or declared by a harness that does not know the question (D6, «level 2 reinterpreted fails the same way as guessing»). `attention` does not ask what it is doing: it asks whether it claims you. Also discarded: a numeric urgency (it would force every producer to calibrate against panels it does not see) and having the app fire the system notification when a `wait` appears — that is left for when the field has producers, because **a false interruption costs much more than a false indicator**.

**And the competitive fact that made it urgent:** the claim that «no tool orders tasks by what claims you», written as a Phase 4 differentiator, **stopped being true in August 2026**. Herdr accumulates agent → panel → tab → workspace and orders by who needs attention. What still nobody has is not the **ordering**: it is the **body**.

## Where it lives

`docs/02-decisions.md` → **D18**. `docs/04-panel-contract.md` → Part I, the `attention` field and its table. `docs/05-roadmap.md` → milestone **F2.8**, brought forward ahead of F2.6 and F2.7, and the correction of the Phase 4 sentence. See also [[the-transport-is-not-the-contract]].
