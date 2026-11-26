---
id: asp_powershell-module-path-crosses-an-indirect-launch_ayk95b
slug: powershell-module-path-crosses-an-indirect-launch
name: >-
  PowerShell 5 launched through Python inherits PowerShell 7 module paths and
  can fail to load its own cmdlets
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T22:33:12.644Z'
updated_at: '2026-11-25T22:33:12.644Z'
last_verified: '2026-11-25T22:33:12.644Z'
file_paths:
  - scripts/windows-codex-route-mutations.py
  - packaging/windows/test-codex-route-owner.ps1
symbols:
  - owner_case
content_hashes:
  scripts/windows-codex-route-mutations.py: 'sha256:3e87fe88be9a5382016415bea8a2e0ccf8392246089f632d19cda7c1623b3372'
  packaging/windows/test-codex-route-owner.ps1: 'sha256:45b319078388d862abf314b68479452739e375a2578838499407bd67e0dbc356'
tags:
  - windows
  - powershell
  - environment
  - native-fixtures
feature_id: feat_term-core_nx0g1r
---

## What it does

PowerShell 7 adjusts module paths when it starts Windows PowerShell directly. That adjustment does not cross an intermediate Python or cmd process: the child inherits the unmodified PSModulePath, can resolve Microsoft.PowerShell.Security to an incompatible PowerShell 7 module, and fails Get-Acl before the intended native security witness runs.

Remove PSModulePath case-insensitively only from the environment passed to powershell.exe. Windows PowerShell then constructs its own default module paths. Preserve the parent environment and unrelated child variables; changing the user's profile or switching the witness to a different shell would avoid the actual launch boundary instead of testing it.

Observed in the native owner baseline of #173: the Rust harness built successfully, then Get-Acl failed with CouldNotAutoloadMatchingModule. Child-only environment isolation let the owner baseline and actual owner-check mutation run. Final Windows run45696334411 passed. Microsoft documents this exact pwsh -> Python -> powershell.exe path and the child-environment remedy in [about_PSModulePath](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_psmodulepath?view=powershell-7.6). Related: [[launching-a-console-on-windows-three-traps]].

## Where it lives

scripts/windows-codex-route-mutations.py::owner_case constructs the Windows PowerShell child environment; packaging/windows/test-codex-route-owner.ps1 uses the native ACL cmdlets.
