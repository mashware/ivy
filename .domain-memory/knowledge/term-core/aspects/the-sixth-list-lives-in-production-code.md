---
id: asp_the-sixth-list-lives-in-production-code_nerc71
slug: the-sixth-list-lives-in-production-code
name: >-
  A module split in `crates/app` breaks SIX file lists, not five — and since
  #149 only the sixth still fails silently
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:23:16.847Z'
updated_at: '2026-09-28T21:23:09.467Z'
last_verified: '2026-09-28T21:23:09.467Z'
file_paths:
  - crates/app/tests/shape.rs
  - crates/app/src/settings.rs
  - crates/app/tests/prose.rs
  - crates/app/tests/glossary.rs
  - crates/app/tests/scripts.rs
  - crates/app/tests/common/mod.rs
symbols:
  - MODULES
  - SOURCES
  - COMMENT_LINES_AT_LEAST
  - CITATIONS_CHECKED
  - GLOSSARY_CLEAN_FILES
  - GLOSSARY_PENDING_FILES
  - list_against_disk
  - rust_sources
  - NOT_JUDGED
content_hashes:
  crates/app/tests/shape.rs: 'sha256:df9130232ffc0b12289f9b3af78f80a8cbb5adf37d10b575bc6944555d757111'
  crates/app/src/settings.rs: 'sha256:0f5d3514c630adc0cc813d9b0a408717d52b6a6770e45cce09f24d5d3276bb49'
  crates/app/tests/prose.rs: 'sha256:3ce39d39bbf9d44e6542d61c9a9caf682dc064f5dc965f79c7b711d98407f68c'
  crates/app/tests/glossary.rs: 'sha256:ee4d11471b96130526bb96dce2664bcc1063278f4bb286b5ba4ae51b28dc5d50'
  crates/app/tests/scripts.rs: 'sha256:f9ea30e1c87ca5f0c0d7efca283359b0fdabc552d0e10125daca9a1d986a9b82'
tags:
  - guards
  - d28
  - d27
  - module-split
feature_id: feat_term-core_nx0g1r
---

## What it does

`crates/app/tests/shape.rs`'s own header names **six** file lists. A delivery that greps the test directory finds only five of them.

| List | File | What it guards | How it fails when you forget a file |
|---|---|---|---|
| `MODULES` | `crates/app/tests/shape.rs` | the 600-line module ceiling and the 300-line `impl` ceiling | **Loudly since #149**: checked against `src/` by `common::list_against_disk`, naming the file with no row |
| the citations lists | `crates/app/tests/glossary.rs` (**two entries, two lists**) | comments citing identifiers the crate no longer has | **Loudly**: *«`app/src/<name>.rs` is a file of `app` and is not on the citations list»* |
| `SOURCES` | `crates/app/tests/prose.rs` | that every comment is in English | **Loudly**: checked against `src/` and `examples/` |
| `MODULES` | `crates/app/tests/scripts.rs` | the scripts these crates run | **Loudly since #149**: checked against `app` and `term-core` `src`, `examples` and `tests`, minus `NOT_JUDGED` (scripts.rs itself, whose fixtures carry the defect on purpose) |
| **the sentence walk** | **`crates/app/src/settings.rs`**, inside its `#[cfg(test)] mod tests` | that user-facing message literals are not Spanish | **Still silently**, and it is the one a grep of `tests/` never finds |

**The #149 measurement, September 2026.** `scripts.rs` had a count floor of 43, and every delivery kept that number current. The list was still **12 files behind** `app/src` + `term-core` (`src`, `examples`). Checking it against the tree found them. Widening the walk found **14 more** in `app/examples`, `app/tests` and `term-core/tests`, and `idle_console_cpu.rs` runs raw-mode scripts. One of the first twelve, `term-core/examples/termios_probe.rs`, carried the exact `stty raw -echo; printf ready` defect the guard exists for. **A count floor raised by hand binds only the delivery that remembers to raise it.** `shape.rs`'s floor had also gone slack again, at 37 against 41.

**The shared check needs a fixture of its own.** On a green day every list matches the tree. So a `list_against_disk` that always answered «they match» left all three lists that use it green, and that was measured. `a_list_is_told_apart_from_the_disk_in_both_directions` in `prose.rs` is its subject. It returns `Result<(), String>` rather than `Option` so that a dropped call warns.

**Earlier measurement (the sixth list).** The delivery that moved the keys' walk and the wheel out of `lib.rs` paid five lists and missed the sixth. Three sentence-shaped literals travelled into `keyboard.rs` and `mouse.rs`, and the corpus fell **76 → 73** against a floor of **72**. That stayed green.

> **A floor only fires when somebody REMOVES an entry, never when somebody forgets to add one.** That asymmetry is why the sentence walk is still the dangerous list, and why [[a-total-floor-cannot-answer-whether-one-file-is-read]] added a per-file floor.

**A brand-new file trips the lists too**, not only a split: measured on #147 and again on #149. Two places catch it:

- **The glossary citations lists.** A new file anywhere in `crates/app` fails `no_comment_cites_a_name_the_code_no_longer_has` until it is in `CITATIONS_CHECKED` and in one of `GLOSSARY_CLEAN_FILES` / `GLOSSARY_PENDING_FILES`. From then on every backticked token in its comments is read as an identifier of the crate, so write a decision as plain D28, not in backticks.
- **The other lists.** A new file under `src/` also has to be added to prose `SOURCES` (with its comment-line floor), shape `MODULES` and scripts `MODULES`.

Only `cargo test --workspace` runs all of them.

## Where it lives

`crates/app/tests/common/mod.rs` holds `rust_sources` and `list_against_disk`, which prose, shape and scripts use. `MODULES` in `crates/app/tests/scripts.rs` carries `NOT_JUDGED` and its tree walk in `the_list_still_names_every_module`. The sentence walk is the `SOURCES` array inside `#[cfg(test)] mod tests` in `crates/app/src/settings.rs`. The citations lists are `CITATIONS_CHECKED`, `GLOSSARY_CLEAN_FILES` and `GLOSSARY_PENDING_FILES` at the top of `crates/app/tests/glossary.rs`.
