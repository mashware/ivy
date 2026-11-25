# Windows Codex route verification

The native PR job exercises private route storage, shared readers, locks, cancellation and rollback.
It does not establish shared mobile/panel relay support; that relay remains disabled on Windows.

Run the native registry and file suites, then the mutation driver from the checkout:

```text
cargo test -p panel-contract --lib windows_private_file::tests::
cargo test -p panel-control --lib codex_route::tests::
cargo test -p panel-control --test codex_route_windows_sharing
cargo test -p panel-control --test ivy_codex_launch
python scripts/windows-codex-route-mutations.py
```

The driver requires each named baseline to pass and each mutant to fail. Its logs are uploaded
as `windows-codex-route-evidence`; cross-compilation is not native execution.

## Privileged wrong-owner check


Prerequisite: native Windows NTFS and an elevated administrator token whose Administrators SID is eligible as an owner. This is a separate privileged manual witness, not a proposed ordinary-user CI test. If `Set-Acl` fails, record **fixture unavailable**, never pass or skip it as proof. No new user account or password is needed.

Run the following in elevated PowerShell, assigning the actual native checkout path to `$Repo`. The standalone temporary harness invokes the public file API; PowerShell independently observes owner and DACL before and after. No repository file is changed.

```powershell
$ErrorActionPreference = 'Stop'
$Repo = 'C:\path\to\agent-terminal'
$Harness = Join-Path $env:TEMP ('ivy-owner-negative-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $Harness 'src') | Out-Null
$Dependency = (Join-Path $Repo 'crates/panel-contract').Replace('\', '/')
@"
[package]
name = "ivy-owner-negative-manual"
version = "0.0.0"
edition = "2024"
[dependencies]
panel-contract = { path = "$Dependency" }
"@ | Set-Content -Encoding utf8 (Join-Path $Harness 'Cargo.toml')
@'
use std::{io, path::PathBuf};
fn main() {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().unwrap();
    let path = PathBuf::from(args.next().unwrap());
    if mode == "create" {
        use std::io::Write;
        let mut file = panel_contract::windows_private_file::create_new(&path).unwrap();
        file.write_all(b"owner-negative-sentinel").unwrap();
        file.sync_all().unwrap();
    } else {
        let error = panel_contract::windows_private_file::open(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(std::fs::read(&path).unwrap(), b"owner-negative-sentinel");
    }
}
'@ | Set-Content -Encoding utf8 (Join-Path $Harness 'src/main.rs')
cargo build --manifest-path (Join-Path $Harness 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Harness build failed' }
$Exe = Join-Path $Harness 'target/debug/ivy-owner-negative-manual.exe'
$File = Join-Path $Harness 'capability'
& $Exe create $File
if ($LASTEXITCODE -ne 0) { throw 'Fixture creation failed' }
$Before = Get-Acl -LiteralPath $File
$User = [Security.Principal.WindowsIdentity]::GetCurrent().User
if ($Before.GetOwner([Security.Principal.SecurityIdentifier]) -ne $User) { throw 'Unexpected initial owner' }
$Dacl = $Before.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access)
$Foreign = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
if ($Foreign -eq $User) { throw 'Foreign SID equals caller' }
$Before.SetOwner($Foreign)
Set-Acl -LiteralPath $File -AclObject $Before
$Changed = Get-Acl -LiteralPath $File
if ($Changed.GetOwner([Security.Principal.SecurityIdentifier]) -ne $Foreign) { throw 'Owner change not established' }
if ($Changed.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access) -ne $Dacl) { throw 'DACL changed with owner' }
& $Exe reject $File
if ($LASTEXITCODE -ne 0) { throw 'Foreign-owner refusal failed' }
$After = Get-Acl -LiteralPath $File
if ($After.GetOwner([Security.Principal.SecurityIdentifier]) -ne $Foreign) { throw 'Opener repaired owner' }
if ($After.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access) -ne $Dacl) { throw 'Opener changed DACL' }
Remove-Item -LiteralPath $Harness -Recurse -Force
```

To mutate this manual witness, temporarily remove only the `EqualSid(owner, user.User.Sid)` refusal in `windows_security::validate_acl`, rebuild the standalone harness, repeat the fixture, and require the `reject` invocation to fail because `unwrap_err()` receives a successful handle. Restore source immediately afterward. This mutation/manual harness has **not run** locally.

