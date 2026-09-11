---
id: asp_a-new-module-in-app-joins-five-lists_14yxj6
slug: a-new-module-in-app-joins-five-lists
name: >-
  Adding a module to `crates/app` means adding it to FIVE lists, and four of
  them fail silently
type: aspect
status: superseded
superseded_by: asp_the-sixth-list-lives-in-production-code_nerc71
confidence: 80
created_at: '2026-09-09T21:53:59.611Z'
updated_at: '2026-09-11T20:23:21.337Z'
last_verified: '2026-09-09T21:53:59.611Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/tests/glossary.rs
  - crates/app/tests/prose.rs
  - crates/app/tests/scripts.rs
symbols:
  - MODULES
  - SOURCES
  - CITATIONS_CHECKED
  - GLOSSARY_CLEAN_FILES
content_hashes:
  crates/app/tests/shape.rs: 'sha256:bf1c3e3e97025d5673275d093a3e3a61547426e2d2dd86ee30aa6f1e03a88d5f'
  crates/app/tests/glossary.rs: 'sha256:59311b64e32e0fe6c9e2d3e3b4113364627528cc6cef64cf1c76aa506c16a3ef'
  crates/app/tests/prose.rs: 'sha256:812b010bd385793ef348c438c1436a776b59900403919f140e1602bdb497a992'
  crates/app/tests/scripts.rs: 'sha256:182e267eec01fbd37c8fcd549fa888d0cfdb0ea90f7d0ac4f6f3b10d2fd2755e'
tags:
  - D28
  - guards
  - testing
feature_id: feat_term-core_nx0g1r
---

## What it does

**A delivery that moved a method out of `lib.rs` into a new module registered it in the one list its design had named, and the build then failed twice more, at different phases, for two lists it had never heard of.**

| List | File | What it guards | How it fails when you forget |
|---|---|---|---|
| `MODULES` | `crates/app/tests/shape.rs` | the 600-line module ceiling and the 300-line `impl` ceiling | **Silently** — the module is born with no ceiling at all |
| the citations lists | `crates/app/tests/glossary.rs` (**two entries, two lists**) | comments citing identifiers the crate no longer has | **Loudly**: *«`app/src/<name>.rs` is a file of `app` and is not on the citations list»* |
| `SOURCES` | `crates/app/tests/prose.rs` | that every comment is in English | **Silently** — the floor's slack absorbs the drop |
| `MODULES` | `crates/app/tests/scripts.rs` | the scripts these crates run | **Silently**, and its own docblock narrates this exact failure happening before |

Two of the five carry a floor, and those floors are the only reason the silent ones are catchable at all — but **a floor only fires when somebody REMOVES an entry, never when somebody forgets to add one**. So a new module joins the ceiling guard, gets refused by the glossary, and quietly falls out of the English guard and the scripts guard unless a reviewer reads for it.

> **«The guard passed» and «the guard looked» are different facts, and a list-driven guard cannot tell you which one you got.** The floor is what turns the second into the first, and it has to be raised **in the same commit** that adds the entry — otherwise the next delivery inherits the slack. Same lesson as [[a-guard-binds-only-when-a-number-forces-it]], applied to a walk instead of a ratchet.

Evidence: verified by adding `crates/app/src/alarms.rs` and watching `shape.rs` fire twice on the ratchet, `glossary.rs` refuse the file outright, and `prose.rs` and `scripts.rs` stay green while blind to it.

## Where it lives

`crates/app/tests/` — `shape.rs`, `glossary.rs`, `prose.rs` and `scripts.rs`. The order pieces leave `lib.rs` in is **D28**.
