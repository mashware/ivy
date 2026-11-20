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
updated_at: '2026-11-20T20:30:00.000Z'
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

### The app crate cannot be cross-checked the same way

`cargo check -p app --target x86_64-pc-windows-msvc --lib --tests` fails
before it reaches our code: `ring` (through `ureq`, a normal dependency)
runs a C build script that needs `lib.exe`, and the development machine
has no MSVC or LLVM tools. Measured on #177, 20 November 2026.

What works, the same trick as for macOS:

- Put the Win32 code in one self-contained module
  (`crates/app/src/windows_fs.rs`) and `#[path]`-include it from a scratch
  crate whose only dependency is `windows-sys` at the lockfile's version.
- `cargo clippy --offline --target x86_64-pc-windows-msvc --all-targets -- -D warnings`
  there type-checks every Win32 call. `windows-sys` needs no C.
- The real check is the PR-only `windows-startup-checks` job, which runs
  `cargo test -p app --lib` with a name filter on `windows-2025`.

Also: `std::os::windows::fs::MetadataExt::{volume_serial_number, file_index}`
are still unstable on 1.97.1 (`windows_by_handle`, E0658). Use
`windows-sys`'s `GetFileInformationByHandle`.

## Where it lives

`crates/term-core/src/lib.rs` (imports at the top; `hang_up` has a `cfg(unix)` and a
`cfg(not(unix))` version).
