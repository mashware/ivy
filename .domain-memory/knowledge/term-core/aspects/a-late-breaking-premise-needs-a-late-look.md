---
id: asp_a-late-breaking-premise-needs-a-late-look_esnfy5
slug: a-late-breaking-premise-needs-a-late-look
name: >-
  When a test's premise can be broken a moment AFTER the action, looking at once
  makes the mutation pass: look after a pause, or the defect shows only as a
  rare flake
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T20:14:28.810Z'
updated_at: '2026-09-26T20:14:28.810Z'
last_verified: '2026-09-26T20:14:28.810Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - close_one_that_refuses
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:598307ef482b8847d279ad556b074bb929e86396d7284e83bd995094bd17b1e8'
tags:
  - test-disguise
  - mutation
  - flaky
  - race
feature_id: feat_term-core_nx0g1r
---

## What it does

**Test disguise met on #288 (26 September 2026).** The D56 test closed a console and at once asked `kill(pid, 0)` whether the program was alive. What broke the premise — the kernel's hang-up on the last master close, which can come from a never-joined thread after the close returns — lands about a millisecond later. Looking at once, the broken premise passed on almost every run: removing the premise (the program's `trap '' HUP`) was a mutation that stayed **green**, and the defect surfaced once as a flake in a loaded full-workspace run.

**The fix is a pause before the look** (200 ms in `close_one_that_refuses`): with it, the same mutation fails **every** run. A 300 ms pause was the reproduction that made the flake deterministic in the first place.

**The general rule**: when an assertion is «X is still true after the action», list what could make X false *after* the action returns (threads not joined, kernel work triggered by a last close, a child dying) and look after that window, not inside it. A zero delay is the number where the defect and the fix give the same result — this project's second classic disguise, in time instead of in magnitude.

## Where it lives

`close_one_that_refuses` in crates/term-core/src/lib.rs (the 200 ms pause and its comment)
