---
id: asp_cfg-unix-imports-hide-windows-breakage_qv8tyr
slug: cfg-unix-imports-hide-windows-breakage
name: >-
  term-core imports some std items only under cfg(unix): code moved onto a
  cross-platform path compiles on Linux and breaks Windows
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-19T14:50:29.296Z'
updated_at: '2026-11-19T14:50:29.296Z'
last_verified: '2026-11-19T14:50:29.296Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - hang_up
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7ee8c68e3c4920c6671a448d0eebf260bbbb8bf6d4e66405f81fcab7e0108a65'
tags:
  - windows
  - cfg
  - build
feature_id: feat_term-core_nx0g1r
---

## What it does

**term-core imports some std items only under `#[cfg(unix)]`**,
for example `std::time::Instant`.
Code moved from a Unix-only path onto a cross-platform one
compiles and passes every test on Linux, and breaks the Windows build.

In #138 the review caught it only by running
`cargo check -p term-core --target x86_64-pc-windows-msvc --lib --tests`.
The target is installed on the development machine.

`--all-targets` fails there on the Unix-only `termios_probe` example.
That failure is pre-existing and not a signal.

**Run that check whenever a change touches `cfg(unix)` / `cfg(not(unix))` code in term-core.**

## Where it lives

`crates/term-core/src/lib.rs` (imports at the top; `hang_up` has a `cfg(unix)` and a
`cfg(not(unix))` version).
