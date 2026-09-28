---
id: asp_confronting-the-published-document-with-the-code_ek9r8d
slug: confronting-the-published-document-with-the-code
name: >-
  The reviewer who finds the most is the one who confronts the published
  document with the source: nine mismatches in one PR, none visible from the diff
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:23:40.325Z'
updated_at: '2026-08-13T13:23:40.325Z'
last_verified: '2026-08-13T13:23:40.325Z'
file_paths:
  - docs/04-panel-contract.md
  - docs/05-roadmap.md
  - CLAUDE.md
symbols: []
content_hashes:
  docs/04-panel-contract.md: 'sha256:74a34b90ad96f4314965ff99fe2a644715d2ae032f5914bdfe890ef4d0608c4d'
  docs/05-roadmap.md: 'sha256:f4fad6e2e1bd05b99530a83799119485719ec743b268d99c9786de393e4b9225'
  CLAUDE.md: 'sha256:45a196c9ea4057500d64d1d1b866c2cb0e2cf95ce7189355f259456702b0ad39'
tags:
  - review
  - documentation
  - contract
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The finding, in one sentence:** of three reviewers on the last PR of F1.6, two of them blind, **by far the most valuable was the one who only confronted the published contract with the source code**. It found **nine** mismatches; the rest of the panel found four blockers between them all.

The nine, compressed: `updated_at` said it is painted and it is not painted · «the waste is invisible» contradicted the bullet next to it · «the three ways» of failing are four · two pending items already done and not struck through · the file's location written in the future tense when it already existed · three silent clips not declared (512 characters, the reference, and the indent against the width) · `CLAUDE.md` claimed the outbound queue has no ceiling, when it has had one since D8 · and that there are decisions D1–D7, when there are nine · the milestone left unmarked in the roadmap.

> **The drift is not inside the diff: it is between the diff and a file the diff does not touch.** A reviewer who reads the design and the change cannot see it, however good they are. Somebody is needed whose only job is to open the published document and ask the source whether it is still true.

**Why it weighs so much here and is not a mania for tidiness.** The design lives in `.claude/work/`, which is in `.gitignore` and does not survive the session. The contract is versioned and is **what whoever is going to write a producer opens** — a CI, a script, an MCP server, or whoever ports the prototype. Between the two, the one that wins is the one that can be read. It had already happened once with D6 ([[d6-comes-back-through-the-document-not-the-code]]): a decision discarded in the design came back through the public document.

**Turned into practice:** on every PR that changes behavior described in a versioned document, one of the reviewers goes in blind with that single assignment. It costs one agent and it finds what nobody else looks at.

## Where it lives

`docs/04-panel-contract.md` (the public `panel.json` contract), `docs/05-roadmap.md` and `CLAUDE.md`. Blind reviewer role used in the review of the last PR of F1.6.
