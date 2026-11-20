---
id: asp_searching-a-cut-line-needs-context_8kn6ks
slug: searching-a-cut-line-needs-context
name: >-
  A regex over a cut piece of a line turns the cut into a line start: search the
  piece as a span of a longer haystack, and skip word runs after a whole-word
  rejection
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T16:09:03.179Z'
updated_at: '2026-11-20T16:09:03.179Z'
last_verified: '2026-11-20T16:09:03.179Z'
file_paths:
  - crates/term-core/src/search/walk.rs
  - crates/term-core/src/search/line.rs
  - crates/term-core/src/search.rs
symbols:
  - mark
  - 'Pattern::find_in'
  - past_rejected
  - 'Logical::span'
content_hashes:
  crates/term-core/src/search.rs: 'sha256:5eaffbbfb7637417c739c81e918fe85219e698d07705e0961aa21915c3536590'
tags:
  - regex
  - search
  - marking
  - performance
feature_id: feat_term-core_nx0g1r
---

## What it does

Marking reads at most `MARKING_REACH` rows past the screen, so a long wrapped line is searched
as a cut piece. Searched alone, the cut becomes a fake start or end: `^`, `$`, `\b` and the
whole-word check all treat it as a boundary, and marking lights matches a step (which reads the
whole line) never selects. `regex-automata`'s `Input::new(haystack).range(span)` keeps look-around
reading the whole haystack while matches stay inside the span, so the fix is one row of context
past each cut, and the real rows as the span.

Second trap, same code: after a whole-word candidate is rejected, retrying one character on is
quadratic over a long run of word characters (`\d+` over «x» and forty thousand digits never
finishes). No start inside a run of word characters can stand apart, so skip the rest of the run.
A candidate rejected only at its END is still retried one character on; a pattern built for that
(`-+\w` over a long run of dashes) stays quadratic, accepted as a hostile pattern (D61).

Evidence: #121 review rounds 1-3; `marking_judges_the_rows_it_cuts_by_what_lies_past_the_cut`
fails without the context rows, `a_long_run_of_rejected_candidates_is_walked_once` hangs
without the skip.

## Where it lives

`crates/term-core/src/search/walk.rs` (`mark`, `matches`, `past_rejected`),
`search/line.rs` (`Logical::span`), `search.rs` (`Pattern::find_in`).
