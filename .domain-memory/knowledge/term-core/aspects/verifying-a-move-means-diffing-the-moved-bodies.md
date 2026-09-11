---
id: asp_verifying-a-move-means-diffing-the-moved-bodies_3sytzr
slug: verifying-a-move-means-diffing-the-moved-bodies
name: >-
  A review of a pure move audits everything except whether the move is faithful
  — four specialists, fifteen findings, and not one diffed a moved body
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:22:21.263Z'
updated_at: '2026-09-11T20:22:21.263Z'
last_verified: '2026-09-11T20:22:21.263Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/keyboard.rs
  - crates/app/src/mouse.rs
  - crates/app/src/measure/header.rs
symbols:
  - send_keys
  - handle_wheel
  - send_mouse_to_the_program
content_hashes:
  crates/app/src/lib.rs: 'sha256:171c1421de9c4a6d0646eb0c98664b64626350b32415c7e52c7c788d0838ed09'
  crates/app/src/keyboard.rs: 'sha256:674ef9ba2cadfedfe82afb9a68b85b927a7365571191ca99b6863ddc634c1af3'
  crates/app/src/mouse.rs: 'sha256:e8c985b9f8dea4455c50839a5e964b9b5b14986b9777c111184a90ac510835b1'
tags:
  - review
  - d28
  - refactoring
  - method
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured on the delivery that took three blocks out of `crates/app/src/lib.rs` and `measure.rs` (September 2026): four specialist reviewers — comments, tests, silent failures, type design — produced fifteen findings between them, every one about prose, guard floors, seams or visibility, and not one compared a moved body against the deleted one.** A coverage auditor, handed only the file list and one line per reviewer, named it in three sentences: *«it is the premise the whole PR rests on»*.

The reason is structural rather than sloppy: a move produces a diff that is enormous textually and null semantically, so every reviewer reads the *new* side and asks whether it belongs there. Nobody re-reads the old side, because it is gone.

**The check is mechanical and takes one command.** Extract each block from the base commit, extract what now stands, and normalise **only the transformations the move itself declares** — here the module prefixes dropped inside their own module (`keyboard::`, `mouse::`) and the visibility widened to keep the call sites compiling. Then diff. Every surviving difference is either a deliberate edit you can name or a silent one you cannot.

> What it found, and it is the shape to expect: the third block came back **795 lines to 795, zero differences**; the other two had two and zero. Both survivors were explainable — a doc link (``[`Window`]``) that stopped resolving across the module boundary, and one `let` that **`rustfmt` rejoined** because dropping a prefix made the line short enough to fit.

**That last one is the point.** `rustfmt` reflowing a moved line is what makes «I only moved it» untrue without anybody editing anything, so the normalisation must not swallow it — look at it and name it. See [[the-formatter-decides-what-the-ratchet-counts]] for the same formatter deciding a line count nobody asked it to.

## Where it lives

The technique, not a location: `git show <base>:<file>` for the deleted block against the receiving module's `impl` block today. In this delivery the blocks were `Pane::send_keys` (`keyboard.rs`), the wheel-and-gesture quartet (`mouse.rs`) and the per-session half of `measure.rs` (`measure/header.rs`).
