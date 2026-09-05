---
id: asp_a-test-that-pins-the-helper-does-not-pin-the-path_hts3p9
slug: a-test-that-pins-the-helper-does-not-pin-the-path
name: >-
  A test that pins the helper does not pin the real path: nine times, and the
  ninth inside the fix for the eighth
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:06:42.827Z'
updated_at: '2026-08-13T15:25:30.209Z'
last_verified: '2026-08-13T15:25:30.209Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/app/src/panel.rs
  - crates/app/src/lib.rs
  - crates/app/src/watcher.rs
  - crates/app/src/measure.rs
symbols:
  - read
  - read_capped
  - paint
  - panel_color
  - paint_pane
  - 'Window::new'
  - set_the_alarm
  - publish_spans
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:024c1d104e63a6ddc80b1dc2596686d28bff05a93cc3f44ef57dc86b59e4ff0a'
  crates/app/src/panel.rs: 'sha256:bb0c2831a73e9318e084df0e0ac888d48ef188b7bb87d678dea35bc49458c572'
  crates/app/src/lib.rs: 'sha256:68622c95bad38cef50f3eb8b4de8f2407dddf761bf81edc119d00fcaf5a24223'
  crates/app/src/watcher.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
  crates/app/src/measure.rs: 'sha256:2c620cfa14e9ac593b3293628869a8ddb5d48b5ba3de54e08eb72f775d3ebe17'
tags:
  - tests
  - mutation
  - measurement
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The pattern, with its tally.** **Nine times** in this project, in different phases and with the suite **green** all nine. It is not carelessness: it is the natural way to write a test when the property lives in a small, well named function.

> **The mutation has to be made where the mechanism matters, not where the mechanism is written.** Mutating the helper and watching it fall proves that the helper works; it does not prove that anybody calls it, nor that they call it right.

**The ninth is the one that teaches most, because it was committed while fixing the eighth.** The work consisted of correcting a span of the report that was mislabeled, and the test was written **on purpose** to pin down what the report really prints instead of asserting over a constant — with that reasoning written in the function's comment. But a **wrapper** was left in front: `publish_spans()` with no arguments, which called the inner function with standard output, and the test came in through the inner one. **Emptying the wrapper left the 289 green with the report mute.**

> **A one-line wrapper between the test and production is a helper in disguise as the real path.** If the door production uses is not exactly the function the test calls, there is a gap, however short the difference is.

**And the fix matters as much as the finding: the wrapper was removed, another test was not added.** `publish_spans` now **receives the destination**, so the function under test *is* the one production calls and the only thing the caller contributes is where to write. The mutation stops being possible instead of being covered. When the gap is structural, plugging it with another test leaves the gap there.

**The nine cases:**

1. **The read ceiling** — the test called `read_capped`; replacing that call inside `read` with a `std::fs::read` left 41 green.
2. **The panel's color** — `Theme::panel_color` was tested; its only caller could ignore the style or paint nothing at all.
3. **Where each row lands** — content and order were asserted, not position: the lines were painted on top of one another with 122 green.
4. **The continuation of the wrapping** — «it falls further right than the symbol» is satisfied by any indent other than zero.
5. **The mark-to-symbol map** — the set was checked, not the pairs: it was fully permutable.
6. **The paths the window gives the watcher** — passing it an empty list leaves it armed and no panel moves in the whole session.
7. **That the window asks for a frame** — the test counted calls to an alarm **it built itself**.
8. **The heartbeat** — it called `logic` on its own with the panel already re-read by a helper, and asserted over a boolean that anything satisfies.
9. **The report's text** — the wrapper above.

**How it is recognized before it bites.** Ask yourself which **production** mutation puts the test in red, and check that it is on the path that is executed. Three signals: the test calls something that the real code calls in turn; the test builds the collaborator's double; **or there is an intermediate function the test skips**.

**Three corollaries, all three at somebody's expense:**

- **There are properties the result does not tell apart** — the ceiling returns the same error whether a quarter of a megabyte or two are reserved: the **resource has to be measured**, with the accounting per thread because the tests run in parallel.
- **There are properties that are only seen in the position** — an empty line enqueues no shape; what proves it left its place is the **distance** between its neighbors.
- **An assertion can be empty without looking like it** — the one for the watcher's fallback checked that a repaint was requested, and a freshly created egui context already asks for one on its own.

**The sibling trap:** a test that asserts **against the very constant it means to protect**. The one for the panel's margin compared against `MARGIN * width`; with the margin at zero, both sides changed together.

## Where it lives

Spread across the whole of F1.6 and the work that follows it: `crates/panel-contract/src/panel.rs` (the read ceiling), `crates/app/src/panel.rs` (the color, the position of the rows, the continuation, the map of marks), `crates/app/src/lib.rs` (the tiling, the alarm, the heartbeat), `crates/app/src/watcher.rs` (the paths and the meaning of the flag) and `crates/app/src/measure.rs` (the report's text).
