---
id: asp_a-record-for-one-transport-costs-the-other-nothing_d3hzyx
slug: a-record-for-one-transport-costs-the-other-nothing
name: >-
  A report only one transport needs, added to the reader both share, lands
  inside the frame unless the silent path skips it entirely
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T23:14:49.116Z'
updated_at: '2026-09-25T23:14:49.116Z'
last_verified: '2026-09-25T23:14:49.116Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/panel-contract/src/dropped.rs
  - crates/panel-contract/src/patch.rs
  - docs/02-decisions.md
symbols:
  - parse_noting
  - vocabulary
  - Dropped
  - parse_document_noting
  - parse_ops_noting
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:6ae8abe521ff7f03c57eded34d78ab82781cb19eeccd21ba4d895e7ac59a93e4'
  crates/panel-contract/src/patch.rs: 'sha256:43bdc42651cf6fb4b9608cffadbe356676b2a0604b601c0c6f25d15d9dcb35ba'
  docs/02-decisions.md: 'sha256:d2dcb9abf55336e4eec456ec7bdf9faf9da3aadc5306c5f699b6fc7bbdf5fa83'
tags:
  - frame-cost
  - D13
  - D58
  - transports
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The panel reader is shared by two transports (D17)**: the file's, which runs inside the frame, and the channel's, which runs on a connection thread. #255 (D58) made the tolerant reader record the vocabulary values it drops, so the channel's reply can name them.

The first version always recorded, and let the file's reader throw the record away. So every `panel.json` re-read inside the frame rendered each dropped value — a non-string value serialised whole — for a note nobody read. That is undeclared frame work, which D13 forbids, because the frame's cost is split by subtraction. It was invisible to the bench, whose documents use valid words.

**The fix is an `Option<&mut Dropped>` sink.** `None` — the file reader, `parse`, `parse_ops` — records nothing at all. `Some` is only the channel's `parse_document_noting` / `parse_ops_noting`. A bounded writer serialises a non-string value only as far as the 32-character cut. The built-in review found it, and a peak-plus-reservation-count test now pins it ([[a-peak-memory-test-cannot-see-work-that-is-freed-at-once]]).

**Rule: a shared reader's extra output for one caller is an optional sink, never a record the other callers build and throw away** — above all when one of those callers runs inside the frame.

## Where it lives

`crates/panel-contract/src/panel.rs` — `parse` (passes `None`), `parse_noting`, `read_line_noting`, `vocabulary`, `parse_document_noting`. `crates/panel-contract/src/patch.rs` — `parse_ops` / `parse_ops_noting` over `read_ops`. `crates/panel-contract/src/dropped.rs` — `Dropped`, `Head` (the bounded writer). D58, D13 in `docs/02-decisions.md`.
