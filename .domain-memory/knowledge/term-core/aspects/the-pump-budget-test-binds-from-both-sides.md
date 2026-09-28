---
id: asp_the-pump-budget-test-binds-from-both-sides_63y28v
slug: the-pump-budget-test-binds-from-both-sides
name: >-
  A test of the per-console pump budget has to bind it from BOTH sides: `<=`
  lets a lowered or shared budget through, and a quiet console binds nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T23:23:54.540Z'
updated_at: '2026-09-28T23:23:54.540Z'
last_verified: '2026-09-28T23:23:54.540Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - PUMP_BUDGET
  - 'Pane::pump'
  - a_dump_in_one_console_neither_stalls_nor_dirties_the_other
content_hashes:
  crates/app/src/lib.rs: 'sha256:efe916712c9ee4891dd3eb7496dd4753e955ad8f0007ef8926380e144fe5cefe'
tags:
  - test-disguise
  - mutation
  - D13
feature_id: feat_term-core_nx0g1r
---

## What it does

**Checking only `received.len() <= PUMP_BUDGET` is half a test, twice over, and both halves kept the suite green.**

1. **A console that prints a short word stays under ANY ceiling**, so its half of the loop binds nothing. The dump test had `printf quiet` beside a 400 KB dump and asserted the bound for both: mutating `Pane::pump` to `PUMP_BUDGET * 2` with the check restricted to that console **passed**. Every console whose bound is claimed needs more than the budget queued — `wait_for_it_to_die` guarantees the whole output already sits in the queue.
2. **Even with both dumping, `<=` lets a LOWERED or SPLIT budget through** (`PUMP_BUDGET / 2`, `/ panes`). A console that takes less than its program writes ends up stopping it, not slowing it — the reason D13 never lowers the budget ([[the-budget-is-not-lowered-for-a-background-one]]). With the queue longer than the budget, `Pane::pump` ends with **exactly** `PUMP_BUDGET` in `received`, so `assert_eq!(received.len(), PUMP_BUDGET)` per console is deterministic and catches a budget lowered at the pickup: ÷2 fails with 65 536 against 131 072. The sibling single-console test (`a_dump_bigger_than_the_budget_takes_several_frames_and_arrives_whole`) had the same one-sided `<=`, and its `frames >= …` bounds frames only from below.

**What the exact length rests on, and what it does not reach.** `PtyOutput::pick_up` **appends** to its destination (`the_destination_keeps_what_it_already_had` pins that); the length equals the budget only because `Pane::pump` clears `received` first. Reusing the assertion on another caller needs that clear. And it binds how much is taken **out of the queue** — which is what brakes the program — not how much of it reaches the grid: a pump that fed `screen.advance` only part of `received` would keep it green.

**A fixture trap on the way:** a word printed before a 128 KiB dump with no newlines is scrolled off the grid by the dump, so «the other console delivered this frame» has to look for the dump's own letter (`bbbb`), not a leading word. Giving each console its own letter also lets the test say whose output reached which grid (`aaaa` and no `b`, and the other way round).

## Where it lives

`crates/app/src/lib.rs`, `tests::a_dump_in_one_console_neither_stalls_nor_dirties_the_other`; the budget is asked for in `Pane::pump` (`pick_up(&mut self.received, PUMP_BUDGET, …)`). Found and mutated on #145, September 2026.
