---
id: asp_writing-the-windows-console-yourself_pgybk6
slug: writing-the-windows-console-yourself
name: >-
  Writing Ivy's Windows console yourself needs WriteConsoleW, and SetStdHandle
  works because Rust re-reads the handle on every write
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T18:32:32.349Z'
updated_at: '2026-11-21T18:32:32.349Z'
last_verified: '2026-11-21T18:32:32.349Z'
file_paths:
  - crates/app/src/diagnostics_log/swap.rs
symbols:
  - Console
  - whole_characters
  - swap
content_hashes: {}
tags:
  - windows
  - stderr
  - console
feature_id: feat_term-core_nx0g1r
---

## What it does

Ivy has no `windows_subsystem` attribute, so on Windows it is a console program and its
standard error is usually a console handle. Code that writes that handle itself, instead of
through `std::io::stderr`, must convert to UTF-16 and use `WriteConsoleW`, as Rust's own
Stderr does: raw UTF-8 through `WriteFile` is read in the console's OEM code page and
garbles any non-ASCII path. Keep an incomplete character for the next write without
turning the rest into U+FFFD (`whole_characters` in swap.rs, tested on every platform).

Rust's Windows stdio fetches `GetStdHandle` on every write
(library/std/src/sys/stdio/windows.rs: "Don't cache handles but get them fresh for every
read/write"), which is what makes `SetStdHandle` redirect `eprintln!`; a null handle
discards the write as Ok. On Unix the runtime reopens a closed fd 0-2 on /dev/null before
`main`, so a closed fd 2 never reaches app code.

Evidence: #139 review round 1 (built-in) and a read of rust-lang/rust main; the console
writer type-checked for x86_64-pc-windows-msvc in an isolated crate, not run on Windows.

## Where it lives

crates/app/src/diagnostics_log/swap.rs (platform::original, Console, whole_characters)
