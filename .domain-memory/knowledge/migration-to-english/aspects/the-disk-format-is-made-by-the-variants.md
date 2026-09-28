---
id: asp_the-disk-format-is-made-by-the-variants_ypabp4
slug: the-disk-format-is-made-by-the-variants
name: >-
  The on-disk format is made by the fields AND the variants: with `rename_all`,
  renaming a variant rewrites the file and nothing that is run daily catches it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:11:25.448Z'
updated_at: '2026-09-03T16:11:25.448Z'
last_verified: '2026-09-03T16:11:25.448Z'
file_paths:
  - crates/app/src/session.rs
  - crates/app/src/measure.rs
  - scripts/measure-frame.sh
symbols:
  - StoredOrientation
  - StoredNode
  - StoredDocument
  - StoredProbe
  - VERSION
content_hashes:
  crates/app/src/session.rs: 'sha256:b7824b96a3e438e7695ed87ccecbe9d65ed9dd16258b5e74ba3c39c8008f09e3'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  scripts/measure-frame.sh: 'sha256:ff632681ce0123f34a58a161ee3726107de47669051545e12a59b4968f47bd42'
tags:
  - d27
  - f2.12
  - serde
  - disk-format
  - guards
feature_id: feat_migration-to-english_xnx8mc
---

## What it does

**The keys of `layout.json` come out of Rust names without a single literal, and the dangerous hole is not the fields: it is the VARIANTS.**

In `crates/app/src/session.rs` there are **only two serde attributes in the whole tree** — the two `rename_all = "lowercase"` — and **zero `#[serde(rename = …)]`**. Verified in September 2026: it is still 2 and 0. Everything else that reaches the file **is** the name of the field or of the variant.

**The fields were already protected by a doctrine written in the file itself**: `StoredOrientation` is a type separate from `crate::Orientation` **on purpose**, because the names of that enum *are* the format — they appear literally in the file of whoever uses the application, and reusing the live type would tie the file to a name the code may want to change tomorrow. The same goes for `StoredNode`, `StoredChild` and `StoredTab`, which exist twice for that reason. And `StoredProbe` exists because *looking at the version after fitting the shape is having no version at all*, with its rule for when it goes up: **exactly with what an old reader would misread — a key renamed, removed, or given another meaning**.

> **What that doctrine did NOT cover, and it is where it bites: with `rename_all`, the names of the VARIANTS are the file.** `Columns`/`Rows` and `Leaf`/`Branch` travel to disk in lowercase. Renaming `Filas` to `Rows` leaves the file writing `"rows"` while the measurement script keeps seeding `"orientacion":"filas"`.

**And nothing that is run daily catches that**: not the compiler (it compiles), not `cargo test` if no test confronts the format with what the script seeds, and not a prefix in the name of the type. It was caught **only** by the `restored:` guard of batch `L` of the bench, and only if someone ran it — the same failure mode as [[a-batch-has-saving-vetoed]]: a guard nobody runs for weeks.

### How it was closed, and why this finding still holds

It is the reason **AC10** of the milestone exists: when renaming fields or variants, the version goes up in the same commit **and the script seeds the new format**. Verified in September 2026: the script already seeds `"orientation":"rows"` and `"columns"`, that is, both halves moved together. And the half that could not be seen is tied down by a test that **derives** instead of copying: `measure.rs:2879` asserts `SCRIPT.contains(crate::session::FILE)` over the text of the script read with `include_str!`.

**The practical rule it leaves:** in this project, «does this change the on-disk format?» is answered by looking at **fields, variants and `rename_all`** — never at the fields alone. And any test that wants to tie it down has to **derive** the format from what the script seeds, not assert a literal written right beside it.

## Where it lives

`crates/app/src/session.rs` — the two `rename_all` and the doctrine of the duplicated types (`StoredOrientation`, `StoredNode`, `StoredChild`, `StoredTab`, `StoredProbe`), plus `VERSION`, `DIRECTORY` and `FILE`.

`scripts/measure-frame.sh` — where the format is seeded by hand for the batch that restores.

`crates/app/src/measure.rs:2879` — `the_script_seeds_the_same_layout_file_the_application_writes`, the derived guard that closes the half that could not be seen; its own comment explains the disguise it replaces.

`docs/02-decisions.md` — **AC10** of milestone F2.12, and the annex of **D27** in the section on which names are the on-disk format.
