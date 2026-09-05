---
id: asp_how-much-the-grid-absorbs_4jamm7
slug: how-much-the-grid-absorbs
name: >-
  How much the grid absorbs in a massive dump, and what budget it leaves per
  frame
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-07T19:58:41.554Z'
updated_at: '2026-08-07T19:58:41.554Z'
last_verified: '2026-08-07T19:58:41.554Z'
file_paths:
  - crates/term-core/examples/throughput.rs
  - crates/term-core/src/screen.rs
  - docs/05-roadmap.md
symbols:
  - 'Screen::advance'
content_hashes:
  crates/term-core/examples/throughput.rs: 'sha256:114cac38ded03ca2b308e5cd055560ec9226105303f1aecc74c6bda8430318ca'
  crates/term-core/src/screen.rs: 'sha256:f940fcd9032bb9284759fdadb03ff4f1606d548d260b538bc389d35f44b13f02'
  docs/05-roadmap.md: 'sha256:6fe1601f30d97065dad492557fd89921971490411bcf276336b7c3129a8bd7d4'
tags:
  - performance
  - phase-1
  - measurement
  - grid
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured, not estimated:** `Screen::advance` absorbs **64.5 MiB/s** in `--release` — 10 MiB in 155
ms, fed in chunks of 8 KiB (the same size as the PTY's read buffer), which gives **~121 µs per
chunk**. In a debug build it comes out several times worse; the number that counts is the
`--release` one.

It was measured with a pattern that carries line breaks and color escapes on purpose: feeding the
parser a single repeated character walks its fast path and gives a number that lies about the real
case.

**What the number is for.** The risk of the massive dump (a `cat` of a large file) was decided not
to be mitigated inside the box: `term-core` imposes no ceiling, and whoever consumes decides how
much it processes per pass. That decision is only sensible if the ceiling is chosen with a datum.
Direct translation: with a budget of **2 ms per frame** about 16 chunks fit, that is **~130 KiB per
frame**; a `cat` of 10 MiB would take ~80 frames, a little over a second at 60 fps, **without losing
a single one**.

The one who has to choose that budget is F1.3, when there is a real painting loop. What this datum
takes away from it is the blindness, not the decision.

**When it would stop being valid:** if the parser's version changes, if the read chunk stops being 8
KiB, or if it is measured on a different machine. Running `cargo run --release --example throughput
-p term-core` again costs 155 ms and returns that machine's number.

## Where it lives

`crates/term-core/examples/throughput.rs` is the measurement, reproducible with `cargo run --release
--example throughput -p term-core`. The risk that motivates all of this is declared in
`docs/05-roadmap.md`, table of concrete risks, assigned to F1.2.
