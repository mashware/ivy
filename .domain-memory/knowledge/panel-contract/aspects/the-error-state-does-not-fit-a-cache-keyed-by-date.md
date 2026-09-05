---
id: asp_the-error-state-does-not-fit-a-cache-keyed-by-date_ntvqc4
slug: the-error-state-does-not-fit-a-cache-keyed-by-date
name: >-
  A passing failure left the panel accused for life: the error state does not
  fit in a cache that skips the work by comparing dates
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:23:16.227Z'
updated_at: '2026-08-13T13:23:16.227Z'
last_verified: '2026-08-13T13:23:16.227Z'
file_paths:
  - crates/app/src/panel.rs
symbols:
  - 'PanelState::refresh'
  - 'PanelState::warning'
  - problem
content_hashes:
  crates/app/src/panel.rs: 'sha256:568e67aba57fd9f60e9ca802161c515776057b5d550f6074bf1af4cf561d98a8'
tags:
  - cache
  - errors
  - EMFILE
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The failure.** A **passing** read failure —an `EMFILE`, and a terminal host is exactly who runs out of descriptors— left the panel marked as unreliable **forever**. The reason is the optimization itself: the problem was only cleared by a good re-read, and **a file nobody touches again does not change its date**, so there is never another re-read. The user saw «what you are seeing is not the latest» over a panel that was in fact perfectly up to date.

> **The condition that clears an error state is «we tried again and it worked», and the optimization of a cache keyed by date is precisely not trying again.** The two things cannot live in the same place. Every time a failure is stored next to a cached datum, it has to be said explicitly what erases it.

**The fix.** The problem is also cleared when the re-read returns «there is nothing new to read»: that is already proof that the previous failure is over — the date could be looked at, which is what the failure prevented.

**And the criterion that collapsed along with it.** There were **two** parallel criteria for «there is something to show»: the width tiling asked whether there was a good panel, and the painting asked something else. With a problem and no good panel, one said yes and the other said no. Unified into `is_visible()`, which is the only question: is there a good panel **or** is there something to confess?

## Where it lives

`PanelState::refresh` in `crates/app/src/panel.rs`: it re-reads the `panel.json` only if its date changed, and keeps the last good panel and the last problem separately.
