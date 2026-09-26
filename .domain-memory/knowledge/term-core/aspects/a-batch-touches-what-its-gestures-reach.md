---
id: asp_a-batch-touches-what-its-gestures-reach_qgzv2y
slug: a-batch-touches-what-its-gestures-reach
name: >-
  What a measurement batch exercises is decided by what its gestures DO, not by
  what the script's text says — grepping the script is how a false premise
  reaches a decision record
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T17:09:26.559Z'
updated_at: '2026-09-09T17:09:26.559Z'
last_verified: '2026-09-09T17:09:26.559Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/pointer.rs
  - docs/02-decisiones.md
symbols:
  - 'Window::handle_the_mouse'
  - 'Tab::focus'
content_hashes:
  scripts/measure-frame.sh: 'sha256:ae5f21bf87adf36350d78dad4ef3cc4970d0620f8506024e51a596dc48c8d112'
  crates/app/src/pointer.rs: 'sha256:1e956a5d4794d7e182236be288b3231dd7aa3b92304a91df977ac1e81d555c95'
  docs/02-decisiones.md: 'sha256:e7f276490256e6c54c6bf249aa1729ba670046841ab85b4dade0182ae0e1fcf8'
tags:
  - measurement
  - batches
  - decision-records
  - prose
feature_id: feat_term-core_nx0g1r
---

## What it does

**The case, and it reached a decision record before a reviewer caught it.** The delivery that told the focused console's cursor apart (**D36**, September 2026) needed to say whether any measurement batch exercised the focus. It answered with `grep -n focus /absolute/path/scripts/measure-frame.sh`, which returns **nothing**, and published «no batch of `scripts/measure-frame.sh` touches the focus».

**It is false.** A press moves the focus — `crates/app/src/pointer.rs`, `if response.clicked() || response.drag_started() { self.active_mut().focus = name; }` — so **every batch that injects a press moves it**: `J` (dragging panes), `K` (the panel's border), `M` (the tree's border), `N` (marking text) and `O` (the mouse handed to the program). The delivery's own new test, which asserts that a click moves the filled cursor **in the frame of the click**, proves it three files away from the sentence that denied it.

> **The rule: to know whether a batch exercises a mechanism, walk from the batch's INJECTED GESTURES to the code they reach — never from the mechanism's name to the script's text.** The script names loads, tabs, panes and gestures. It does not name the *state* those gestures change, and it never will: that is what the application is for. Anything a press, a release, a drag or a key can reach is reachable from `J`, `K`, `M`, `N`, `O` and the paste batches whether or not the script ever says its name.

**Why it matters more here than the mistake looks.** The conclusion happened to survive — what the argument actually needed was that **exactly one console has the focus in every run**, which is true before and after — so nothing downstream was wrong. But the reasoning published to defend it was, and this project's own rule is that **a mislabelled number is worse than no number**. A decision record is read years later by somebody who cannot re-derive it; a premise that was never checked is indistinguishable from one that was.

**The cheap check that would have caught it**: `grep -n 'focus' /absolute/path/crates/app/src/pointer.rs` — one file, the router, where every gesture ends up. Two minutes against a sentence that would have stood for years.

## Where it lives

`scripts/measure-frame.sh` and the batch table in `CLAUDE.md`; the gesture that moves the focus is in `crates/app/src/pointer.rs`. The corrected paragraph is the «What is not measured, and why no batch changes» block of **D36** in `docs/02-decisiones.md`, which now names the five batches that do move it and says where the wrong premise came from — kept rather than edited out, for the same reason [[two-annexes-from-different-days-are-not-compared]] keeps its own.

Found by a comment auditor reading the decision record against the code during the review of PR #212, in a delivery whose author had already run fourteen mutations against the painting and never once questioned the sentence about the bench.
