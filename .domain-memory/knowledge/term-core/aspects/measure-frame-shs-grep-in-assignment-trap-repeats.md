---
id: asp_measure-frame-shs-grep-in-assignment-trap-repeats_ss93ek
slug: measure-frame-shs-grep-in-assignment-trap-repeats
name: >-
  measure-frame.sh runs under set -eu: a grep with no match inside an assignment
  ends the script silently, and the documented trap was repeated a fourth time
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:06:28.625Z'
updated_at: '2026-09-18T18:06:28.625Z'
last_verified: '2026-09-18T18:06:28.625Z'
file_paths:
  - scripts/measure-frame.sh
symbols: []
content_hashes:
  scripts/measure-frame.sh: 'sha256:180b8517595b278aa0eb55476f7fdeaab7a027bf5da0ea0dcd80803c2a95bd15'
tags:
  - measurement-bench
  - shell
  - set-eu
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

`scripts/measure-frame.sh` runs under `set -eu`, so `X=$(... | grep ...)` with no match gives the assignment status 1 and ENDS THE SCRIPT right there — the report is not renamed `.INVALID`, nothing is printed, and every guard below it never runs, including the `$STATUS != 0` check that would have said the application panicked. It is documented in the file and was repeated anyway: the settings guard carries the whole autopsy («the grep goes INSIDE the if and not in an assignment ... the first version of this guard was written that way and was dead in both cases it exists to catch — checked by running the script, not the bare grep, which is how it was verified wrongly the first time. And being above the status check, it took every other guard down with it.»). The delivery that added the strip's guard (PR #4 of 234, September 2026) wrote exactly that shape again. Three PRE-EXISTING ones have it too and are on `main`: `DRAWN` and `PRESSED` (read off `buttons:`) and `APPLIED` (off `control:`) — they survive only because those lines are always written, and the case that fires them is precisely the one the status check exists for: a report truncated by a panic, so the run that most needs diagnosing is the one published unmarked. Ticketed separately as #239 (not this PR's, since these three predate it). The rule, both halves: (1) pull a number out of the report with `sed -n 's/.../\1/p'`, which exits 0 with no output, never with a `grep` as the last command of an assignment; (2) verify it BY RUNNING THE SCRIPT, not the bare `grep` — the bare command exits 1 visibly and reads like the guard working when it is actually silently dead. And the order is part of the guard: a block that reads a line the report may not have goes BELOW the `$STATUS` check, or a panicked run is diagnosed as «it asked for X and measured none» instead of as a crash; the «the line is missing» case goes FIRST inside the block, as the precondition it is.

## Where it lives

`scripts/measure-frame.sh` (the settings guard's own autopsy comment, and the `DRAWN`/`PRESSED`/`APPLIED` guards that repeat the shape, tracked as issue #239).
