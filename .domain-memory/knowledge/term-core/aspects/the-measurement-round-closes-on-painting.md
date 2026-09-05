---
id: asp_the-measurement-round-closes-on-painting_szjxfy
slug: the-measurement-round-closes-on-painting
name: >-
  A pane's measurement round is only closed by painting it, and what it carries
  undeclared lives in three places — whoever looks at one declares zero
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:25:24.471Z'
updated_at: '2026-08-17T12:01:16.017Z'
last_verified: '2026-08-17T12:01:16.017Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
symbols:
  - paint_pane
  - close_frame
  - record_round
  - 'Spans::of_a_round'
  - 'RestOfFrame::record'
  - close_the_unpainted_rounds
  - 'Pane::take_what_this_frame_left'
  - 'Timings::take_pending'
content_hashes:
  crates/app/src/lib.rs: 'sha256:69e903da2ae25795351e9b8a05001efaad7c4703b59964f4e430a94a6176c041'
  crates/app/src/measure.rs: 'sha256:627412ac283ad6138a2b514940f63e45eb84d875a0405dd62991852b0442a69c'
tags:
  - measurement
  - F2.1
  - F2.2
  - solved
feature_id: feat_term-core_nx0g1r
---

## What it does

**The only place that closed a pane's measurement round was painting it.** `Pane::pump`
leaves the spans in `pending_measurement` and only `paint_pane` picked them up. A pane that
parses bytes and **is not painted** never closed its round: `Timings::close_frame` returned
`None`, the window's walk skipped it, and `RestOfFrame`'s subtraction **gave that time away to
the UI framework** — the figure D3 decides with whether egui holds up. And the sample was not
doubled: it was **overwritten in silence**.

It is the same class of failure as
[[whoever-does-not-declare-their-work-attributes-it-to-someone-else]], with a **new trigger**: that
one showed up on going from one console to two; this one shows up **the first time there is a pane
that is pumped and not painted**, that is, a background tab. Bias estimated with a single background
tab dumping: the «rest» would go from 0.50 ms to ~2.7 — **more than five times**.

> **And the fix did not need a new series, which is the trap of this finding.**
> `Timings::close_frame` only records into the four series **inside the `if let Some(esperado)`**,
> and even so it returns what was parsed **outside** that `if`. It is enough to close the round with
> `Tramos::de_una_vuelta(None, recogido_en, interpretado, ZERO, ZERO)`: the subtraction is done and
> **no mean gets dirtied**. A design that added a series of its own for this was cut by the
> challenger after reading those lines.

**Solved in F2.1** with `close_the_unpainted_rounds`, which runs **at the end of the UI pass** —
not in the logical half, because there it is not yet known what is going to be painted. The mutation
that proves it: sweeping **before** painting passed the 216 tests and broke the whole measurement,
because no mean would receive a sample in the entire session.

> **And it had a sibling this fix did not touch**, found later while reviewing PR #64: the round not
> only has to be closed, it has to **not be lost**. In a frame whose pass egui repeats, the pumping
> rearmed the round and `record_round` **overwrote** it, with the same final destination —the parsed
> time published as the framework's cost— through a different door. See
> [[the-repeated-pass-re-pumps-and-overwrites-the-round]].

## And what the pane that is **closed** carries undeclared lives in THREE places (F2.2, PR #74)

When a pane is let go of, what it has done has to be taken first, or the subtraction keeps its work.
**The error is not forgetting: it is looking at a single place.** The frame's real order decides it,
and it is counterintuitive:

1. `logic` **closes the previous frame**, which **empties** `medida.pendiente`.
2. `logic` **pumps**, which fills `pending_measurement` with what the console has just parsed.
3. The UI pass handles **the keyboard before the painting**, so the closing chord arrives **here**.

> **Consequence: at the instant of the close, `Timings::take_pending()` is ALWAYS zero.** A
> first version of closing a pane looked only at that place and declared zero in precisely the frame
> in which a console **that was dumping** is closed — milliseconds of parsing, published as the
> framework's cost.

The three places are: **what was recorded** (`medida.pendiente`, non-empty only if egui repeated the
pass and the painting got as far as recording), **what the pumping left** (`pending_measurement`, the
normal case) and **`panel_reread_unpainted`** — what it cost to re-read its `panel.json` while its tab was
in the background, which `close_the_unpainted_rounds` only picks up **if it also parsed
something**, so a console that is stopped —an agent waiting for an answer— carries it over into the
next frame. `Pane::take_what_this_frame_left` picks up all three.

**And the test for this is written wrong by default**: seeding the value into `medida.pendiente` and
driving the close without going through `logic` asserts about a state the real frame **never
produces**, so it passes with the defect inside. You have to seed all three, with **distinct
numbers**, and assert about the sum — with the same number in two places, dropping one still leaves
plenty. It is the disguise of [[a-test-that-pins-the-helper-does-not-pin-the-path]] applied to a
field instead of to a function.

**Still alive on the sibling path:** `Window::close_the_active_tab` has the same defect —it
looks only at what was recorded— since F2.1.

## Where it lives

`Window::close_the_unpainted_rounds`, `Window::paint_pane`, `Window::close_the_focused`
and `Pane::take_what_this_frame_left` in `crates/app/src/lib.rs`;
`Timings::close_frame`, `Timings::take_pending` and `RestOfFrame::record` in
`crates/app/src/measure.rs`
