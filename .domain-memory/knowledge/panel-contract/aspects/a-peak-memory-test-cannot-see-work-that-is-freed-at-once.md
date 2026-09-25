---
id: asp_a-peak-memory-test-cannot-see-work-that-is-freed-at-once_m4235t
slug: a-peak-memory-test-cannot-see-work-that-is-freed-at-once
name: >-
  A peak-memory test cannot see work that allocates and frees before the peak:
  to pin 'this path does no extra work', count reservations too
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T23:14:49.241Z'
updated_at: '2026-09-25T23:14:49.241Z'
last_verified: '2026-09-25T23:14:49.241Z'
file_paths:
  - crates/panel-contract/src/panel.rs
symbols:
  - while_measuring
  - while_counting
  - the_silent_readers_reserve_nothing_for_what_they_drop
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:6ae8abe521ff7f03c57eded34d78ab82781cb19eeccd21ba4d895e7ac59a93e4'
tags:
  - testing
  - memory
  - mutation
  - measurement
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The crate's counting allocator measured only the peak of live bytes.** A peak cannot see work that allocates and frees before the peak is reached.

#255 pinned "the file's reader records nothing for what it drops" by comparing the peaks of two same-size documents, one with known marks and one with dropped marks. A reviewer showed that rendering each dropped value and freeing it at once would leave the peak unchanged: the peak comes at the end, with the whole parsed value and every line live.

The counter now also counts reservations (a `RESERVATIONS` thread-local), and `while_counting` returns `(peak, reservations)`. Mutant M17 — render into a throwaway record on the silent path — is killed only by the count. Measured by the round-4 review with the silent document reader broken on purpose:

| | Reservations | Peak |
|---|---|---|
| Silent reader intact | 3004 | 397865 bytes |
| Silent reader broken | 4005 | 397982 bytes |

The peak moved by 117 bytes out of about 400 000; the count moved by a third.

**Rule: "this path does no extra work" needs traffic — allocation count or total bytes — as well as peak.** Peak alone pins what is held, not what is done. The allocator also lives only in the test binary, so the test compares two runs rather than asserting an absolute number.

## Where it lives

`crates/panel-contract/src/panel.rs` test module — `counting::{LIVE, PEAK, RESERVATIONS}`, the `Counter` global allocator, `while_measuring`, `while_counting`, and the test `the_silent_readers_reserve_nothing_for_what_they_drop`.
