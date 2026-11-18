---
id: asp_alacritty-search-api-traps_gnxtgh
slug: alacritty-search-api-traps
name: >-
  alacritty_terminal 0.26's search API: search_next re-finds the match at its
  origin, walks from the logical line's start every call, and leaves a match's
  ends on different halves of a wide character
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-10-10T22:19:33.924Z'
updated_at: '2026-11-18T23:35:34.820Z'
last_verified: '2026-11-18T23:35:34.820Z'
file_paths:
  - crates/term-core/src/screen.rs
  - crates/term-core/src/search.rs
  - crates/term-core/examples/search.rs
symbols:
  - search_next
  - RegexSearch
  - RegexIter
  - line_search_left
  - line_search_right
  - mark_matches
  - marking_bounds
  - MARKING_REACH
  - next_match_that_counts
  - is_whole_word
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:cb48be0778bd3e5570d25402497018352ebc9e25e344026917ac16f14cfa64af'
  crates/term-core/src/search.rs: 'sha256:5eaffbbfb7637417c739c81e918fe85219e698d07705e0961aa21915c3536590'
  crates/term-core/examples/search.rs: 'sha256:46a590806c9f3525ce28e9852ade64976ecb08c363d46bccc98cee0285f88c32'
tags:
  - alacritty
  - search
  - f2.13
  - d61
  - whole-word
feature_id: feat_term-core_nx0g1r
---

## What it does

Four traps in `alacritty_terminal-0.26.0/src/term/search.rs`, met building the scrollback search
(#177, D61) and its options (#120).

**1. `Term::search_next` counts a match that sits exactly on the origin.**
`next_match_left`/`next_match_right` compare the match's side with the origin using `<=` / `>=`
(search.rs:171,210). Handing it the current match (the live selection) as the origin returns that
same match on every press — Enter gets stuck. Alacritty steps the origin one cell past the current
match in the walking direction (`range.start.sub(.., 1)` older, `range.end.add(.., 1)` newer).
Verified alongside: `search_next` wraps (`unwrap_or(first_match)`), and the engine's selection is
rotated under new output and dropped when its lines are evicted (term/mod.rs:752,778) — which is
what makes the selection a safe cursor between presses. `RegexSearch::new` is smart-case
(insensitive unless the pattern holds an uppercase letter); since #120 an inline `(?i)` / `(?-i)`
ahead of the escaped literal overrides it, because `has_uppercase` reads the pattern string while
the inline flag wins over the syntax config.

**2. `line_search_left` / `line_search_right` follow a wrapped line to its end with no limit**
(search.rs:593-616, row by row while `WRAPLINE` is set). Clamping their *answer* to a reach bounds
where the regex walk starts, not what the frame pays: one line wrapped across the 3 500-line history
is still scanned on every call. #177's per-frame marking first clamped after calling them;
`Screen::marking_bounds` now follows the wrap flags itself and stops at `MARKING_REACH` lines beyond
the screen.

**3. Every `search_next` call starts at the origin's logical line start (or end) and walks the
buffer with a fresh `RegexIter`.** So a filter that rejects a match and calls `search_next` again
from just past it is quadratic in the candidates of one logical line. Measured in #120 (`cargo run
--release --example search -p term-core`, 3 549 rows of 180 columns): 33-42 ms a press over lines,
and over a history that is one wrapped line twenty presses did not finish in ten minutes. Walking
`RegexIter` yourself with the same start, the same past-the-origin test and the same wrap-to-first,
skipping rejected matches in place, is one walk: 14.1-14.9 ms in every case. `RegexIter`,
`line_search_left/right` and `expand_wide` are public; `match_side` is not. Matches never overlap,
so a whole word overlapping a rejected candidate (`1.1` in `v1.1.1`) is found walking one way and
missed the other.

**4. A match's two ends sit on different halves of a wide character.** A rightward match ends on the
wide character's spacer (`skip_fullwidth`, search.rs:442-449); a leftward one starts on the wide
cell, and a row that had no room for a wide character ends in a `LEADING_WIDE_CHAR_SPACER`. Any
check that reads the cell just outside a match (whole word) must step over `WIDE_CHAR_SPACER` and
`LEADING_WIDE_CHAR_SPACER`, or the cell before a wide character reads as a blank. Regex `\b` is no
substitute: ASCII `(?-u:\b)` takes every non-ASCII character for a non-word one (`日本` matches inside
`日本語`), and the lazy DFAs do not take a Unicode `\b` over non-ASCII input.

Evidence: cargo registry source read by #177's challenger and reviewers and by #120's design
challenger and skeptics; tests `each_step_moves_on_from_the_current_match_and_back`,
`marking_never_walks_further_than_its_reach_beyond_the_screen`,
`whole_word_reads_past_a_wide_character_s_spacer_on_both_sides_and_both_ways` and
`whole_word_follows_a_wrap_to_the_character_on_the_other_row` go red under the respective mutations.

## Where it lives

crates/term-core/src/screen.rs (`Screen::find`, `Screen::next_match_that_counts`,
`Screen::mark_matches`, `Screen::marking_bounds`, `MARKING_REACH`); crates/term-core/src/search.rs
(`is_whole_word`, tests); crates/term-core/examples/search.rs (the measurement). Decision: D61 in
docs/02-decisions.md.
