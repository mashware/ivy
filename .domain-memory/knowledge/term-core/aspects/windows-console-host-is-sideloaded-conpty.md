---
id: asp_windows-console-host-is-sideloaded-conpty_aek06n
slug: windows-console-host-is-sideloaded-conpty
name: >-
  On Windows the console host is whatever conpty.dll LoadLibrary finds by name:
  ship Microsoft's pair beside the exe, pin each file, and answer the cursor
  question (D112)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T23:30:28.527Z'
updated_at: '2026-11-20T23:30:28.527Z'
last_verified: '2026-11-20T23:30:28.527Z'
file_paths:
  - scripts/conpty.py
  - packaging/windows/conpty.json
  - crates/term-core/src/lib.rs
symbols:
  - windows_conpty_osc7_reaches_the_screen_through_the_bundled_host
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:f8d5a82b78d2ff9bcd1495c511f420d2bfd7ef9fbb45e0c3f5a12ecca71404b0'
  scripts/conpty.py: 'sha256:86588ad4235da81ea48268214791e95b529ae544376cc4f06c021c150b9985d2'
  packaging/windows/conpty.json: 'sha256:f1dca3967b04e168cd865b88ba4668654567bc6269a63a545a61fff80b071801'
tags:
  - windows
  - conpty
  - portable-pty
  - packaging
feature_id: feat_term-core_nx0g1r
---

## What it does

Read in portable-pty 0.9.0 and in conpty.dll 1.25.260930003 on 20 November 2026 (#169).
The conpty.dll part is a disassembly by a review pass, not run on Windows.

**1. portable-pty picks the host at first use, by bare name.** It opens kernel32's
`CreatePseudoConsole`, then `LoadLibrary("conpty.dll")` and prefers it when found
(`src/win/psuedocon.rs:45-59`). A bare-name load searches the application directory, then
the system directories, then the current directory and PATH. With no `conpty.dll` beside
the exe, one planted in the folder Ivy starts from or on PATH is what gets loaded.

**2. The sideloaded conpty.dll is reported to start `OpenConsole.exe` beside itself first,
then `<arch>\OpenConsole.exe`** (x86, x64, arm64: three-character UTF-16 strings that
`strings` hides), and to fall back to the system `conhost` only when none is found. A found
`OpenConsole.exe` that fails to start (antivirus, WDAC) is reported to return an error with
no fallback: every console fails with portable-pty's 'failed to create psuedo console'.

**3. portable-pty hard-codes the flags** (`INHERIT_CURSOR | RESIZE_QUIRK | WIN32_INPUT_MODE`).
No text-width rule is passed (`flags & 0x18` would pick graphemes/wcswidth), and
`PSEUDOCONSOLE_PASSTHROUGH_MODE` is defined but never used.

**4. With INHERIT_CURSOR the host sends `ESC[6n` and holds the program until a reply.**
A test that feeds output with `Screen::advance` and never writes back times out and
blames the host. Use `advance_with_replies` with `Pty::write`, from the first byte.

**5. Pin the extracted files, not the .nupkg.** Every NuGet package carries a repository
signature (`.signature.p7s` dated after the payload); a re-sign changes the archive and not
the binaries.

**6. The inbox host re-renders from its buffer** and is reported to swallow OSC 10/11;
whether OSC 7 survives it was never measured, which is why D112 ships the pair.

## Where it lives

packaging/windows/conpty.json pins the pair; scripts/conpty.py fetches and verifies it;
scripts/package-windows.py stages it flat beside Ivy.exe; the PR Windows job runs the term-core
test from a folder holding only its binary and the pair. Decision D112 in docs/02-decisions.md.
