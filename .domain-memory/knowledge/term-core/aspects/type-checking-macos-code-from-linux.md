---
id: asp_type-checking-macos-code-from-linux_3fr8rq
slug: type-checking-macos-code-from-linux
name: >-
  macOS-only code cannot be checked by building the app for Apple from Linux —
  `ring` needs an Apple C compiler — but term-core checks directly and a scratch
  crate type-checks app modules
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-10T12:13:42.018Z'
updated_at: '2026-11-18T19:59:44.667Z'
last_verified: '2026-11-18T19:59:44.667Z'
file_paths:
  - crates/app/src/chrome/traffic_lights.rs
  - crates/app/Cargo.toml
  - crates/term-core/src/lib.rs
  - crates/term-core/src/foreground/macos.rs
symbols: []
content_hashes:
  crates/app/src/chrome/traffic_lights.rs: 'sha256:6cba9889fa388116a1b361f3428d4cc8982b6cffd4884ce298e5c4c7814598c7'
  crates/app/Cargo.toml: 'sha256:3e8a7b0870379c0db6768e7362286f01005ea14c57e546eecfac445c8f63f25f'
  crates/term-core/src/lib.rs: 'sha256:7ee8c68e3c4920c6671a448d0eebf260bbbb8bf6d4e66405f81fcab7e0108a65'
  crates/term-core/src/foreground/macos.rs: 'sha256:e7d198225b5ec0328c919b9b1333d1121a6319a6202f2f77e1053c9077cee4b2'
tags:
  - macos
  - cross-compile
  - objc2
  - method
feature_id: feat_term-core_nx0g1r
---

## What it does

**`cargo check -p app --target aarch64-apple-darwin` fails on Linux before it
reaches our code**: `ring` (pulled by the update checker's HTTPS) runs a C build
script, and the host `cc` rejects `-arch` / `-mmacosx-version-min`. This machine
has no clang either. Measured on #34, November 2026, and again on #116.

**`term-core` needs no workaround**: it has no C dependency, so
`cargo check -p term-core --all-targets --target aarch64-apple-darwin` (and
`clippy`) runs directly in seconds once `rustup target add aarch64-apple-darwin`
is done. On #116 it also showed two clippy lints in `foreground/macos.rs` that
no Linux run can see: CI lints and tests nothing for macOS.

**For app modules, a scratch crate works:** a throwaway crate in the scratchpad
whose only dependencies are the ones the macOS module uses, pinned to the
lockfile's versions (`objc2-app-kit =0.3.2`, `objc2-foundation =0.3.2`,
`raw-window-handle =0.6.2`), with the module included as `pub mod x;` (not
`include!`, which rejects a file's `//!` docs).
`cargo clippy --offline --target aarch64-apple-darwin -- -D warnings` then
type-checks every AppKit call and every objc2 feature flag; the only expected
errors are dead-code. objc2's crates need no C, so they build for Apple from
Linux.

**The gap this leaves, measured on #116:** a feature written as
`#[cfg(target_os = "linux")]` with a `None` fallback compiles everywhere and
fails nowhere, so it can be missing on macOS for weeks. `Pty::working_directory`
was Linux-only, so Files, project colours, split directory and restore were all
dead on macOS, and its test was Linux-only too. `Pty::foreground_program` still
has the same shape. When reviewing, grep for `cfg(not(target_os = "linux"))`
branches that return `None`.

**What none of this proves:** anything the OS does at runtime. Keep pure logic
out of `cfg(target_os = "macos")` code so Linux tests can hold it
(`traffic_lights.rs` splits `container` / `button_y` from `center`).

## Where it lives

Method. App-side macOS dependencies are under
`[target.'cfg(target_os = "macos")'.dependencies]` in `crates/app/Cargo.toml`;
term-core's macOS FFI lives in `crates/term-core/src/foreground/macos.rs`.
