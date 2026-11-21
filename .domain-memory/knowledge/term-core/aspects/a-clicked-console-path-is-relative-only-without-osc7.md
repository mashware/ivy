---
id: asp_a-clicked-console-path-is-relative-only-without-osc7_fk5bk0
slug: a-clicked-console-path-is-relative-only-without-osc7
name: >-
  A path clicked in a console arrives relative exactly when the console sent no
  OSC 7: the handler must join it onto the console's real directory
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T23:24:45.428Z'
updated_at: '2026-11-20T23:24:45.428Z'
last_verified: '2026-11-20T23:24:45.428Z'
file_paths:
  - crates/app/src/links/local.rs
  - crates/app/src/links/console.rs
  - crates/app/src/viewer/origin.rs
symbols:
  - 'LocalTarget::parse'
  - 'PaintedConsole::local_at'
  - source
content_hashes:
  crates/app/src/links/local.rs: 'sha256:6e622c5a0d70a236ed429826022ff1ac39d5155129ca7357ee0cce6768eb860f'
  crates/app/src/links/console.rs: 'sha256:51c861fc5153b239d68594c8376f29f0e01f512c88d7918c9731f8157c7dc993'
  crates/app/src/viewer/origin.rs: 'sha256:8d544008dc9d84092682c9fd8dc0465dba57b31c7dfea78ca988ed1b25539327'
tags:
  - links
  - osc7
  - trap
feature_id: feat_term-core_nx0g1r
---

## What it does

`LocalTarget::parse` joins a relative path onto the cwd it is given, and its only production
caller (`PaintedConsole::local_at`) passes the SCREEN's OSC 7 directory. So a clicked path reaches
its handler still relative exactly when the console published no usable OSC 7 report (plain bash on
Linux, default zsh on macOS). The console's real directory from native discovery
(`pane.pty.working_directory()`, Linux and macOS) is only consulted by `viewer::origin::source()`
and by Files' `nav.source`.

Rule: any new handler of a clicked path joins a relative path onto `source(window, pane).cwd`
itself, and refuses only when that is `None` (a remote or invalid OSC 7 report). Refusing relative
paths breaks every shell without OSC 7 while the same path through Files works.

Evidence: regression in #178's image viewer found by the built-in review and confirmed by a skeptic
reading local.rs, console.rs and origin.rs at 4a333ce; fixed and tested in the same PR.

## Where it lives

crates/app/src/links/local.rs; crates/app/src/links/console.rs; crates/app/src/viewer/origin.rs (source)
