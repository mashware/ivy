---
id: asp_immutable-executable-fixture-etxtbsy_x995dx
slug: immutable-executable-fixture-etxtbsy
name: A forked sibling can keep a rewritten executable busy despite CLOEXEC
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-28T17:39:54.134Z'
updated_at: '2026-11-28T17:39:54.134Z'
last_verified: '2026-11-28T17:39:54.134Z'
file_paths:
  - crates/panel-control/tests/ivy_codex_launch.rs
  - crates/panel-control/src/bin/ivy-codex.rs
symbols:
  - preserves_client_signal
content_hashes:
  crates/panel-control/tests/ivy_codex_launch.rs: 'sha256:d486ce12f10ca1bfa7fe04f0ae0814aa33e3c3a1492beb053341d282b1b66c29'
  crates/panel-control/src/bin/ivy-codex.rs: 'sha256:dc102c184e260e6fb93efbeecd13829ef59da471fb71950e4c3b63e53f168f96'
tags:
  - testing
  - fork
  - cloexec
  - etxtbsy
  - signals
feature_id: feat_term-core_nx0g1r
---

## What it does

Closing a temporary executable's writer in the launching thread does not prove that no process
holds it writable. A sibling test can fork while that descriptor is open and retain it until
exec; CLOEXEC acts at exec, not at fork. The attempted launch can then fail with ETXTBSY before
the fake client runs, turning a signal-passthrough assertion into exit 127 with no signal. A
controlled inherited-writable-descriptor harness reproduced that exact failure. An immutable
/bin/sh client retained SIGTERM under the same condition, and the original temporary client
signalled normally after the descriptor was released. When the subject is exit or signal
passthrough, use an immutable executable on the same classification branch and retain the exact
signal assertion; accepting startup failure, retries or looser assertions would hide the fixture
defect.

## Where it lives

crates/panel-control/tests/ivy_codex_launch.rs (preserves_client_signal);
crates/panel-control/src/bin/ivy-codex.rs (passthrough classification). Verified 2026-11-28 with
forced ETXTBSY and immutable-client controls; the full workspace passed after the isolated
test-only repair.
