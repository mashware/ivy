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
    foreach ($Pair in @(@('app.exe', 'Ivy.exe'), @('panel-mcp.exe', 'panel-mcp.exe'))) {
        if ((Get-FileHash (Join-Path $BinaryDir $Pair[0])).Hash -ne (Get-FileHash (Join-Path $Destination $Pair[1])).Hash) {
            throw "Installed executable differs: $($Pair[1])"
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
}
# DIAGNOSTIC, removed before merge: the installed app exits with 0x80070057 right after its shell
# starts, with the bundled and the system ConPTY alike. One launch under the runner's debugger prints
# every first-chance exception and the stack at each process-exit call, naming the module that ends
# it. It never fails the job.
$Debugger = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Debuggers\x64\cdb.exe'
$Trace = Join-Path $Evidence 'diagnostic-cdb'
New-Item -ItemType Directory $Trace | Out-Null
if (Test-Path $Debugger -PathType Leaf) {
    $Script = Join-Path $Trace 'commands.txt'
    @(
        'sxn *'
        'bp ntdll!RtlExitUserProcess "kb 60; g"'
        'bp ntdll!NtTerminateProcess "kb 60; g"'
        'bp KERNELBASE!TerminateProcess "kb 60; g"'
        'g'
    ) | Set-Content $Script
    $Log = Join-Path $Trace 'cdb.log'
    $Profile = Join-Path $Trace 'roaming'
    New-Item -ItemType Directory $Profile | Out-Null
    $Start = [Diagnostics.ProcessStartInfo]::new($Debugger)
    foreach ($Argument in '-G', '-y', 'srv*C:\symbols*https://msdl.microsoft.com/download/symbols', '-logo', $Log, '-cf', $Script, $Installed) {
        $Start.ArgumentList.Add($Argument)
    }
    $Start.UseShellExecute = $false
    $Start.WorkingDirectory = $Trace
    foreach ($Name in @($Start.Environment.Keys)) {
        if ($Name -match '^(XDG_|MEASURE_|IVY_)' -or $Name -in @('SHELL', 'PANEL_ID', 'PANEL_DIR')) { [void]$Start.Environment.Remove($Name) }
    }
    $Start.Environment['APPDATA'] = $Profile
    $Session = [Diagnostics.Process]::Start($Start)
    if (-not $Session.WaitForExit(120000)) {
        Write-Output 'DIAGNOSTIC cdb: Ivy still running after 120 s'
        $Session.Kill($true)
    }
    Write-Output '----- DIAGNOSTIC cdb log -----'
    Get-Content $Log -ErrorAction SilentlyContinue | Select-Object -Last 400
    Write-Output '----- end of DIAGNOSTIC cdb log -----'
} else {
    Write-Output "DIAGNOSTIC cdb: no debugger at $Debugger"
}
# The ordinary launch goes first: a control that fails for an unexpected reason would otherwise stop
# the run before anyone learns whether Ivy starts at all.
& $Launch -Executable $Installed -EvidenceDir (Join-Path $Evidence 'normal')
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
# Repeat installation to exercise replacement without claiming an older-version upgrade trial.
Install-Candidate
$Process = Start-Process (Join-Path $Destination 'unins000.exe') -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
if ($Process.ExitCode -ne 0) { throw "Uninstaller exited $($Process.ExitCode)" }
foreach ($Name in @('Ivy.exe') + $ConsoleHost) {
    if (Test-Path (Join-Path $Destination $Name)) { throw "Uninstall retained $Name" }
}
if (Test-Path (Join-Path ([Environment]::GetFolderPath('Programs')) 'Ivy.lnk')) { throw 'Uninstall retained shortcut' }
foreach ($Name in $Before.Keys) {
    if ((Get-FileHash (Join-Path $Preferences $Name)).Hash -ne $Before[$Name]) { throw "User data changed: $Name" }
}
Write-Output 'Installed payload hashes, replacement, shortcut, uninstall and user-data retention passed.'
