---
id: asp_alacritty-search-gives-up-and-cuts-the-walk_fwnpnd
slug: alacritty-search-gives-up-and-cuts-the-walk
name: >-
  When alacritty's search gives up it does not only answer «no match»: RegexIter
  stops early, so a walk built on it returns the wrong match or marks half a
  screen
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T16:09:02.875Z'
updated_at: '2026-11-20T16:09:02.875Z'
last_verified: '2026-11-20T16:09:02.875Z'
file_paths:
  - crates/term-core/src/search/walk.rs
symbols:
  - RegexIter
  - RegexSearch
content_hashes: {}
tags:
  - alacritty
  - search
  - d61
feature_id: feat_term-core_nx0g1r
---

## What it does

`alacritty_terminal` 0.26's lazy DFA gives up after three cache clears with poor efficiency
(`minimum_cache_clear_count(Some(3))`), and the private `regex_search` turns that error into
`None` with only a `warn!` log. Through the public `RegexIter` that is not a clean «nothing»:
the iterator just stops early. A walk that keeps the first wrapped match as a fallback then
returns an EARLIER, wrapped match and skips the real one, and marking marks only part of the
screen. So auditing with another engine when the answer is «nothing» cannot catch it — #121
replaced the walk instead (logical lines as text, `regex-automata`'s meta engine).

It only happens over text that varies: over a repeated line the automaton's states repeat and
the engine never gives up. A test of it must draw varied text and check the premise against the
old engine on the same grid (`a_pattern_the_terminal_engine_gave_up_on_is_found` does).

One more difference: walking older, alacritty runs a reverse DFA right to left, so `aa` over
`aaa` selects columns 1-2; a forward leftmost-first search over the line string selects 0-1.

Evidence: alacritty_terminal-0.26.0/src/term/search.rs lines 260-268 (`regex_search`) and
651-683 (`RegexIter::next` returns on `next_match()?`); the #121 premise test.

## Where it lives

`alacritty_terminal-0.26.0/src/term/search.rs`; the replacement in
`crates/term-core/src/search/walk.rs` and `search/line.rs`.
