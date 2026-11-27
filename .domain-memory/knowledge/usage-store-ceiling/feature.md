---
id: feat_usage-store-ceiling_wv1bbd
slug: usage-store-ceiling
name: >-
  The usage store's page ceiling must be sized from the bytes a real event
  takes: a guessed 192 MiB stopped a real Claude history at a sixth of the event
  ceiling
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-27T15:47:15.578Z'
updated_at: '2026-11-27T15:47:15.578Z'
last_verified: '2026-11-27T15:47:15.578Z'
file_paths:
  - crates/usage-core/src/ledger.rs
  - crates/usage-core/src/ledger_tests.rs
  - crates/usage-core/examples/measure_usage.rs
symbols:
  - MAX_PAGES
  - MAX_EVENTS
  - QUERY_TIME
content_hashes:
  crates/usage-core/src/ledger.rs: 'sha256:14642300ceacc1bee34910eb8f1b824ccd16a749d4c295ba9deadf489b214578'
  crates/usage-core/src/ledger_tests.rs: 'sha256:3d939cf772779756808c4e18c39e0b6926265487c19cea239ec7afb1317fcd52'
  crates/usage-core/examples/measure_usage.rs: 'sha256:f55b8aa1d2857f5e5118f9f1e885f477c4a662546f79e3a7c8b7713fb9be3f24'
tags:
  - usage
  - sqlite
  - measurement
---

## What it does

A real Claude Code event costs about 1.19 KB of SQLite pages in the usage ledger, with two
observations per event on average: 169 223 events filled exactly 49 152 pages of 4 KiB, and a
full 9.7 GB history of 360 671 events took 102 496 pages (~400 MiB).

The 192 MiB ceiling shipped with the first usage delivery was set without measuring an event. It
fired at a sixth of the one-million-event ceiling, before a first success time was recorded, and
the window then looked empty (#281, D127 amended: 1.25 GiB).

The synthetic witness `examples/measure_usage.rs` fills its gigabyte with user records that carry
no usage, so it cannot size the store. Any change to the schema, an index or either ceiling must
be checked against bytes per real event; `page_ceiling_fits_the_event_ceiling_at_the_bytes_an_event_takes`
ties the two ceilings at the measured 1 190 bytes. Still open: whether a 365-day query stays under
the 2 s QUERY_TIME near one million events.

Evidence: probe through the public Service and sqlite counts over the reporter's real store,
2026-11-27.

## Where it lives

crates/usage-core/src/ledger.rs (MAX_PAGES, MAX_EVENTS, QUERY_TIME); crates/usage-core/src/ledger_tests.rs; crates/usage-core/examples/measure_usage.rs
