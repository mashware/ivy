---
id: asp_a-shared-test-helper-lives-inside-mod-tests_wygfef
slug: a-shared-test-helper-lives-inside-mod-tests
name: >-
  A test helper shared by several `app` modules lives at `src/tests/<name>.rs`,
  declared inside `lib.rs`'s `mod tests` — and `shape.rs` still holds it, with
  two traps
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T21:23:09.716Z'
updated_at: '2026-09-28T21:23:09.716Z'
last_verified: '2026-09-28T21:23:09.716Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/src/tests/markdown.rs
  - crates/app/src/lib.rs
symbols:
  - every_impl_header_is_one_the_type_walk_can_read
  - production_lines
content_hashes:
  crates/app/tests/shape.rs: 'sha256:df9130232ffc0b12289f9b3af78f80a8cbb5adf37d10b575bc6944555d757111'
  crates/app/src/lib.rs: 'sha256:b8e2b40e3f5448afc482f1d2615f0bffc7628342cc4774ef4b631336e8bd3e9a'
tags:
  - d28
  - shape
  - test-helpers
feature_id: feat_term-core_nx0g1r
---

## What it does

**Where to put a `#[cfg(test)]` helper that several modules of `app` need**, measured on #149 in September 2026. The answer is `pub(crate) mod <name>;` declared **inside** `lib.rs`'s `mod tests`, with its file at `src/tests/<name>.rs`, reached as `crate::tests::<name>`. It costs `lib.rs`'s D28 ratchet **zero** lines, because `production_lines` stops at the first `mod tests`. The alternatives, `#[cfg(test)] mod x;` at the crate root or a child of one module, add lines to a counted file.

**Two traps of `tests/shape.rs` that the file still falls into:**

1. **The type walk refuses `impl<'a> T<'a> {`.** `every_impl_header_is_one_the_type_walk_can_read` fails. A helper type with a lifetime parameter must own its data (`markdown::Table` holds `Vec<String>`) or become free functions. A struct with lifetimes but **no** `impl` (`markdown::Fenced<'a>`) passes.
2. **The whole file is production code to `shape`.** It has no inner `mod tests`, so every line counts against the 600-line module ceiling and in the type sizes, even though all of it is test code.

The new file still has to go on every file list (see [[the-sixth-list-lives-in-production-code]]): shape `MODULES`, prose `SOURCES` with its comment-line floor, scripts `MODULES`, and both glossary lists.

## Where it lives

`crates/app/src/tests/markdown.rs` (document tables by header, fenced blocks), declared at the top of `mod tests` in `crates/app/src/lib.rs`. The walks are `production_lines` and `impl_blocks` in `crates/app/tests/shape.rs`.
