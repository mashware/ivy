---
id: asp_files-search-flags-stay-off-tree-keys_2hf95d
slug: files-search-flags-stay-off-tree-keys
name: >-
  Every field of the Files Request is a cache key: a new search option must be
  cleared on the tree's keys, or flipping it reloads the whole tree
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T16:09:03.335Z'
updated_at: '2026-11-20T16:09:03.335Z'
last_verified: '2026-11-20T16:09:03.335Z'
file_paths:
  - crates/app/src/explorer/view.rs
  - crates/app/src/explorer.rs
  - crates/app/src/explorer/load.rs
symbols:
  - Request
  - 'Explorer::request'
  - root_loaded
content_hashes:
  crates/app/src/explorer/view.rs: 'sha256:b1c6df02b30c285dfb076415b50f27a484c78c573f2772d039bb751411c90595'
  crates/app/src/explorer.rs: 'sha256:547c35343305d9f200a6451fc93c98b8d015ba103e8ee4f4b9fd9187eb95beb1'
  crates/app/src/explorer/load.rs: 'sha256:be49888c8211abcbd91f9ca3306c2404fb45a519c6d166f0f1b23574ab408152'
tags:
  - files
  - explorer
  - cache
  - test-disguise
feature_id: feat_term-core_nx0g1r
---

## What it does

The Files explorer caches listings by the whole `Request` — pane, generation, epoch, path, query,
the pattern flag, reveal. The tree builds each folder's key from `Explorer::request` and then
clears the search fields (`explorer/view.rs`, `key.query.clear()`). A search option added to
`Request` must be cleared there too, and `Explorer::request` sets it only with a query: #121's
`.*` toggle first kept the flag on tree keys, so flipping it gave every folder a new key and the
whole tree went back to «Loading…».

The test disguise that hid it: asserting the OLD key is still in the cache (it always is — nothing
evicts it) instead of asserting no listing was loaded under a new key.

Still open, pre-existing: `root_loaded` (`explorer/view.rs` ~379) builds the root key with the
search query, so while a query is typed the tree's scroll position is not recorded.

Evidence: built-in review of #121 round 1; the interaction test now asserts no key with an empty
query carries the flag and fails under the mutation.

## Where it lives

`crates/app/src/explorer.rs` (`Explorer::request`), `crates/app/src/explorer/view.rs`
(tree keys, `root_loaded`), `crates/app/src/explorer/load.rs` (`Request`).
