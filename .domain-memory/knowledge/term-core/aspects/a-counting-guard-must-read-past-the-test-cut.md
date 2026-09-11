---
id: asp_a-counting-guard-must-read-past-the-test-cut_ktg434
slug: a-counting-guard-must-read-past-the-test-cut
name: >-
  `production_lines` stops at the first `mod tests`, so a guard that counts
  callers of a private item can be walked straight past — twice, measured
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:22:43.578Z'
updated_at: '2026-09-11T20:22:43.578Z'
last_verified: '2026-09-11T20:22:43.578Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/src/keyboard.rs
  - crates/app/src/pty_output.rs
symbols:
  - production_lines
  - outside_the_test_modules
  - a_gate_that_is_private_on_purpose_still_has_its_counted_callers
  - paste
content_hashes:
  crates/app/tests/shape.rs: 'sha256:dafb615269cba5e6910c01d4fe77981f158c73a3cec9b2e736bd3728e0f7de99'
  crates/app/src/keyboard.rs: 'sha256:674ef9ba2cadfedfe82afb9a68b85b927a7365571191ca99b6863ddc634c1af3'
  crates/app/src/pty_output.rs: 'sha256:e86ddf6c839cd5597c18e2a1a61ee4128cae5d001d589c146bead8f237cf3f2d'
tags:
  - guards
  - d28
  - d30
  - mutation
feature_id: feat_term-core_nx0g1r
---

## What it does

**`production_lines` in `crates/app/tests/shape.rs` breaks at the FIRST line starting with `mod tests`, so every top-level item declared BELOW a test module is invisible to every guard built on it.** Measured September 2026: `crates/app/src/keyboard.rs` keeps **162 production lines** down there (`Action`, `Chord`, `key_named`, `Keymap` and four `impl` blocks, from line 2129) and `pty_output.rs` keeps `DeafConsole`.

Consequences, all verified by mutation rather than argued:

- `no_module_is_over_its_ceiling` reads `keyboard.rs` as 388 lines when it is about 550.
- `no_impl_block_is_over_its_ceiling` never sees those four blocks.
- **A guard counting the doors to a deliberately private gate can be walked straight past.** Appending `pub fn compose_a_paste_without_the_gate(t: &str) -> Vec<u8> { paste(t, false) }` to the end of `keyboard.rs` — a third, public, ungated door to the paste gate, in the very module the guard names — left it **GREEN**.

**A second bypass of the same guard, and it is the shape the crate is now using**: it looked its module up by **exact name** in `MODULES`, so a child module could not reach it. Creating `src/keyboard/held.rs` with `super::paste(text, false)`, registered in `MODULES` as the list rule demands, also left it **GREEN**. `measure.rs` was split into `measure/header.rs` in that same delivery, so the shape is live, not hypothetical.

> **A counting guard over a private item has to remove the test blocks by counting braces — not truncate at the first one — and to read every `MODULES` entry under the target's own path.** Otherwise it counts the doors it can see and calls that all of them. Both fixes were re-mutated and both go red.

**The ceilings still truncate, deliberately**: changing `production_lines` would move every recorded ratchet number at once. The blindness was already written in `settings.rs`'s prose as «filed rather than fixed»; what it had never had is a measurement showing a guard passing over a real defect because of it. It is in `docs/DEBT.md`.

## Where it lives

`crates/app/tests/shape.rs` — `production_lines`, `outside_the_test_modules` and `a_gate_that_is_private_on_purpose_still_has_its_counted_callers`. The gate itself is the private `paste` in `crates/app/src/keyboard.rs`, whose docblock says why it is not `pub`.
