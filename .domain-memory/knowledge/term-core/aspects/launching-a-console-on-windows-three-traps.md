---
id: asp_launching-a-console-on-windows-three-traps_r0yz3e
slug: launching-a-console-on-windows-three-traps
name: >-
  Launching a console on Windows: a bare program name is searched again, a Git
  Bash SHELL is not a Windows path, and ConPTY writes before the program does
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T16:47:44.346Z'
updated_at: '2026-11-20T16:47:44.346Z'
last_verified: '2026-11-20T16:47:44.346Z'
file_paths:
  - crates/app/src/shell_integration.rs
  - crates/app/src/launch.rs
symbols:
  - the_users_shell
  - fallback_shell
  - windows_shell
  - windows_cmd
content_hashes:
  crates/app/src/shell_integration.rs: 'sha256:f88a46ad39aff725f3ff173d4a45dffb9f24543438576d6916fa279ec060d39c'
tags:
  - windows
  - conpty
  - portable-pty
  - testing
feature_id: feat_term-core_nx0g1r
---

## What it does

Found reviewing #161 (20 November 2026), the first fix that made Ivy start on
Windows (D107). Read in source, not measured on Windows.

**1. A bare program name is searched again by the launcher, current directory and
relative search-path entries included.** portable-pty's Windows `search_path`
(`cmdbuilder.rs:581-607`) accepts relative PATH entries and finally hands the bare
name to `CreateProcessW`, which resolves it against the current directory. So
returning `cmd.exe` as a fallback lets a repository that holds a `cmd.exe` run in
place of the system's. Pass an absolute path: `%ComSpec%` when absolute, else
`%SystemRoot%\System32\cmd.exe`. Searching only absolute directories in your own
lookup is not enough if the result you hand over is still a bare name.

**2. The shell variable on Windows is often a path in another namespace.** Git Bash,
MSYS2 and Cygwin export `SHELL=/usr/bin/bash`, which Windows cannot launch. Taking
it as given made every console fail into the fallback with an error line. On
Windows it counts only as an absolute existing Windows path.

**3. ConPTY writes its own sequences into the output pipe as soon as the
pseudoconsole exists, whatever the child does.** A test that asserts «some bytes
arrived» passes with a program that starts and exits at once: the D30 disguise on
Windows. Write a command whose result differs from its echo (cmd `echo IVY^_OK`,
PowerShell `Write-Output ('IVY' + '_OK')`) and wait for the result.

Also: no Windows type-check is possible from the Linux dev machine, because `ring`
needs the MSVC `lib.exe`; the pull-request Windows job is the only compiler.

## Where it lives

crates/app/src/shell_integration.rs (the_users_shell, fallback_shell, windows_shell,
windows_cmd); crates/app/src/launch.rs (the fallback retry)
