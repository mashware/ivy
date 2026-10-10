---
id: asp_alacritty-search-api-traps_gnxtgh
slug: alacritty-search-api-traps
name: >-
  alacritty_terminal 0.26's search API: search_next re-finds the match at its
  origin, and the engine's walk to a line's ends has no bound
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-10-10T22:19:33.924Z'
updated_at: '2026-10-10T22:19:33.924Z'
last_verified: '2026-10-10T22:19:33.924Z'
file_paths:
  - crates/term-core/src/screen.rs
  - crates/term-core/src/search.rs
symbols:
  - search_next
  - RegexSearch
  - RegexIter
  - line_search_left
  - line_search_right
  - mark_matches
  - marking_bounds
  - MARKING_REACH
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:9a165fd3a69d74c5e8b04a0808b1d146e374f1e86e8206186e78088c67f984b8'
tags:
  - alacritty
  - search
  - f2.13
  - d61
feature_id: feat_term-core_nx0g1r
---

## What it does

Two traps in `alacritty_terminal-0.26.0/src/term/search.rs`, both met building the scrollback search (#177, D61).

**1. `Term::search_next` counts a match that sits exactly on the origin.** `next_match_left`/`next_match_right` compare the match's side with the origin using `<=` / `>=` (search.rs:171,210). Handing it the current match (the live selection) as the origin returns that same match on every press — Enter gets stuck. Alacritty steps the origin one cell past the current match in the walking direction (`range.start.sub(.., 1)` older, `range.end.add(.., 1)` newer). Verified alongside: `search_next` wraps (`unwrap_or(first_match)`), the engine's selection is rotated under new output and dropped when its lines are evicted (term/mod.rs:752,778) — which is what makes the selection a safe cursor between presses — and `RegexSearch::new` is smart-case (insensitive unless the pattern holds an uppercase letter), so a literal escaped with `regex_syntax::escape` keeps the rule.

**2. `line_search_left` / `line_search_right` follow a wrapped line to its end with no limit** (search.rs:593-616, row by row while `WRAPLINE` is set). Clamping their *answer* to a reach bounds where the regex walk starts, not what the frame pays: one line wrapped across the 3 500-line history is still scanned on every call. #177's per-frame marking first clamped after calling them; two round-2 reviewers caught it, and `Screen::marking_bounds` now follows the wrap flags itself and stops at `MARKING_REACH` lines beyond the screen.

Evidence: read in the cargo registry source by the design challenger and the round-2 type and correctness reviewers of #177 (October 2026); tests `each_step_moves_on_from_the_current_match_and_back` and `marking_never_walks_further_than_its_reach_beyond_the_screen` go red under the respective mutations.

## Where it lives

crates/term-core/src/screen.rs (`Screen::find`, `Screen::mark_matches`, `Screen::marking_bounds`, `MARKING_REACH`); crates/term-core/src/search.rs (tests). Decision: D61 in docs/02-decisions.md.
