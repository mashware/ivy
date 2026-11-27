---
id: asp_testing-what-a-modal-shows-first_77205n
slug: testing-what-a-modal-shows-first
name: >-
  Testing what an egui Modal shows without scrolling: two passes, and the window
  size the person had
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-27T15:47:15.883Z'
updated_at: '2026-11-27T15:47:15.883Z'
last_verified: '2026-11-27T15:47:15.883Z'
file_paths:
  - crates/app/src/usage.rs
symbols:
  - figures_are_laid_out_above_the_configuration
content_hashes:
  crates/app/src/usage.rs: 'sha256:e3a05a3cdeef91ae0debabc6daa3067fdc0c2dd80b8de7bc81dd871aed236c24'
tags:
  - egui
  - testing
  - mutation
feature_id: feat_term-core_nx0g1r
---

## What it does

To prove something is visible in an egui 0.36 Modal without scrolling, read the painted Text
shapes of the SECOND pass (the first only measures the modal and paints nothing) and give
RawInput the screen_rect the person actually had. The inner ScrollArea takes its max height from
the window and culls what lies below its viewport, so the painted texts are what a person sees
first. A 4000 px tall window proves nothing about the fold.

In #281 the figures-first test, at the reported 1100x740, fails against main's old usage.rs
(«figures in the first view») and passes with the fix; a reviewer reasoning from the 4000 px
version wrongly concluded it passed on the old layout. Mutate against the real old file, not a
hand-made approximation of it.

Also: `egui::Context::run` does not exist here, use `run_ui`; clear `textures_delta` of every
pass, or dropping the output panics.

## Where it lives

crates/app/src/usage.rs tests (figures_are_laid_out_above_the_configuration)
