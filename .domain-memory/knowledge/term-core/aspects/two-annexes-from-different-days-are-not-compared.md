---
id: asp_two-annexes-from-different-days-are-not-compared_t7pske
slug: two-annexes-from-different-days-are-not-compared
name: >-
  Two annexes from different days are not compared: the binary changes
  underneath. A before/after alternates the variable over THE SAME binary,
  compiled on purpose
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T22:26:20.874Z'
updated_at: '2026-08-24T22:26:20.874Z'
last_verified: '2026-08-24T22:26:20.874Z'
file_paths:
  - docs/02-decisiones.md
  - scripts/measure-frame.sh
  - crates/term-core/src/screen.rs
symbols:
  - SCROLLBACK
content_hashes:
  docs/02-decisiones.md: 'sha256:2201996a78171d760ec80e0cf445419050c036975883ef44094d7ec0ca3013f9'
  scripts/measure-frame.sh: 'sha256:5164b53754c528c121b0dc462bd6b6ded7dd92b97e504c321ea968fdd7d77d2f'
  crates/term-core/src/screen.rs: 'sha256:53c0f8b50a8a8c82c87d3d9afc31d84ff9f171f5720cf419937e1ac25464f193'
tags:
  - measurement
  - method
  - d21
  - d3
feature_id: feat_term-core_nx0g1r
---

## What it does

**The case, and it is the worst kind of mistake this project can make: a badly labelled number published in the record of decisions.**

The annex of **D21** published that turning on the scrollback **doubled the full round** — «37.1 → 75.4 ms», with the text claiming that the two runs were «of the same binary of the day and half an hour apart». Both things were false:

1. **The control column was from another day and another binary.** It came out of a run of **18 August**; the other one, of the **24th**. Whole milestones had landed in between.
2. **And the conclusion was not supported by the data.** There are runs of the same batch of **19 August**, with the scrollback **off**, that already gave **73.7 and 74.0 ms**. The jump from 37 to 74 had already happened **before** turning anything on: attributing it to the scrollback was hanging someone else's cost on it.

**Really measured**, with four consecutive runs of the same binary —two compiling with the ceiling at zero and two with the 3 500 lines—: the scrollback costs **+39 % of interpreting** and **+1.2 % of full round**, with the framework residue unmoved. That is **an order of magnitude less** than what was published, and in a different span.

> **The rule that remains: a before/after is measured by alternating the variable over THE SAME binary, and that normally requires compiling on purpose the state that no longer exists.** Turn the constant off, compile, measure, restore, compile, measure. There is no shortcut: looking in `target/measurements/` for an old run «from before» **looks like** the shortcut and is the trap, because the file does not say which binary it was taken with.

**Why it is easy to fall for, and why this store already warned about half of it.** The reports pile up in `target/measurements/` with their date in the name, so there is always «one from before» at hand. No guard of the script catches it —they check that the run measured what it promises, not with which code— and the comparator only looks at **the window and the grid** matching, which is another thing. The sibling warning already filed, [[the-desktop-picks-the-size-and-the-batch-is-not-reproducible]], covers the variable **size**; this one covers the variable **code**, and it is the one that is not visible in the report.

**Practical consequence for the published annexes:** they are **dated records**, not current figures. The same batch `C` gives 52 ms on 17 August, 37 on the 18th, 74 on the 19th and 75 on the 25th — and only the last pair is comparable with the one before it. An annex reread a month later has to be read with its date in front, and **crossing two annexes of different dates to draw a conclusion is exactly what produced this mistake**. When the comparison is needed, it gets measured again.

**And it leaves an open question that was not the milestone's:** between 18 and 19 August the full round of the same batch, with the same window and the same grid, went from 37 to 74 ms with the scrollback off — **and it is not CPU cost**: interpreting moved from 763 to 851 µs and the framework residue from 220 to 240. The frames are similar in number and in cost, so what doubled is **what each byte waits**, not what it costs to attend to it. It points to when a repaint is asked for, not to how much painting costs.

## Where it lives

The corrected annex and the withdrawal of the previous one are in `docs/02-decisiones.md`, decision **D21**, in «Measured — 25 August 2026: what the scrollback does to the real window». The constant that is turned off for the control is `term_core::SCROLLBACK` in `crates/term-core/src/screen.rs`. The batches are launched with `scripts/measure-frame.sh C`, and the reports are left in `target/measurements/`, which is **not versioned**.
