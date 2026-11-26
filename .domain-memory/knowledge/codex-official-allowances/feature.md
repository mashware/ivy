---
id: feat_codex-official-allowances_k0d5m7
slug: codex-official-allowances
name: Codex allowance reads need response identity and startup evidence
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T13:42:32.917Z'
updated_at: '2026-11-26T13:42:32.917Z'
last_verified: '2026-11-26T13:42:32.917Z'
file_paths:
  - crates/app/src/usage/allowances/native.rs
  - crates/app/src/usage/allowances/wire.rs
symbols: []
content_hashes: {}
tags:
  - ai-usage
  - codex
  - native-client
---

## What it does

In Codex CLI 0.167.2, a rate-limit response can contain valid-looking allowance windows even after the app-server fails to match the backend result to its active user. The result account ID identifies the workspace, which can be shared by several users. The Linux native fixture returned populated windows with the same workspace ID but omitted `ordinaryUsageAllowed` when the user changed. A matching response can carry `false` in that field. Its presence is the release-specific identity witness. Require the complete, stable session, user and workspace tuple and the matching response account ID. A fresh process reads the selected account for each new observation. It does not make a reading follow an external account switch after publication.

Private stdio isolates the connection but says nothing about app-server startup. The installed client's default startup contacted unrelated plugin services in the loopback fixture. Using `--disable plugins` and the `excludeResetCreditDetails` quota option reduced the observed routes to account routing and allowance reads without editing configuration. Native startup also creates its own runtime files and can request account routing before the first RPC. Admission evidence needs the launch and auth paths, not just the method allowlist.

## Where it lives

The native launch and handshake live in `crates/app/src/usage/allowances/native.rs`. Response binding and quota parsing live in `crates/app/src/usage/allowances/wire.rs`. Product behavior and scope are recorded in D129. Codex release rust-v0.167.2 applies the active-account-and-user check in its app-server account processor.
