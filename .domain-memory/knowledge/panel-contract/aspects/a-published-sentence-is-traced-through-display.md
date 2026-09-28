---
id: asp_a-published-sentence-is-traced-through-display_xhmnw4
slug: a-published-sentence-is-traced-through-display
name: >-
  A contract fixes the WORDS a caller reads back, so the thing to check is the
  error's `Display` — never the site that builds it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-19T14:29:26.668Z'
updated_at: '2026-09-19T14:29:26.668Z'
last_verified: '2026-09-19T14:29:26.668Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/panel-contract/src/patch.rs
symbols:
  - Problem
  - 'Problem::Invalid'
  - 'Problem::Refused'
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:727d7c9c02c7493955a20386e77019ff65755565091bbdd38665e1740bd6c6b8'
tags:
  - contract
  - panel-patch
  - review-technique
  - d47
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**`panel-contract`'s `Problem::Invalid(String)` renders as `not understood ({reason})`** — right for a document that could not be read, false for an instruction that was read perfectly and refused.

A patch naming a key nobody carries was built as:

```rust
Problem::Invalid("no line of this panel carries the key \"tets\"".to_owned())
```

Correct at every construction site, and it reached the wire as:

```json
{"ok":false,"error":"not understood (no line of this panel carries the key \"tets\")"}
```

which is **not** the sentence `docs/04-panel-contract.md` promises a producer.

> **A published sentence has two halves — the words and the wrapper — and the diff only shows the first.**

## Why nothing caught it

The suite asserted `refusal.to_string().contains("\"tets\"")`, which passes on both forms. **Any test asserting on a refusal with `contains` is blind to the wrapper**; where the contract fixes the wording, assert the whole string.

It was found by a **contract verifier blinded to the design** — given only the literal contract block and the diff hunks that build the shapes, and told in so many words to trace what a reader actually receives rather than stop at the construction site. That instruction is the reusable part: a reviewer who is handed the design's prose rationalises the mismatch from the justification next to it.

## The fix, and why it is a variant and not a second type

`Problem::Refused(String)`, whose `Display` is the bare sentence and nothing around it. A second error enum for refusals was rejected because `serialize`'s ceiling already answers in `Problem::TooBig`: two vocabularies for one verb makes every caller translate between them to say anything at all.

**Adding a variant to a public error enum leaves its own docblock lying**, which is the predictable second defect: `Problem`'s header said «Why a panel could not be read», and a refused instruction is not that. The variant carries the widening in its own doc.

## Where it lives

`crates/panel-contract/src/panel.rs` — `enum Problem` and its `Display`. The refusals themselves are built in `crates/panel-contract/src/patch.rs` (`fn refused`), and the sentences they have to match are published in `docs/04-panel-contract.md` under `panel_patch`.
