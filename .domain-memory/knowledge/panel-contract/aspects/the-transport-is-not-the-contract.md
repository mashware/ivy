---
id: asp_the-transport-is-not-the-contract_0n14aw
slug: the-transport-is-not-the-contract
name: >-
  The file cannot be the source of truth: the contract is the document and
  `panel.json` is only one of its transports
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T08:26:17.392Z'
updated_at: '2026-08-24T08:26:17.392Z'
last_verified: '2026-08-24T08:26:17.392Z'
file_paths:
  - docs/02-decisions.md
  - docs/03-architecture.md
  - docs/04-panel-contract.md
symbols: []
content_hashes:
  docs/02-decisions.md: 'sha256:97753287aa018f84f87473e2872f7347a4aacb199a69cc2007289f3c34ee1475'
  docs/03-architecture.md: 'sha256:04750055e66a4260a0e5342bf03a5b365859fca8a888a7e9b61b6826701ff321'
  docs/04-panel-contract.md: 'sha256:95629fb310672544479a213c3275547f03e137cc6cdad1da3920c8076c83eba9'
tags:
  - contract
  - transport
  - d17
  - mcp
  - osc
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**What was wrong, and it was not a design preference: it was a collision already scheduled.** The plan had **two writers of the same place** — the producer by file and the agent by MCP's `panel_set` in Phase 3 (D5) — and nobody to decide who rules. With neither source nor order, whoever wrote last wins, that is, the operating system's scheduling. The symptom does not show up today because there is only one real producer.

**The decision (D17), in three parts:** the contract is **the document** (`phase`, `lines`, marks, styles, expiry), which is what gets versioned; **the source of truth is the state the app holds per pane**, not the file; and every delivery brings **source and order** so that two producers are not settled by chance.

> **And a reason of origin worth admitting: the medium was chosen with only one kind of producer in sight.** The first one was an agent that writes files, and for that the file is unbeatable. The three reasons that justified it —failure isolation, it can be read with `cat`, it crosses SSH with no protocol of its own— are still good ones, but **none of them says why it has to be the only one**.

**The axis that decides between transports is not elegance: it is what the producer has to do in order to publish.**

| | File | MCP | Socket | OSC over the PTY | HTTP API |
|---|---|---|---|---|---|
| From a script | `cat >` | no | needs a client | `printf` | `curl` + a port |
| Crosses container / SSH | only with a volume | no | no | **free** | with network and a port |
| Survives the producer | **free** | if the app stores it | if the app stores it | if the app stores it | the same |
| Knows who writes | **nobody** | per session | the process's credentials | **by construction: its PTY** | only by authenticating |
| Warns the producer that its document is broken | **no** | yes | yes | no | yes |

**What was decided with that table:** the file stays as **the default one** —it is the only one where the dumbest producer publishes without writing code, and the only one where the state survives the producer dying for free—; **MCP delivers the document without writing the file** (writing it would be the app detecting itself through the watcher, a detour with a frame of delay); and **OSC over the PTY stays as a candidate after F2.7**, because it is the only one that crosses an `ssh` or a container with no volume **and** the only one where the per-pane scoping cannot be faked.

**Discarded, with their reason:** the **HTTP API** (everything it brings is covered by another one and it turns «whoever writes in your directory» into «anybody who can reach your machine»); the **socket as the main one** (this is what Herdr does: knowing which process writes does not make up for losing the container, another machine's CI and `scp`); **replacing the file instead of accompanying it** (any transport with a client raises the bar to entry, and the bar is what allows the format to have producers, which is the only thing that can turn into a moat).

**Two warnings for whoever comes back here:**

- **D6 is not to be touched.** This is about where the state arrives from, not who decides it. See [[d6-comes-back-through-the-document-not-the-code]].
- **The watcher is not an argument against the file.** It was checked before writing it: the app uses **a single watcher** for all the directories, not one per pane, and the development machine's limits are 1 024 instances and 524 288 paths. Watching files does not scale badly here.

**What is deliberately left open:** how the source and the order are expressed in the document. Herdr resolves it with a stable `--source` per integration —which gives authority and is released on exit— and a strictly increasing `--seq` that discards whatever arrives late. With a single transport alive, fixing the shape now would be inventing it before having the second case in front of us.

## Where it lives

`docs/02-decisions.md` → **D17** (the whole decision with the table and what was discarded) and **D5** (the table of channels, which no longer names `panel.json` as if it were the channel). `docs/04-panel-contract.md`, split into **Part I — The document** and **Part II — The transports**. `docs/03-architecture.md` → «The three channels», channel 1.
