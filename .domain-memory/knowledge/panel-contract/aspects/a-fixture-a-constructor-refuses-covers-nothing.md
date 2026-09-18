---
id: asp_a-fixture-a-constructor-refuses-covers-nothing_1sra6c
slug: a-fixture-a-constructor-refuses-covers-nothing
name: >-
  A sixth test disguise: the fixture a checking constructor silently refuses —
  the branch it was meant to exercise ends up covered by nothing, and the test
  reads as thorough
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T21:01:11.082Z'
updated_at: '2026-09-18T21:01:11.082Z'
last_verified: '2026-09-18T21:01:11.082Z'
file_paths:
  - crates/panel-contract/src/panel.rs
symbols: []
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:86d7abe2bfa147112ee41dbec48c61ebd692bd74450c0239a534e744ec90f221'
tags: []
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The writer's `actions` branch could be DELETED ENTIRELY with all 82 tests green.** The test meant to cover it, `every_shape()`, built its buttons with `Action::new(&format!("button {at}"), &format!("echo {at}\n"))` — and `Action::new` refuses any character the panel would not paint as itself, a line break included. `filter_map` dropped all eight, `panel.actions` came out empty, and `if !panel.actions.is_empty()` was never true in any test of the crate.

> **The disguise is not in the assertion this time — it is in the fixture.** Every previous one this project has recorded is about what a test *checks*; this one is about what a test *builds*. The assertions were right, the panel was right, and the value they were written for never existed.

**The general rule: a fixture that goes through a constructor returning `Option` or `Result` is asserted on — count what came back — or the test covers whatever the constructor let through, which may be nothing.** `filter_map`, `flatten`, `ok()` and `unwrap_or_default` over a fixture are the shapes to distrust: each one turns a refused fixture into an empty one, silently, with the test still passing.

**How it was found, and it is worth copying**: a blinded contract reviewer noticed the `send` carried a newline while reading `Action::new`'s refusals for a different reason. It was then confirmed the only way that counts — by deleting the branch and watching the suite stay green — and fixed by dropping the newline, after which the same deletion fails `a_document_that_made_one_round_trip_never_changes_again`.

## Where it lives

`crates/panel-contract/src/panel.rs` — `Action::new` (the checking constructor), `is_shown_as_itself` (what it refuses), and the `every_shape()` fixture in its test module.
