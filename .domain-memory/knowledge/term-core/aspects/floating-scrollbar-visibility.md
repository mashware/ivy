---
id: asp_floating-scrollbar-visibility_tj3rcr
slug: floating-scrollbar-visibility
name: Floating scrollbars can be invisible even when always enabled
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T21:13:26.138Z'
updated_at: '2026-11-25T21:13:26.138Z'
last_verified: '2026-11-25T21:13:26.138Z'
file_paths:
  - crates/app/src/viewer/code.rs
  - crates/app/src/ui/style.rs
symbols:
  - show
content_hashes:
  crates/app/src/viewer/code.rs: 'sha256:dbb3a2f6a2e89eb1b6682aeb6e996694dd2ad410cebde0a993472110a6203a99'
  crates/app/src/ui/style.rs: 'sha256:5d6f44f1c29ed1f565b9aeef4433419e3076f029ddaea38a253ea78bc07faef1'
tags:
  - tests
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does
 egui 0.36 default ScrollStyle::floating uses zero dormant handle/background opacity
independently of ScrollBarVisibility. AlwaysVisible alone cannot establish an idle
position indicator; a custom selection-owned drag also suppresses ScrollArea hover
activation. Test settled visible painted handles and real handle-driven document motion,
rather than checking the visibility enum or scroll extents.

## Where it lives

crates/app/src/viewer/code.rs, crates/app/src/ui/style.rs; symbols: show. Evidence:
issue 245 regression tests and targeted mutation checks (November 2026).
