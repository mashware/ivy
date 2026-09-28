---
id: asp_the-sixth-list-lives-in-production-code_nerc71
slug: the-sixth-list-lives-in-production-code
name: >-
  A module split in `crates/app` breaks SIX file lists, not five — and the one
  that gets missed is the one that does not live under `tests/`
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:23:16.847Z'
updated_at: '2026-09-28T16:49:39.417Z'
last_verified: '2026-09-28T16:49:39.417Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/src/settings.rs
  - crates/app/tests/prose.rs
  - crates/app/tests/glossary.rs
  - crates/app/tests/scripts.rs
symbols:
  - MODULES
  - SOURCES
  - COMMENT_LINES_AT_LEAST
  - CITATIONS_CHECKED
  - GLOSSARY_CLEAN_FILES
  - GLOSSARY_PENDING_FILES
content_hashes:
  crates/app/tests/shape.rs: 'sha256:b8474648d5df70f6dad440a1c2864dd695f49fb5efd5a6ba66be8d1a5ccc0cca'
  crates/app/src/settings.rs: 'sha256:31ae91ea13ad8b095237c0ac344e4507116cdffd046b4969b0b833ae5c3cc222'
  crates/app/tests/prose.rs: 'sha256:ab0b9e74bd4600f582efddfc69ed57e17ebd23567f4fc186e61044fe0943baa0'
  crates/app/tests/glossary.rs: 'sha256:90d6c32c3639ed72476a08556a5782aedb6529d909f3629eb7791aafedf52ae2'
  crates/app/tests/scripts.rs: 'sha256:0203fb55ee365b3246dc5f31fd1f042cc21f6e8746e7aaa8c25bd89c8663cfd0'
tags:
  - guards
  - d28
  - d27
  - module-split
feature_id: feat_term-core_nx0g1r
---

## What it does

**Supersedes the earlier card that said five.** `crates/app/tests/shape.rs`'s own header names **six**, and a delivery that greps the test directory finds only five of them.

| List | File | What it guards | How it fails when you forget |
|---|---|---|---|
| `MODULES` | `crates/app/tests/shape.rs` | the 600-line module ceiling and the 300-line `impl` ceiling | **Silently** — the module is born with no ceiling at all |
| the citations lists | `crates/app/tests/glossary.rs` (**two entries, two lists**) | comments citing identifiers the crate no longer has | **Loudly**: *«`app/src/<name>.rs` is a file of `app` and is not on the citations list»* |
| `SOURCES` | `crates/app/tests/prose.rs` | that every comment is in English | **Silently** — the floor's slack absorbs the drop |
| `MODULES` | `crates/app/tests/scripts.rs` | the scripts these crates run | **Silently**, and its own docblock narrates this exact failure happening before |
| **the sentence walk** | **`crates/app/src/settings.rs`**, inside its `#[cfg(test)] mod tests` | that user-facing message literals are not Spanish | **Silently**, and it is the one a grep of `tests/` never finds |

**Measured, September 2026.** The delivery that moved the keys' walk and the wheel out of `lib.rs` paid five lists and missed the sixth: three sentence-shaped literals travelled with the moved blocks into `keyboard.rs` and `mouse.rs`, neither of which that walk names, so its corpus fell **76 → 73** against a floor of **72**. Green, with three user-facing messages outside the guard's sight. **That list's own comment had already narrated the same failure twice**, from two earlier splits, and it still did not defend against it — because a list only defends the files it names.

> **A floor only fires when somebody REMOVES an entry, never when somebody forgets to add one.** That asymmetry is why the sixth list is the dangerous one and why the per-file floor of [[a-total-floor-cannot-answer-whether-one-file-is-read]] had to be added beside its total.

**And check the floors, not just the lists.** Four of the six carried an off-by-one inherited from earlier deliveries (a floor of 21 against 22 entries, 30 against 31), and `prose.rs`'s corpus floor was **2 392 lines** behind the tree. Run each guard's floor against its list and confirm the slack is the number the comment claims, not merely positive.

**Adding the sixth list has a cost worth expecting**: it walks files that were never checked, and the first thing it found was a user-facing message carrying an em dash. See [[user-facing-text-is-ascii-comments-are-not]].

**Not only splits: a brand-new file trips the citations lists too** (measured on #147, 28 September 2026). A new integration test under `crates/app/tests/` fails `no_comment_cites_a_name_the_code_no_longer_has` until it is in `CITATIONS_CHECKED` (with its crate) **and** in one of `GLOSSARY_CLEAN_FILES` / `GLOSSARY_PENDING_FILES`, which partition that manifest. Once it is in, the guard reads **every backticked token in its comments as an identifier of the crate**: a decision cited as `` `D28` `` fails it — the crate's idiom is plain D28 or **D28**. Neither failure shows in `cargo test -p app --test <new file>`; only `cargo test --workspace` runs the guard.

## Where it lives

`crates/app/tests/shape.rs`'s module header names all six. The fifth and sixth are the ones to look up by hand: `MODULES` in `crates/app/tests/scripts.rs`, and the `SOURCES` array inside `#[cfg(test)] mod tests` in `crates/app/src/settings.rs`. The citations lists are `CITATIONS_CHECKED`, `GLOSSARY_CLEAN_FILES` and `GLOSSARY_PENDING_FILES` at the top of `crates/app/tests/glossary.rs`.
