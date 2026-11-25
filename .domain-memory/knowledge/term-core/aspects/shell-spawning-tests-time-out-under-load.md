---
id: asp_shell-spawning-tests-time-out-under-load_8y1ab8
slug: shell-spawning-tests-time-out-under-load
name: >-
  Shell startup fixtures need controlled input and startup before attributing
  timeouts to load
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T10:20:36.485Z'
updated_at: '2026-11-25T21:13:56.556Z'
last_verified: '2026-11-25T21:13:56.556Z'
file_paths:
  - crates/app/src/header/splitting.rs
symbols:
  - split_directory_tracks_cd_for_both_orientations
  - Sandbox
  - split_directory_unknown_after_source_exit_uses_ordinary_launch_directory
content_hashes:
  crates/app/src/header/splitting.rs: 'sha256:0c3946b4b6fdadbb792ce180780bb6467f729b80785b2b78c50731ceccc072d7'
tags:
  - tests
  - shell
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


### Controlled startup evidence from issue 245

The split-directory fixtures depended on arbitrary user-shell startup finishing and preserving queued input within five seconds. A controlled six-second externally selected shell reproduces the original unknown-directory timeout (5.01s). Those Unix fixtures now use the existing thread-local TestShell injection with a real fixture-owned sh launcher; the default-directory case explicitly changes and checks its startup directory, retaining the original routing assertion. All six cases pass in0.11s under the same externally selected slow shell, without increasing deadlines. This repairs that fixture dependency rather than treating its red result as an excused flake.

The earlier load observation remains historical evidence, not a diagnosis for every timeout. The split-size test mentioned above was not changed by this fixture repair.

## Where it lives

crates/app/src/header/splitting.rs: Sandbox, split_directory_tracks_cd_for_both_orientations, split_directory_unknown_after_source_exit_uses_ordinary_launch_directory. crates/app/src/lib.rs: tests::the_program_of_the_console_born_on_splitting_sees_the_size_of_its_pane.
