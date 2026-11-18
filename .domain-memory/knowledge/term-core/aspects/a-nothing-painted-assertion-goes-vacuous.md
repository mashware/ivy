---
id: asp_a-nothing-painted-assertion-goes-vacuous_2bnw62
slug: a-nothing-painted-assertion-goes-vacuous
name: >-
  A 'nothing was painted' assertion goes vacuous, silently, the day the thing it
  guards is drawn with another primitive
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T19:35:25.646Z'
updated_at: '2026-11-18T19:35:25.646Z'
last_verified: '2026-11-18T19:35:25.646Z'
file_paths:
  - crates/app/src/tests/integrated_chrome.rs
  - crates/app/src/ui/icons.rs
symbols:
  - strokes_in
  - painted_icon
  - install_icons_only
content_hashes:
  crates/app/src/tests/integrated_chrome.rs: 'sha256:6afc96f41006ad5fa7a7cdfe24109e5c1b3b246669049edd58ef6aa9b550fb5c'
tags:
  - tests
  - mutation
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

A test that proves something is NOT painted by counting shapes of one kind (`Path`, `LineSegment`)
inside a rect stops guarding anything when the subject moves to another primitive. #114 moved every
chrome icon from strokes to Phosphor glyphs (`Shape::Text`): `strokes_in` in
`tests/integrated_chrome.rs` kept counting paths and segments, so 'hidden controls were painted' and
'a maximized console painted a split' asserted `== 0` over shapes that no longer existed. Worse,
those tests ran on a bare `egui::Context::default()`, where the icon family is not installed and
`ui::icons::paint` paints nothing, so even a text count would have been zero. Their positive twins
(`> 0` on the focused header) still passed, satisfied by the header's rule segments alone. The suite
was green; two review agents caught it.

Rule: an `== 0` paint assertion needs a positive twin that only the same primitive, in the same
context, can satisfy — and a context that can paint it (`fonts::install_icons_only` for tests
written against egui's own faces). The same family of disguise reached the test helper itself in
round 2: `painted_icon` read only the shape's fallback colour, so an icon laid out in a fixed colour
passed every colour test; it now reads the section colour unless it is `PLACEHOLDER`.

Evidence: mutation `let shown = true || ...` in `header/integrated.rs` stayed green before the fix
and fails after it; mutation `PLACEHOLDER -> WHITE` in `ui::icons::paint` fails three tests after
round 2 (branch 114).

## Where it lives

crates/app/src/tests/integrated_chrome.rs (strokes_in); crates/app/src/ui/icons.rs (painted_icon)
