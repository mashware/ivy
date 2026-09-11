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
updated_at: '2026-09-11T20:23:16.847Z'
last_verified: '2026-09-11T20:23:16.847Z'
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
content_hashes:
  crates/app/tests/shape.rs: 'sha256:dafb615269cba5e6910c01d4fe77981f158c73a3cec9b2e736bd3728e0f7de99'
  crates/app/src/settings.rs: 'sha256:b4bcf5132ae7c60dc9cccb4fedae1c93d77362d46527808e07d10c98d53a1040'
  crates/app/tests/prose.rs: 'sha256:6a3dff196aac01ef74b59d6dc3d077e0ddd4c211cae097434f47b9b3a542f533'
  crates/app/tests/glossary.rs: 'sha256:c547773d4b9a1fbc748d72b9ce900c2a11cfbbdd6cc9ac26304077c77dfbd5f5'
  crates/app/tests/scripts.rs: 'sha256:6e13f37687b1d193d47c56b76a1f0d223d922f19b6ec7903752e0bfde4bc1672'
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

## Where it lives

`crates/app/tests/shape.rs`'s module header names all six. The fifth and sixth are the ones to look up by hand: `MODULES` in `crates/app/tests/scripts.rs`, and the `SOURCES` array inside `#[cfg(test)] mod tests` in `crates/app/src/settings.rs`.
