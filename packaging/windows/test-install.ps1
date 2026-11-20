param([Parameter(Mandatory=$true)][string]$Installer,
      [Parameter(Mandatory=$true)][string]$BinaryDir)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:IVY_DISPOSABLE_RUNNER -ne 'yes') {
    throw 'Installation checks require a disposable GitHub Windows runner.'
}
$Installer = (Resolve-Path $Installer).Path
$BinaryDir = (Resolve-Path $BinaryDir).Path
$Root = Join-Path $env:RUNNER_TEMP ('ivy-install-' + [guid]::NewGuid())
$Destination = Join-Path $Root 'Ivy'
$env:XDG_CONFIG_HOME = Join-Path $Root 'preferences'
$Preferences = Join-Path $env:XDG_CONFIG_HOME 'ivy'
New-Item -ItemType Directory -Path $Preferences -Force | Out-Null
[IO.File]::WriteAllText((Join-Path $Preferences 'config'), "# preserve preferences`n")
[IO.File]::WriteAllText((Join-Path $Preferences 'layout.json'), '{"fixture":"preserve"}')
$Before = @{}
Get-ChildItem $Preferences -File | ForEach-Object { $Before[$_.Name] = (Get-FileHash $_.FullName).Hash }
function Install-Candidate {
    $Process = Start-Process $Installer -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DIR=`"$Destination`"") -Wait -PassThru
    if ($Process.ExitCode -ne 0) { throw "Installer exited $($Process.ExitCode)" }
    foreach ($Pair in @(@('app.exe', 'Ivy.exe'), @('panel-mcp.exe', 'panel-mcp.exe'))) {
        if ((Get-FileHash (Join-Path $BinaryDir $Pair[0])).Hash -ne (Get-FileHash (Join-Path $Destination $Pair[1])).Hash) {
            throw "Installed executable differs: $($Pair[1])"
        }
    }
    foreach ($Name in @('LICENSE', 'COPYRIGHT', 'THIRD-PARTY.md')) {
        if (!(Test-Path (Join-Path $Destination "licenses/$Name"))) { throw "Missing $Name" }
    }
    Get-ChildItem (Join-Path $BinaryDir 'notices') -Recurse -File | ForEach-Object {
        $Relative = [IO.Path]::GetRelativePath((Join-Path $BinaryDir 'notices'), $_.FullName)
        if ((Get-FileHash $_.FullName).Hash -ne (Get-FileHash (Join-Path $Destination "licenses/cargo/$Relative")).Hash) {
            throw "Installed notice differs: $Relative"
        }
    }
    $Shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Ivy.lnk'
    $Shell = New-Object -ComObject WScript.Shell
    if ($Shell.CreateShortcut($Shortcut).TargetPath -ne (Join-Path $Destination 'Ivy.exe')) { throw 'Invalid Start menu shortcut' }
}
Install-Candidate
$Launch = Join-Path $PSScriptRoot 'test-launch.ps1'
$Installed = Join-Path $Destination 'Ivy.exe'
$Evidence = Join-Path (Join-Path $env:GITHUB_WORKSPACE 'windows-launch-evidence') ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $Evidence -Force | Out-Null
foreach ($Control in 'crash', 'shell', 'layout', 'deadline') {
    $Rejected = $false
    try {
        & $Launch -Executable $Installed -EvidenceDir (Join-Path $Evidence $Control) -Control $Control -TimeoutSeconds 20
    } catch {
        $ExpectedStage = @{ crash = 'startup'; shell = 'shell'; layout = 'layout'; deadline = 'deadline' }[$Control]
        if ($_.Exception.Message -notmatch "^${ExpectedStage}:") { throw }
        Write-Output "Launch control '$Control' rejected as expected: $($_.Exception.Message)"
        $Rejected = $true
    }
    if (-not $Rejected) { throw "Launch control '$Control' incorrectly passed" }
}
& $Launch -Executable $Installed -EvidenceDir (Join-Path $Evidence 'normal')
# Repeat installation to exercise replacement without claiming an older-version upgrade trial.
Install-Candidate
$Process = Start-Process (Join-Path $Destination 'unins000.exe') -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
if ($Process.ExitCode -ne 0) { throw "Uninstaller exited $($Process.ExitCode)" }
if (Test-Path (Join-Path $Destination 'Ivy.exe')) { throw 'Uninstall retained application' }
if (Test-Path (Join-Path ([Environment]::GetFolderPath('Programs')) 'Ivy.lnk')) { throw 'Uninstall retained shortcut' }
foreach ($Name in $Before.Keys) {
    if ((Get-FileHash (Join-Path $Preferences $Name)).Hash -ne $Before[$Name]) { throw "User data changed: $Name" }
}
Write-Output 'Installed payload hashes, replacement, shortcut, uninstall and user-data retention passed.'
