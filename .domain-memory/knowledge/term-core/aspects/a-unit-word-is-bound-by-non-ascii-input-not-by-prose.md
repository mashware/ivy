---
id: asp_a-unit-word-is-bound-by-non-ascii-input-not-by-prose_evaxr8
slug: a-unit-word-is-bound-by-non-ascii-input-not-by-prose
name: >-
  A byte count called «characters» is invisible to every prose guard; only a
  test with non-ASCII input binds the unit, and every held-paste fixture was
  ASCII
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T19:29:58.705Z'
updated_at: '2026-09-28T19:29:58.705Z'
last_verified: '2026-09-28T19:29:58.705Z'
file_paths:
  - crates/app/src/keyboard.rs
  - crates/app/tests/prose.rs
  - crates/app/src/panel.rs
symbols:
  - the_longest_line
  - 'HeldPaste::sentence'
  - sanitize
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:705ffab7bb4b6c5c5c0e8cbcf1ba93021b4f54c21baa691bcb0858b7b0dee681'
  crates/app/tests/prose.rs: 'sha256:3ce39d39bbf9d44e6542d61c9a9caf682dc064f5dc965f79c7b711d98407f68c'
  crates/app/src/panel.rs: 'sha256:f495bf7dc35ff9c87608b823ccfdcc53446b6a03ad80f35f2729ae7951844dfa'
tags:
  - test-disguise
  - guards
  - units
  - measurement
feature_id: feat_term-core_nx0g1r
---

## What it does

**A mislabelled unit gets past a prose guard for the same reason a mislabelled number does, one level down: the word is spelled correctly and the identifier next to it exists.** In #160 the held-paste question said «a line of {} characters» while `{}` came from `<[u8]>::len`, and a comment called the kernel's 4 095 bytes «characters». Both passed `prose.rs`, `glossary.rs`, `shape.rs` and clippy. The security and comment reviewers caught them.

### Why no narrow guard works (measured September 2026, #169)

| Heuristic over `src/` and `examples/` of every crate | Hits (September 2026) | True | Caught of the 2 that shipped |
|---|---|---|---|
| «character» within ±3 lines of `len()`/`as_bytes`/`u8`/`bytes()` | 28 | 0 | 0 |
| same, ±14 lines | 111 | 0 | 1 |
| a number or `{…}` followed by «character(s)» | 6 | 0 | 2 |

The byte length was measured 12–25 lines away, in another function, so whether «characters» is right depends on the type of an expression a line walk cannot see. The only variant that caught both was 0 for 6 when measured: as a guard it would really be an allowlist. #169 made `prose.rs` declare the hole in its header and assert the #160 sentence as an evasion, the same treatment as «formato desconocido».

### What does bind it: input where the two units differ

**Every held-paste test was built from ASCII (`x`, `a`, `b`), and ASCII is the one input where a byte and a character are the same number.** Two mutations left the other 53 tests in `keyboard::tests` green: `the_longest_line` counting characters, and the sentence saying «characters». A single `ñ` line, `boundary / 2 + 1` characters and so just over the boundary in bytes, fails both: it must be held, and the question must say «<bytes> bytes». This is the project's usual disguise, «numbers where the defect and the fix give the same result», with ASCII as the number.

### The same mix-up in code, next door

`panel.rs`'s long-phase test bounded `painted` (a `chars().count()`) by `MAX_CHARACTERS + "a1b2 · ".len()`. `·` is two bytes, so that added 8 for a 7-character prefix. The deeper error was that `sanitize(&identity)` caps the **whole** identity line, prefix included, so the prefix never belonged outside the cap: the bound sat 8 above anything the cap can produce, and a cap loosened by two passed it. The bound is now `MAX_CHARACTERS`, and the same mutation fails it at 513.

## Where it lives

`crates/app/src/keyboard.rs` tests: `a_line_under_the_boundary_in_characters_and_over_it_in_bytes_is_held_and_said_in_bytes`. `crates/app/tests/prose.rs` header («It does not judge a unit») and `the_oracle_answers_what_it_promises`. `crates/app/src/panel.rs` tests: `an_absurdly_long_phase_is_not_painted_whole`.
