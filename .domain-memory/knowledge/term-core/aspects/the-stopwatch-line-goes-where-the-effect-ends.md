---
id: asp_the-stopwatch-line-goes-where-the-effect-ends_2g7jfp
slug: the-stopwatch-line-goes-where-the-effect-ends
name: >-
  The stopwatch line goes where the EFFECT ends, not where the call returns —
  and the name of the function lies about where that line is
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T11:01:01.360Z'
updated_at: '2026-08-24T11:01:01.360Z'
last_verified: '2026-08-24T11:01:01.360Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
  - crates/app/src/session.rs
  - docs/02-decisiones.md
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:695b2cbd997807c09dca51b084938c643b9c16d416f9dfbc830ffa69ae766479'
  crates/app/src/measure.rs: 'sha256:635e8c4744515c06065172fe15fa48e988d3d1f8d39a906942d9e25212a5cfe7'
  crates/app/src/session.rs: 'sha256:e830c75bff3ba304a58835e5647deeb2ef55331849d5092066f7d021d49030c6'
  docs/02-decisiones.md: 'sha256:2d9d5fed62b28f810dee8f7e3130a4e54dea4f09de41cde4dcd5ae464dc835ae'
tags:
  - measurement
  - stopwatch
  - mutation
  - tests
  - f2.5
feature_id: feat_term-core_nx0g1r
---

## What it does

**The case, with its number.** The annex of D16 published **4.45 ms** as «what it costs to open the window with the consoles you left». The stopwatch closed right after `Window::materialize`, that is, **when the processes existed**. Behind it were left two jobs that **grow with the number of consoles** and that nobody was counting: building each console's directory watcher and walking the whole tree for the first mirror. Measured again with the line properly placed: **5.42 ms**. **15 % of the number was outside the number.**

> **A stopwatch's line does not go where the call that gives the span its name returns: it goes where the effect the span promises is complete.** And the name of the function is precisely what pushes one to put it in the wrong place: `materialize` sounds like «this is where materializing ends», so the line is put there without asking what else is needed for the span's promise —«the window open with those consoles»— to be true.

**How it was caught, which matters as much as the finding:** a code reviewer reading the diff, not a test. **This cannot be pinned by timing** and the reason is measured: the two missing jobs are microseconds against the milliseconds of the `fork`s, so any threshold that told «it stopped earlier» from «it stopped later» would be an unjustifiable number that would fail on a loaded machine — the same conclusion [[seeding-can-be-pinned-timing-not-always]] reached by another route. **What holds that line up is the comment explaining where it is and why, not a test.**

**And a second finding from the same day, about how a stopwatch is tested:**

> **`is_some()` on a time pins nothing: `Some(0)` is also something.** The first version of the test of `Persistence::load_time` asserted that there was a number; its own mutation —replacing the stopwatch with `Duration::ZERO`— **left it green**. One has to demand **`> Duration::ZERO`**, which is what the defect cannot fake. It is the disguise `CLAUDE.md` names —«choosing numbers, a zero, with which the defect and the fix give the same result»— applied to the instrument that measures.

**And a third, finer one, about two neighbouring stopwatches:** asserting `> 0` on each **does not stop them from being crossed**. Wiring the time to read the file into the stopwatch for opening the consoles passes both assertions. What pins it down is the **equality** with the source (`declared.read == persistence.load_time()`), and the number that justifies it came out of the mutation itself: reading costs ~104 µs and opening three consoles ~3.9 ms, **37 times more**.

**And a product consequence that was not about measurement:** the write veto of the measurement sessions looked only at the two variables that ask for consoles, so the batch that **restores** —which deliberately does not set them, so that the file rules— was **the only measurement session capable of trampling the user's layout**, and its own control batch was not. An asymmetry between two batches that are compared row by row. It now vetoes any of the five variables of the measurement gate.

## Where it lives

The stopwatch and its line: `Window::opening`, branch `OpenWith::Restored`, in `crates/app/src/lib.rs`. The type that is published: `measure::Restoring` (`consoles`, `read: Option<Duration>`, `opening`). The reading time: `session::Persistence::load_time`. The numbers and the autopsy of the 15 %, in the annex «Measured — 24 August 2026» of **D16** in `docs/02-decisiones.md`. See also [[the-measurement-round-closes-on-painting]] and [[a-test-that-pins-the-helper-does-not-pin-the-path]].
