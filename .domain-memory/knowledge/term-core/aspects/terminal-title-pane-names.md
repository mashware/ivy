---
id: asp_terminal-title-pane-names_d0v7md
slug: terminal-title-pane-names
name: >-
  Follow explicit terminal titles without treating them as canonical session
  identity
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-08T18:56:12.993Z'
updated_at: '2026-11-08T18:56:12.993Z'
last_verified: '2026-11-08T18:56:12.993Z'
file_paths:
  - crates/term-core/src/screen.rs
  - crates/term-core/src/screen/title.rs
  - crates/app/src/pane_view.rs
  - docs/02-decisions.md
  - docs/08-configuration.md
symbols:
  - 'Screen::terminal_title'
  - 'pane_view::display_label'
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:4c3448d86f920fed011b8bf70967493a48437869bfa11df8249520c753c11236'
  crates/term-core/src/screen/title.rs: 'sha256:70ae33512342ed95bb8cc6bc68b1b3cca86b4bed21839731b6d214e51e56e085'
  crates/app/src/pane_view.rs: 'sha256:7e4634f41064e4f81d4ead98ff350a5a19d4b80eacb554b80c3352a449aac1cf'
  docs/02-decisions.md: 'sha256:f763764fbf47658b7e46de1125f7d73b942e2959520609df645d3c1a5afbc821'
  docs/08-configuration.md: 'sha256:67c688ada14ab3bf8b9463f97c4850553ecc40676d188e65a7f419ef24b06fd3'
tags:
  - terminal-title
  - pane-names
  - session-identity
feature_id: feat_term-core_nx0g1r
---

## What it does

The user explicitly accepted client-published terminal titles, including prefixes and
shortening, for pane naming. These are producer declarations, not inferred TUI state or
authenticated session identity. A nonblank manual pane label wins; received titles remain
transient because layout restore launches fresh programs. This avoids per-client private-file
readers and adapters when a shared title protocol meets the requested behavior. Silent process
exit is not a title reset and must not be guessed from unrelated output.

## Where it lives

Decision D69, feature #334. Evidence: explicit user choices and desktop/client acceptance on
2026-11-08; standard parser title events exercised by six new core tests and pane behavior by
synthetic UI/PTY tests. Codex, Claude Code and OpenCode formatting/configuration limits are
documented in docs/08-configuration.md. User confirmed the real desktop and client checklist; no
screenshot or measured performance claim.
