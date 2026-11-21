---
id: asp_an-untouched-window-writes-no-layout_q13th9
slug: an-untouched-window-writes-no-layout
name: >-
  A window nobody rearranged writes no layout.json, and none at all under the
  default startup.restore = none: a launch test must opt in and change the
  layout
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T18:35:17.120Z'
updated_at: '2026-11-21T18:35:17.120Z'
last_verified: '2026-11-21T18:35:17.120Z'
file_paths:
  - crates/app/src/opening.rs
  - crates/app/src/session.rs
  - crates/app/src/restoration.rs
  - packaging/windows/test-launch.ps1
symbols:
  - with_these_tabs
  - save_window
  - can_save
  - check_whether_the_layout_moved
content_hashes:
  crates/app/src/opening.rs: 'sha256:4eae9f0281c9ba064354085d257cdf5c0213f3b9cbd8348c9cbf93f8cfd6ad0a'
  crates/app/src/session.rs: 'sha256:f47ef55c198eec500782c1db2fe3614047735bf73dec0af10a58348c03833e89'
  crates/app/src/restoration.rs: 'sha256:f5cda054f46073a9d3f8db05b985a27b466f23ff2c9096922737fc359b18cb01'
tags:
  - layout
  - persistence
  - testing
feature_id: feat_term-core_nx0g1r
---

## What it does

Found on 21 November 2026 when the installed-launch trial of #162 closed Ivy cleanly and found
no layout.json.

**Two separate gates, both by design.**

1. **`startup.restore = none` is the default**, and in that mode
`restoration::State::can_save()` is false, so nothing is read or written.
2. **An untouched window never writes.** `with_these_tabs` (opening.rs) records the window's
first mirror as `last_seen`, so `check_whether_the_layout_moved` never arms `save_due`, and
`save_window` returns early, on_exit included. This is D16's choice not to write half a
second after every opening.

A test that expects a saved layout must therefore opt in (`startup.restore = layout` in the
profile's ivy/config) **and** change the layout before closing. The installed trial presses
Ctrl+Shift+D, waits for the second shell, and requires exactly two saved consoles. Writing only
the setting still yields 'no saved layout'.

## Where it lives

crates/app/src/opening.rs (with_these_tabs), crates/app/src/session.rs (check_whether_the_layout_moved, save_window), crates/app/src/restoration.rs (can_save), packaging/windows/test-launch.ps1
