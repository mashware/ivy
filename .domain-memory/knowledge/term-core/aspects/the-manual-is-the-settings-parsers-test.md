---
id: asp_the-manual-is-the-settings-parsers-test_zp2gmv
slug: the-manual-is-the-settings-parsers-test
name: >-
  There is no list of accepted settings keys in the code: the list is a table in
  the manual, a test walks it, and the walk carries a floor
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:54:15.630Z'
updated_at: '2026-09-09T21:54:15.630Z'
last_verified: '2026-09-09T21:54:15.630Z'
file_paths:
  - crates/app/src/settings.rs
  - docs/08-configuration.md
symbols:
  - apply
  - the_document_publishes_keys_the_application_accepts
content_hashes:
  crates/app/src/settings.rs: 'sha256:4094267b69277606646a9cfd8696332fa80ffdc67b1b23f5414a628ca6011d3a'
  docs/08-configuration.md: 'sha256:fa17a9be37ccb673b4a05d37ab8a826ee33729472a50e1366fad8621caee7210'
tags:
  - settings
  - guards
  - D26
feature_id: feat_term-core_nx0g1r
---

## What it does

**`the_document_publishes_keys_the_application_accepts` (`crates/app/src/settings.rs`) `include_str!`s `docs/08-configuration.md`, hooks on the line `| Key |`, takes the first backticked cell of every row after it, and feeds each one through the real `apply`.** Verified by reading the function, not from a summary.

Two consequences for anyone adding a key:

- **The doc edit is not documentation, it is the test.** A key added to `apply` and not to that table is never exercised; a key in the table that `apply` rejects fails the suite.
- **The walk carries a hard floor of rows**, and its own comment says why: *«without a floor this test passes by being empty»* — change the table's header and the `skip_while` never hooks, leaving a green test that reads nothing. **Adding a key means editing the table AND bumping the floor in the same commit**, and forgetting the second is a red suite, which is the point.

The same is true of the sibling guard that re-parses the manual's **example blocks** through the same parser, with a floor of its own: an example that stops being valid fails the build. Adding one documented example therefore moves two numbers.

> **A guard that reads another file dies silently when that file changes shape.** This one is alive because somebody gave it a number to disagree with — the same lesson as [[a-guard-binds-only-when-a-number-forces-it]], applied to a walk instead of a ratchet, and the same family as [[a-new-module-in-app-joins-five-lists]].

One practical note: the walk tries each key with two values (`#ff0000`, `14`) and only asks that the key is not rejected as **unknown**, so a key whose values are neither a colour nor a number — `cursor.blink`, which takes `on`/`off` — still passes by returning an invalid-value error. The table proves the key exists; its values need their own test.

## Where it lives

`crates/app/src/settings.rs`, in the `mod tests` block, against `docs/08-configuration.md`'s `| Key |` table and its fenced example blocks.
