---
id: feat_panel-contract_8y2gc0
slug: panel-contract
name: >-
  panel-contract — the panel.json contract, how it is read and the directory
  where it lives
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:06:14.300Z'
updated_at: '2026-08-12T14:06:14.300Z'
last_verified: '2026-08-12T14:06:14.300Z'
file_paths:
  - crates/panel-contract/src/lib.rs
  - crates/panel-contract/src/panel.rs
  - docs/04-contrato-panel.md
symbols:
  - Panel
  - Line
  - Mark
  - Style
  - Problem
  - read
  - PanelDir
  - MAX_BYTES
  - MAX_INDENT
  - STALE_AFTER
  - PANEL_FILE
content_hashes:
  crates/panel-contract/src/lib.rs: 'sha256:eef1e94af37a47b31674f9c0a32cbb016c13a971396af680b7e6c0e74a2de17a'
  crates/panel-contract/src/panel.rs: 'sha256:2f88d49d982871a2367457566cc72b098caee9f6217d84e0ed9db26e29c6b7b7'
  docs/04-contrato-panel.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
tags:
  - contract
  - panel
  - untrusted-input
  - D6
---

## What it does

The box that knows **what** `panel.json` says and how to read it without anything it brings taking
the window down. And also where that file lives: each pane's directory, with its identity.

It exists apart from the painting layer for the same economic reason as `term-core`: the interface
is going to be rewritten (D3), and *what* is published and *where* are part of the contract, not of
the framework of the day. Nothing about generating a name, creating a directory and deleting it has
anything to do with painting.

**Its input is untrusted by definition.** The file is written by another process —an agent, a CI, a
deployment— and it can come from a cloned repo or from a container. That decides three things about
the design of the read: tolerance is **per field and not per document** (whoever publishes is an
agent writing JSON by hand, and losing the whole panel over a field nobody looks at would be the
worst thing that could be done); the size ceiling is applied **while reading**, not by checking
afterwards; and a regular file is demanded **before opening**, because a named pipe would block the
opening and reading happens inside the frame.

**And it does not interpret the producer's vocabulary** (D6): it keeps the six marks and the seven
styles with their literal names, and `phase` is free text that is painted as is. Translating it into
a color or an icon would be the app deciding what somebody else's work means.

## Where it lives

`crates/panel-contract/`. The types, the reading and the staleness in `src/panel.rs`; the pane's
directory (`PanelDir`) and the public constants in `src/lib.rs`. The specification this box
implements is in `docs/04-contrato-panel.md`, and it is a **public document of the project**:
whoever ports the prototype reads that, not the design. The bench that re-runs the numbers the code
claims, in `examples/read.rs`.
