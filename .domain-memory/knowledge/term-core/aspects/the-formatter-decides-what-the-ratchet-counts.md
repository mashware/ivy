---
id: asp_the-formatter-decides-what-the-ratchet-counts_p5cykt
slug: the-formatter-decides-what-the-ratchet-counts
name: >-
  The formatter decides whether one more argument costs the ratchet: sixty
  characters of arguments, not a hundred columns of line
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T22:23:57.903Z'
updated_at: '2026-09-10T22:23:57.903Z'
last_verified: '2026-09-10T22:23:57.903Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pty_output.rs
  - crates/app/tests/shape.rs
symbols:
  - Silences
  - pick_up
  - PUMP_BUDGET
content_hashes:
  crates/app/src/lib.rs: 'sha256:171c1421de9c4a6d0646eb0c98664b64626350b32415c7e52c7c788d0838ed09'
  crates/app/src/pty_output.rs: 'sha256:e86ddf6c839cd5597c18e2a1a61ee4128cae5d001d589c146bead8f237cf3f2d'
  crates/app/tests/shape.rs: 'sha256:dafb615269cba5e6910c01d4fe77981f158c73a3cec9b2e736bd3728e0f7de99'
tags:
  - d28
  - ratchet
  - rustfmt
  - measured
feature_id: feat_term-core_nx0g1r
---

## What it does

**Whether one more argument costs `crates/app/src/lib.rs` production lines is decided by rustfmt, not by the code.** Measured while adding a fourth argument to `PtyOutput::pick_up`: the arguments' text went from 51 to 71 characters and the formatter split them **one per line**, taking the call from 3 lines to 6 — the file to 1 364 against the 1 362 D28's ratchet records, and the guard fired.

The threshold is rustfmt's `fn_call_width`, **60 characters by default** (60% of `max_width` 100) — not the 100-column line limit everybody reasons about. A call whose arguments fit in 60 stays on one line however long the receiver chain is; one character over and every argument gets its own line, plus the closing paren. So **the ratchet is spent in fours, not in ones**.

> A delivery that has to widen a call reached from `lib.rs` should count the arguments' characters **before** designing the signature. Under 60 is free; over 60 costs three or four lines in the file that may not grow.

**Binding the new value to a local does not help**: it buys the single-line call and pays a line for the binding. Measured: 3 lines before, 3 after — no growth, but no room either, and two names where there was none.

**What worked was one value instead of two arguments** — `Silences { deaf, quiet }`, built by `Silences::of(&Pty)` in the module that reads it: arguments back to 51 characters, `lib.rs` unchanged. The alternatives were worse for reasons worth keeping: a combined accessor on `Pty` is discarded by **D38** (two lifetimes behind one accessor); `#[rustfmt::skip]` is an attribute whose only purpose is a line count; and taking an unrelated piece out of `lib.rs` to pay for two lines is the ratchet's sanctioned move but not honest when the delivery does not need the room.

**And a `mod` line is the same problem one level up.** Adding a module to the crate root costs `lib.rs` exactly one production line, which the zero-tolerance ratchet refuses. The way through, used by the delivery that split the notices out of `painter.rs`: declare the new module as a **child** of the module it came from (`painter/notices.rs`, declared from `painter.rs`), which pays the line in a file that has room. `painter/blocks.rs` was already the precedent.

## Where it lives

`crates/app/tests/shape.rs` holds the ratchet (`OVER_CEILING_TODAY`, zero tolerance, and `no_entry_of_the_ratchet_is_stale` asserting recorded **==** real). The call this was measured on is `Pane::pump` in `crates/app/src/lib.rs`; the value that fixed it is `Silences` in `crates/app/src/pty_output.rs`. There is no `rustfmt.toml` in the repo, so every default applies as shipped.
