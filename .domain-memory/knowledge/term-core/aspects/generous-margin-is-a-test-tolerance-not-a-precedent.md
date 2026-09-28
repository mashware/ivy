---
id: asp_generous-margin-is-a-test-tolerance-not-a-precedent_g112rd
slug: generous-margin-is-a-test-tolerance-not-a-precedent
name: >-
  GENEROUS_MARGIN reads like a production margin and is a test tolerance: the
  output side's real ceiling is READS_IN_FLIGHT × READ_SIZE plus the kernel's
  buffer, about 90-100 KiB
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T14:25:15.266Z'
updated_at: '2026-09-28T14:25:15.266Z'
last_verified: '2026-09-28T14:25:15.266Z'
file_paths:
  - crates/term-core/src/lib.rs
  - docs/02-decisiones.md
symbols:
  - GENEROUS_MARGIN
  - without_consuming_it_what_arrives_is_bounded
  - READS_IN_FLIGHT
  - READ_SIZE
  - MAX_OWED
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:a9ffaf445eb42e55e3727ab8952a28d8012b37eaedc1a7bc9bc1e21335d03f16'
  docs/02-decisiones.md: 'sha256:b53044393a7694d2c1f56d6f41680d1dd0df011e6f7448de57e3b6d5edc66e4d'
tags:
  - d8
  - d60
  - measured
  - precedent
feature_id: feat_term-core_nx0g1r
---

## What it does

**The trap: a constant whose name sounds like a decision.** `GENEROUS_MARGIN` (1 MiB) lives inside one unit test of `term-core`, `without_consuming_it_what_arrives_is_bounded`, and its own doc comment says what it is: an assertion bound left **ten times looser** than the real ceiling so the test does not depend on this machine's kernel slack. It is not a production number, it is not in `pty_output.rs`, and it is not a precedent for anything.

**What the real number is.** The output side's ceiling is `READS_IN_FLIGHT` (8) × `READ_SIZE` (8 KiB) = 64 KiB of channel, plus the kernel's own pseudoterminal buffer (~20 KiB measured on Linux), plus at most one read already pulled and waiting for room: on the order of **90-100 KiB**. That is the figure to anchor to when a new ceiling needs a precedent in this codebase.

**How it bit.** The design for #161 (a ceiling on the *input* queue) first cited `GENEROUS_MARGIN` as 'the app's output-side margin, mirrored' and chose 1 MiB on that basis — 16× looser than the brake it claimed to echo. The design challenger caught it by reading the line; D60 records the corrected anchor (256 KiB, from ~90-100 KiB with headroom) and says in writing that the number is reasoned, not measured.

**The rule:** before citing an existing constant as the precedent for a new one, read where it is declared and what its comment says it is for. A name is a claim, and a constant declared inside `#[test]` is a claim about a test.

## Where it lives

`crates/term-core/src/lib.rs`: `GENEROUS_MARGIN` inside `without_consuming_it_what_arrives_is_bounded` (the comment above that test carries the arithmetic); `READS_IN_FLIGHT` and `READ_SIZE` near the top of the file; `MAX_OWED`, whose doc comment names this trap. The decision that uses the corrected anchor is **D60** in `docs/02-decisiones.md`.
