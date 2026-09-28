---
id: asp_paying-the-ceiling-with-child-modules_veedx5
slug: paying-the-ceiling-with-child-modules
name: >-
  A child module per touched file pays D28's MODULE ceiling but, since #187, not
  the TYPE ceiling — and the guards only run under `cargo test --workspace`
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T19:51:41.460Z'
updated_at: '2026-09-28T19:04:41.247Z'
last_verified: '2026-09-28T19:04:41.247Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/tests/prose.rs
  - crates/app/tests/glossary.rs
symbols: []
content_hashes:
  crates/app/tests/shape.rs: 'sha256:b8474648d5df70f6dad440a1c2864dd695f49fb5efd5a6ba66be8d1a5ccc0cca'
  crates/app/tests/prose.rs: 'sha256:ab0b9e74bd4600f582efddfc69ed57e17ebd23567f4fc186e61044fe0943baa0'
  crates/app/tests/glossary.rs: 'sha256:90d6c32c3639ed72476a08556a5782aedb6529d909f3629eb7791aafedf52ae2'
tags:
  - D28
  - guards
  - modules
  - workflow
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured on #257**: four WIP steps sat over five D28 ceilings at once (`lib.rs`'s ratchet, `panel.rs`, `painter.rs`, `pointer.rs`, `impl InjectedDrags`) and nobody saw it, because each step was checked with `cargo test -p app <filter>`, which never runs the integration guards in `crates/app/tests/`. Run `cargo test --workspace` before each WIP commit.

**The cheap way to pay the MODULE ceiling**: give the feature a child module of each file it touches (`panel/links.rs`, `pointer/links.rs`, `injected/links.rs`, `links/console.rs`). A child module sees its parent's private types, fields and helpers, so an `impl PanelState` or `impl Window` block moves there with no visibility widened. A `pub(super)` method in the child is callable from the parent.

**What it no longer pays (since #187, D28 amended 28 September 2026)**: `shape.rs` holds the 300 per TYPE as well as per block — a type's impl blocks summed across `MODULES` (up to each module's first `mod tests`), keyed by the last path segment (`impl crate::Window` == `impl Window`), **including impls of traits the crate declares**; only foreign traits (`Display`, `Drop`, `eframe::App`) stay out. Moving methods into another block, another file or `impl OurTrait for Window` moves the lines without paying anything. Five types are ratcheted with zero tolerance — `Window` 1 782, `Pane` 591, `PanelState` 378, `InjectedDrags` 334, `Node` 321 — so one more line of any of them, in any file, fails `no_type_is_over_its_ceiling` and `no_entry_of_the_ratchet_is_stale`: take the same lines out of THAT type, or split it into two types with a seam. The walk refuses `impl<`, `impl dyn`, rustfmt-wrapped headers and indented `impl`s (`every_impl_header_is_one_the_type_walk_can_read`) — teach `type_of` before introducing one.

**What a split costs in the same commit** (on top of [[the-sixth-list-lives-in-production-code]]):

- `prose.rs` `SOURCES` holds a per-file comment-line FLOOR: moving comments out of a file lowers that file's count, so its floor has to be re-measured and lowered — not only the new file added — or the guard fails on the old file.
- `shape.rs` `MODULES` plus its module-count floor.
- `glossary.rs` `GLOSSARY_CLEAN_FILES` and `CITATIONS_CHECKED`; a comment citing a word in backticks that is not an identifier (`https`, `cmd`, `javascript`) fails the citations guard — use «» for words that are not names.

**And the ratchet is spent by rustfmt**, see [[the-formatter-decides-what-the-ratchet-counts]]: a method call over the chain width wraps to three lines; a free-function path call wraps per argument once its arguments pass 60 characters. When `lib.rs` still needs lines, move a piece the diff is not about but that belongs elsewhere (#257 moved the pointer-to-cell helpers to `mouse.rs` and the bench's drag-count reading to `injected.rs`) and write the tally into the ratchet's reason — and if the piece is a method of a ratcheted type, the type's number does not move.

## Where it lives

`crates/app/tests/shape.rs` (`type_sizes`, `type_of`, `OVER_CEILING_TODAY`), `crates/app/tests/prose.rs`, `crates/app/tests/glossary.rs`; D28 «Amended: the `impl` ceiling holds a type, not only a block» in `docs/02-decisiones.md`.
