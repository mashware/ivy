---
id: asp_receipt-order-is-not-freshness_90edd5
slug: receipt-order-is-not-freshness
name: Freshness timestamps cannot order independent same-second publications
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-28T17:39:40.372Z'
updated_at: '2026-11-28T17:39:40.372Z'
last_verified: '2026-11-28T17:39:40.372Z'
file_paths:
  - crates/app/src/usage/quota_broker.rs
  - crates/app/src/usage/quota_view.rs
  - crates/app/src/usage/quota_broker/tests.rs
symbols:
  - chip_source
content_hashes:
  crates/app/src/usage/quota_broker.rs: 'sha256:1783da61fb6d2229b29d6210f48419152ba4dbf2fb045e2ffc2f348aacf8260a'
  crates/app/src/usage/quota_view.rs: 'sha256:456c689f5c2dead178c4292751926a7cfd3950268b89ccfba2306657be431c7c'
  crates/app/src/usage/quota_broker/tests.rs: 'sha256:70426eeb3eb75f86c7b52b7d47bc372a00e165c38ca07b5c8fabd03f056dc2f0'
tags:
  - ordering
  - freshness
  - concurrency
  - timestamps
feature_id: feat_term-core_nx0g1r
---

## What it does

A freshness timestamp and a publication order answer different questions. Whole epoch seconds
can date a reading but cannot select the latest independent producer when two readings arrive in
the same second: max_by_key over installation order chooses its last tied element. The observed
A/B reducer selected B=22 after a later A=88 at second 100; advancing A to 101 changed the
result, exposing the tie. Use a strict shared receipt sequence for source selection while
retaining epoch timestamps for freshness and display. Read the sequence and snapshot
consistently under the same owner lock. Expiry and termination are state changes, not new
producer receipts, so they must not renew receipt order or freshness; repeated fresh same-launch
snapshots retain their existing wake coalescing.

## Where it lives

crates/app/src/usage/quota_broker.rs (Model publication and Wake receipt sequence);
crates/app/src/usage/quota_view.rs (chip_source); crates/app/src/usage/quota_broker/tests.rs.
Verified 2026-11-28 with same-second latest-source and expiry controls plus mutations restoring
timestamp-only selection or renewed expiry order.
