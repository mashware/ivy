---
id: asp_virtualized-markdown-selection-can-copy-other-paragraphs_v8w258
slug: virtualized-markdown-selection-can-copy-other-paragraphs
name: Virtualized Markdown selection can copy a different paragraph
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T11:35:18.939Z'
updated_at: '2026-11-26T11:35:18.939Z'
last_verified: '2026-11-26T11:35:18.939Z'
file_paths:
  - crates/app/src/viewer/markdown.rs
  - crates/app/src/viewer/markdown/selection.rs
  - crates/app/src/viewer/markdown/selection_tests.rs
symbols:
  - 'MarkdownDocument::show'
content_hashes:
  crates/app/src/viewer/markdown.rs: 'sha256:5329e3a35bbc70c7e01ad659203dad0864a3d4ae16df9f97afbe279d1f6fcadd'
  crates/app/src/viewer/markdown/selection.rs: 'sha256:b373d35e373ea47682529673540ef783207f97b5ad39af26780b229f91f4a7dd'
  crates/app/src/viewer/markdown/selection_tests.rs: 'sha256:47d8e17bb28e54fdeca08b87f2077940b2c71f239a722feee570603328365271'
tags:
  - egui
  - markdown
  - selection
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36.1 removes a label selection when either endpoint widget is absent. A
viewport-virtualized document can also reuse a widget identity for a different paragraph and
silently copy that paragraph. In the Ivy viewer reproduction the copy command changed from
paragraph 001 to paragraph 150 when the first paragraph moved out of view. Store both endpoints
in document coordinates. A regression must compare the exact copied text after moving the
original text out of view. An OS clipboard check must seed the clipboard before copying again so
stale data cannot satisfy it.

## Where it lives

The Markdown scroll area in crates/app/src/viewer/markdown.rs lays out only visible blocks. The
document selection in crates/app/src/viewer/markdown/selection.rs owns stable positions.
Headless input and clipboard exactness checks live in
crates/app/src/viewer/markdown/selection_tests.rs.
