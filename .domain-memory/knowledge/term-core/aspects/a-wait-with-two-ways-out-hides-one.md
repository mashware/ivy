---
id: asp_a-wait-with-two-ways-out-hides-one_ejscwy
slug: a-wait-with-two-ways-out-hides-one
name: >-
  A wait with two ways out cannot have one tested by a scenario where the other
  also fires: every close kills the program, and the hang-up covered for a
  missing knock
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T19:31:39.142Z'
updated_at: '2026-09-26T19:31:39.142Z'
last_verified: '2026-09-26T19:31:39.142Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/src/nonblocking.rs
symbols:
  - 'Writable::wait'
  - Drop for Pty
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:598307ef482b8847d279ad556b074bb929e86396d7284e83bd995094bd17b1e8'
tags:
  - testing
  - mutation
  - disguise
  - pty
feature_id: feat_term-core_nx0g1r
---

## What it does

**Test disguise met validating #159 (26 September 2026).** The writer's wait for room ends either on the console's **knock** (closing drops a pipe's writing end) or on the master's **hang-up** (the program's side closed). Every test that closes a console also kills its program, so the hang-up arrives within ~1 ms and ends the wait on its own. Measured: removing the knock entirely — not dropping it, or not polling it — left the three closing tests **green**.

**The only scenario that isolates the knock is a survivor holding the slave**: `{ setsid sleep 10 & } && stty raw -echo && …` (the brace group is load-bearing — see the survivor test's own comment). No hang-up comes, so nothing but the knock can end the wait.

**Second trap in the same place: a trigger dropped as a struct field at the end of `Drop` is equivalent to an explicit early drop under any generous deadline.** Both arrive within microseconds of each other in the tests' terms. What pins the order is asserting the writer is **already finished** at the moment a close returns. The premise to assert first is that the close took at least `READER_GIVE_UP` (100 ms): that proves the survivor held the slave, and it gives an early knock that whole margin. Both mutations — no early drop, knock not polled — went red only with that test.

**The general rule**: before trusting a test of one exit of a multi-exit wait, list the other exits and check the scenario does not trigger them.

## Where it lives

`crates/term-core/src/lib.rs` tests: `closing_a_console_whose_program_left_a_survivor_ends_its_writer_before_the_close_returns` (the knock), `a_program_that_dies_with_its_writer_waiting_for_room_makes_the_writer_give_up` (the hang-up, console left open); `crates/term-core/src/nonblocking.rs` `Writable::wait`.
