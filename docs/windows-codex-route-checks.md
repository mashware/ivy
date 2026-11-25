# Windows Codex route verification

The native PR job exercises private route storage, shared readers, locks, cancellation and rollback.
It does not establish shared mobile/panel relay support; that relay remains disabled on Windows.

Run the native registry and file suites, then the mutation driver from the checkout:

```text
cargo test -p panel-contract --lib windows_private_file::tests::
cargo test -p panel-control --lib codex_route::tests::
cargo test -p panel-control --test codex_route_windows_sharing
cargo test -p panel-control --test codex_route_windows_privacy
cargo test -p panel-control --test ivy_codex_launch
python scripts/windows-codex-route-mutations.py
```

The driver requires each named baseline to pass and each mutant to fail. Its logs are uploaded
as `windows-codex-route-evidence`; cross-compilation is not native execution.

## Native wrong-owner check

The existing `windows-2025` job can exercise the owner guard using the Administrators SID as
the foreign owner: [GitHub-hosted Windows runners run as administrators with UAC disabled](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
No new account, password or privilege grant is needed. Ordinary local Windows tokens need not
permit this fixture; the native check fails loudly if owner assignment is unavailable.

The single source of this check is `packaging/windows/test-codex-route-owner.ps1`.
Run it in an elevated native PowerShell session, supplying the absolute checkout path:

```powershell
& 'C:\path\to\agent-terminal\packaging\windows\test-codex-route-owner.ps1' -Repo 'C:\path\to\agent-terminal'
```

The script builds a disposable harness with the repository's pinned Rust toolchain and cached
dependencies (`cargo build --offline`). It creates a file through the public private-file API,
independently checks its owner and exact protected user/SYSTEM DACL, changes its owner to
Administrators with [SetNamedSecurityInfoW](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfow)
using only `OWNER_SECURITY_INFORMATION` and null DACL arguments. It verifies that the public opener
rejects it with `PermissionDenied` while the sentinel remains readable. It also checks that the
opener changed neither owner nor DACL.
The temporary harness is cleaned in `finally`, including after a failed fixture or assertion.

Success prints exactly `PASS foreign-owner route file rejected`. If the owner guard accepts the
foreign-owned file, the script fails with `Foreign-owner refusal failed`. The native mutation
driver uses those markers to distinguish a detected owner-check regression from fixture or build
failure; removing only the owner equality refusal must turn this witness red.

Local Linux cross-compilation does not execute this fixture. Native baseline and mutation logs
are required to establish its Windows result.
