---
id: feat_repository-test-guards_7afk35
slug: repository-test-guards
name: 'Repository source guards: external test module identity'
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T10:55:30.641Z'
updated_at: '2026-11-25T10:55:30.641Z'
last_verified: '2026-11-25T10:55:30.641Z'
file_paths:
  - crates/app/tests/waits.rs
  - crates/app/tests/scripts.rs
  - crates/panel-control/src/codex_relay/transport_tests.rs
symbols:
  - every_blocking_wait_in_production_code_is_bounded_or_declared_abandoned
  - the_list_still_names_every_module
content_hashes: {}
tags:
  - testing
  - source-guards
  - rust
---

## What it does

The repository's source guards inspect external Rust files independently. An outer #[cfg(test)] on a parent's mod declaration does not identify the external source as test-only to that scanner; an all-tests file must carry an inner #![cfg(test)] attribute. Otherwise a legitimate test-thread join is classified as a production blocking wait, and adding artificial production wait explanations would disguise the file identity problem.

A separate manual module census includes the source text in the shell-script safety scanner. A new external test file therefore also needs its MODULES entry so the guard reads its scripts rather than silently losing coverage.

Evidence on 25 November 2026: an extracted transport test module produced four undeclared-production-join findings; the inner marker made all 14 wait-guard tests pass while the three transport regressions still ran. Adding the census include made its five script-guard tests pass. Neither guard's assertions were weakened.

## Where it lives

crates/app/tests/waits.rs: every_blocking_wait_in_production_code_is_bounded_or_declared_abandoned and external-source test-only attribute fixtures. crates/app/tests/scripts.rs: MODULES and the_list_still_names_every_module. Concrete correctly marked/registered fixture: crates/panel-control/src/codex_relay/transport_tests.rs. Evidence: issue 251 full workspace validation.
