---
id: asp_re-serialising-a-reply-sorts-its-keys_z9709h
slug: re-serialising-a-reply-sorts-its-keys
name: >-
  serde_json without preserve_order sorts a map's keys, so anything read through
  a Value hands back a document its producer did not publish
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T21:01:25.203Z'
updated_at: '2026-09-18T21:01:25.203Z'
last_verified: '2026-09-18T21:01:25.203Z'
file_paths:
  - crates/panel-control/src/client.rs
  - crates/panel-control/src/wire.rs
symbols: []
content_hashes:
  crates/panel-control/src/client.rs: 'sha256:7dc82db426c7ba17cbe2309b369574c7f2af74ecfcb91c01e68fae396f57c2ce'
  crates/panel-control/src/wire.rs: 'sha256:90675d332e6786a426c0c017bd6ef5a3c219f69534f287ab7ae62001270e9244'
tags: []
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**`serde_json`'s map is sorted unless the `preserve_order` feature is on, and it is not on anywhere in this workspace.** So `Value::to_string()` over something that arrived as JSON returns the same document with its keys alphabetised.

On the control channel that quietly undid the one decision the reply was built around. `Reply::Document` is the **only** arm that interpolates its payload raw instead of escaping it — because escaping a document would roughly double a large one and make the client refuse an answer the file transport accepts, which is the two-transport drift the contract forbids. The client then read that reply through a `Value`: a document published as `{"updated_at":…,"phase":…,"lines":…}` came back `{"lines":…,"phase":…,"updated_at":…}` — the same panel, and not the same bytes — while `Answered::Document`'s own docblock said it was *the JSON text it arrived as*.

> **The fix is the one the request side already takes for the same reason**: borrow the field as a `RawValue` and hand back `.get()`. The manifest turns `raw_value` on precisely for this, and the comment explaining why lives on the request's envelope — which is why the reply side was written without it.

**And the test that binds it asserts on the ORDER of the keys.** A test that parsed the answer and compared documents — the obvious shape — passes on exactly the broken version, because the panel really is the same. Reproduced by printing `raw.get("document")` on a known line before fixing anything.

## Where it lives

`crates/panel-control/src/client.rs` (`Producer::read_reply`, the borrowed `Reply<'a>` struct, `Answered::Document`) and `crates/panel-control/src/wire.rs` (`Reply::Document`, the arm that does not escape).
