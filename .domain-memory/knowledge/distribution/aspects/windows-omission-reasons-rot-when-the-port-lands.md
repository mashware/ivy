---
id: asp_windows-omission-reasons-rot-when-the-port-lands_66wd22
slug: windows-omission-reasons-rot-when-the-port-lands
name: >-
  A Windows unix-only omission reason is a claim nothing re-checks: when a port
  lands, the omissions naming it keep lying
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-27T13:50:21.875Z'
updated_at: '2026-11-27T13:50:21.875Z'
last_verified: '2026-11-27T13:50:21.875Z'
file_paths:
  - scripts/windows-test-omissions.json
  - scripts/windows-test-report.py
  - crates/panel-control/src/codex_relay.rs
  - crates/panel-control/src/session/tests.rs
symbols:
  - windows_unix_omission
  - 'codex_relay::supported'
  - validate_cwd
content_hashes:
  scripts/windows-test-omissions.json: 'sha256:71d725fedb376221cda97e025cfcfc0f51092ee00792008a31ef51350adaa9f4'
  scripts/windows-test-report.py: 'sha256:fdf10b369903662da9ea4234663603238fd857dbe22c3366852f66cf44a68057'
  crates/panel-control/src/codex_relay.rs: 'sha256:211080a6b0cb3afe880e7d58a3c5cb556c583cedc653c271c32bf3cfc3d0fb0c'
  crates/panel-control/src/session/tests.rs: 'sha256:08e046bcb0511a41462363d46ba4a680bc7b2b0f2533e4972ac646256da85f1c'
tags:
  - windows
  - tests
  - omissions
feature_id: feat_distribution_kb0x1w
---

## What it does

A `unix-only:` reason in `scripts/windows-test-omissions.json` claims something about the
code as it is now. `scripts/windows-test-report.py` only checks that the `#[ignore]` string
matches the inventory entry exactly. It never checks that the reason is true.

Two cases found in #185 (PR #279):
- #256 ported Codex route atomic replacement to Windows, but three `codex_relay` tests stayed
  omitted as "binding a Codex route needs atomic replacement, which is not ported to Windows".
- Three `session::tests` were omitted because their fixtures hard-coded `/tmp`. The code
  (`validate_cwd`, `Path::is_absolute`) accepts `C:\...`.

**Rule:** a PR that ports a capability to Windows also searches the omission inventory for
reasons that name the capability. Each of those tests is either un-gated, or given a reason
that is still true.

**Facts that decide reasons in this area:**
- The shared Codex relay runs on Linux and macOS only (`codex_relay::supported`).
- `IVY_CODEX_ROUTE` is set only by the relay, so routed panel-mcp bridges do not exist on
  Windows.
- The Windows control channel opens with `authority = None`, so it has no hook authority
  (#278).

**Checking for Windows from Linux:**
- `cargo clippy --tests --target x86_64-pc-windows-msvc -p panel-control -- -D warnings`
  works, because the target is installed.
- `-p app` cannot be cross-checked: `ring` needs MSVC's `lib.exe`.
- The real test is the PR's `windows-startup-checks` job. Its test output comes in the
  `windows-fixture-evidence` artifact (`gh run download <run> -n windows-fixture-evidence`).

## Where it lives

scripts/windows-test-omissions.json, scripts/windows-test-report.py, crates/panel-control/src/lib.rs (windows_unix_omission!), crates/panel-control/src/codex_relay.rs (supported)
