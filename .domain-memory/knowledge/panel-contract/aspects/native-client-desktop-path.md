---
id: asp_native-client-desktop-path_gzk5fy
slug: native-client-desktop-path
name: >-
  Native client discovery must account for desktop and shell environment
  differences
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-19T12:42:23.485Z'
updated_at: '2026-11-19T12:42:23.485Z'
last_verified: '2026-11-19T12:42:23.485Z'
file_paths:
  - crates/app/src/client_attestation/programs.rs
  - crates/app/src/panel/overlay/tests.rs
  - docs/02-decisions.md
symbols:
  - discover
  - resolve_launch
  - native_claude_outside_desktop_path_admits_hook_and_clears_wait
content_hashes:
  crates/app/src/client_attestation/programs.rs: 'sha256:457ed91fa2b5b6af8d8e8d8eb0935fe5e2a811fb9a89498ab9d514704fe84fd8'
  crates/app/src/panel/overlay/tests.rs: 'sha256:975d9f80249b528b1f46a2586a1d7ba9061a72ebf0913a8d6034a4a6b733a2b5'
  docs/02-decisions.md: 'sha256:1a742c3f1efe68e3e62abebe4dd2c375942cc48826f986a0cb28a602e7d1147a'
tags:
  - native-clients
  - hook-publication
  - environment
  - regression-testing
feature_id: feat_panel-contract_8y2gc0
---

## What it does

A desktop application's PATH is not evidence of what its interactive shell can launch: shell startup can add native user installations after Ivy starts. A supported client can hold its panel yet lack the native witness required for descendant hook publications. Adding source labels or copying pane identity cannot substitute for that witness. Recognize a bounded installation candidate while retaining exact canonical executable, native peer ancestry, foreground, lifetime and same-invocation checks.

Tests that inject already-recognized programs bypass this discovery boundary. Cover at least one native-installation discovery through actual socket peer/PTY ancestry and assert the observable effect (a hook clears waiting attention); remove only the discovery fallback to prove the test fails. Unsupported existing PATH candidates must retain precedence rather than silently choosing another installation.

## Where it lives

Issue 142, verified 2026-11-19 on Linux with native Claude 2.1.328. Live Ivy PATH omitted ~/.local/bin while its foreground Claude invocations used that directory. The same compiled discovery source failed under desktop PATH and succeeded under console PATH. The fix's discovery and socket/PTY regression failed when the fallback was removed, and passed after restoration. Native macOS and the installed desktop/Claude hook cycle were not verified.
