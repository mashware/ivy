---
id: asp_redundant-guards-mask-each-other-under-mutation_kjjd2z
slug: redundant-guards-mask-each-other-under-mutation
name: >-
  Two guards that reject the same input mask each other under mutation: each
  needs a case only it can catch
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T16:08:28.673Z'
updated_at: '2026-09-26T16:08:28.673Z'
last_verified: '2026-09-26T16:08:28.673Z'
file_paths:
  - crates/app/src/panel/color.rs
symbols:
  - slot
content_hashes: {}
tags:
  - testing
  - mutation
  - d30
feature_id: feat_term-core_nx0g1r
---

## What it does

The panel's `slot` checks both the first and the last glyph's character. A drift test built on a **one-character** emoji stayed green with the first check deleted, because for a one-character span the first and last glyph are the same glyph and the second check caught the drift alone. It took a case **only the first check can catch** — an emoji carrying its selector, whose last glyph legitimately carries the base's character — to bind it.

The general rule for this repo's mutation passes: when a function has two conditions that reject overlapping inputs, one test per condition is not enough — each test's input must satisfy every other condition and fail only its own. Found by a 17-mutation run in a separate worktree during #178's validation; that one survived and was fixed.

## Where it lives

crates/app/src/panel/color.rs test `a_glyph_that_is_not_the_expected_character_gives_no_slot`
