---
id: asp_a-floor-made-of-the-constant-cannot-see-it-shrink_7w6czz
slug: a-floor-made-of-the-constant-cannot-see-it-shrink
name: >-
  A test whose bound IS the production constant guards the mechanism, not the
  value: shrink the constant and the floor shrinks with it, green
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T16:08:12.942Z'
updated_at: '2026-09-25T16:08:12.942Z'
last_verified: '2026-09-25T16:08:12.942Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - HANGUP_GRACE
  - a_program_that_ignores_the_hang_up_is_killed_after_its_grace
  - a_program_that_catches_the_hang_up_finishes_saving_before_it_is_killed
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7d458cd12c3301833284d892b02b1731909b406a21779d771cb71eec244fddb9'
tags:
  - method
  - mutation
  - test-disguise
  - D30
  - D55
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured on D55 (September 2026).** `a_program_that_ignores_the_hang_up_is_killed_after_its_grace` asserts `took >= HANGUP_GRACE`. Mutating the grace 200 → 50 ms left the **whole term-core suite green**: the floor moved with the constant, so the test proved «the close waits out whatever the grace is» and nothing about how long that is.

**The repo's usual advice cuts both ways.** «Bound on the constant, not a literal, so the test moves with it» (used for `READER_GIVE_UP`) is right when the constant is *incidental* — a reserve whose exact value nobody promised. It is wrong when the constant **is the promise** — a grace a program is owed, a ceiling a decision chose. Same family as [[only-a-full-buffer-read-binds-the-buffers-size]], where both sides of a test moved with `READ_SIZE`.

**The fix states the promise from the other side, with a literal.** `a_program_that_catches_the_hang_up_finishes_saving_before_it_is_killed`: a shell traps SIGHUP, spends a literal 100 ms, writes `done` to a file whose path arrives as `$0` (never spliced into the script); after the close the file must say `done`, and the close must be under the grace (it left through its own `exit`). Red under grace 50 ms and under an immediate SIGKILL.

**Rule:** when a constant encodes a promise, at least one test must express that promise without naming the constant.

## Where it lives

`crates/term-core/src/lib.rs`, `mod tests`: the two grace tests and the `time_the_close` helper (hang-safe: drop on a thread, `recv_timeout` 2 s).
