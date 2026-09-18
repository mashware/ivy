---
id: asp_the-measurement-round-closes-on-painting_szjxfy
slug: the-measurement-round-closes-on-painting
name: >-
  A pane's measurement round is only closed by painting it, and what it carries
  undeclared lives in three places — and what is painted on EVERY frame must not
  travel inside a round at all
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:25:24.471Z'
updated_at: '2026-09-18T18:07:05.495Z'
last_verified: '2026-09-18T18:07:05.495Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
  - scripts/measure-frame.sh
symbols:
  - paint_pane
  - close_frame
  - record_round
  - 'Spans::of_a_round'
  - 'RestOfFrame::record'
  - close_the_unpainted_rounds
  - 'Pane::take_what_this_frame_left'
  - 'Timings::take_pending'
  - 'report::Strips'
content_hashes:
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
  crates/app/src/measure.rs: 'sha256:245ca6e629c98ae88e943d924b4d341d9057b226617541d5ff854688780da5f3'
  scripts/measure-frame.sh: 'sha256:180b8517595b278aa0eb55476f7fdeaab7a027bf5da0ea0dcd80803c2a95bd15'
tags:
  - measurement
  - F2.1
  - F2.2
  - f2.11
  - solved
feature_id: feat_term-core_nx0g1r
---

## What it does

**The only place that closed a pane's measurement round was painting it.** `Pane::pump` leaves the spans in `pending_measurement` and only `paint_pane` picked them up. A pane that parses bytes and IS NOT PAINTED never closed its round: `Timings::close_frame` returned `None`, the window's walk skipped it, and `RestOfFrame`'s subtraction gave that time away to the UI framework — the figure D3 decides with whether egui holds up. And the sample was not doubled: it was OVERWRITTEN IN SILENCE.

It is the same class of failure as [[whoever-does-not-declare-their-work-attributes-it-to-someone-else]], with a new trigger: that one showed up on going from one console to two; this one shows up the first time there is a pane that is pumped and not painted, that is, a background tab. Bias estimated with a single background tab dumping: the «rest» would go from 0.50 ms to ~2.7 — more than five times.

> **And the fix did not need a new series, which is the trap of this finding.** `Timings::close_frame` only records into the four series inside the `if let Some(esperado)`, and even so it returns what was parsed OUTSIDE that `if`. It is enough to close the round with `Tramos::de_una_vuelta(None, recogido_en, interpretado, ZERO, ZERO)`: the subtraction is done and no mean gets dirtied. A design that added a series of its own for this was cut by the challenger after reading those lines.

**Solved in F2.1** with `close_the_unpainted_rounds`, which runs at the end of the UI pass — not in the logical half, because there it is not yet known what is going to be painted. The mutation that proves it: sweeping BEFORE painting passed the 216 tests and broke the whole measurement, because no mean would receive a sample in the entire session.

> **And it had a sibling this fix did not touch**, found later while reviewing PR #64: the round not only has to be closed, it has to NOT BE LOST. In a frame whose pass egui repeats, the pumping rearmed the round and `record_round` OVERWROTE it, with the same final destination — the parsed time published as the framework's cost — through a different door. See [[the-repeated-pass-re-pumps-and-overwrites-the-round]].

## And what the pane that is CLOSED carries undeclared lives in THREE places (F2.2, PR #74)

When a pane is let go of, what it has done has to be taken first, or the subtraction keeps its work. The error is not forgetting: it is looking at a single place. The frame's real order decides it, and it is counterintuitive:

1. `logic` closes the previous frame, which empties `medida.pendiente`.
2. `logic` pumps, which fills `pending_measurement` with what the console has just parsed.
3. The UI pass handles the keyboard before the painting, so the closing chord arrives HERE.

> **Consequence: at the instant of the close, `Timings::take_pending()` is ALWAYS zero.** A first version of closing a pane looked only at that place and declared zero in precisely the frame in which a console that was dumping is closed — milliseconds of parsing, published as the framework's cost.

The three places are: what was recorded (`medida.pendiente`, non-empty only if egui repeated the pass and the painting got as far as recording), what the pumping left (`pending_measurement`, the normal case) and `panel_reread_unpainted` — what it cost to re-read its `panel.json` while its tab was in the background, which `close_the_unpainted_rounds` only picks up if it also parsed something, so a console that is stopped — an agent waiting for an answer — carries it over into the next frame. `Pane::take_what_this_frame_left` picks up all three.

**And the test for this is written wrong by default**: seeding the value into `medida.pendiente` and driving the close without going through `logic` asserts about a state the real frame never produces, so it passes with the defect inside. You have to seed all three, with DISTINCT numbers, and assert about the sum — with the same number in two places, dropping one still leaves plenty. It is the disguise of [[a-test-that-pins-the-helper-does-not-pin-the-path]] applied to a field instead of to a function.

**Still alive on the sibling path:** `Window::close_the_active_tab` has the same defect — it looks only at what was recorded — since F2.1.

## What is painted on EVERY frame must NOT travel inside a round (F2.11, September 2026)

**`measure::Spans` is a record of a ROUND, and a round is only closed on a frame where that console produced OUTPUT** — `Timings::close_frame` records the series inside `if let Some(waited) = spans.waited`. That is right for what it was built for: a frame at rest is not a lightning-fast round trip, it is a round trip that did not happen.

> **So anything painted on every frame must not travel inside a round.** Put there, it is sampled over the dumping frames alone, a quiet console publishes nothing at all, and the mean describes a population nobody named. Measured on the strip painted over each console: 436 frames painted one, only 359 produced output — a FIFTH of them would have been silently dropped from the published figure, in the very batch whose only purpose is to measure what that painting costs.

**The honest home is a span of the WINDOW**, next to `TabBar` and `Blinking`: accumulated per pane and per pass, recorded ONCE when the frame closes, with its own line in the report's header (`Strips`, not a round-scoped span).

**Two corrections to this card's own earlier framing, both easy to get wrong and both were:**

- **The subtraction was never the problem, and a followup wrongly said it was.** `close_frame` returns `Some(spans.declarable())` OUTSIDE the `waited` guard, so a quiet frame's cost DID reach «the rest of the frame» and no number was ever published as the framework's. What was actually lost was only the SERIES SAMPLE. Check `declarable()` before believing a D13 leak is present — the leak and the lost-sample bug look identical from the published number alone and are not the same defect.
- **`Option<Duration>`, never a zero.** «Painted, faster than the clock can see» and «nothing painted» are different answers, and a measurement script that reads a count in both directions cannot tell them apart from a plain duration. The tab bar's own accumulator gates on `!is_zero()` and therefore UNDER-counts; do not copy that half when giving a new per-frame cost its home.

## Where it lives

`Window::close_the_unpainted_rounds`, `Window::paint_pane`, `Window::close_the_focused` and `Pane::take_what_this_frame_left` in `crates/app/src/lib.rs`; `Timings::close_frame`, `Timings::take_pending` and `RestOfFrame::record` in `crates/app/src/measure.rs`. F2.11's window-level span: `crates/app/src/measure.rs` / `crates/app/src/measure/header.rs` (`report::Strips`), `scripts/measure-frame.sh` (the `header:` report line).
