---
id: asp_measure-frame-shs-grep-in-assignment-trap-repeats_ss93ek
slug: measure-frame-shs-grep-in-assignment-trap-repeats
name: >-
  measure-frame.sh runs under set -eu: what kills the script is a grep LAST in
  the pipeline, not a grep in an assignment — and the trap was repeated a fifth
  time
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:06:28.625Z'
updated_at: '2026-09-18T21:00:43.338Z'
last_verified: '2026-09-18T21:00:43.338Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/tests/batches.rs
symbols: []
content_hashes:
  scripts/measure-frame.sh: 'sha256:180b8517595b278aa0eb55476f7fdeaab7a027bf5da0ea0dcd80803c2a95bd15'
  crates/app/tests/batches.rs: 'sha256:e7236d7aafdb21e69cb4435981784470bd64fadf38f967d5bce2597cf0f9460e'
tags: []
feature_id: feat_term-core_nx0g1r
---

## What it does

`scripts/measure-frame.sh` runs under `set -eu`, so an assignment whose command substitution ends with a status of 1 ENDS THE SCRIPT right there — the report is not renamed `.INVALID`, nothing is printed, and every guard below it never runs, including the `$STATUS != 0` check that would have said the application panicked. It is documented in the file and has been repeated anyway, most recently by the `documents:` guard of issue 243 (September 2026). Three PRE-EXISTING ones have it too and are on `main`: `DRAWN` and `PRESSED` (read off `buttons:`) and `APPLIED` (off `control:`) — they survive only because those lines are always written, and the case that fires them is precisely the one the status check exists for: a report truncated by a panic, so the run that most needs diagnosing is the one published unmarked. Ticketed separately as #239.

**The fifth repetition is the one that names the real rule.** The 243 guard was written as `X=$(printf '%s' "$LINE" | grep -oE "[0-9]+ read back" | grep -oE "^[0-9]+")` — fatal — **immediately below a line of the same block that was safe**: `LINE=$(grep -E "^documents:" "$REPORT" | head -1)`.

> **What separates them is not `grep` in an assignment: it is whether `grep` is the LAST command of the pipeline.** A pipeline's status is its last command's, so `grep … | head -1` exits 0 whatever `grep` matched, while `… | grep -oE …` inherits the 1. That is why the file's two existing warnings did not prevent this one — they say «do not put a grep in an assignment», and the assignment right above does exactly that and is correct.

Reproduced before and after: `sh -c 'set -eu; X=$(printf "" | grep -oE "a" | grep -oE "b"); echo SURVIVED'` prints nothing and exits 1.

**The shape that survives, and it is four things together**: the *line* with `grep … | head -1`; the *numbers* with `sed -n 's/…/\1/p'`, which exits 0 with no output; a `[ -z "$LINE" ]` branch so the missing-line case reaches an `invalidate` instead of ending the run; and the whole block **below** the `$STATUS` check, so a report truncated by a panic is still named.

**And the line cannot also move to `sed`**, which is how this was got wrong twice in the same delivery: `crates/app/tests/batches.rs`'s `every_line_the_script_reads_is_one_the_application_writes` asserts the script reads it with the literal `grep -E "^<name>:"`. The line keeps that shape and only the numbers change.

## Where it lives

`scripts/measure-frame.sh` (the `control:`, `buttons:`, `header:`, settings and `documents:` guards), and `crates/app/tests/batches.rs` (`every_line_the_script_reads_is_one_the_application_writes`, which pins the `grep` shape of the line itself).
