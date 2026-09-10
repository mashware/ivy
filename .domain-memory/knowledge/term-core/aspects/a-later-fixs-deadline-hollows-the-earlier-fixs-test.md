---
id: asp_a-later-fixs-deadline-hollows-the-earlier-fixs-test_y38fen
slug: a-later-fixs-deadline-hollows-the-earlier-fixs-test
name: >-
  A test whose only assertion is «it came back» stops binding the moment
  somebody puts a deadline on the same path — and the hollowing happens in a
  different delivery
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:23:35.551Z'
updated_at: '2026-09-10T15:23:35.551Z'
last_verified: '2026-09-10T15:23:35.551Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - deliver
  - 'Pty::drop'
  - READER_GIVE_UP
  - closing_with_the_channel_full_does_not_hang
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
tags:
  - tests
  - mutation
  - D30
  - D32
  - deadlines
feature_id: feat_term-core_nx0g1r
---

## What it does

**A test can go blind because of somebody else's fix, and nothing marks the moment it does.**

The case, measured September 2026. `closing_with_the_channel_full_does_not_hang` exists for the stop flag: with the output channel full, the reader thread is parked in the delivery waiting for room, and **only the flag can reach it there** — not the kill, not the end of file. Its whole assertion was «the drop finished inside 5 s».

Then **D32** gave `Drop for Pty` a `recv_timeout(READER_GIVE_UP)` and the right to **abandon** that thread. From that commit on **the close returns whether or not the flag is ever consulted**. Mutation `if false && stop.load(...)` — the flag never read — left that test **green**; the only red was a unit test with the flag pre-raised, which never even reaches the send.

> **The flag's remaining observable effect is the COST, not the outcome.** The flag makes the thread *end*, and a thread that has ended is not waited for. Measured: **50.36 ms** at baseline against **150.25 ms** under the mutation — the 50 ms of the library's own kill poll plus the whole 100 ms reserve. A **3×** separation, so asserting «the close cost less than the abandon deadline» is a real assertion and not a margin.

> **The rule: when a delivery adds a timeout, a deadline or an abandon to a path, every existing test on that path whose assertion is «it returned / it did not hang» has to be re-mutated.** The new deadline now satisfies them. The replacement assertion is whatever the original mechanism **buys over the deadline** — here the time not spent — and it is bound to the constant rather than to a literal, so it moves when the constant does.

**Where the sibling already said it and nobody carried it across.** A test named `closing_an_ordinary_console_does_not_spend_the_abandon_deadline` was written for exactly this reasoning on the *read* path, and its own comment says its channel drain is what keeps it **off** the delivery path. The delivery path's twin was missing for three commits.

> **A pair of tests where one names the path it excludes is a standing invitation to check that the excluded one has a test of its own.**

## Where it lives

`crates/term-core/src/lib.rs`: `deliver`'s stop check, `Drop for Pty`'s `READER_GIVE_UP` arm, and the pair `closing_an_ordinary_console_does_not_spend_the_abandon_deadline` / `closing_with_the_channel_full_does_not_spend_the_abandon_deadline`. The deadline that did the hollowing is **D32**.
