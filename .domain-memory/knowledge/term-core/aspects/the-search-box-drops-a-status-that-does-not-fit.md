---
id: asp_the-search-box-drops-a-status-that-does-not-fit_2q0953
slug: the-search-box-drops-a-status-that-does-not-fit
name: >-
  The console search box drops a status that does not fit its field: a refusal
  dropped that way reads as silence, and only a test of the painted text sees it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T16:09:03.030Z'
updated_at: '2026-11-20T16:09:03.030Z'
last_verified: '2026-11-20T16:09:03.030Z'
file_paths:
  - crates/app/src/search/floating.rs
  - crates/app/src/search.rs
symbols:
  - paint_the_field
  - FIELD_CELLS
  - short_refusal
content_hashes:
  crates/app/src/search.rs: 'sha256:3d8185b5168474a80aedde600e4ffc04d24a4725471a9a257d516eb99ce3ca00'
tags:
  - search
  - egui
  - test-disguise
feature_id: feat_term-core_nx0g1r
---

## What it does

`paint_the_field` paints the box's status only where it leaves six cells for the query; the field
is `FIELD_CELLS` = 26 cells, so about 20 cells of status fit. Anything longer is left out.
Harmless for «no match»; fatal for a refusal: «too complex to search» (21) and «invalid pattern:
unclosed group» (31) were painted as nothing, so a query that was never searched with looked
exactly like one that found nothing — the defect #121 set out to remove. Since #121 a refusal is
never left out: it falls back to a short form («invalid pattern», «too complex»).

Any new status string over ~20 characters must be checked against this rule. **Test the painted
galley text, not `Mode::status()`**: round 1's tests asserted `status()` and stayed green while
the box painted nothing.

Evidence: built-in and silent-failure reviewers of #121 round 1;
`a_refusal_is_painted_in_a_field_of_the_box_s_own_width` fails with the refusal dropped.

## Where it lives

`crates/app/src/search/floating.rs` (`paint_the_field`, `FIELD_CELLS`);
`crates/app/src/search.rs` (`Mode::status`, `Mode::short_refusal`).
