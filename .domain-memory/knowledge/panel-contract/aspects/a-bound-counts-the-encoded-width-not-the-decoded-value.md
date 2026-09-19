---
id: asp_a-bound-counts-the-encoded-width-not-the-decoded-value_0rc43h
slug: a-bound-counts-the-encoded-width-not-the-decoded-value
name: >-
  A bound on a JSON field counts the ENCODED width, a line's bound does not
  count its delimiter, and a field with NO bound spends the line's budget —
  three ways one transport kills a legal message
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T18:59:35.266Z'
updated_at: '2026-09-19T14:30:01.805Z'
last_verified: '2026-09-19T14:30:01.805Z'
file_paths:
  - crates/panel-control/src/wire.rs
  - crates/panel-control/src/bin/panel-mcp.rs
  - crates/panel-control/src/listener.rs
symbols:
  - MAX_SOURCE
  - ENVELOPE
  - MAX_REQUEST
  - Framing
  - panel_patch
content_hashes:
  crates/panel-control/src/wire.rs: 'sha256:d13326fcea3c48d140e2b92d8ab60bd05016e1d1b791a31da2e7581501c948f8'
  crates/panel-control/src/bin/panel-mcp.rs: 'sha256:5018d8564ce29fe9dd695b78796ffe1bba1a1c44eadb760fc1a3d8afe93240ac'
  crates/panel-control/src/listener.rs: 'sha256:1ea36077d3d28cb88cfb2fef742dd403cfe9d3f67e84e4009e833b377d5d0921'
tags:
  - transport
  - bounds
  - wire
  - off-by-one
  - review
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Three bounds on the same transport, wrong in three different ways, and none of them visible in a diff: the refusals they produce are for messages the other transport accepts.**

The control channel carries a request as one JSON line: `{"tool":…,"source":…,"document":…}\n`. Two ceilings meet there — the contract's own on the document (`panel_contract::MAX_BYTES`, the same one the file transport enforces) and the line's (`MAX_REQUEST`, a transport bound that is the document's ceiling plus `ENVELOPE`). **A publication the file accepts and the socket refuses is the two-transport drift the contract forbids**, and here it did not merely refuse: an over-long line is `Unrecoverable`, so the connection ends.

**One: the bound was counted on the decoded value, and the wire carries the encoded one.** `MAX_SOURCE` (256) was checked as `source.len()` on the decoded `String`. A name of 256 scalars written as `\uXXXX` escapes is **1 536 bytes** on the wire — six times the bound, and a non-BMP character is two escapes — so a legal name plus a document at the ceiling produced a line past `MAX_REQUEST`. The fix checks `source.get().len()`, the raw JSON as it arrived, **before the document is even looked at**, and answers `Malformed`, so the connection lives on and the producer can retry under a shorter name.

> **What a ceiling protects decides which form you count.** The reservation the parser makes is about the decoded value; the room a line needs is about the encoded one. `MAX_SOURCE` protects the line, so it counts bytes with their quotes and escapes — and the test sizes its name at `MAX_SOURCE - 2` precisely because the quotes are part of what is measured.

**Two: the delimiter was counted as content.** The MCP bridge (`panel-mcp`) reads with `take(MAX_REQUEST + 1).read_until(b'\n')` and compared `line.len() > MAX_REQUEST` **with the trailing newline still in the line**. A message of exactly the ceiling was flagged over — and the failure does not stop there: the over-long path then swallows to the next newline, and since this line had already ended, the swallow **ate the following message**, which the client was left waiting on. The fix is two lines: `let ended = line.last() == Some(&b'\n'); let content = line.len() - usize::from(ended);` and swallowing **only when `!ended`**.

**Three (September 2026, `panel_patch`): a NEW field of the envelope shipped with no bound at all, and it is the same failure from the other side.** `ENVELOPE` is 1 KiB, sized for the fixed keys plus a source name. `ops` is neither: it is the one field of this channel that can legitimately carry a whole document's worth of text — sixty `replace` ops rewriting keyed lines of a near-ceiling panel — and nothing held it. Past `MAX_REQUEST` the line is `Unrecoverable`, the connection ends, **and because a patch requires the hold, the hold dies with it**: the panel falls back to its file and the producer has to republish the whole document, which is exactly the cost the verb exists to remove. Fixed the way `MAX_SOURCE` already does it — `ops.get().len()` against `MAX_BYTES`, refused as `Malformed`.

> **The rule, now with three instances behind it: every new field of this envelope needs a bound of its own, on the encoded bytes, or it silently spends the line's budget and breaks some OTHER field's legal request.**

**And a test for such a bound has to be sized into the window where the verb can answer at all.** Built at twice the ceiling it proves nothing: the framing rejects the line before any verb is known, so it fails down the connection-ending path and reads exactly as though the bound were missing. The reachable window is `ENVELOPE` wide — ops a little over `MAX_BYTES` still make a line under `MAX_REQUEST`. The first version of that test was written at `MAX_BYTES * 2` and failed for this reason.

**How all three were found: by reading the code, not the diff.** None is reachable from the changed lines — the first needs you to ask what `len()` counts on a `&RawValue`, the second what `read_until` leaves in the buffer, the third what `ENVELOPE` was sized against. A reviewer handed the diff sees bounds being enforced and nothing else.

**And the mirror trap, from the same round, is a false positive worth keeping.** A reviewer claimed `stream.set_recv_timeout(..).and(stream.set_send_timeout(..))` short-circuits, leaving the send timeout unarmed when the first call fails. **It does not**: `Result::and` takes its argument **by value**, so the argument is evaluated before the call — both timeouts are always armed, and only the returned error is the first one's. A short-circuit claim in Rust is about `&&`, `?`, `and_then` and `or_else`, never about `and`/`or`.

## Where it lives

`crates/panel-control/src/wire.rs` — `MAX_SOURCE` and its check on `source.get()`, the same shape on `ops` in `fn panel_patch`, `ENVELOPE`, `MAX_REQUEST`, `Framing`; tests `a_source_over_the_bound_is_refused_without_ending_the_connection` and `a_patch_the_wire_can_refuse_refuses_the_line_and_not_the_connection`. `crates/panel-control/src/bin/panel-mcp.rs` — the `ended`/`content` pair before `content > MAX_REQUEST`, and `swallow_to_newline`. `crates/panel-control/src/listener.rs` — the `and` of the two socket deadlines. Related cards: [[the-worst-case-for-painting-is-the-inverse-of-reading]], [[a-window-bound-request-must-wake-the-frame]], [[a-published-sentence-is-traced-through-display]].
