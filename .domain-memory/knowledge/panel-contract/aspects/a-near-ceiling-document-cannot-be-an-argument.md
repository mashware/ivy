---
id: asp_a-near-ceiling-document-cannot-be-an-argument_6p1dj2
slug: a-near-ceiling-document-cannot-be-an-argument
name: >-
  A near-ceiling panel cannot travel as one argv string (E2BIG), and the
  written-out size is fed by reads and patches, never by a publish
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T15:18:55.034Z'
updated_at: '2026-09-25T15:18:55.034Z'
last_verified: '2026-09-25T15:18:55.034Z'
file_paths:
  - crates/panel-control/examples/publish.rs
  - scripts/measure-frame.sh
  - crates/app/src/control.rs
symbols:
  - largest_serialized
  - document_as_a_producer_reads_it
  - MAX_ARG_STRLEN
content_hashes:
  crates/panel-control/examples/publish.rs: 'sha256:d9d74c01484e59293aff51e3d25d692ebf7d32f5940d2875f59d33358647ad95'
  scripts/measure-frame.sh: 'sha256:465f0d9927c9fbebf177895a7c675b43daa5fd1baa5c2ab6e50b91c84020a588'
  crates/app/src/control.rs: 'sha256:9a23d2967d0b7b20be3119428b4b413d87bd5a04d7d43cc64418104913283879'
tags:
  - measurement-bench
  - panel-control
  - d47
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**One: a document near the contract's 256 KiB ceiling cannot be handed to a process as ONE argument.** Linux refuses any single argument longer than `MAX_ARG_STRLEN` — 32 pages, 128 KiB with 4 KiB pages — with `E2BIG`, independently of `ARG_MAX`. The bench publisher (`crates/panel-control/examples/publish.rs`) took its document as argv and could never have published the large pair's 249 KB panel; it now reads it from standard input for every channel batch, and the script pipes `document_of` into it. The bytes on the wire do not change: `Producer::publish` re-parses the document and sends the compact value (`client.rs:69`).

**Two: `largest_serialized` — the report's «bytes the largest written out» — is fed by reads AND patches, never by a publication** (`control.rs`: the read path and the patch arm). Two consequences, both met in #248:
- A test that reads the document back BEFORE asserting a patch's written-out size is asserting the READ's number — the patch could feed nothing and the assertion would pass. Take the counters first, read after.
- A batch that only publishes (S, X) reports 0, so the report cannot prove how large its document was. The large pair's control proves its size before anything is published, on a lower bound of the compact form (bytes minus three per newline, because each newline of that body is followed by at most two spaces), and the suite runs the script's own functions to check the compact size lands between 7/8 of the ceiling and the ceiling.

**Measured with it (25 September 2026):** one patch costs 4.13–4.68 µs on a 456-byte panel and 1.823–1.835 ms on a 249 462-byte one, in release — about 11 % of a 60 Hz frame, before the re-layout. Recorded in D47.

## Where it lives

`crates/panel-control/examples/publish.rs` (stdin), `scripts/measure-frame.sh` (`publish_over_the_channel`, `large_filler`, `LARGE_FLOOR`), `crates/app/src/control.rs` (`a_patch_near_the_ceiling_applies_and_is_counted_at_that_size`, `measure_a_patch_at_the_ceiling`), D47 in `docs/02-decisiones.md`.
