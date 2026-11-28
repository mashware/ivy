---
id: feat_codex-official-allowances_k0d5m7
slug: codex-official-allowances
name: Codex allowance reads need response identity and startup evidence
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T13:42:32.917Z'
updated_at: '2026-11-28T17:50:36.931897Z'
last_verified: '2026-11-28T17:50:36.931897Z'
file_paths:
- crates/app/src/usage/allowances/native.rs
- crates/app/src/usage/allowances/wire.rs
- crates/app/src/usage/allowances.rs
- crates/app/src/usage/report.rs
- docs/02-decisions.md
symbols: []
content_hashes: {}
tags:
- ai-usage
- codex
- native-client
---

## What it does

In Codex CLI 0.167.2, a rate-limit response can contain valid-looking allowance windows even after the app-server fails to match the backend result to its active user. The result account ID identifies the workspace, which can be shared by several users. The Linux native fixture returned populated windows with the same workspace ID but omitted `ordinaryUsageAllowed` when the user changed. A matching response can carry `false` in that field. Its presence is the release-specific identity witness. Require the complete, stable session, user and workspace tuple and the matching response account ID. A fresh process reads the selected account for each new observation. It does not make a reading follow an external account switch after publication.

D132 (#283) removes the 0.167.2 release pin. On Linux x86_64, each read still starts a fresh official client, requires its app-server to report the same release, and checks the stable complete session/user/workspace tuple, matching response workspace ID and boolean `ordinaryUsageAllowed` field. The account-and-user binding meaning of that field was tested with matching and mismatching users only on 0.167.2; 0.172.0 was observed with a matching user only. A release that emits the boolean unconditionally could therefore pass another user's reading from the same workspace. The cross-release semantic gap is explicitly accepted in D132; field presence and type do not establish universal identity binding.

Closing AI usage leaves allowance consent, the observation and the in-flight read active. While enabled, the next read starts five minutes after the previous read finishes, including while the modal is closed or the window is too narrow to draw the chip. Disabling clears the observation and cancels the read. A published observation remains bound to the identity checked during that read; later scheduled reads can observe a changed login, but this is not a live account watcher.

Private stdio isolates the connection but says nothing about app-server startup. The installed client's default startup contacted unrelated plugin services in the loopback fixture. Using `--disable plugins` and the `excludeResetCreditDetails` quota option reduced the observed routes to account routing and allowance reads without editing configuration. Native startup also creates its own runtime files and can request account routing before the first RPC. Admission evidence needs the launch and auth paths, not just the method allowlist.

## Where it lives

Product behavior and scope are recorded in D129, as amended by D132. The background consent/cadence lifecycle lives in `crates/app/src/usage/allowances.rs`; measurement suppresses chip/background work through `crates/app/src/usage/report.rs` and `usage::begin_pass`. The native launch and handshake remain in `crates/app/src/usage/allowances/native.rs`, and response binding/parsing remain in `crates/app/src/usage/allowances/wire.rs`. The native account-and-user mismatch evidence described above applies to Codex release rust-v0.167.2.
