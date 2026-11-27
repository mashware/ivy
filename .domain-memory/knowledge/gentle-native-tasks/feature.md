---
id: feat_gentle-native-tasks_1hj07x
slug: gentle-native-tasks
name: >-
  Gentle helper reporting uses owner-projected snapshots and independent
  sequence epochs
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-27T22:15:00.757Z'
updated_at: '2026-11-27T22:15:00.757Z'
last_verified: '2026-11-27T22:15:00.757Z'
file_paths:
  - crates/app/src/chrome/gentle_task/transport/helpers.rs
  - crates/app/src/chrome/gentle_task/transport/process.rs
  - prototypes/gentle-rpc/helpers.py
symbols:
  - Monitor
  - Snapshot
content_hashes:
  crates/app/src/chrome/gentle_task/transport/helpers.rs: 'sha256:54c7f5f5fd4995a4935fe6debf82dd86801fc6ceb66aa39575309f51ccceaa1c'
  crates/app/src/chrome/gentle_task/transport/process.rs: 'sha256:0a866d838498c60b3e3b3bde172d9953272c3dc27295e0a179ce661a76d2ed7f'
  prototypes/gentle-rpc/helpers.py: 'sha256:5c3a8c579f69db765cad58f28853ba1050345c2c2c8cc1fe48a312117f999ebc'
tags:
  - gentle
  - rpc
  - helpers
  - ownership
  - testing
---

## What it does

Gentle Shell 5.1.0 projects at most 50 helpers through both list_subagents and agents_update. These are full snapshots of the active native owner, so a missing helper cannot be interpreted as completed. The client's private RPC pipe establishes session ownership; helper names can repeat across independent sessions.

The desktop event sequence is shared by all event types. Gaps between helper updates are normal, and a native session switch restarts the sequence and may publish replacement events before its receipt. Reset pending requests and sequence state at that ownership boundary. A newer full helper push supersedes both an outstanding list response and its timeout: protecting against the late response alone still lets a missing response mark newer state unavailable.

Capability advertisement is insufficient evidence that an owner exists. A real Pi host loading only the Desktop extension advertises list_subagents but answers agents_unavailable. Preserve the last-known snapshot with an explicit unavailable or disconnected indication when observation fails.

Evidence: issue #298 monitor regressions, independent owner/reset and late-timeout tests, and a successful two-client Gentle 5.1.0 probe where A cancelled while B stayed running and completed independently.

## Where it lives

crates/app/src/chrome/gentle_task/transport/helpers.rs — Monitor and Snapshot; crates/app/src/chrome/gentle_task/transport/process.rs — owner-generation coalescing; prototypes/gentle-rpc/helpers.py — real two-client probe.
