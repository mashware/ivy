---
id: feat_codex-shared-relay_5eba8q
slug: codex-shared-relay
name: 'Shared Codex relay: official catalog frame sizing'
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T10:55:28.666Z'
updated_at: '2026-11-25T10:55:28.666Z'
last_verified: '2026-11-25T10:55:28.666Z'
file_paths:
  - crates/panel-control/src/codex_relay.rs
  - crates/panel-control/src/codex_relay/transport_tests.rs
  - docs/02-decisions.md
symbols:
  - connect_upstream
  - serve
  - read_upstream
  - ProxyStream
content_hashes: {}
tags:
  - codex
  - websocket
  - transport
  - catalog
---

## What it does

The shared Codex relay keeps the official server as conversation owner while forwarding the client's protocol. Official plugin catalogs may arrive as a single large WebSocket frame, so frame length and aggregate-message length are separate compatibility constraints. Setting a message limit alone does not allow a frame above the decoder's independent frame limit.

A read-only Codex 0.172.0 probe on 25 November 2026 returned one 48,300,053-byte plugin/list frame. The relay's former tungstenite 0.30.0 default constructor rejected the declared header length against 16,777,216 bytes even though the message was below the existing 64 MiB ceiling. The configured constructor and actual complete relay forwarded that catalog and a subsequent request; reverting the constructor made the portable regression fail with the exact former capacity error. This is measured version-specific evidence, not a version-based eligibility rule.

Counting inbound byte progress prevents false idle expiry during slow frames, but does not override capacity checks. Keep the finite frame/message envelope, authentication, per-console proof and shared owner when addressing catalog compatibility. Protocol payload bounds do not equal total process RSS because decoded frames, JSON and serialization introduce additional transient allocations.

## Where it lives

crates/panel-control/src/codex_relay.rs: connect_upstream, ProxyStream, read_upstream and serve. Portable framing/routing/bounds regression: crates/panel-control/src/codex_relay/transport_tests.rs. Decision rationale: docs/02-decisions.md D92 catalog amendment. Evidence: issue 251, real official proxy frame and old-constructor mutation, 25 November 2026.
