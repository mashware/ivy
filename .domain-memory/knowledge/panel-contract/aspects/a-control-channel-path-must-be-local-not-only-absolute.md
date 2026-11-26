---
id: asp_a-control-channel-path-must-be-local-not-only-absolute_24d6w2
slug: a-control-channel-path-must-be-local-not-only-absolute
name: >-
  A path that arrives over the control channel must be checked as local, not
  only absolute: on Windows is_absolute lets UNC and device paths through
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T18:37:21.298Z'
updated_at: '2026-11-22T18:37:21.298Z'
last_verified: '2026-11-22T18:37:21.298Z'
file_paths:
  - crates/app/src/viewer/load.rs
  - crates/panel-control/src/wire.rs
symbols: []
content_hashes:
  crates/app/src/viewer/load.rs: 'sha256:0a8f7015078b4f4646688ae5a29277ba050f48b056eb6f82d20a40c2ae317b03'
  crates/panel-control/src/wire.rs: 'sha256:fd6b245377b462701043e9698d8fa5990cb2a08bf61de2f3dcddd8bbbcc3273f'
tags:
  - security
  - windows
  - control-channel
  - image
feature_id: feat_panel-contract_8y2gc0
---

## What it does

A path an agent sends over the control channel (a tool call, a hand-written wire line) is
read by the app itself, without the agent client's own read prompt. On Windows
`Path::is_absolute` is true for UNC (`\\host\share\a.png`), verbatim UNC (`\\?\UNC\...`)
and device (`\\.\...`) paths, so an is_absolute check lets them through and the image
worker's `open_regular` (load.rs) then authenticates over SMB to a remote host on a
prompt-injected agent's word. Refuse `Prefix::UNC`, `VerbatimUNC` and `DeviceNS` at the wire.

The same design round settled two more things about such a path. An agent names its file
explicitly, so the format is decided by the bytes (decode), not by an extension gate:
`is_image` exists to spot paths in printed console text. And `MAX_REQUEST` framing already
bounds a line, so a separate path-length cap adds nothing.

Evidence: the trust and design challengers of #198 read load.rs:179-206 and
wire.rs:52,60,265 on origin/main baf8f0b; std docs for `Path::is_absolute` and `Prefix`.

## Where it lives

crates/panel-control/src/wire.rs (where an agent's path is parsed); crates/app/src/viewer/load.rs (the worker that opens it)
