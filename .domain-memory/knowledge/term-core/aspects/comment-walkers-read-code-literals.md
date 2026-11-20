---
id: asp_comment-walkers-read-code-literals_bb7cew
slug: comment-walkers-read-code-literals
name: >-
  The comment-walker guards scan text, not Rust: a block-comment opener in a
  string, a quote byte literal or a backticked name in a comment breaks them
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T17:25:08.224Z'
updated_at: '2026-11-20T17:25:08.224Z'
last_verified: '2026-11-20T17:25:08.224Z'
file_paths:
  - crates/app/tests/prose.rs
  - crates/app/tests/glossary.rs
  - crates/app/src/viewer/highlight.rs
symbols:
  - NOT_OURS
content_hashes:
  crates/app/tests/prose.rs: 'sha256:0028dba27063c64b691b8b24a2f5374ae5c7a96860ce86695c38fc0935a96206'
  crates/app/tests/glossary.rs: 'sha256:012663d65f0754924ee333ded4568d5c81ce6d00f4009b747b64486b8c934820'
tags:
  - guards
  - tests
  - comments
feature_id: feat_term-core_nx0g1r
---

## What it does

`crates/app/tests/prose.rs` and `glossary.rs` walk source text with hand-written scanners,
not a Rust parser. Three things written in ordinary code fail the build through them:

- a string literal holding a block-comment opener reads as an unclosed block comment;
- a byte literal of a double quote (`b'` + quote + `'`) reads as the start of a string;
- a backticked word in a comment (`x2`, `syntect`) reads as a citation of an identifier
  the crate must have, unless it is listed in `NOT_OURS`.

Spell such markers as escapes (`"/\x2a"`, `0x22`) behind a named constant, keep code-like
words out of backticks in comments, and add genuine foreign names to `NOT_OURS`.
Which lists a new module must be added to is the separate card
[[the-sixth-list-lives-in-production-code]].

Evidence: three guard failures while adding `viewer/highlight.rs` and its tests in
#159 PR #1.

## Where it lives

crates/app/tests/prose.rs, crates/app/tests/glossary.rs (NOT_OURS)
