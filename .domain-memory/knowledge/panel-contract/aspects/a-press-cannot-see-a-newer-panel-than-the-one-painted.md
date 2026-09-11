---
id: asp_a-press-cannot-see-a-newer-panel-than-the-one-painted_r40ked
slug: a-press-cannot-see-a-newer-panel-than-the-one-painted
name: >-
  A press cannot land on a panel newer than the one painted — and the change
  that breaks it is draining a new transport EARLY, which reads like the careful
  option
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:21:38.146Z'
updated_at: '2026-09-11T20:21:38.146Z'
last_verified: '2026-09-11T20:21:38.146Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
symbols:
  - refresh_its_panel
  - handle_the_mouse
  - answer_a_clicked_question
content_hashes:
  crates/app/src/painter.rs: 'sha256:513ecc9b307d4ea57309d81df9e173ea4065954165363487664a6d14a608625e'
  crates/app/src/lib.rs: 'sha256:171c1421de9c4a6d0646eb0c98664b64626350b32415c7e52c7c788d0838ed09'
  crates/app/src/pointer.rs: 'sha256:519b2f15603bb3a173795911756dffec61265f3377a282b8cc054f883f6aa4a8'
tags:
  - frame-order
  - panel
  - gestures
  - phase-3
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Today a press cannot land on a panel newer than the one that was painted, and that property is load-bearing without being written down anywhere.** Verified in the tree, not reasoned: the panel is re-read **inside the painting** (`crates/app/src/painter.rs:815` calls `pane.refresh_its_panel()`), and the mouse is attended **after** it, in the same interface pass (`crates/app/src/lib.rs:3442`, `let of_the_mouse = self.handle_the_mouse(...)`). The only other production call site is `lib.rs:2669`, which skips the active tab.

`answer_a_clicked_question` relies on exactly this: it explicitly accepts measuring a press against the previous frame's box (`crates/app/src/pointer.rs:760-768`, «left as it is, on purpose»), and that is only safe because nothing can have repainted a *different* document underneath it first.

> **So the dangerous change is not adding a new inbound transport — it is draining it early.** A design that applies incoming documents in `logic`, before the interface pass, is the one thing that puts a new document between the paint and the press.

**And it is the option that looks careful.** 226's approach panel proposed exactly that, as «one door, one span»: a single apply point in the frame, under a declared span, with a bounded queue — every instinct a reviewer has says yes. A challenger caught that it **creates** the window the same design then had to build a mechanism to survive. Apply incoming state where the file's refresh already happens.

What still remains across frames — frame N paints document A, frame N+1 refreshes to B and paints it, and the press arrives after that paint — is real, and is what an identity check has to cover. See [[the-panel-layout-cache-is-not-an-identity]].

## Where it lives

`crates/app/src/painter.rs:815` (the refresh, inside the painting) against `crates/app/src/lib.rs:3442` (`handle_the_mouse`, after it). The acceptance of the one-frame gap is written at `crates/app/src/pointer.rs:760-768`.
