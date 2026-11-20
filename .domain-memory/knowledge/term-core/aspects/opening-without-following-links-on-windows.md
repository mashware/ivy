---
id: asp_opening-without-following-links-on-windows_hh9t4z
slug: opening-without-following-links-on-windows
name: >-
  Opening a file on Windows without following a link: the flag also bypasses
  every other reparse point, and the tag must not be read by hand
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T18:22:43.905Z'
updated_at: '2026-11-20T18:22:43.905Z'
last_verified: '2026-11-20T18:22:43.905Z'
file_paths:
  - crates/app/src/windows_fs.rs
  - crates/app/src/viewer/load.rs
symbols:
  - open_no_follow
  - identity
content_hashes:
  crates/app/src/viewer/load.rs: 'sha256:0a8f7015078b4f4646688ae5a29277ba050f48b056eb6f82d20a40c2ae317b03'
tags:
  - windows
  - security
  - filesystem
feature_id: feat_term-core_nx0g1r
---

## What it does

Two traps, found reviewing #177 (20 November 2026) and fixed in
`windows_fs::open_no_follow`.

**1. `FILE_FLAG_OPEN_REPARSE_POINT` applies to every reparse point, not only
links.** A deduplicated (`IO_REPARSE_TAG_DEDUP`), WOF-compressed or
dehydrated cloud file opened that way bypasses the filter that owns it,
so the read returns its stub: zeros, shown as a binary file. Refuse only
name surrogates (symbolic links, junctions). Reopen any other reparse point
without the flag, and keep the second handle only if its volume serial and
file index match the first, so a file swapped in between is not accepted.

**2. Do not read `FILE_ATTRIBUTE_TAG_INFO.ReparseTag` by hand.** The tag is
undefined when `FILE_ATTRIBUTE_REPARSE_POINT` is clear. std's
`File::metadata()` on the open handle queries `FileAttributeTagInfo` only
when that attribute is set, and `Metadata::is_symlink()` applies the
name-surrogate rule. Use it.

Evidence: raised independently by the built-in review and the
silent-failure reviewer; the std behaviour confirmed from std's Windows fs
source by two reviewers. On the `windows-2025` runner the no-follow test
passes, and goes red when the arm is reverted to a plain `File::open`
(PR #181, runs 44791059106 and 44788772288). The reopen of a non-link
reparse point has no test: no runner can create one.

## Where it lives

`crates/app/src/windows_fs.rs` (`open_no_follow`, `identity`), used by the Windows arm
of `open_untracked` in `crates/app/src/viewer/load.rs`. Decision D108.
