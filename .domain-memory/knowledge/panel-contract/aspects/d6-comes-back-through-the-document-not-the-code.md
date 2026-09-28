---
id: asp_d6-comes-back-through-the-document-not-the-code_5wvfyb
slug: d6-comes-back-through-the-document-not-the-code
name: >-
  A decision discarded in the design comes back through the public document: D6
  was still being asked for in the contract months after letting it go
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:07:41.498Z'
updated_at: '2026-08-12T14:07:41.498Z'
last_verified: '2026-08-12T14:07:41.498Z'
file_paths:
  - docs/04-panel-contract.md
  - docs/02-decisions.md
symbols: []
content_hashes:
  docs/04-panel-contract.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
  docs/02-decisions.md: 'sha256:f46d49ee51def350ecc462a6930c4d7cc8b7941ecc69b6b15f8b2c791dc05955'
tags:
  - D6
  - contract
  - documentation
  - prototype
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**What happened.** The prototype kept quiet about the stale notice when the work was closed —an old summary of something finished is correct, and the idea is reasonable—. But to know that it is closed you have to **read the phase of somebody else's `meta.json` and decide what it means**, which is exactly what D6 forbids: the app does not deduce the state. The design of F1.6 declared it out, with its reason, and it even warned that «whoever ports the prototype will reintroduce it without noticing».

**And the document itself reintroduced it.** `docs/04-panel-contract.md` still said, in its list of behavior rules, «the app flags it — unless the work is closed, where an old summary is correct». It was caught by a blind reviewer confronting code against the published contract; nobody had seen it in the design nor in the review, because both were looking at the design.

> **The warning written in the design protects against nothing if the request is still alive in the document people read.** The design lives in `.claude/work/`, which is in `.gitignore` and does not survive the session. The contract is versioned and it is what whoever is going to write a producer opens —a CI, a script, an MCP server— or to port the prototype. Between the two, the one that wins is the one that can be read.

**It was not the only mismatch, and that is the part that generalizes.** The same code-against-contract confrontation turned up five: the table of fields contradicted a new rule about where staleness comes from; «there are only two ways for a file not to be valid» against five real variants of failure; a ceiling applied in the code (the indent, 64) that the contract did not mention, so a producer who asks for `indent: 100` gets 64 without finding out; and `link` promised as a clickable link when the code reads it and discards it.

> **When a public document is a deliverable of a PR, reviewing it against the diff is an angle of its own.** The reviewers who look at code do not open it, and the ones who look at design do not either.

**How the rule ended up**: the stale notice has no exceptions. Whoever does not want it says so **in their own file**, with a high `stale_after_minutes` in their last publication — which is the form D6 admits, because the semantics are still set by whoever does the work.

## Where it lives

`docs/04-panel-contract.md`, section «Behavior rules». The original decision, in `docs/02-decisions.md` → D6.
