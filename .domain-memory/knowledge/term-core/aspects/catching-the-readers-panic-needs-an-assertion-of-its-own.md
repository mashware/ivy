---
id: asp_catching-the-readers-panic-needs-an-assertion-of-its-own_c9y6q6
slug: catching-the-readers-panic-needs-an-assertion-of-its-own
name: >-
  Catching the reader thread's panic is possible in this workspace and will not
  compile without asserting unwind safety — and the catch makes a comment in the
  destructor false
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:24:48.913Z'
updated_at: '2026-09-10T15:24:48.913Z'
last_verified: '2026-09-10T15:24:48.913Z'
file_paths:
  - crates/term-core/src/lib.rs
  - Cargo.toml
symbols:
  - 'Pty::spawn_with_env'
  - 'Pty::drop'
  - try_clone_reader
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
  Cargo.toml: 'sha256:51bcda70fa5f02b270cb6883d6d1d1875b50b25c48da19d04b278a2b300cbd61'
tags:
  - panics
  - threads
  - std
  - F2.7
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two facts anybody wrapping this crate's reader thread in `catch_unwind` needs before starting, and they pull in opposite directions.**

**1. It can be caught at all.** No `[profile]` section exists in the workspace manifest or in either crate's `Cargo.toml`, so there is no `panic = "abort"` and a panic in a spawned thread unwinds. Checked by grep over all three manifests; the only `panic` hit is a comment about the drawing library.

**2. It will not compile without asserting.** The reader owns the `Box<dyn Read + Send>` that `try_clone_reader` returns. `Box<T>: UnwindSafe` requires `T: UnwindSafe`, and a bare `dyn Read + Send` carries no such bound — so `AssertUnwindSafe` is **required**, while `&AtomicBool` and `&SyncSender<Vec<u8>>` are `UnwindSafe` on their own.

> **So the assertion is not a formality and must not be written to silence the compiler.** It claims that a channel and a reader which a panic may have left half-used are still sound to observe afterwards. For this thread the claim holds — the standard library's channels do not poison and the reader is dropped straight after — but **the reason belongs where the assertion is**, because the next person to widen that closure inherits the claim without being asked.

**And what a caught panic changes for the destructor.** `Drop for Pty`'s `Disconnected` arm carries the comment *«a reader that panicked looks exactly like this, and the `join` here discards that too»*. Catch the panic and the thread ends **normally**, so that comment becomes false — the class this project counts its review blockers in. Rewriting it is part of the same change, not a follow-up.

**What it does not fix.** A panic that is *not* caught leaves nothing recorded, so a host asking how the reading ended gets «nothing was recorded» about a console that is over. That is why an accessor's absence must never be documented as «still reading».

## Where it lives

`crates/term-core/src/lib.rs` — the reader thread in `Pty::spawn_with_env`, and `Drop for Pty`'s three closing arms. The manifests checked are the workspace root and both crates'.
