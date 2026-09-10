---
id: asp_undoing-a-mutation-with-checkout-destroys-the-fix-round_b445wg
slug: undoing-a-mutation-with-checkout-destroys-the-fix-round
name: >-
  git checkout is the right way to undo a mutation and the wrong way the moment
  there are uncommitted fixes underneath it: commit, then mutate, then restore
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:23:52.735Z'
updated_at: '2026-09-10T15:23:52.735Z'
last_verified: '2026-09-10T15:23:52.735Z'
file_paths: []
symbols: []
content_hashes: {}
tags:
  - mutation
  - D30
  - method
  - git
feature_id: feat_term-core_nx0g1r
---

## What it does

**`git checkout -- <file>` restores the file to `HEAD`, so it takes the mutation *and everything else not committed* with it.**

Measured on 10 September 2026, in a review round on this repo: a fix batch of about ten edits — a new enum variant, a renamed function, a discriminated error, four rewritten comments, three rewritten tests — was in the working tree, **uncommitted**, when a mutation was applied on top and reverted with `git checkout --`. **The whole batch went with it.** Nothing warned; the command succeeded; the file came back as the last commit, which was the state before any of the fixes existed.

> **The order is the rule, and it is the opposite of what feels natural while iterating: commit, then mutate, then restore.** A WIP commit costs nothing here — they are squashed on merge — and it makes the mutation's undo **exact** instead of approximate. Mutating on top of uncommitted work means the undo has no definition.

**This is the mirror of [[an-agent-that-dies-can-leave-the-mutation-in-place]]**: that card is about a mutation surviving, this is about the fix not surviving, and both come from mutating a tree whose clean state is recorded nowhere. The check that catches both is the same — `git status --porcelain` empty before and after the round — and it only means anything **if `HEAD` is where you want to land**.

**And it is worth telling a delegated agent explicitly.** A brief that says «revert your mutations» will get `git checkout` unless it says otherwise, because that is the obvious command. The brief that survived this said: revert by editing the file back, never with checkout, and verify with `git diff --stat` that only the intended additions remain — and the agent that got that brief left the tree clean.

## Where it lives

No file: it is a method rule for the mutation practice **D30** demands. The place it is enforced is the brief handed to any agent asked to mutate, and the habit of a WIP commit before each round.
