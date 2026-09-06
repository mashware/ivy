---
id: asp_a-cited-number-is-copied-from-the-docblock-next-door_h3zmkt
slug: a-cited-number-is-copied-from-the-docblock-next-door
name: >-
  A number cited in a comment is copied from the docblock next door, never from
  the record it names: one wrong figure reached thirteen places in five files
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T12:42:12.338Z'
updated_at: '2026-09-06T12:42:12.338Z'
last_verified: '2026-09-06T12:42:12.338Z'
file_paths:
  - crates/app/src/fonts.rs
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
  - crates/app/src/settings.rs
  - scripts/measure-frame.sh
  - docs/02-decisiones.md
symbols:
  - ensure_the_collection
  - use_the_family
content_hashes:
  crates/app/src/fonts.rs: 'sha256:e64e2947f2bc20575da8a4a16877b532af56a559637f28262753778f7df63c98'
  crates/app/src/lib.rs: 'sha256:b7ed6e7ac1a1934a329301543ff11d9378fcf086b1e9d2d646abcbee1e6507d6'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  crates/app/src/settings.rs: 'sha256:ec91bf9d6dff6a0da719ea1943a774d0b78a862e8a00a50292a677857318b8e1'
  scripts/measure-frame.sh: 'sha256:0d3e8ba30e5e7e7750fde4fd84725803c5a98cf5879f8dde30b20974618caef0'
  docs/02-decisiones.md: 'sha256:7cacc3ed2d651a60a94b720172661e7827062b623ffab79104d7654feee799b7'
tags:
  - measurement
  - comments
  - D24
  - guards
feature_id: feat_term-core_nx0g1r
---

## What it does

**«Between 15.9 and 31.0 ms (D24)» appears nowhere in D24.** The record publishes **17.3–23.2 ms** for enumerating the system fonts (cold, three runs), 15.1 ms warm, and 23.5–31.0 ms for the start-up sum of enumerating **plus registering four faces**. The figure 15.9 is in none of its tables.

It was found by the comment reviewer of F2.10's fourth delivery, in September 2026, and by then it lived in **thirteen places across five files** — `fonts.rs`, `lib.rs`, `measure.rs`, `settings.rs` and `measure-frame.sh`, code comments and a shell guard's prose alike.

**Twelve of the thirteen were written by the delivery that found it, and that is the finding.** The thirteenth predates it: a single docblock in `fonts.rs` that had carried the wrong range since F2.9. Every new citation was copied **from the comment next door**, because that is what is on screen while you write — never from the record. A wrong number does not decay; it **multiplies at the rate the surrounding code is touched**.

> This is the project's own rule —*a mislabelled number is worse than no number*— failing in the one place the rule was not looking: not in a published measurement, but in the prose that cites one.

### What no guard sees

None of the four file-walking guards catches it. `glossary.rs` reads identifiers in backticks and checks the tree has them; a **number** in a comment is not an identifier. `prose.rs` judges language, not arithmetic. `shape.rs` counts lines. And the ratchet test only reads its own table. A figure attributed to a decision record is, for every guard in this repository, ordinary prose.

### The cheap rule that would have stopped it

**The first citation of a number opens the record; the rest are copies of the first.** So the check has to happen at the first one — and it is worth writing the citation as the record writes it, verbatim range included, so a later reader diffing the two sees a mismatch instead of a paraphrase.

## Where it lives

`crates/app/src/fonts.rs` (the original, in `ensure_the_collection`'s docblock), plus `lib.rs`, `measure.rs`, `settings.rs` and `scripts/measure-frame.sh`. The record itself is **D24** in `docs/02-decisiones.md`, section «The numbers, measured here».
