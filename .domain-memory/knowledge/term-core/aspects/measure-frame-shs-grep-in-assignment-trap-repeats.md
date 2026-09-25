---
id: asp_measure-frame-shs-grep-in-assignment-trap-repeats_ss93ek
slug: measure-frame-shs-grep-in-assignment-trap-repeats
name: >-
  measure-frame.sh runs under set -eu: what kills the script is a grep LAST in
  the pipeline — and making it survive wakes the `:-0` default that could never
  run
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-18T18:06:28.625Z'
updated_at: '2026-09-25T14:02:41.040Z'
last_verified: '2026-09-25T14:02:41.040Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/tests/batches.rs
  - crates/app/src/measure/header.rs
symbols:
  - DRAWN
  - PRESSED
  - APPLIED
  - run_the_guards
content_hashes:
  scripts/measure-frame.sh: 'sha256:465f0d9927c9fbebf177895a7c675b43daa5fd1baa5c2ab6e50b91c84020a588'
  crates/app/tests/batches.rs: 'sha256:ed7f6662b7fe6768618501a19661b55b628c6a2eb84dca8e5e130b4f80c5aba8'
  crates/app/src/measure/header.rs: 'sha256:1de81a58761d6cb65bfddb9b6f0222b4639b5e4b62a99f539af7a7caf69ceb55'
tags: []
feature_id: feat_term-core_nx0g1r
---

## What it does

`scripts/measure-frame.sh` runs under `set -eu`, so an assignment whose command substitution ends with a status of 1 ENDS THE SCRIPT right there — the report is not renamed `.INVALID`, nothing is printed, and every guard below it never runs. It was repeated six times in this one file; the last three (`DRAWN`, `PRESSED` off `buttons:`, `APPLIED` off `control:`) were fixed by #239 in September 2026.

> **What separates a fatal extraction from a safe one is whether `grep` is the LAST command of the pipeline**, not whether there is a grep in an assignment. A pipeline's status is its last command's, so `grep … | head -1` exits 0 whatever `grep` matched, while `… | grep -oE …` inherits the 1. The 243 guard was written fatal immediately below a safe `LINE=$(grep -E "^documents:" "$REPORT" | head -1)`.

Reproduced: `sh -c 'set -eu; X=$(printf "" | grep -oE "a" | grep -oE "b"); echo SURVIVED'` prints nothing and exits 1.

**The trap behind the trap (found by #239's review).** Making the extraction survive — `sed -n 's/…/\1/p'`, which exits 0 with no output — wakes up the `X=${X:-0}` on the next line, which could never run before because the script died first. A missing or REWORDED line then goes from a silent death to a **silent zero**, and a zero satisfies the «declared none, measured none» direction of every pair, so both directions go quiet for every batch that does not declare that load. **When you make an extraction non-fatal, delete its `:-0` in the same change**: an empty capture invalidates («the report's X line could not be read»), as the links and documents guards already did, and the words the `sed` anchors on go into `ANCHORS` in `crates/app/tests/batches.rs` so a rewording of `crates/app/src/measure/header.rs` fails the suite.

**The crash check is the FIRST guard** since #239, right after `invalidate()`: a report truncated by a panic is missing lines, so any guard above it invalidates for the line the crash took away — true, and it hides the crash.

**The line itself cannot move to `sed`**: `every_line_the_script_reads_is_one_the_application_writes` asserts the literal `grep -E "^<name>:"`. Only the numbers change shape.

**It is now held by running the script, not by reading it.** `run_the_guards` in `batches.rs` carves the script from `invalidate()` to the end, runs it under `sh` with `set -eu` and a given status over a fixture report. Each fixture must carry every line above the guard it aims at and assert that guard's own message — the first version asserted only «INVALID BATCH» and passed on the later `documents:` guard. Mutations checked: reverting any extraction, removing either empty-capture check, or moving the status check back each turn a test red.

## Where it lives

`scripts/measure-frame.sh` (the status check right after `invalidate()`; the `buttons:`, `links:`, `control:`, `documents:`, `header:` and settings guards) and `crates/app/tests/batches.rs` (`run_the_guards`, `a_crashed_run_is_invalidated_for_the_crash_whatever_its_report_lost`, `a_report_missing_the_line_a_guard_reads_is_invalidated_by_that_guard`, `ANCHORS`, `every_line_the_script_reads_is_one_the_application_writes`).
