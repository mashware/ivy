---
id: asp_the-desktop-opener-and-its-traps_bjwp53
slug: the-desktop-opener-and-its-traps
name: >-
  The desktop URL opener: egui's `links` swallows its error, `webbrowser`
  returns Ok on spawn, runs on the caller's thread, and under WSL hands the
  address to cmd.exe
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T19:51:41.363Z'
updated_at: '2026-09-22T19:51:41.363Z'
last_verified: '2026-09-22T19:51:41.363Z'
file_paths:
  - crates/app/src/links.rs
  - crates/app/Cargo.toml
  - Cargo.toml
  - docs/02-decisions.md
symbols:
  - WebTarget
  - Desktop
  - safe_for_the_opener
content_hashes:
  crates/app/Cargo.toml: 'sha256:5bf10ac6e13903ae27496c807266875a38d5bcd2ebf15220c80021be08345660'
  Cargo.toml: 'sha256:51bcda70fa5f02b270cb6883d6d1d1875b50b25c48da19d04b278a2b300cbd61'
  docs/02-decisions.md: 'sha256:ecc41dc2084fbb08906e9695aecb329047cf089cc71722a9e293a06f8a26b2b9'
tags:
  - links
  - webbrowser
  - egui
  - wsl
  - security
  - D51
feature_id: feat_term-core_nx0g1r
---

## What it does

All read in source (egui-winit 0.36.1, webbrowser 1.2.4) while making links openable (#257, D51):

1. **egui's `links` feature handles `OutputCommand::OpenUrl` by calling `webbrowser::open` synchronously on the UI thread, and reports a failure only with `log::warn!`** (`egui-winit/src/lib.rs:1397`). This binary installs no `log` backend, so a person with no configured browser clicked and nothing happened, anywhere. The app therefore turns `links` off and calls `webbrowser` itself, on a thread per click, printing one stderr line when it errs.
2. **`webbrowser::open` returns `Ok` as soon as a process spawns** (`common.rs` `run_command`): it cannot report a browser that started and then failed (locked profile, a launcher exiting non-zero). It also drops the `Child` without waiting, leaving a zombie per click for the life of the process.
3. **On Linux it runs `xdg-settings get default-web-browser` with `.output()` on every call, then the desktop entry's `Exec`**, and waits with `.status()` for a text browser or a `Terminal=true` entry — any of which would stall a UI thread; hence the thread.
4. **Under WSL, `http(s)` goes to `cmd.exe /c start <url>` escaping only `^` and `&`** (`unix.rs:242`), falling back to `powershell.exe Start <url>`. The `url` crate leaves `|` and a `%` that is not an escape untouched, so untrusted link text could pipe into a command; the app percent-encodes `| " < >` and a lone `%`. Two residuals are knowingly left: two valid escapes can spell a `%NAME%` cmd expands, and PowerShell reads `;`, `$` and more.
5. **The opener re-parses the address and hands the browser `Url::as_str()`** (host lowercased, punycode, percent-encoding), so the tooltip must show that parsed form — keeping the producer's spelling shows one address and loads another (the homograph case).

On macOS it goes through AppKit `NSWorkspace`; on Windows it runs the registered `http` handler's command line with the address substituted.

## Where it lives

`crates/app/src/links.rs` (`WebTarget`, `safe_for_the_opener`, `Desktop`), `crates/app/Cargo.toml` (`webbrowser` comment), root `Cargo.toml` (eframe features), D51.
