param(
    [Parameter(Mandatory = $true)]
    [string]$Repo
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if ($Repo -notmatch '^(?:[A-Za-z]:[\\/]|\\\\[^\\]+\\[^\\]+(?:\\|$))') {
    throw 'Repo must be an absolute checkout path'
}
$Repo = (Resolve-Path -LiteralPath $Repo).Path
$ToolchainFile = Join-Path $Repo 'rust-toolchain.toml'
$ToolchainMatch = [regex]::Match([IO.File]::ReadAllText($ToolchainFile), '(?m)^\s*channel\s*=\s*"([^"]+)"')
if (-not $ToolchainMatch.Success) { throw 'Pinned Rust toolchain not found' }
$Toolchain = $ToolchainMatch.Groups[1].Value
$Harness = Join-Path ([IO.Path]::GetTempPath()) ('ivy-owner-negative-' + [guid]::NewGuid().ToString('N'))
$Utf8 = [Text.UTF8Encoding]::new($false)

try {
    New-Item -ItemType Directory -Path (Join-Path $Harness 'src') | Out-Null
    $Dependency = (Join-Path $Repo 'crates/panel-contract').Replace('\', '/')
    $DependencyLiteral = ConvertTo-Json -InputObject $Dependency -Compress
    $Manifest = Join-Path $Harness 'Cargo.toml'
    [IO.File]::WriteAllText($Manifest, @"
[package]
name = "ivy-owner-negative-native"
version = "0.0.0"
edition = "2024"
[dependencies]
panel-contract = { path = $DependencyLiteral }
windows-sys = { version = "0.61.2", features = ["Win32_Foundation", "Win32_Security", "Win32_Security_Authorization"] }
"@, $Utf8)
    [IO.File]::WriteAllText((Join-Path $Harness 'src/main.rs'), @'
use std::{io, path::PathBuf};
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Security::OWNER_SECURITY_INFORMATION;
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSidToSidW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
fn main() {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().unwrap();
    let path = PathBuf::from(args.next().unwrap());
    if mode == "create" {
        use std::io::Write;
        let mut file = panel_contract::windows_private_file::create_new(&path).unwrap();
        file.write_all(b"owner-negative-sentinel").unwrap();
        file.sync_all().unwrap();
    } else if mode == "owner" {
        let sid_text: Vec<u16> = "S-1-5-32-544".encode_utf16().chain(Some(0)).collect();
        let mut sid = null_mut();
        assert_ne!(unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) }, 0,
            "{}", io::Error::last_os_error());
        let mut name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let status = unsafe {
            SetNamedSecurityInfoW(name.as_mut_ptr(), SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION, sid, null_mut(), null_mut(), null_mut())
        };
        unsafe { LocalFree(sid); }
        assert_eq!(status, ERROR_SUCCESS, "{}", io::Error::from_raw_os_error(status as i32));
    } else {
        let error = panel_contract::windows_private_file::open(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(std::fs::read(&path).unwrap(), b"owner-negative-sentinel");
        println!("PASS private opener refused readable foreign-owner file");
    }
}
'@, $Utf8)
    $Target = Join-Path $Harness 'target'
    & cargo "+$Toolchain" build --offline --manifest-path $Manifest --target-dir $Target
    if ($LASTEXITCODE -ne 0) { throw 'Harness build failed' }
    $Exe = Join-Path $Target 'debug/ivy-owner-negative-native.exe'
    $File = Join-Path $Harness 'capability'
    & $Exe create $File
    if ($LASTEXITCODE -ne 0) { throw 'Fixture creation failed' }
    $Before = Get-Acl -LiteralPath $File
    $User = [Security.Principal.WindowsIdentity]::GetCurrent().User
    if ($Before.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $User.Value) {
        throw 'Unexpected initial owner'
    }
    if (-not $Before.AreAccessRulesProtected) { throw 'Initial DACL is not protected' }
    $Rules = @($Before.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]))
    $ExpectedSids = @($User.Value, 'S-1-5-18') | Select-Object -Unique
    if ($Rules.Count -ne @($ExpectedSids).Count) { throw 'Unexpected initial ACE count' }
    foreach ($Rule in $Rules) {
        if ($Rule.IdentityReference.Value -notin $ExpectedSids -or
            $Rule.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow -or
            $Rule.FileSystemRights -ne [Security.AccessControl.FileSystemRights]::FullControl -or
            $Rule.InheritanceFlags -ne [Security.AccessControl.InheritanceFlags]::None -or
            $Rule.PropagationFlags -ne [Security.AccessControl.PropagationFlags]::None -or
            $Rule.IsInherited) {
            throw 'Initial DACL is not exact private user/SYSTEM access'
        }
    }
    $ActualSids = @($Rules | ForEach-Object { $_.IdentityReference.Value } | Sort-Object -Unique)
    if (@(Compare-Object @($ExpectedSids | Sort-Object) $ActualSids).Count -ne 0) {
        throw 'Initial DACL principals are not user/SYSTEM'
    }
    $Dacl = $Before.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access)
    $Foreign = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
    if ($Foreign.Value -eq $User.Value) { throw 'Foreign SID equals caller' }
    $SavedErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $OwnerOutput = & $Exe owner $File 2>&1
        $OwnerStatus = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $SavedErrorAction
    }
    $OwnerOutput | ForEach-Object { Write-Host $_ }
    if ($OwnerStatus -ne 0) { throw 'Owner assignment failed' }
    $Changed = Get-Acl -LiteralPath $File
    if ($Changed.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $Foreign.Value) {
        throw 'Owner change not established'
    }
    $ChangedDacl = $Changed.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access)
    if (-not $Changed.AreAccessRulesProtected -or $ChangedDacl -ne $Dacl) {
        throw "DACL changed with owner: expected=$Dacl actual=$ChangedDacl protected=$($Changed.AreAccessRulesProtected)"
    }
    $SavedErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $RejectOutput = & $Exe reject $File 2>&1
        $RejectStatus = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $SavedErrorAction
    }
    $RejectOutput | ForEach-Object { Write-Host $_ }
    $Rejected = @($RejectOutput | ForEach-Object { "$_" }) -contains 'PASS private opener refused readable foreign-owner file'
    if ($RejectStatus -ne 0 -or -not $Rejected) { throw 'Foreign-owner refusal failed' }
    $After = Get-Acl -LiteralPath $File
    if ($After.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $Foreign.Value) {
        throw 'Opener repaired owner'
    }
    $AfterDacl = $After.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access)
    if (-not $After.AreAccessRulesProtected -or $AfterDacl -ne $Dacl) {
        throw "Opener changed DACL: expected=$Dacl actual=$AfterDacl protected=$($After.AreAccessRulesProtected)"
    }
} finally {
    if (Test-Path -LiteralPath $Harness) {
        Remove-Item -LiteralPath $Harness -Recurse -Force
    }
}

Write-Output 'PASS foreign-owner route file rejected'
