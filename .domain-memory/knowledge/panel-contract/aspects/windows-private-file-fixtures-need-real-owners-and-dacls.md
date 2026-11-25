---
id: asp_windows-private-file-fixtures-need-real-owners-and-dacls_yx72gz
slug: windows-private-file-fixtures-need-real-owners-and-dacls
name: >-
  Private Windows capability fixtures need an actual owner and DACL, and
  owner-only mutation must preserve that DACL
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-25T22:33:12.446Z'
updated_at: '2026-11-25T22:33:12.446Z'
last_verified: '2026-11-25T22:33:12.446Z'
file_paths:
  - crates/panel-contract/src/windows_private_file.rs
  - crates/panel-contract/src/windows_security.rs
  - packaging/windows/test-codex-route-owner.ps1
  - crates/panel-control/tests/codex_route_windows_privacy.rs
  - crates/panel-control/tests/codex_route_windows_sharing.rs
  - scripts/windows-codex-route-mutations.py
symbols:
  - file_symlink_with_private_acl_is_rejected
  - mutate_owner
  - >-
    every_created_registry_object_has_an_independently_verified_private_descriptor
content_hashes:
  crates/panel-contract/src/windows_private_file.rs: 'sha256:9951a42e2604727f2189e64e1c4e028b8ab8859c02c3d39b9530443d9dc3a27f'
  crates/panel-contract/src/windows_security.rs: 'sha256:f1db5f5d0635d9200d71fd12e8d0f488ebe46305369b6ae71a182e8e600ae158'
  packaging/windows/test-codex-route-owner.ps1: 'sha256:45b319078388d862abf314b68479452739e375a2578838499407bd67e0dbc356'
  crates/panel-control/tests/codex_route_windows_privacy.rs: 'sha256:7fe42e14f60c4936138736253f8d66a34aca81e0bcb8328cd7a3622ad8a61f31'
  crates/panel-control/tests/codex_route_windows_sharing.rs: 'sha256:52347060a0eef510cfa1894b6baebe852db5fdda35162f027ec4e00faf65af6a'
  scripts/windows-codex-route-mutations.py: 'sha256:3e87fe88be9a5382016415bea8a2e0ccf8392246089f632d19cda7c1623b3372'
tags:
  - windows
  - acl
  - owner
  - capability-files
  - mutation
  - d92
feature_id: feat_panel-contract_8y2gc0
---

## What it does

A private ACL is not proof of the owner. On the hosted Windows runner, a file symlink created through std had an owner different from TOKEN_USER even inside a private user-owned directory. Setting only its DACL left a reparse-refusal witness failing its privacy prerequisite before the reparse check ran. Establish and independently read back the owner as well as the protected user/SYSTEM DACL before exercising the intended refusal.

A foreign-owner witness needs a SID the token can assign as owner (an owner-eligible group or SeRestorePrivilege); an arbitrary foreign SID is not a portable fixture. The hosted administrator token supplied the Administrators group without creating accounts or granting privileges. Set-Acl on a fetched FileSecurity object changed the descriptor before the opener was tested. SetNamedSecurityInfoW with only OWNER_SECURITY_INFORMATION and null DACL arguments preserved the exact protected DACL. Independent Get-Acl checks bracket the actual opener call; the ordinary read must still return the sentinel, and the opener must not repair either owner or DACL. Removing only owner equality then makes the real opener accept the file and breaks the witness.

Do not treat the absence of FILE_SHARE_DELETE as a universal replacement-failure oracle. Rust 1.97.1 first tries MoveFileExW and has an ACCESS_DENIED-only POSIX rename fallback. Require either atomic replacement with old readers retaining their bytes, or an error with exact prior registry state and temporary-file cleanup; retry after releasing the external reader. Copy-in-place mutations must fail the held-reader witness.

These are runtime capability files: D92 rejects reparse-backed storage. Do not copy the document viewer's filter-hydration policy from [[opening-without-following-links-on-windows]] into this boundary.

Evidence: PR #256, native Windows run45696334411. Independent descriptor inspection covers the root and all five registry file kinds; all12 semantic mutation pairs were detected. [Win32 owner API](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfow) and pinned Rust std filesystem source establish the fixture constraints; native baseline/mutant logs establish the effects.

## Where it lives

crates/panel-contract/src/windows_private_file.rs and windows_security.rs hold the creation and refusal policy. packaging/windows/test-codex-route-owner.ps1 changes only the owner and independently checks the descriptor. crates/panel-control/tests/codex_route_windows_privacy.rs and codex_route_windows_sharing.rs inspect actual descriptors and external readers; scripts/windows-codex-route-mutations.py proves the guards can fail. docs/windows-codex-route-checks.md gives the native commands.
