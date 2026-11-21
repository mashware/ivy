---
id: asp_mesa-llvmpipe-exits-on-hosted-windows-runners_he0rs2
slug: mesa-llvmpipe-exits-on-hosted-windows-runners
name: >-
  On a hosted Windows runner Mesa llvmpipe kills the app on its first
  SwapBuffers with 0x80070057 and no message: draw the launch trials with
  softpipe
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T18:35:16.767Z'
updated_at: '2026-11-21T18:35:16.767Z'
last_verified: '2026-11-21T18:35:16.767Z'
file_paths:
  - packaging/windows/test-install.ps1
  - .github/workflows/ivy-prerelease.yml
symbols: []
content_hashes:
  packaging/windows/test-install.ps1: 'sha256:d0f95651745ec9edac7b76a07e3cf642a916cc512b7d68a47c38f9717000db7d'
  .github/workflows/ivy-prerelease.yml: 'sha256:10bc02cffee7a35a00e562e2fb4fd6625b91eaf0a655c0c3efb3b6586e39554b'
tags:
  - windows
  - ci
  - opengl
  - mesa
feature_id: feat_distribution_kb0x1w
---

## What it does

Measured on 21 November 2026 in the installer job of #162 (PR #183), with Mesa 25.3.6 from
pal1000/mesa-dist-win copied beside the installed Ivy.exe so egui_glow gets OpenGL on a runner
with no GPU.

**llvmpipe ends the whole process about a second after the window opens.** The exit code is
0x80070057 (E_INVALIDARG), standard error is empty, and the console and its shell have already
started. Under cdb the stack is ntdll!NtTerminateProcess <- KERNEL32!ExitProcess <- libgallium_wgl
(llvmpipe JIT code) <- st_context_flush <- DrvSwapBuffers <- gdi32full!SwapBuffers <- app. It
happens with the bundled and with the system ConPTY alike, so the console host is not the cause.

**Fix: GALLIUM_DRIVER=softpipe**, which draws the same OpenGL without a JIT. It is set in
test-install.ps1 only when the trials borrow Mesa. With it the installed app opens, runs typed
input, and exits 0.

**How to find the module behind an HRESULT exit code with no message.** Rust's own error paths
exit 1 or 101, so an HRESULT points below Rust. Run the exe once under the runner's cdb
(C:\\Program Files (x86)\\Windows Kits\\10\\Debuggers\\x64\\cdb.exe, flags -G -cf <script>) with
`sxn *` and `bp ntdll!RtlExitUserProcess "kb 60; g"`; the stack names the module. Before that,
bisect the console host: Ivy falls back to the system ConPTY when no OpenConsole.exe sits beside it.

## Where it lives

packaging/windows/test-install.ps1 (the borrowed Mesa and GALLIUM_DRIVER); CI run 44924706567 holds the cdb stack, 44930708731 the green launch.
