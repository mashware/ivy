---
id: asp_a-restored-mutant-leaves-the-test-binary-mutated_xy57h6
slug: a-restored-mutant-leaves-the-test-binary-mutated
name: >-
  Restoring a mutant with mv leaves the test binary mutated: the restored file
  is older than the artifact, so cargo does not rebuild
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T15:18:55.151Z'
updated_at: '2026-09-25T15:18:55.151Z'
last_verified: '2026-09-25T15:18:55.151Z'
file_paths:
  - crates/app/src/panel/driving.rs
  - scripts/measure-frame.sh
symbols: []
content_hashes:
  crates/app/src/panel/driving.rs: 'sha256:a6b1e599ab71f802d787b5bd63044a1978873d2773a615869307f9f98c6e41bd'
  scripts/measure-frame.sh: 'sha256:465f0d9927c9fbebf177895a7c675b43daa5fd1baa5c2ab6e50b91c84020a588'
tags:
  - testing
  - mutation
  - cargo
feature_id: feat_term-core_nx0g1r
---

## What it does

A mutation loop of the shape `cp f f.orig; sed -i … f; cargo test; mv f.orig f` leaves the **test binary** built from the mutant. The restored file carries the `.orig` copy's mtime, taken BEFORE the mutant was compiled, so it is older than the artifact and cargo's mtime fingerprint sees nothing to rebuild. The next ordinary `cargo test` runs the mutant.

Measured in #248: after a mutation round the full suite failed on the AC1 test with the mutant's own error string (`Failed("half")`) while `git status` was clean and a grep of the source found nothing. For a file pulled in with `include_str!` (the measure script, read by `crates/app/tests/batches.rs`) it can go the other way: a test passes on a mutant the source no longer has.

**The verdicts of the round stay valid** — each `sed -i` gives the mutant a fresh mtime, so each mutant is compiled. What is wrong is the state left behind.

**The fix: `touch` every restored file.** `cp -p` is not the fix: it preserves the old time. Pair it with the checkout card's rule (commit, then mutate, then restore) and `git status --porcelain` empty before and after.

## Where it lives

Any mutation script over this workspace; the round-2 script of #248 touches restored files.
