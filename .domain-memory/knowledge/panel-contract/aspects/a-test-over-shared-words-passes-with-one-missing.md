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
updated_at: '2026-11-25T22:33:12.917Z'
last_verified: '2026-11-25T22:33:12.917Z'
file_paths:
  - crates/panel-control/src/bin/panel-mcp.rs
  - crates/panel-contract/src/panel.rs
  - crates/app/src/explorer/interaction_tests.rs
symbols:
  - 'Mark::ALL'
  - 'Style::ALL'
  - 'Attention::ALL'
  - listed
  - files_text_visible
  - wait_for_file_text
  - markdown_single_click_opens_the_file_from_the_active_console
content_hashes:
  crates/panel-control/src/bin/panel-mcp.rs: 'sha256:b00f0d0e4b06cdf58ea3b7951a021b4f8ec0014c3082a56dc44a23ecf0980ce8'
  crates/panel-contract/src/panel.rs: 'sha256:379db8c10384c56e6b8ec0e72adb1aae0c84bddcd8d001e874b0fcf44ef8a87a'
  crates/app/src/explorer/interaction_tests.rs: 'sha256:fbdef7c785ad947fe85f3002b1fc9222093e2c411e5384cf83eea6e98209cfc3'
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

### The same disguise across two painted surfaces

In #173, the old Markdown viewer still painted handoff.md while Files followed a second console and painted Loading. A readiness helper accepted text from either Files or the viewer; the subsequent click searched only Files. The wait succeeded on the wrong surface, then the real click had no target. Require the exact name inside the rendered Files geometry. The repaired witness deliberately holds the new listing at Loading, proves the old viewer still has that name and Files does not, then releases the actual directory worker and clicks the real row. Broadening the predicate to accept the old viewer fails this controlled case.

An already-open document also hid a missing keyboard activation. Close it before Enter and require the real reopen/content; replacing Enter with Escape then fails that effect. Both mutations were detected, and the repaired interaction passed on native Windows in run45696334411. The transferable rule is the same: a shared word or existing result must be tied to the group or surface whose behavior is being proved.

## Where it lives

`crates/panel-control/src/bin/panel-mcp.rs` — `tools()` builds the `panel_set` description with `listed(&X::ALL.map(X::name))`; the test `the_publishing_description_names_each_vocabulary_under_its_own_field`. `crates/panel-contract/src/panel.rs` — `Mark::ALL`, `Style::ALL`, `Attention::ALL` and the readers `mark`/`style`/`attention_named` that look words up in them.

crates/app/src/explorer/interaction_tests.rs: files_text_visible, wait_for_file_text and markdown_single_click_opens_the_file_from_the_active_console reproduce the old-viewer/new-listing case and actual keyboard reopening.
