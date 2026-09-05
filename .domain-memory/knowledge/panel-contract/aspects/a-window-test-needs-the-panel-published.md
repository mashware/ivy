---
id: asp_a-window-test-needs-the-panel-published_8gg9pt
slug: a-window-test-needs-the-panel-published
name: >-
  The panel's strip does not exist for a test until somebody publishes a real
  panel: pinning the grid by hand invents a strip the tiling never put there
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T09:23:22.822Z'
updated_at: '2026-08-25T09:23:22.822Z'
last_verified: '2026-08-25T09:23:22.822Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/panel.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:6ffd89344e0593f6d7362dccb0c76cc2ac211086a11fe9f50adb258313184450'
  crates/app/src/panel.rs: 'sha256:d9844bb19996c434945fd41689580cbaa88ac00d1919dea2597ba2c5346c7319'
tags:
  - tests
  - panel
  - f2.6
  - harness
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**A panel nobody has published takes up NO columns**: the tiling only sets width aside for it when its state says it is visible, and with neither a good read nor a warning to give, the console keeps the whole pane. It is correct in production —a console with no producer must not lose room— and it is a trap in the tests.

**The trap, with its two faces.** A test that wants to exercise «pressing on the panel's columns» has two ways of being set up and both fail if they are done carelessly:

1. **Letting the console take the width of the whole pane** —what comes out of measuring the rectangle and nothing more—: then **the panel's strip does not exist** and every press falls «inside the grid». The scenario does not happen and the test passes **without exercising it**: checked by mutating it, the guard could be removed entirely with the suite green.
2. **Pinning the grid by hand to something narrower** —«half the columns»—: it invents a strip the real tiling never put there. The test exercises **something**, but not what the window does.

**What does set the scenario up: publishing a real `panel.json`** in a directory of its own, refreshing the state through the normal door, checking that the panel **is visible**, and **deriving** the grid from the same tiling the window uses. Three more lines and the scenario is the real one.

> This was **declared as an uncovered gap** ever since the selection delivery —«the test pins that grid by hand instead of deriving it from the tiling»— and it bit one delivery later, when the same defect appeared on the mouse path and the first test written to catch it **did not catch it**.

**Two details of the harness that go with this, because they cost the same to discover:**

- **The test panel's directory carries the name of the test**, not just that of the process. The suite runs in parallel inside the same process: without that discriminant, one test's deletion falls between another's write and read.
- **A frame of «moving within the same cell» has to exceed the threshold with which egui decides that a gesture is a drag** —some six points— or that frame does not reach the path that is meant to be tested. With one point of movement, the corresponding mutation does not bring the test down.

## Where it lives

The tiling between console and panel is in `crates/app/src/panel.rs` and is invoked from the pane in `crates/app/src/lib.rs`; the test helper that publishes the panel and derives the grid lives in that same file's test module. The declared gap this closes was written in the validation artifact of the selection delivery.
