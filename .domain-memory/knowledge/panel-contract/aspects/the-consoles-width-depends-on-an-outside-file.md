---
id: asp_the-consoles-width-depends-on-an-outside-file_x62rkj
slug: the-consoles-width-depends-on-an-outside-file
name: >-
  The width announced to a console depends on a file from outside: whoever
  computes grids without re-reading the `panel.json` is wrong by 36 columns
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T16:50:20.249Z'
updated_at: '2026-08-16T16:50:20.249Z'
last_verified: '2026-08-16T16:50:20.249Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/panel.rs
  - crates/app/src/watcher.rs
symbols:
  - 'Window::measure_the_ones_out_of_sight'
  - 'Pane::refresh_its_panel'
  - 'Pane::tile_with_its_panel'
  - 'panel::set_aside'
  - 'Flag::take'
content_hashes:
  crates/app/src/lib.rs: 'sha256:b27adaa55359fbc81c3b0a62f6993d4aecce7bd71aeff4f8faee1484d59dbe87'
  crates/app/src/panel.rs: 'sha256:bb0c2831a73e9318e084df0e0ac888d48ef188b7bb87d678dea35bc49458c572'
  crates/app/src/watcher.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
tags:
  - panel
  - grid
  - measurement
  - F2.1
  - tabs
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**A pane's panel state was only refreshed when the pane was painted**, and **how much width its console has left** depends on that state: `panel::set_aside` takes `DEFAULT_WIDTH = 36` columns away from it when there is a panel to show (`panel.rs:32`). The two things together produce a failure that is not obvious from either of them separately.

**The concrete case (F2.1, PR #64).** When giving a grid to the tabs that are not painted, the first version did not re-read their `panel.json`: for it «there is no panel», so it measured the console with the **whole rectangle** — 93 columns instead of 57. And it does not last a frame: it lasts **the whole time in the background**. That is, the same defect the fix came to remove ([[the-background-tab-lives-in-another-window]]), with the sign flipped and **bigger** than the original one of 80 columns.

**The justification that let it through was false, and it is worth writing down because it sounds reasonable:** «re-reading is touching the disk once per pane per frame». It is not. `refresh_its_panel` goes out through `Flag::take`, an **atomic `swap`** (`watcher.rs:73`), and it only reaches the file on the frame in which the watcher gave notice that **that** `panel.json` changed — which is exactly when it is needed. The watcher dispatches by exact path to all the watched ones, including those of background tabs.

**The worst case does exist and it is the degraded mode:** with the watcher blind, the logic half raises the flags of **all** the panes every 500 ms. With 8 tabs that is 16 reads concentrated in one frame instead of 2, some 100-200 µs. It is declared in full, so it does not falsify D3's figure.

**Resolved in PR #64 (`9702da5`):** `Window::measure_the_ones_out_of_sight` re-reads the panel of every background pane, tiles the gap with `tile_with_its_panel` and **times what it costs**, which travels in `panel_reread_unpainted` until the pumping's round trip — not as a round trip of its own, for what is said in [[the-repeated-pass-re-pumps-and-overwrites-the-round]].

> **The rule:** when the size announced to a program depends on a file from outside, **whoever computes that size has to read that file**, or they will be measuring against an old photo. And if they read it inside the frame, they have to declare what it costs: in an instrument that works by subtraction, the undeclared read is published as the framework's cost.

**And one about testing:** no test pinned the panel's gap — the mutation to `rect.size()` survived with 261 green, because the test panes are born with no panel directory and for them the two expressions give the same thing. The number chosen was what decided whether the defect showed.

## Where it lives

`crates/app/src/lib.rs` — `Window::measure_the_ones_out_of_sight`, `Pane::refresh_its_panel`, `Pane::tile_with_its_panel`. `crates/app/src/panel.rs:32` — `panel::set_aside` and its `DEFAULT_WIDTH`. `crates/app/src/watcher.rs:73` — `Flag::take`, the atomic `swap`.
