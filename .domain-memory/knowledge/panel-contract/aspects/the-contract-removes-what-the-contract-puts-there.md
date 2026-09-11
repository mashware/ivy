---
id: asp_the-contract-removes-what-the-contract-puts-there_j6f4e8
slug: the-contract-removes-what-the-contract-puts-there
name: >-
  The pane's directory is cleaned up non-recursively on purpose, so a second
  file of the app's own makes every pane close print an error
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:21:55.481Z'
updated_at: '2026-09-11T20:21:55.481Z'
last_verified: '2026-09-11T20:21:55.481Z'
file_paths:
  - crates/panel-contract/src/lib.rs
symbols:
  - 'PanelDir::drop'
  - remove
  - PANEL_FILE
content_hashes:
  crates/panel-contract/src/lib.rs: 'sha256:89f2403cb9e6b68c083cc6b3305065b05da811214801cb502ad15341310a9e5a'
tags:
  - lifecycle
  - phase-3
  - cleanup
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Anything the app leaves in `$PANEL_DIR` besides `panel.json` makes every pane close print an error on stderr.** `remove` deletes `PANEL_FILE` and then calls a plain `std::fs::remove_dir` (`crates/panel-contract/src/lib.rs:127-136`); a non-empty directory fails, and `Drop for PanelDir` prints *«the panel directory {} remains ({failure})»* (`:99-113`).

The non-recursion is deliberate and its reason must not be traded away: *«the directory belongs to this crate, but its content belongs to the producer: a recursive delete would wipe out whatever it had left there, and would turn any badly built path into data loss instead of an error.»*

> **So a milestone that puts a second thing of its own in that directory — a control socket, a lock, a pid file — pays for it in `panel-contract`'s `remove`, beside `PANEL_FILE`, and nowhere else.** The endpoint is the crate's, exactly like the document, so removing it there keeps the stated rule true rather than bending it.

And **not** by unlinking it from the app before the drop: destructor ordering is something this repo has already been bitten by, where moving a value out of a struct field into a thread silently moved *when* its destructor ran and hid a month-old hang. See [[moving-a-value-into-a-thread-moves-its-drop]].

This is the concrete trap that sank the «one socket per pane, in `$PANEL_DIR`» proposal as it was first written in 226's approach panel: the idea was right — the directory's `0700` becomes the access control and `PanelDir::create` is already the uniqueness check — and the cleanup was the part nobody had read.

## Where it lives

`crates/panel-contract/src/lib.rs` — `Drop for PanelDir` at `:99`, the free `remove` at `:119-136`, `PANEL_FILE` at `:38`, and the `0o700` in `create_dir` at `:143-155`.
