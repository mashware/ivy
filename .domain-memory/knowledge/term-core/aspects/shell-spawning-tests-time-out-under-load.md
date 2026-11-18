---
id: asp_shell-spawning-tests-time-out-under-load_8y1ab8
slug: shell-spawning-tests-time-out-under-load
name: >-
  Tests that spawn a shell and wait on it time out when the machine is loaded,
  and pass alone: load, not a regression
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T10:20:36.485Z'
updated_at: '2026-11-18T10:20:36.485Z'
last_verified: '2026-11-18T10:20:36.485Z'
file_paths:
  - crates/app/src/header/splitting.rs
symbols:
  - split_directory_tracks_cd_for_both_orientations
content_hashes:
  crates/app/src/header/splitting.rs: 'sha256:fa3ca30e297072c8ee0c5416dfaa85286ab2168501c5792a14bfb76c1942b096'
tags:
  - tests
  - flaky
  - load
feature_id: feat_term-core_nx0g1r
---

## What it does

During #104 (November 2026), with several builds running in other panes (load average about 35 on a
desktop), tests that launch a real shell and wait for it failed in full or filtered runs and passed
alone and on the next run:

- `header::splitting::split_directory_*` (five of them): 'directory operation timed out', several
  times, including while a mutation loop was compiling.
- `tests::the_program_of_the_console_born_on_splitting_sees_the_size_of_its_pane`: 'the console's
  program wrote nothing', once.

None of them touched the diff under test. Before calling such a red a regression, check `uptime` and
rerun the test alone; before calling it a flake, run it alone and in the whole suite on the base
too, as flow-core's rule for a red test outside the change asks. In a mutation loop, these tests
failing under a mutant are noise: judge a mutant by the tests that assert its subject.

## Where it lives

crates/app/src/header/splitting.rs (tests), crates/app/src/lib.rs
(tests::the_program_of_the_console_born_on_splitting_sees_the_size_of_its_pane)
