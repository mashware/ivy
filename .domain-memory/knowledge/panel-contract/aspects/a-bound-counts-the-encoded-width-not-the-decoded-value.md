---
id: asp_a-bound-counts-the-encoded-width-not-the-decoded-value_0rc43h
slug: a-bound-counts-the-encoded-width-not-the-decoded-value
name: >-
  A bound on a JSON field counts the ENCODED width, and a line's bound does not
  count its delimiter — each turns a publication the file accepts into a killed
  connection
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T18:59:35.266Z'
updated_at: '2026-09-16T18:59:35.266Z'
last_verified: '2026-09-16T18:59:35.266Z'
file_paths:
  - crates/panel-control/src/wire.rs
  - crates/panel-control/src/bin/panel-mcp.rs
  - crates/panel-control/src/listener.rs
symbols:
  - MAX_SOURCE
  - MAX_REQUEST
  - ENVELOPE
  - Framing
  - swallow_to_newline
content_hashes:
  crates/panel-control/src/wire.rs: 'sha256:90675d332e6786a426c0c017bd6ef5a3c219f69534f287ab7ae62001270e9244'
  crates/panel-control/src/bin/panel-mcp.rs: 'sha256:4e0f13f9047be03d870d44cb842e191f356352e238e05672d7e23219c342373d'
  crates/panel-control/src/listener.rs: 'sha256:2af6d0bfb151c93e2fc56a245fa3a08b7264d1237e385787c3b65fe30d8779d6'
tags:
  - transport
  - bounds
  - wire
  - off-by-one
  - review
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Two bounds on the same transport, wrong in opposite directions, and neither is visible in a diff: the refusals they produce are for messages the other transport accepts.**

The control channel carries a `panel_set` as one JSON line: `{"tool":…,"source":…,"document":…}\n`. Two ceilings meet there — the contract's own on the document (`panel_contract::MAX_BYTES`, the same one the file transport enforces) and the line's (`MAX_REQUEST`, a transport bound that is the document's ceiling plus `ENVELOPE`). **A publication the file accepts and the socket refuses is the two-transport drift the contract forbids**, and here it did not merely refuse: an over-long line is `Unrecoverable`, so the connection ends.

**One: the bound was counted on the decoded value, and the wire carries the encoded one.** `MAX_SOURCE` (256) was checked as `source.len()` on the decoded `String`. A name of 256 scalars written as `\uXXXX` escapes is **1 536 bytes** on the wire — six times the bound, and a non-BMP character is two escapes — so a legal name plus a document at the ceiling produced a line past `MAX_REQUEST`. The fix checks `source.get().len()`, the raw JSON as it arrived, **before the document is even looked at**, and answers `Malformed`, so the connection lives on and the producer can retry under a shorter name.

> **What a ceiling protects decides which form you count.** The reservation the parser makes is about the decoded value; the room a line needs is about the encoded one. `MAX_SOURCE` protects the line, so it counts bytes with their quotes and escapes — and the test sizes its name at `MAX_SOURCE - 2` precisely because the quotes are part of what is measured.

**Two: the delimiter was counted as content.** The MCP bridge (`panel-mcp`) reads with `take(MAX_REQUEST + 1).read_until(b'\n')` and compared `line.len() > MAX_REQUEST` **with the trailing newline still in the line**. A message of exactly the ceiling was flagged over — and the failure does not stop there: the over-long path then swallows to the next newline, and since this line had already ended, the swallow **ate the following message**, which the client was left waiting on. The fix is two lines: `let ended = line.last() == Some(&b'\n'); let content = line.len() - usize::from(ended);` and swallowing **only when `!ended`**.

**How both were found: by reading the code, not the diff.** Neither is reachable from the changed lines — the first needs you to ask what `len()` counts on a `&RawValue`, the second what `read_until` leaves in the buffer. A reviewer handed the diff sees two bounds being enforced and nothing else.

**And the mirror trap, from the same round, is a false positive worth keeping.** A reviewer claimed `stream.set_recv_timeout(..).and(stream.set_send_timeout(..))` short-circuits, leaving the send timeout unarmed when the first call fails. **It does not**: `Result::and` takes its argument **by value**, so the argument is evaluated before the call — both timeouts are always armed, and only the returned error is the first one's. A short-circuit claim in Rust is about `&&`, `?`, `and_then` and `or_else`, never about `and`/`or`.

## Where it lives

`crates/panel-control/src/wire.rs` — `MAX_SOURCE` and its check on `source.get()`, `ENVELOPE`, `MAX_REQUEST`, `Framing`; test `wire::tests::a_source_over_the_bound_is_refused_without_ending_the_connection`, whose second half asserts that a name at the bound plus a ceiling document is still accepted. `crates/panel-control/src/bin/panel-mcp.rs` — the `ended`/`content` pair before `content > MAX_REQUEST`, and `swallow_to_newline`. `crates/panel-control/src/listener.rs` — the `and` of the two socket deadlines. Related cards: [[the-worst-case-for-painting-is-the-inverse-of-reading]], [[a-window-bound-request-must-wake-the-frame]].
