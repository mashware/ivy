---
id: asp_only-a-full-buffer-read-binds-the-buffers-size_0s6bd9
slug: only-a-full-buffer-read-binds-the-buffers-size
name: >-
  Halving the reader's buffer left eighty-three tests green — and the test that
  fixed it could still be defeated by halving the constant, because both its
  sides move together
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:23:16.371Z'
updated_at: '2026-09-10T16:08:21.312Z'
last_verified: '2026-09-10T16:08:21.312Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - READ_SIZE
  - READS_IN_FLIGHT
  - feed
  - without_consuming_it_what_arrives_is_bounded
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
tags:
  - tests
  - mutation
  - D8
  - D30
  - brake
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured September 2026: change the reading loop's buffer to `READ_SIZE / 2` and the whole suite stays green.**

`READS_IN_FLIGHT × READ_SIZE` = 64 KiB is the ceiling **D8** chose, and the only thing standing between a program dumping hundreds of MiB/s and unbounded memory is that this ceiling makes the reader **stop reading**. The mutation takes it to 32 KiB with nothing noticing.

**Why every existing test was blind, and it generalises**: they all hand over a handful of letters, and a handful of letters reads identically at any buffer size. The one test that goes through a real pseudoterminal to check the channel is bounded has a margin of **1 MiB** — thirty times the mutated ceiling — deliberately generous so as not to depend on this kernel's slack, which is exactly what makes it unable to see the ceiling move.

> **A chunk of exactly the buffer's length is the only one that says how long the buffer is.** It is also the one length at which `buf[..n]` and `buf` are the same slice, so it doubles as the boundary case of the read's length.

### And the fix was itself a disguise, which a later review caught

The test written for it scripts a turn of `READ_SIZE` bytes and asserts the delivered chunk is `READ_SIZE` long. **Both sides move with the constant.** It catches the *buffer* being changed to `READ_SIZE / 2` — the mutation it was written for — and stays green when the **constant itself** is set to 4096, while the brake halves exactly as before. That is this project's second named disguise, a number where the defect and the fix agree, arriving one level up: not in the assertion, in the thing the assertion is written in terms of.

> **Two guards are needed and only one of them is a guard: the product has to be asserted against a LITERAL.** `assert_eq!(READ_SIZE * READS_IN_FLIGHT, 64 * 1024)` is the only assertion a rename or a new value cannot follow, and it is what makes the figure **D8** measured the thing being defended rather than an expression. Verified: with the constant halved, that one goes red and nothing else does.

**Two mechanics worth reusing.** A scripted fake should **clamp** a turn to the buffer and **push the tail back**, so a turn of exactly `READ_SIZE` comes back whole while a longer one becomes two reads — discarding the remainder is a fake that lies about the short read it claims to model, which a review also had to point out. And assert length and content **apart**: one `assert_eq!` over 8 192 bytes prints 60 KB of decimal on failure, which is most of the way back to having no test.

## Where it lives

`crates/term-core/src/lib.rs`: `READ_SIZE` beside `READS_IN_FLIGHT`, the reading loop, the test tying a delivered chunk to the constant, and `the_brake_is_still_the_sixty_four_kibibytes_the_decision_chose` — the one with the literal in it.
