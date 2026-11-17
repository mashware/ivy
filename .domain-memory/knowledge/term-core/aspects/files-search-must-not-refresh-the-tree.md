---
id: asp_files-search-must-not-refresh-the-tree_fmsxhf
slug: files-search-must-not-refresh-the-tree
name: >-
  Changing the Files name-search query must not call Navigation::refresh: it
  drops the whole cached tree
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T21:02:08.266Z'
updated_at: '2026-11-17T21:02:08.266Z'
last_verified: '2026-11-17T21:02:08.266Z'
file_paths:
  - crates/app/src/explorer.rs
  - crates/app/src/explorer/search.rs
  - crates/app/src/explorer/model.rs
symbols:
  - 'Navigation::refresh'
  - 'Explorer::reconcile'
  - Request
content_hashes:
  crates/app/src/explorer.rs: 'sha256:547c35343305d9f200a6451fc93c98b8d015ba103e8ee4f4b9fd9187eb95beb1'
  crates/app/src/explorer/model.rs: 'sha256:92ee26c6aca8d1014b434b3cb195468e117b95f8ee5b04b6c9d1ecbb45cff3c8'
tags:
  - explorer
  - cache
  - search
feature_id: feat_term-core_nx0g1r
---

## What it does

In the Files explorer, `Navigation::refresh()` bumps the pane's generation.
`Explorer::reconcile` then evicts every cached listing of an older generation, so the whole
visible tree reloads from disk. The name search's `Request` already carries the query, so a
changed query needs no refresh: only a search still running for the old query is let go
(`loader.cancel()` when `loading` is a search of that pane).

Evidence: with a refresh per keystroke, the tree under the floating list collapsed to
«Loading…» on every key, and the search waited behind the tree reload in the single worker slot.
`typing_a_search_keeps_the_tree_loaded_and_the_keys_in_files` kills that mutation (#93 PR 6).

## Where it lives

crates/app/src/explorer.rs (reconcile, request), crates/app/src/explorer/search.rs (field), crates/app/src/explorer/model.rs (refresh)
