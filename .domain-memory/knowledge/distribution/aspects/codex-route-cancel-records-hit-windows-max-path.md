---
id: asp_codex-route-cancel-records-hit-windows-max-path_cwrdmg
slug: codex-route-cancel-records-hit-windows-max-path
name: >-
  Codex route cancel records have names of up to 207 characters, and Windows
  refuses a path over 260 with ERROR_PATH_NOT_FOUND
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-27T13:50:22.003Z'
updated_at: '2026-11-27T13:50:22.003Z'
last_verified: '2026-11-27T13:50:22.003Z'
file_paths:
  - crates/panel-control/src/codex_route.rs
  - crates/panel-control/src/codex_relay.rs
  - crates/panel-contract/src/windows_private_file.rs
symbols:
  - cancel_path
  - 'Registry::cancel_epoch'
content_hashes:
  crates/panel-control/src/codex_route.rs: 'sha256:d05e3f7725ada762ea0e06c320a3411b175ea22a7533d285e82609db9927999e'
  crates/panel-control/src/codex_relay.rs: 'sha256:211080a6b0cb3afe880e7d58a3c5cb556c583cedc653c271c32bf3cfc3d0fb0c'
  crates/panel-contract/src/windows_private_file.rs: 'sha256:9951a42e2604727f2189e64e1c4e028b8ab8859c02c3d39b9530443d9dc3a27f'
tags:
  - windows
  - codex
  - max-path
feature_id: feat_distribution_kb0x1w
---

## What it does

`codex_route::cancel_path` names a cancel record `cancel-<hex of the epoch>`. A 64-hex epoch
from `codex_relay::nonce` gives a 135-character name. The longest allowed epoch, 100 bytes,
gives 207.

`windows_private_file` opens paths without the `\\?\` verbatim prefix, so on Windows the
whole path must stay under 260 characters.

**What happened.** On the windows-2025 runner, `cancel_epoch` failed with Os code 3
(ERROR_PATH_NOT_FOUND), not with a length error. The path was temp_dir, plus an 80-character
test home, plus `ivy-panel-routes`, plus the record. The fix was a test home with a short name
(PR #279).

**Production is not affected today**, because the relay does not run on Windows. Whoever ports
the relay to Windows must either bound these paths or open them with the verbatim prefix.

**Tests that create route storage use short home names.** A NotFound from a Windows route test
may mean the path was too long, not that a directory is missing.

## Where it lives

crates/panel-control/src/codex_route.rs (cancel_path, Registry::cancel_epoch); crates/panel-contract/src/windows_private_file.rs
