---
id: asp_a-typed-option-cannot-tell-null-from-absent_brhqqy
slug: a-typed-option-cannot-tell-null-from-absent
name: >-
  Moving a decode from serde_json::Value to typed Option fields silently turns a
  present JSON null into «absent»
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T13:22:57.189Z'
updated_at: '2026-09-26T13:22:57.189Z'
last_verified: '2026-09-26T13:22:57.189Z'
file_paths:
  - crates/panel-control/src/client.rs
  - crates/panel-control/src/wire.rs
  - crates/panel-control/src/bin/panel-mcp.rs
symbols:
  - Reply
  - present
  - 'Answered::Opened'
content_hashes:
  crates/panel-control/src/client.rs: 'sha256:cb907b2ff0c927e12a152eeafd05ed3c70fb58295deeb2325e065170bb063d32'
  crates/panel-control/src/wire.rs: 'sha256:dcd3c35e99c8dc840b65976eff5d342da50d6e8826d64063cd1b0ab352e41433'
  crates/panel-control/src/bin/panel-mcp.rs: 'sha256:632271ddff7cdd1720765291f7fc98fe0447c5d157c3c13ee3c8aae8f05de875'
tags:
  - serde
  - 'null'
  - control-channel
  - test-disguise
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**`Value::get("pane")` returns `Some(Value::Null)` for a present null; serde's own `Deserialize` for `Option<T>` returns `None` for it — the same as a missing field.** So swapping a `Value` lookup for a typed struct of `Option<&RawValue>` fields changes what a null means, and nothing in the diff shows it.

That swap happened on the control channel's reply in #244, for a good reason (a document read through a `Value` came back with its keys sorted — [[re-serialising-a-reply-sorts-its-keys]]). But `{"ok":true,"pane":null}` — an open whose console has no addressable identity — then fell through the whole decode chain to `Answered::Ok`, and the MCP bridge worded that «published». The comment above the arm, «Present and possibly null», was carried across unchanged and became false. Fixed in #279.

**Why no test saw it:** the round-trip test opened with `Opened(Some("h-1"))` only, the wire's test checks the bytes it writes (`{"ok":true,"pane":null}`) and never what the client reads back, and the bridge's text for `Opened(None)` was never asserted from any value. Each half was tested; the null crossing the boundary was not.

**The fix keeps the field type**: `#[serde(default, borrow, deserialize_with = "present")]`, where `present` deserialises the `&RawValue` and wraps it in `Some`. serde only calls `deserialize_with` for a field that is present, and `default` supplies `None` for an absent one — so a null arrives as `Some(RawValue("null"))`. `Option<Option<_>>` does not help on its own: serde folds the null into the outer `None` there too.

**Rule:** where a null and an absence are two different answers, a plain `Option` cannot carry them — and the test must drive the null through the real decode (here: a real listener answering `Opened(None)`), never assert on a hand-built value. The request side (`wire.rs`) has the same null-as-absent fields; harmless today because null and absent mean the same there or are refused.

## Where it lives

`crates/panel-control/src/client.rs` — `Reply::pane`, `present`, the decode in `read_reply`; test `an_open_with_no_addressable_identity_comes_back_as_an_open`. Bridge text pinned in `crates/panel-control/src/bin/panel-mcp.rs` by `an_open_answers_with_its_handle_or_says_it_has_none`.
