---
id: asp_rejected-optional-scope-properties-can-leave-stub-units_dazajx
slug: rejected-optional-scope-properties-can-leave-stub-units
name: >-
  Rejected optional systemd properties can leave a stub: basic isolation needs a
  distinct retry name
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-19T00:15:55.724Z'
updated_at: '2026-11-19T00:15:55.724Z'
last_verified: '2026-11-19T00:15:55.724Z'
file_paths:
  - crates/term-core/src/scope.rs
  - crates/term-core/src/scope/protocol_tests.rs
  - docs/02-decisions.md
symbols:
  - place_connected
  - unsupported_oom_property
content_hashes:
  docs/02-decisions.md: 'sha256:26e0f37182cb3eb25578f22cdfa6006c6946c11b3453ec592de5729c143f198d'
tags:
  - linux
  - systemd
  - console-isolation
  - compatibility
  - testing
feature_id: feat_term-core_nx0g1r
---

## What it does

Rejecting an optional property does not guarantee that StartTransientUnit left no unit behind. An unsupported OOMPolicy request can leave a stub until manager garbage collection; retrying the same name therefore races that collection and can discard otherwise supported basic isolation.

The compatibility retry uses another owned scope name while preserving the PID and app.slice placement. Only an exact unsupported-property response permits this retry: authorization failures or invalid values must not silently change meaning. A successful basic scope remains referenced through final-result observation even when OOM attribution is unavailable.

Evidence: #124 exposed the retained-stub behavior against the real user manager. The private real-D-Bus protocol test asserts both requests, unchanged PID/Slice, omitted optional property only on retry, RefUnit retention and eventual release. Removing the retry or reference makes those assertions fail. Verified 2026-11-19; production candidate6cb89bb, native CI44425194003 passed.

## Where it lives

crates/term-core/src/scope.rs — place_connected and unsupported_oom_property; crates/term-core/src/scope/protocol_tests.rs — unknown_oom_policy_retries_the_actual_protocol_and_retains_the_basic_scope; docs/02-decisions.md D104.
