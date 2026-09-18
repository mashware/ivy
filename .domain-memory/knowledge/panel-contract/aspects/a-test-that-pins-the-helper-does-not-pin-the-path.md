---
id: asp_a-test-that-pins-the-helper-does-not-pin-the-path_hts3p9
slug: a-test-that-pins-the-helper-does-not-pin-the-path
name: >-
  A test that pins the helper does not pin the real path: ten times, and the
  ninth inside the fix for the eighth
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:06:42.827Z'
updated_at: '2026-09-18T18:04:51.575Z'
last_verified: '2026-09-18T18:04:51.575Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/app/src/panel.rs
  - crates/app/src/lib.rs
  - crates/app/src/watcher.rs
  - crates/app/src/measure.rs
  - crates/app/src/panel/pane.rs
symbols:
  - read
  - read_capped
  - paint
  - panel_color
  - paint_pane
  - 'Window::new'
  - set_the_alarm
  - publish_spans
  - cell_under
  - cell_to_extend_to
  - clamped_cell
  - console_within
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:86d7abe2bfa147112ee41dbec48c61ebd692bd74450c0239a534e744ec90f221'
  crates/app/src/panel.rs: 'sha256:478252fb6a0db92f2d9b5089e14629342494b55d19ecb9df289680c603f1d547'
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
  crates/app/src/watcher.rs: 'sha256:c6fd561c46c58a536845d23d75d13f97ba6bda3949206d095866f3f98e6d28a0'
  crates/app/src/measure.rs: 'sha256:245ca6e629c98ae88e943d924b4d341d9057b226617541d5ff854688780da5f3'
  crates/app/src/panel/pane.rs: 'sha256:0b26824e52f24226148312b2bfca49e2a3739c7c7992ac9849e907e610349b72'
tags:
  - tests
  - mutation
  - measurement
  - f2.11
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The pattern, with its tally.** **Ten times** in this project, in different phases and with the suite **green** every time. It is not carelessness: it is the natural way to write a test when the property lives in a small, well named function. The mutation has to be made where the mechanism MATTERS, not where the mechanism is written: mutating the helper and watching it fall proves that the helper works, it does not prove that anybody calls it, nor that they call it right.

**The tenth (F2.11, September 2026): a seam-moving refactor's test compared the helper against the very expression it delegates to.** PR #1 of the header milestone changed three pointer helpers (`cell_under`, `cell_to_extend_to`, `clamped_cell`) from taking the pane's rectangle to taking the console's — equal at the time, because `panel::set_aside` anchors the console at the pane's own top-left corner. The first test written for it compared `Pane::console_within` with `tile_with_its_panel(..).console`, which is the very expression `console_within` delegates to internally: it could not fail for ANY implementation of the pointer path, and reverting all six call sites left it green. What binds the helper's real contract is testing it directly — hand the same point to two rectangles whose corners are one cell apart and require the rows to differ by exactly one — which turns `point - console.min` mutated into `point - Pos2::ZERO` red. Rule for a seam-moving refactor specifically: test the parameterised behaviour of the helper on its own terms (it counts from what it is given), state in the artifact that the WIRING to real call sites is still unpinned, and name the later delivery that will pin it — do not compare the helper against its own implementation.

**The ninth is the one that teaches most, because it was committed while fixing the eighth.** The work consisted of correcting a span of the report that was mislabeled, and the test was written on purpose to pin down what the report really prints instead of asserting over a constant — with that reasoning written in the function's comment. But a wrapper was left in front: `publish_spans()` with no arguments, which called the inner function with standard output, and the test came in through the inner one. Emptying the wrapper left the 289 green with the report mute.

> **A one-line wrapper between the test and production is a helper in disguise as the real path.** If the door production uses is not exactly the function the test calls, there is a gap, however short the difference is.

**And the fix matters as much as the finding: the wrapper was removed, another test was not added.** `publish_spans` now receives the destination, so the function under test IS the one production calls and the only thing the caller contributes is where to write. The mutation stops being possible instead of being covered. When the gap is structural, plugging it with another test leaves the gap there.

**The ten cases:**

1. **The read ceiling** — the test called `read_capped`; replacing that call inside `read` with a `std::fs::read` left 41 green.
2. **The panel's color** — `Theme::panel_color` was tested; its only caller could ignore the style or paint nothing at all.
3. **Where each row lands** — content and order were asserted, not position: the lines were painted on top of one another with 122 green.
4. **The continuation of the wrapping** — «it falls further right than the symbol» is satisfied by any indent other than zero.
5. **The mark-to-symbol map** — the set was checked, not the pairs: it was fully permutable.
6. **The paths the window gives the watcher** — passing it an empty list leaves it armed and no panel moves in the whole session.
7. **That the window asks for a frame** — the test counted calls to an alarm it built itself.
8. **The heartbeat** — it called `logic` on its own with the panel already re-read by a helper, and asserted over a boolean that anything satisfies.
9. **The report's text** — the wrapper above.
10. **The pointer path's seam move** — comparing the helper against the expression it delegates to, above.

**How it is recognized before it bites.** Ask yourself which PRODUCTION mutation puts the test in red, and check that it is on the path that is executed. Signals: the test calls something the real code calls in turn; the test builds the collaborator's double; there is an intermediate function the test skips; **or, as in case ten, the expected value is computed by calling the very code under test.**

**Corollaries, each at somebody's expense:**

- **There are properties the result does not tell apart** — the ceiling returns the same error whether a quarter of a megabyte or two are reserved: the resource has to be measured, with the accounting per thread because the tests run in parallel.
- **There are properties that are only seen in the position** — an empty line enqueues no shape; what proves it left its place is the distance between its neighbors.
- **An assertion can be empty without looking like it** — the one for the watcher's fallback checked that a repaint was requested, and a freshly created egui context already asks for one on its own.

**The sibling trap:** a test that asserts against the very constant it means to protect. The one for the panel's margin compared against `MARGIN * width`; with the margin at zero, both sides changed together.

## Where it lives

Spread across the whole of F1.6 and the work that follows it: `crates/panel-contract/src/panel.rs` (the read ceiling), `crates/app/src/panel.rs` (the color, the position of the rows, the continuation, the map of marks), `crates/app/src/lib.rs` (the tiling, the alarm, the heartbeat, and case ten's pointer helpers `cell_under`/`cell_to_extend_to`/`clamped_cell`), `crates/app/src/watcher.rs` (the paths and the meaning of the flag), `crates/app/src/measure.rs` (the report's text) and `crates/app/src/panel/pane.rs` (`console_within`, case ten).
