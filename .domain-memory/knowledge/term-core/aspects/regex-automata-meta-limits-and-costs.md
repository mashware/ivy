---
id: asp_regex-automata-meta-limits-and-costs_xtvqnr
slug: regex-automata-meta-limits-and-costs
name: >-
  regex-automata's meta Regex never gives up, but its size limit lives on
  meta::Config, refuses short Unicode counts, and bounds nothing about search
  time
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T16:09:02.688Z'
updated_at: '2026-11-20T16:09:02.688Z'
last_verified: '2026-11-20T16:09:02.688Z'
file_paths:
  - crates/term-core/src/search.rs
  - crates/term-core/examples/search.rs
symbols:
  - 'Pattern::new'
  - SIZE_LIMIT
  - nfa_size_limit
content_hashes:
  crates/term-core/src/search.rs: 'sha256:5eaffbbfb7637417c739c81e918fe85219e698d07705e0961aa21915c3536590'
  crates/term-core/examples/search.rs: 'sha256:46a590806c9f3525ce28e9852ade64976ecb08c363d46bccc98cee0285f88c32'
tags:
  - regex
  - search
  - d61
  - performance
feature_id: feat_term-core_nx0g1r
---

## What it does

`regex_automata::meta::Regex` (0.4.18) never gives up: when its lazy DFA thrashes it falls back
to PikeVM or the backtracker. Three things a search box built on it has to know.

**1. The size limit is `meta::Config::nfa_size_limit`.** `meta::Builder` has no `thompson()`
setter. Its default is 10 MiB, at which `(?i)[\p{L}\d]{200}` builds a 10.2 MB automaton in
26-28 ms and `\w{1000}` takes 26-36 ms just to fail — on every keystroke of a box that compiles
as you type. 2 MiB matches what `alacritty_terminal` 0.26 allowed (its lazy DFA cache capacity)
and refuses `\w{50}` (2.8 MB): every `\w` is every Unicode word character, so a short pattern
with a count is refused — the message must say *too complex*, not *too long*.

**2. The size limit does not bound search time.** Over text that varies, `\p{L}.{170}~`
(about 0.2 MB) costs 0.32-0.33 s a press over 3 549 × 180 cells read as line strings, and
3.2-3.4 s over one line wrapped across the whole history, inside one uninterruptible call;
marking it costs 66-100 ms a frame. Over REPEATED text the same pattern is cheap (6 ms), because
the automaton's states repeat too — a bench of a repeated line measures nothing about the worst
case. The terminal decided to accept that pause (D61).

**3. The common case is cheap.** A literal that is not there costs 2.8-3.1 ms a press over the
same history, almost all of it extracting the strings; the search itself is 0.06-0.10 ms.

Evidence: throwaway bench in the #121 design (synthetic grid), then `cargo run --release
--example search -p term-core` on the real grid, three runs, 20 November 2026.

## Where it lives

`crates/term-core/src/search.rs` (`Pattern::new`, `SIZE_LIMIT`); the bench rows in
`crates/term-core/examples/search.rs`; D61's #121 amendment in `docs/02-decisions.md`.
