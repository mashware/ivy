---
id: asp_the-panel-layout-cache-is-not-an-identity_n41y0g
slug: the-panel-layout-cache-is-not-an-identity
name: >-
  `PanelLayout` looks like it could say whether the panel is still the one that
  was painted, and it cannot — `refresh` throws it away
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:21:46.508Z'
updated_at: '2026-09-11T20:21:46.508Z'
last_verified: '2026-09-11T20:21:46.508Z'
file_paths:
  - crates/app/src/panel.rs
symbols:
  - PanelLayout
  - 'PanelState::refresh'
  - clip
  - lay_out_if_needed
content_hashes:
  crates/app/src/panel.rs: 'sha256:3c0d1e79d24f73e5d63d2afc73f53f5bd0f088c5359920e22440f2eb41c06f1b'
tags:
  - panel
  - gestures
  - caching
  - phase-3
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**`PanelLayout` is a cache key about *what has to be redone*, not a record of *what was drawn*.** Its fields are `width`, `warning` and `rows` (`crates/app/src/panel.rs:197-204`), and `PanelState::refresh` sets `self.layout = None` the moment a newer document arrives (`:212-221`, the `Ok(Some(panel))` arm). So anything that re-walks the layout in a later part of the frame silently gets the **new** layout, with nothing left to compare against.

The temptation this matters for: any feature where a gesture must resolve against what the user actually read — a button that runs a command being the case that forced it. **Two cheap answers both fail:**

- Re-walking `lay_out_if_needed` and `clip` in the pointer pass yields the new layout, because the cache was invalidated between the two.
- Keying on the document's `updated_at`, or on a producer-supplied sequence number, is defeated exactly by a producer that swaps two entries while keeping the date — and the date is the producer's, which is the whole problem.

> **The shape that works is for the painter to record what it drew — the rows, their rectangles and a hash of them — and for the later pass to match against that record.** It is also cheaper than the re-walk, because the frame then pays one layout instead of two, and it removes the argument that the routing needs to live next to the private `clip`.

Same family as the trap in [[a-window-test-needs-the-panel-published]]: the panel's geometry only exists once something real produced it, and inventing it a second time produces something that is not what the window did.

## Where it lives

`crates/app/src/panel.rs` — `PanelLayout` at `:196-204`, `PanelState::refresh` at `:212`, `clip` at `:857`, `lay_out` at `:544`.
