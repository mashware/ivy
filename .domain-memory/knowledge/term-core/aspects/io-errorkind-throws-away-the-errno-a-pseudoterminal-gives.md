---
id: asp_io-errorkind-throws-away-the-errno-a-pseudoterminal-gives_rfjk3g
slug: io-errorkind-throws-away-the-errno-a-pseudoterminal-gives
name: >-
  io::ErrorKind is a coarse bucket, and the errors a pseudoterminal actually
  produces fall in the one that says nothing — the system's number is what
  survives
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:22:37.057Z'
updated_at: '2026-09-10T15:22:37.057Z'
last_verified: '2026-09-10T15:22:37.057Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - Ending
  - 'Ending::Failed'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
tags:
  - std
  - dependencies
  - D38
  - F2.7
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured by compiling and running, not by reading:**

| errno | `e.kind()` | `e.raw_os_error()` |
|---|---|---|
| 5 (`EIO`) | `Uncategorized` | `Some(5)` |
| 13 | `PermissionDenied` | `Some(13)` |
| 32 | `BrokenPipe` | `Some(32)` |
| 70 | `Uncategorized` | `Some(70)` |
| 75 | `Uncategorized` | `Some(75)` |

`EIO` — «the other end of this pseudoterminal is gone», the likeliest real failure on this path — comes out as the bucket whose entire `Debug` form is the single word `Uncategorized`. And that variant is **unnameable in user code**: it is unstable upstream, so a consumer cannot even `match` on it to mean «unknown».

> **A value that carries only the kind cannot be turned into a sentence.** Anything that reports «the read failed» to a person, or on which a platform-portability argument rests, has to carry `raw_os_error()` alongside — an `Option<i32>`, `Copy` and `Eq`, so it costs nothing in shape.
>
> **And the kind is for formatting, never for branching.** `ErrorKind` gains variants between releases, so a `match` over it stops being exhaustive without this crate changing.

**Why it stayed invisible here until it was looked for**: `portable-pty` absorbs `EIO` into `Ok(0)` on **both** platforms (see [[both-platforms-answer-end-of-file-and-windows-never-does]]), so the flattening never shows up in the ordinary path — only in a real failure, which is rare, and on the platform nobody has run.

**How it was found**: a blinded idiom auditor, on a value that had already been declared as a contract, reviewed by a blinded contract verifier that checked its derives character for character and had no reason to question the payload. Two reviewers agreed the shape was right; the one that had not been told what the shape was for asked what the sentence would say.

**And the test that keeps the argument alive** pins the uselessness: it asserts the kind's `Debug` really is that one word. The day that stops being true, the reason for the second field has changed and something goes red instead of the prose quietly rotting.

## Where it lives

`crates/term-core/src/lib.rs` — `Ending::Failed { kind, os_error }` and the test that feeds it `Error::from_raw_os_error(5)`. The decision is **D38**.
