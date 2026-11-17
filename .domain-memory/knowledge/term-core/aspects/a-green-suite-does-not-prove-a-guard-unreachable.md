---
id: asp_a-green-suite-does-not-prove-a-guard-unreachable_e0cca2
slug: a-green-suite-does-not-prove-a-guard-unreachable
name: >-
  A suite that stays green after deleting a guard does not prove the guard
  unreachable: build the input that reaches it first
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T15:32:58.335Z'
updated_at: '2026-11-17T15:32:58.335Z'
last_verified: '2026-11-17T15:32:58.335Z'
file_paths:
  - crates/app/src/settings/editor/view/palette.rs
symbols: []
content_hashes:
  crates/app/src/settings/editor/view/palette.rs: 'sha256:d47191a017ac80ef8c32cddbb9d3fccf7f8401879f32c5c5a8cd427ec6ad61d8'
tags:
  - tests
  - mutation
  - method
feature_id: feat_term-core_nx0g1r
---

## What it does

The suite's inputs may simply avoid the case a guard exists for. In #93 PR #5 (round 13) Settings'
colour floors — hover kept apart from the dialog, the selection kept apart from the navigation —
were removed as dead because the collapsed-palette test stayed green without them. The next round
reached both with one configuration line each: `panel=#1a372e` (Ivy Night's own selection tint, so
the selected section vanished on the navigation) and a narrow theme the roles keep as written
(`background=#000000 foreground=#757575`, hover 1.02:1 over raised). Rule: before calling a guard
dead, construct the input that would reach it and mutate against that input; a guard is dead only
when no writable configuration reaches it. The same review series also showed the opposite trap: a
floor whose result can never change (a 17 % border step against a 6 % surface wherever text reads)
is dead and should go, but that is shown by arithmetic over every reachable input, not by a green
run.

## Where it lives

crates/app/src/settings/editor/view/palette.rs. Evidence: test
`narrow_themes_and_a_panel_on_the_selection_keep_settings_fills_apart`, which kills both removal
mutations.