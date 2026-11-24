param([Parameter(Mandatory=$true)][string]$Installer,
      [Parameter(Mandatory=$true)][string]$BinaryDir,
      [Parameter(Mandatory=$true)][string]$ConptyDir,
      # Hosted runners have no GPU and Windows' own OpenGL is 1.1, below what the painter needs, so
      # the launch trials put a software OpenGL beside the installed executable and take it away
      # before the replacement and uninstall checks.
      [string]$SoftwareOpenGL = '')
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:IVY_DISPOSABLE_RUNNER -ne 'yes') {
    throw 'Installation checks require a disposable GitHub Windows runner.'
}
$Installer = (Resolve-Path $Installer).Path
$BinaryDir = (Resolve-Path $BinaryDir).Path
$ConptyDir = (Resolve-Path $ConptyDir).Path
$Adapters = (Resolve-Path (Join-Path $PSScriptRoot '../../integrations/session-recovery')).Path
$ConsoleHost = @((Get-Content (Join-Path $PSScriptRoot 'conpty.json') -Raw | ConvertFrom-Json).files.PSObject.Properties.Name)
if ($ConsoleHost.Count -ne 2) { throw 'conpty.json does not name the console host pair' }
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
    foreach ($Pair in @(@('app.exe', 'Ivy.exe'), @('panel-mcp.exe', 'panel-mcp.exe'),
                        @('ivy-session.exe', 'ivy-session.exe'), @('ivy-codex.exe', 'ivy-codex.exe'))) {
        if ((Get-FileHash (Join-Path $BinaryDir $Pair[0])).Hash -ne (Get-FileHash (Join-Path $Destination $Pair[1])).Hash) {
            throw "Installed executable differs: $($Pair[1])"
        }
    }
    $InstalledAdapters = Join-Path $Destination 'session-recovery'
    $SourceFiles = @(Get-ChildItem $Adapters -Recurse -File | ForEach-Object {
        [IO.Path]::GetRelativePath($Adapters, $_.FullName)
    })
    $InstalledFiles = @(Get-ChildItem $InstalledAdapters -Recurse -File | ForEach-Object {
        [IO.Path]::GetRelativePath($InstalledAdapters, $_.FullName)
    })
    if (Compare-Object $SourceFiles $InstalledFiles) { throw 'Installed recovery adapter tree differs' }
    foreach ($Relative in $SourceFiles) {
        if ((Get-FileHash (Join-Path $Adapters $Relative)).Hash -ne
            (Get-FileHash (Join-Path $InstalledAdapters $Relative)).Hash) {
            throw "Installed recovery adapter differs: $Relative"
        }
    }
    # Microsoft's console host travels flat beside Ivy.exe (D112).
    foreach ($Name in $ConsoleHost) {
        if ((Get-FileHash (Join-Path $ConptyDir $Name)).Hash -ne (Get-FileHash (Join-Path $Destination $Name)).Hash) {
            throw "Installed console host file differs: $Name"
        }
    }
    foreach ($Name in @('LICENSE', 'COPYRIGHT', 'THIRD-PARTY.md', 'conpty/LICENSE')) {
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
$Borrowed = @()
if ($SoftwareOpenGL) {
    foreach ($Name in 'opengl32.dll', 'libgallium_wgl.dll', 'libglapi.dll') {
        $Source = Join-Path $SoftwareOpenGL $Name
        if (Test-Path $Source -PathType Leaf) {
            if (Test-Path (Join-Path $Destination $Name)) { throw "Installed payload already has $Name" }
            Copy-Item $Source $Destination
            $Borrowed += Join-Path $Destination $Name
        } elseif ($Name -ne 'libglapi.dll') {
            throw "Software OpenGL is missing $Name"
        }
    }
    # Mesa's default llvmpipe ends the whole process with 0x80070057 from inside its JIT on the first
    # SwapBuffers on hosted runners — traced under cdb to ExitProcess called from libgallium_wgl.
    # softpipe draws the same OpenGL without a JIT. Launched processes inherit it; it is never set
    # without the borrowed OpenGL.
    $env:GALLIUM_DRIVER = 'softpipe'
}
# The ordinary launch goes first: a control that fails for an unexpected reason would otherwise stop
# the run before anyone learns whether Ivy starts at all.
& $Launch -Executable $Installed -EvidenceDir (Join-Path $Evidence 'normal')
# Check the installed PE, and start it from a detached parent without changing its own flags.
$Version = (python (Join-Path $env:GITHUB_WORKSPACE 'scripts/release.py') version).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not read workspace version' }
$Info = [Diagnostics.FileVersionInfo]::GetVersionInfo($Installed)
if ($Info.ProductName -ne 'Ivy' -or $Info.FileDescription -ne 'Ivy' -or
    $Info.FileVersion -ne $Version -or $Info.ProductVersion -ne $Version) {
    throw "Installed version resources differ from Ivy $Version"
}
python (Join-Path $PSScriptRoot 'gui-check.py') $Installed
if ($LASTEXITCODE -ne 0) { throw 'Installed GUI witness failed' }

# A separate trial keeps the established startup deadline unchanged. Each publication has an ack;
# screenshots are sampled after acknowledgements and require review of the expected panel captions.
& $Launch -Executable $Installed -EvidenceDir (Join-Path $Evidence 'provenance') -TimeoutSeconds 90 -ProvenancePublisher (Join-Path $PSScriptRoot 'publish-provenance-witness.ps1')
foreach ($Control in 'crash', 'shell', 'layout', 'deadline') {
    $Rejected = $false
    try {
        & $Launch -Executable $Installed -EvidenceDir (Join-Path $Evidence $Control) -Control $Control -TimeoutSeconds 20
    } catch {
        if ($Control -eq 'shell') {
            # Ivy may give up before or after its window appears; either way the refusal has to be
            # the shell's own: the missing interpreter the trial planted, named on standard error.
            if ($_.Exception.Message -notmatch '^(startup|shell):') { throw }
            $Said = Get-Content (Join-Path (Join-Path $Evidence $Control) 'stderr.txt') -Raw -ErrorAction SilentlyContinue
            if ($Said -notmatch 'missing-cmd\.exe') { throw "Launch control 'shell' failed for another reason: $($_.Exception.Message)" }
        } else {
            $ExpectedStage = @{ crash = 'startup'; layout = 'layout'; deadline = 'deadline' }[$Control]
            if ($_.Exception.Message -notmatch "^${ExpectedStage}:") { throw }
        }
        Write-Output "Launch control '$Control' rejected as expected: $($_.Exception.Message)"
        $Rejected = $true
    }
    if (-not $Rejected) { throw "Launch control '$Control' incorrectly passed" }
}
foreach ($File in $Borrowed) { Remove-Item $File }
if ($SoftwareOpenGL) {
    # The same installed binary must explain the original no-driver failure once the borrowed
    # renderer is removed. The separate profile preserves the installer's user-data witnesses.
    & (Join-Path $PSScriptRoot 'test-graphics-failure.ps1') -Executable $Installed -EvidenceDir (Join-Path $Evidence 'graphics-failure')
}
# Repeat installation to exercise replacement without claiming an older-version upgrade trial.
Install-Candidate
$Process = Start-Process (Join-Path $Destination 'unins000.exe') -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
if ($Process.ExitCode -ne 0) { throw "Uninstaller exited $($Process.ExitCode)" }
foreach ($Name in @('Ivy.exe', 'panel-mcp.exe', 'ivy-session.exe', 'ivy-codex.exe', 'session-recovery') + $ConsoleHost) {
    if (Test-Path (Join-Path $Destination $Name)) { throw "Uninstall retained $Name" }
}
if (Test-Path (Join-Path ([Environment]::GetFolderPath('Programs')) 'Ivy.lnk')) { throw 'Uninstall retained shortcut' }
foreach ($Name in $Before.Keys) {
    if ((Get-FileHash (Join-Path $Preferences $Name)).Hash -ne $Before[$Name]) { throw "User data changed: $Name" }
}
Write-Output 'Installed payload hashes, replacement, shortcut, uninstall and user-data retention passed.'
