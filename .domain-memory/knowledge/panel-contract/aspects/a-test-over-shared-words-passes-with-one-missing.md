---
id: asp_a-test-over-shared-words-passes-with-one-missing_ab08p8
slug: a-test-over-shared-words-passes-with-one-missing
name: >-
  A seventh test disguise: a word-by-word check over a text that holds several
  lists passes with a word missing from one list, whenever another list carries
  the same word
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T23:14:48.992Z'
updated_at: '2026-09-25T23:14:48.992Z'
last_verified: '2026-09-25T23:14:48.992Z'
file_paths:
  - crates/panel-control/src/bin/panel-mcp.rs
  - crates/panel-contract/src/panel.rs
symbols:
  - 'Mark::ALL'
  - 'Style::ALL'
  - 'Attention::ALL'
  - listed
content_hashes:
  crates/panel-control/src/bin/panel-mcp.rs: 'sha256:632271ddff7cdd1720765291f7fc98fe0447c5d157c3c13ee3c8aae8f05de875'
  crates/panel-contract/src/panel.rs: 'sha256:6ae8abe521ff7f03c57eded34d78ab82781cb19eeccd21ba4d895e7ac59a93e4'
tags:
  - testing
  - test-disguise
  - mutation
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The panel's three closed vocabularies share three words** — `wait`, `block` and `done` are both marks and attentions. #255 made the `panel_set` tool description name all three vocabularies, and its first test checked every word of the three against the whole description with `contains`. Deleting `wait` from the mark list left the test green: the attention list still supplied ` wait,`.

> **The disguise is in the unit of the assertion.** Each word was really there — just not under the field the test claimed to check.

It was flagged independently by the built-in review and by pr-test-analyzer. The fix asserts each clause whole (`a line's "mark" is done, current, pending, wait, block or info;`) and generates the three lists from each vocabulary's `ALL`, so the description cannot drift from what the reader accepts. After it, mutant M8 (Wait removed from `Mark::ALL`) is killed.

**Rule: when the words under test can belong to more than one group, assert the group, not the word.** The same trap waits wherever a text enumerates overlapping sets — option lists, enum names in help text, error vocabularies.

## Where it lives

`crates/panel-control/src/bin/panel-mcp.rs` — `tools()` builds the `panel_set` description with `listed(&X::ALL.map(X::name))`; the test `the_publishing_description_names_each_vocabulary_under_its_own_field`. `crates/panel-contract/src/panel.rs` — `Mark::ALL`, `Style::ALL`, `Attention::ALL` and the readers `mark`/`style`/`attention_named` that look words up in them.
