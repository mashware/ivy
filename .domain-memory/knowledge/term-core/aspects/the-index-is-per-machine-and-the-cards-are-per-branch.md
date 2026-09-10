---
id: asp_the-index-is-per-machine-and-the-cards-are-per-branch_5wtaf3
slug: the-index-is-per-machine-and-the-cards-are-per-branch
name: >-
  The knowledge index is per machine and its markdown is per branch, so a search
  can return a card whose file is not in this checkout
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:24:08.433Z'
updated_at: '2026-09-10T15:24:08.433Z'
last_verified: '2026-09-10T15:24:08.433Z'
file_paths:
  - .domain-memory/instructions.md
symbols:
  - search_knowledge
  - content_path
content_hashes:
  .domain-memory/instructions.md: 'sha256:42af97e61c14d8acadc4912d4ea90672ae0cf95e7b68e2a2f3742a3d6f074dbf'
tags:
  - knowledge-store
  - method
  - git
feature_id: feat_term-core_nx0g1r
---

## What it does

**A search can hand back a card whose `content_path` does not exist where you are standing, and nothing in the result says so.**

Measured on 10 September 2026, on `main`: a design search returned two cards with summaries and paths under `.domain-memory/knowledge/`. **Both files were absent from that working tree** (`ls` fails) and absent from the base branch (`git cat-file -e` fails). They existed in exactly one place: a commit on an **unmerged work branch**, checked out in another worktree.

**The mechanism.** The index is **git-ignored and per machine**, so it keeps whatever was indexed when a file was last on disk — including a branch you are no longer on. The markdown is **versioned and therefore per branch**. The two halves drift apart the moment a card is written on a branch that has not merged.

> **The consequence for the instruction that says «read the file at `content_path` when the summary is not enough»: that read can fail, and the failure looks like a missing card rather than a card on another branch.** The summary is still trustworthy — it was written from the real content — and the full text is reachable with `git show <branch>:<path>`.

**The consequence that bites later**: a card written on a branch that is abandoned, squash-merged without that file, or rebased away is **gone from disk while still being served by the index**, until the index is rebuilt. The knowledge is then neither lost nor delivered, and the search cannot tell the difference.

**So consolidating is not delivering.** A card is delivered when its markdown is on the base branch. Until then it is one machine's index and one branch's file, and the searcher who finds it may be neither.

## Where it lives

`.domain-memory/instructions.md` describes reading `content_path` when a summary is not enough; the index lives at `.domain-memory/index.sqlite`, which the repo's `.gitignore` excludes along with the staging, while `.domain-memory/knowledge/**` is versioned.
