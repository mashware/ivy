---
id: asp_every-window-is-a-process-sharing-files_cstn7d
slug: every-window-is-a-process-sharing-files
name: >-
  Every Ivy window is a process of its own, so a file kept outside the
  per-window layout is written by several processes: compare identity, never
  count your own bytes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T18:32:32.252Z'
updated_at: '2026-11-21T18:32:32.252Z'
last_verified: '2026-11-21T18:32:32.252Z'
file_paths:
  - crates/app/src/diagnostics_log/sink.rs
  - crates/app/src/session/ownership.rs
symbols:
  - RotatingSink
content_hashes:
  crates/app/src/session/ownership.rs: 'sha256:58a28715ec45188904640f0bc8447424ccd79eb06ccd836f1d5a82883c7c4a88'
tags:
  - log
  - multi-process
  - rotation
feature_id: feat_term-core_nx0g1r
---

## What it does

Each window holds its own lease (crates/app/src/session/ownership.rs) and runs as a
separate process, so any shared file — the D113 log — has several writers at once. A
per-process counter of that file's state is wrong by construction. #139's first rotation
counted its own bytes: the log grew to N x cap, and one window renamed another's freshly
rotated file over the copy that held its own lines. Reading the size alone was not enough
either: a window still holding a moved-aside file, or a deleted one, kept writing where
nobody looks.

What holds: before each write compare the identity of the held file with the file at the
name (dev+ino on Unix, `windows_fs::identity` on Windows) and reopen the name when they
differ; rotate only a file you hold under the name. The same-instant race (two processes
crossing the cap within microseconds) is accepted in D113.

Evidence: review rounds 1 and 2 of #139 (built-in); tests
`two_windows_sharing_one_file_rotate_it_once_and_lose_nothing` and
`a_log_deleted_while_the_window_runs_comes_back_at_the_next_line` in sink.rs, the first
red when the follow-the-name check is removed.

## Where it lives

crates/app/src/diagnostics_log/sink.rs (RotatingSink::write, holds_the_name, rotate)
