---
id: asp_shared-codex-routing-needs-the-route-owning-server_02e6gf
slug: shared-codex-routing-needs-the-route-owning-server
name: >-
  Portable route storage and upstream Windows AF_UNIX do not authenticate the
  server that owns a shared Codex route
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T22:33:12.800Z'
updated_at: '2026-11-25T22:33:12.800Z'
last_verified: '2026-11-25T22:33:12.800Z'
file_paths:
  - crates/panel-control/src/codex_route.rs
  - crates/panel-control/src/codex_relay.rs
  - crates/panel-control/src/client.rs
  - crates/panel-control/src/bin/panel-mcp.rs
  - crates/panel-control/src/bin/ivy-codex.rs
symbols:
  - Identity
  - called_routed
  - 'Producer::of'
  - 'Producer::of_server'
content_hashes:
  crates/panel-control/src/codex_route.rs: 'sha256:d05e3f7725ada762ea0e06c320a3411b175ea22a7533d285e82609db9927999e'
  crates/panel-control/src/codex_relay.rs: 'sha256:211080a6b0cb3afe880e7d58a3c5cb556c583cedc653c271c32bf3cfc3d0fb0c'
  crates/panel-control/src/client.rs: 'sha256:f2a408cc1582b5097a9cbd4926a3c91050e6c07f21f5261df00734215b2bca92'
  crates/panel-control/src/bin/panel-mcp.rs: 'sha256:b00f0d0e4b06cdf58ea3b7951a021b4f8ec0014c3082a56dc44a23ecf0980ce8'
  crates/panel-control/src/bin/ivy-codex.rs: 'sha256:dc102c184e260e6fb93efbeecd13829ef59da471fb71950e4c3b63e53f168f96'
tags:
  - windows
  - codex
  - shared-daemon
  - server-identity
  - d92
feature_id: feat_codex-shared-relay_5eba8q
---

## What it does

The shared daemon environment cannot identify the console that owns an individual route. Ivy strips PANEL_SERVER_PID while starting the shared Codex daemon; route Identity v1 carries no expected server PID. The routed MCP path calls Producer::of, whose Windows connection obtains its expected PID from inherited context, rather than supplying a per-route PID through Producer::of_server.

Therefore storage parity and upstream transport support are different proofs from Windows relay eligibility. The official Codex daemon/proxy investigated for #173 supported Windows AF_UNIX; calling that transport unsupported would hide the actual missing authenticated route identity. A future relay change must carry and verify the route-owning server identity before using that connection; it cannot recover it from the shared daemon environment.

D92 records the product's accepted unsupported-relay behavior. The Windows launcher witness preserves the original client arguments and status and requires the explicit Ivy reason; it does not silently add --no-daemon or claim local routing verified. Evidence: #173 source review and exact native launcher witness in PR #256/run45696334411. This entry records the integration constraint, not a replacement for the decision record.

## Where it lives

crates/panel-control/src/codex_route.rs::Identity; codex_relay.rs daemon output/environment; client.rs::Producer::of and Producer::of_server; bin/panel-mcp.rs::called_routed; bin/ivy-codex.rs launcher witness. D92 in docs/02-decisions.md.
