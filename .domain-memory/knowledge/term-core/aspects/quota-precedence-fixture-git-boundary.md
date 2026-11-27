---
id: asp_quota-precedence-fixture-git-boundary_6gxxeh
slug: quota-precedence-fixture-git-boundary
name: >-
  A temporary quota precedence fixture needs its own Git boundary to exclude
  invalid ancestor markers
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-27T22:15:02.955Z'
updated_at: '2026-11-27T22:15:02.955Z'
last_verified: '2026-11-27T22:15:02.955Z'
file_paths:
  - crates/app/src/usage/quota_setup.rs
symbols:
  - project_local_root
  - preview_uses_project_precedence_and_keeps_inherited_options
content_hashes:
  crates/app/src/usage/quota_setup.rs: 'sha256:f8fbfba51b3cfbfef59fb86ff11137729a58a45fa09d5ace52f665144ac9919a'
tags:
  - testing
  - fixtures
  - git
  - claude
  - settings
feature_id: feat_term-core_nx0g1r
---

## What it does

A temporary settings-precedence fixture can fail before reaching its assertions if it inherits an invalid ancestor .git marker. project_local_root detects the marker with try_exists, but real Git discovery resolves that directory as an ordinary folder; the mismatch is rejected as an unresolved Claude settings scope.

During #298 validation, /tmp/.git was an empty read-only directory. The existing quota precedence test failed in the complete suite and in a forced child-process run using the same temporary root. Giving the test project its own valid repository, with inherited Git routing/config variables removed and empty templates, restored the intended ancestry boundary while keeping the command, refresh interval and padding assertions unchanged. The forced run and full workspace suite then passed.

Changing TMPDIR merely hides this dependency. A test of local/project precedence should own the repository boundary it relies on instead of assuming that all ancestors of a temporary directory are Git-free.

## Where it lives

crates/app/src/usage/quota_setup.rs — project_local_root and preview_uses_project_precedence_and_keeps_inherited_options. Repair commit 9f3527c0; validation artifacts retain forced failing/passing output and the completed workspace suite.
