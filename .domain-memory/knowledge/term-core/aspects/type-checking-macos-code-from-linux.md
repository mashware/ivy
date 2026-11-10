---
id: asp_type-checking-macos-code-from-linux_3fr8rq
slug: type-checking-macos-code-from-linux
name: >-
  macOS-only code cannot be checked by building the app for Apple from Linux —
  `ring` needs an Apple C compiler — but a scratch crate with the same objc2
  versions type-checks it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-10T12:13:42.018Z'
updated_at: '2026-11-10T12:13:42.018Z'
last_verified: '2026-11-10T12:13:42.018Z'
file_paths:
  - crates/app/src/chrome/traffic_lights.rs
  - crates/app/Cargo.toml
symbols: []
content_hashes:
  crates/app/Cargo.toml: 'sha256:7b0e3651494ad06f190b3b78f2e79d3b5a91107f13a0a12f564fce06f76bfb87'
tags:
  - macos
  - cross-compile
  - objc2
  - method
feature_id: feat_term-core_nx0g1r
---

## What it does

**`cargo check -p app --target aarch64-apple-darwin` fails on Linux before it reaches our code**: `ring` (pulled by the update checker's HTTPS) runs a C build script, and the host `cc` rejects `-arch` / `-mmacosx-version-min`. This machine has no clang either. Measured on #34, November 2026.

**What does work, in a few seconds:** `rustup target add aarch64-apple-darwin`, then a throwaway crate in the scratchpad whose only dependencies are the ones the macOS module uses, pinned to the lockfile's versions (`objc2-app-kit =0.3.2`, `objc2-foundation =0.3.2`, `raw-window-handle =0.6.2`), with the module included as `pub mod x;` (not `include!`, which rejects a file's `//!` docs). `cargo clippy --offline --target aarch64-apple-darwin -- -D warnings` then type-checks every AppKit call and every objc2 feature flag; the only expected errors are dead-code, because nothing in the scratch crate calls the module. objc2's crates need no C, so they build for Apple from Linux.

**What it does not prove:** anything AppKit does at runtime. And **keep the geometry out of the `cfg(target_os = "macos")` code** so Linux tests can hold it — `traffic_lights.rs` splits `container` / `button_y` (pure, tested, mutated) from `center` (AppKit). Likewise `chrome_viewport` takes the platform as a parameter so a Linux test sees the macOS branch.

## Where it lives

Method, used for `crates/app/src/chrome/traffic_lights.rs`; the macOS-only dependencies are under `[target.'cfg(target_os = "macos")'.dependencies]` in `crates/app/Cargo.toml`.
