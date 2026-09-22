---
id: asp_paying-the-ceiling-with-child-modules_veedx5
slug: paying-the-ceiling-with-child-modules
name: >-
  A feature that lands across near-ceiling modules pays D28 with a CHILD module
  per file it touches — and the guards only run under `cargo test --workspace`
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T19:51:41.460Z'
updated_at: '2026-09-22T19:51:41.460Z'
last_verified: '2026-09-22T19:51:41.460Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/tests/prose.rs
  - crates/app/tests/glossary.rs
symbols: []
content_hashes:
  crates/app/tests/shape.rs: 'sha256:4e49c6e6bca3380415f52df0f16d6dac3d4ed4b410de94231c83434173a5532f'
  crates/app/tests/prose.rs: 'sha256:ddddbc2381e14ed85612fb9abaa572166e6c51c7ede5a4c0447a5d76647a6920'
  crates/app/tests/glossary.rs: 'sha256:5ac73540dced34e159a1a23fd3bcc877fa09511ed0f97b293746f9a010794ff2'
tags:
  - D28
  - guards
  - modules
  - workflow
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured on #257**: four WIP steps sat over five D28 ceilings at once (`lib.rs`'s ratchet, `panel.rs`, `painter.rs`, `pointer.rs`, `impl InjectedDrags`) and nobody saw it, because each step was checked with `cargo test -p app <filter>`, which never runs the integration guards in `crates/app/tests/`. Run `cargo test --workspace` before each WIP commit.

**The cheap way to pay**: give the feature a child module of each file it touches (`panel/links.rs`, `pointer/links.rs`, `injected/links.rs`, `links/console.rs`). A child module sees its parent's private types, fields and helpers, so an `impl PanelState` or `impl Window` block moves there with no visibility widened, and `shape.rs` counts each `impl` block separately. A `pub(super)` method in the child is callable from the parent.

**What a split costs in the same commit** (on top of [[the-sixth-list-lives-in-production-code]]):

- `prose.rs` `SOURCES` holds a per-file comment-line FLOOR: moving comments out of a file lowers that file's count, so its floor has to be re-measured and lowered — not only the new file added — or the guard fails on the old file.
- `shape.rs` `MODULES` plus its module-count floor.
- `glossary.rs` `GLOSSARY_CLEAN_FILES` and `CITATIONS_CHECKED`; a comment citing a word in backticks that is not an identifier (`https`, `cmd`, `javascript`) fails the citations guard — use «» for words that are not names.

**And the ratchet is spent by rustfmt**, see [[the-formatter-decides-what-the-ratchet-counts]]: a method call over the chain width wraps to three lines; a free-function path call wraps per argument once its arguments pass 60 characters. When `lib.rs` still needs lines, move a piece the diff is not about but that belongs elsewhere (#257 moved the pointer-to-cell helpers to `mouse.rs` and the bench's drag-count reading to `injected.rs`) and write the tally into the ratchet's reason.

## Where it lives

`crates/app/tests/shape.rs`, `crates/app/tests/prose.rs`, `crates/app/tests/glossary.rs`; D28 in `docs/02-decisiones.md`.
