---
id: asp_a-separate-test-file-needs-cfg-test-inside_9kkd8t
slug: a-separate-test-file-needs-cfg-test-inside
name: >-
  A module's tests kept in their own file count as production lines unless the
  file opens with #![cfg(test)]
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T21:02:08.775Z'
updated_at: '2026-11-17T21:02:08.775Z'
last_verified: '2026-11-17T21:02:08.775Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/src/sidebar/tests.rs
symbols: []
content_hashes:
  crates/app/tests/shape.rs: 'sha256:2795855df22338b0169a6e00510ae21efdcaf02769eca8f20a2c6a71fbb12ff5'
  crates/app/src/sidebar/tests.rs: 'sha256:e37c66d388d4d93824a126d2262084e7ff6c73e675c8b015f87392fe17ac6973'
tags:
  - guards
  - shape
  - tests
feature_id: feat_term-core_nx0g1r
---

## What it does

`crates/app/tests/shape.rs` stops counting a module at its first `mod tests` block. A module whose
tests live in a separate file (`mod tests;` pointing at `src/sidebar/tests.rs`) gets that file
measured as a module of its own. All of its lines count against the 600-line ceiling, although
every one is test code.

The file escapes the count when it opens with the inner attribute `#![cfg(test)]`, as
`explorer/interaction_tests.rs` does
(`a_leading_inner_test_attribute_excludes_only_a_test_only_module`).

Evidence: `src/sidebar/tests.rs` reached 689 «production» lines in #93 PR 6 and turned
`no_module_is_over_its_ceiling` red. One line fixed it. See
[[a-shared-test-helper-lives-inside-mod-tests]].

## Where it lives

crates/app/tests/shape.rs; crates/app/src/sidebar/tests.rs
