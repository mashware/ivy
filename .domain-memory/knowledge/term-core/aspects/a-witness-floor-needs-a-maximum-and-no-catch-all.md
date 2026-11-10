---
id: asp_a-witness-floor-needs-a-maximum-and-no-catch-all_zw8cga
slug: a-witness-floor-needs-a-maximum-and-no-catch-all
name: >-
  A report witness the script checks against a floor must be a per-frame
  maximum, must not count the catch-all, and the frames it skipped must be
  bounded
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-10T16:09:17.517Z'
updated_at: '2026-11-10T16:09:17.517Z'
last_verified: '2026-11-10T16:09:17.517Z'
file_paths:
  - crates/app/src/measure/board.rs
  - scripts/measure-frame.sh
  - crates/app/tests/batches.rs
symbols:
  - 'Board::grouped'
  - check_board_report
  - validate_project_records
content_hashes:
  scripts/measure-frame.sh: 'sha256:308bcbf4ef1471e6d1372d9cb37520bb7fc3a808bf830701aa7a3a35af1d88d0'
  crates/app/tests/batches.rs: 'sha256:2d56e751314cc4243161a7f505e52b95656012c6a62d731279d1c96e13c4dd7a'
tags:
  - measurement
  - D13
feature_id: feat_term-core_nx0g1r
---

## What it does

Three ways the board batches' grouping witness could publish a mislabelled number, all caught in
#40's reviews. (1) Summed over frames, two one-repository frames reach a floor of 2: the witness
records the MOST any single projection grouped. (2) Counting sections, one real repository plus
«Other consoles» passes for two: only repository sections count (`repositories=`). (3) Frames
projected before the background Git resolution have no repository; declaring them (`ungrouped=`) is
not enough on its own — a run almost entirely ungrouped would still be averaged as the board's cost,
so the script invalidates when `ungrouped * 2 >= open`. A real `board-4-open` run gave
`repositories=2, ungrouped=1` over 608 open frames. Pinned by
`repositories_keep_the_most_one_projection_grouped…`, the grouping integration test
(`repositories=2, ungrouped=0`) and
`board_guards_require_real_navigation_and_reject_closed_activity`.

## Where it lives

crates/app/src/measure/board.rs (`Board::grouped`), scripts/measure-frame.sh (`check_board_report`,
`validate_project_records`)
