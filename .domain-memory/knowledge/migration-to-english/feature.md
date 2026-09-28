---
id: feat_migration-to-english_xnx8mc
slug: migration-to-english
name: >-
  F2.12 — the repository moves to English (D27): its guards, and what none of
  them sees
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:06:33.222Z'
updated_at: '2026-09-03T16:06:33.222Z'
last_verified: '2026-09-03T16:06:33.222Z'
file_paths:
  - docs/02-decisions.md
  - crates/app/tests/glossary.rs
  - crates/app/tests/prose.rs
  - docs/05-roadmap.md
symbols:
  - FINISHED_CRATES
  - GLOSSARY_CLEAN_FILES
  - NOT_OURS
  - SPANISH_FUNCTION_WORDS
content_hashes:
  docs/02-decisions.md: 'sha256:4eb23bb4f11a5f60d8d236644be773469b8f5a617d1657294e0b9d39d4c9872a'
  crates/app/tests/glossary.rs: 'sha256:bb3266feceb40eb1c3c4847deb16921f3432c141ca48f81cb8d8ab367e00fa32'
  crates/app/tests/prose.rs: 'sha256:b3b194d61079eaeef5626f13b7b1d59f308fe619c376a967181913fc0c47b441'
  docs/05-roadmap.md: 'sha256:053371b4421327ea70ca30a39835b3b60f8726fe9944622dcf4cc402c50cfb0c'
tags:
  - d27
  - f2.12
  - migration
  - guards
  - translation
---

## What it does

This project's own code has been in Spanish **since the very first commit**, and that decision was written down nowhere, which is what let it grow to 43 860 lines. **D27** reverses it: the whole repository moves to English — names, prose, documents and the stored knowledge — **with no alias table on any key**, because nobody has it installed and this is the only window in which it comes for free.

The milestone runs in **16 deliveries** with a glossary of its own that decides, term by term, when to adopt the word WezTerm, Ghostty or tmux already use and when to coin one — and it **forbids adopting a word the project has already spent on something else**. That prohibition is the one most broken in practice, because a badly chosen English word does not look like a mistake.

What this feature records is not how one translates, but **what goes wrong when translating at scale and which of the guards catches it** — because the answer has been, more than once, none of them: the glossary guard catches **retired Spanish** names, and until September 2026 it declared in its own header that comments, rustdoc and literals were **out of its scope**.

## Where it lives

**The decision and the glossary**: `docs/02-decisions.md`, annex of **D27** — the term-by-term table, what is never translated (the batch letters `A`–`Q`, among others) and which names are the on-disk format, which is the only place where renaming breaks something without anything failing.

**The schedule**: `docs/05-roadmap.md`, Part II.

**The guards, and the real scope each one declares**:
- `crates/app/tests/glossary.rs` — identifiers. `FINISHED_CRATES` is the strong claim («no Spanish identifier at all») and as of September 2026 it only carries `panel-contract`, `host-load` and `term-core`; `GLOSSARY_CLEAN_FILES` is the weak one («no name the glossary retired») and it is the one covering the files of `app`. `NOT_OURS` is its list of exemptions, and its size is a signal: every entry silences a whole class.
- `crates/app/tests/prose.rs` — the prose, since delivery #12. Before it, **nothing stopped Spanish from being put back into an already translated file**.
- `crates/app/src/settings.rs` — two sweeps over the phrases that are seen on screen, with `SPANISH_FUNCTION_WORDS` as the oracle: a **closed** list, and none of its words is also an English word, which is why «no» and «sin» cannot go in.

**And the store itself, from delivery #16**: `crates/app/tests/prose.rs` walks `.domain-memory/` too — Spanish coming back fails, a record citing a file the tree does not have fails, and so does the store losing records. It reads the directory rather than naming its files, because a walk **finds** a renamed record instead of skipping it, and the record count is the floor that answers what a walk cannot see.

## Renaming a record needs a step nobody is told about, and the milestone nearly shipped without it

Delivery #16 renamed all 74 files of the store and moved each one's `slug` and `id` with it. That is safe **only** because the `.md` files are the source of truth and `index.sqlite` is gitignored — but «the index is rebuilt from what the files say» is **not** something that happens on its own, and believing it was is how this nearly went wrong.

- **The MCP server never scans disk.** It answers out of the index; nothing in it watches the store.
- **The rebuild is a manual CLI**, `domain-memory reindex`, and it is mentioned in no file of this repository.
- **And bare `reindex` upserts.** It does not delete rows whose files are gone. With 69 of the 72 ids changed by the rename, it would have left **144 active rows**: the 72 real ones plus 69 ghosts still answering their Spanish `topic_key`, still pointing `content_path` at files that no longer exist, and still handing `check_drift` the pre-milestone source paths at PR time. No unique index collides, so none of it says anything.

**So the step is `domain-memory reindex --fresh`**, which deletes the index before rebuilding, and it has to be run by whoever merges — it is a local index, so every checkout needs it once. Written down here because it is the kind of thing that is discovered by somebody getting a wrong answer three weeks later.
