param([Parameter(Mandatory=$true)][string]$Executable,
      [Parameter(Mandatory=$true)][string]$EvidenceDir,
      [ValidateSet('none', 'crash', 'shell', 'layout', 'deadline')][string]$Control = 'none',
      [ValidateRange(1, 120)][int]$TimeoutSeconds = 30)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:IVY_DISPOSABLE_RUNNER -ne 'yes') {
    throw 'Launch checks require a disposable GitHub Windows runner.'
}
$Executable = (Resolve-Path $Executable).Path
$EvidenceDir = [IO.Path]::GetFullPath($EvidenceDir)
if (Test-Path $EvidenceDir) { throw 'Launch evidence directory must be fresh.' }
New-Item -ItemType Directory $EvidenceDir | Out-Null
$Profile = Join-Path $EvidenceDir 'roaming'
New-Item -ItemType Directory $Profile | Out-Null
$Layout = Join-Path $Profile 'ivy/layout.json'
# A fresh install restores nothing and therefore saves nothing (`startup.restore = none`), so the
# trial opts into the layout the way a person would, in the settings file beside it.
New-Item -ItemType Directory (Join-Path $Profile 'ivy') | Out-Null
Set-Content -Path (Join-Path $Profile 'ivy/config') -Value 'startup.restore = layout' -Encoding utf8NoBOM
$Witness = Join-Path $EvidenceDir 'shell-witness.txt'
$Token = [guid]::NewGuid().ToString('N')
if (-not ('IvyLaunchNative' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class IvyLaunchNative {
    public delegate bool EnumProc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr p);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll", SetLastError=true)] static extern uint SendInput(uint n, INPUT[] i, int size);
    [StructLayout(LayoutKind.Sequential)] struct INPUT { public uint type; public UNION u; }
    [StructLayout(LayoutKind.Explicit)] struct UNION {
        [FieldOffset(0)] public KEYBOARD keyboard;
        [FieldOffset(0)] public MOUSE mouse;
    }
    [StructLayout(LayoutKind.Sequential)] struct KEYBOARD {
        public ushort key, scan; public uint flags, time; public UIntPtr extra;
    }
    [StructLayout(LayoutKind.Sequential)] struct MOUSE {
        public int x, y; public uint data, flags, time; public UIntPtr extra;
    }
    public sealed class Window {
        public IntPtr Handle; public string Title; public string Class;
    }
    public static Window[] Windows(int pid) {
        var result = new List<Window>();
        EnumWindows((h, unused) => {
            uint owner; GetWindowThreadProcessId(h, out owner);
            if (owner == pid && IsWindowVisible(h)) {
                var title = new StringBuilder(512); var name = new StringBuilder(256);
                GetWindowText(h, title, title.Capacity); GetClassName(h, name, name.Capacity);
                result.Add(new Window { Handle=h, Title=title.ToString(), Class=name.ToString() });
            }
            return true;
        }, IntPtr.Zero);
        return result.ToArray();
    }
    static INPUT Key(ushort key, ushort scan, uint flags) {
        return new INPUT { type=1, u=new UNION { keyboard=new KEYBOARD { key=key, scan=scan, flags=flags } } };
    }
    public static void Type(string text) {
        var events = new List<INPUT>();
        foreach (char c in text) {
            events.Add(Key(0, c, 4)); events.Add(Key(0, c, 6));
        }
        events.Add(Key(13, 0, 0)); events.Add(Key(13, 0, 2));
        Send(events);
    }
    public static void Chord(params ushort[] keys) {
        var events = new List<INPUT>();
        foreach (ushort key in keys) events.Add(Key(key, 0, 0));
        for (int i = keys.Length - 1; i >= 0; i--) events.Add(Key(keys[i], 0, 2));
        Send(events);
    }
    static void Send(List<INPUT> events) {
        INPUT[] input = events.ToArray();
        if (SendInput((uint)input.Length, input, Marshal.SizeOf<INPUT>()) != input.Length)
            throw new Exception("SendInput failed: " + Marshal.GetLastWin32Error());
    }
}
'@
}
function Expected-Shell {
    foreach ($Name in 'pwsh.exe', 'powershell.exe') {
        foreach ($Directory in ($env:PATH -split ';')) {
            if ([IO.Path]::IsPathFullyQualified($Directory)) {
                $Candidate = Join-Path $Directory $Name
                if (Test-Path $Candidate -PathType Leaf) { return [IO.Path]::GetFullPath($Candidate) }
            }
        }
    }
    if ($env:ComSpec -and [IO.Path]::IsPathFullyQualified($env:ComSpec)) { return $env:ComSpec }
    return Join-Path $env:SystemRoot 'System32/cmd.exe'
}
function Descendants([int]$RootPid, [int[]]$Known = @()) {
    $All = @(Get-CimInstance Win32_Process -OperationTimeoutSec 5)
    $Ids = [Collections.Generic.HashSet[int]]::new()
    [void]$Ids.Add($RootPid)
    foreach ($KnownPid in $Known) { [void]$Ids.Add($KnownPid) }
    do {
        $Added = $false
        foreach ($Child in $All) {
            if ($Ids.Contains([int]$Child.ParentProcessId) -and $Ids.Add([int]$Child.ProcessId)) { $Added = $true }
        }
    } while ($Added)
    return @($All | Where-Object { $_.ProcessId -ne $RootPid -and $Ids.Contains([int]$_.ProcessId) })
}
function Saved-Leaves($Node, [string]$ParentOrientation = '') {
    if ($null -ne $Node.leaf) {
        if ($null -ne $Node.branch -or $Node.leaf.panel_width -isnot [long] -or $Node.leaf.panel_width -le 0 -or $Node.leaf.folded -isnot [bool]) { throw 'layout: invalid saved console' }
        return 1
    }
    if ($null -eq $Node.branch -or $Node.branch.orientation -notin @('columns', 'rows') -or $Node.branch.orientation -eq $ParentOrientation -or @($Node.branch.children).Count -lt 2) { throw 'layout: invalid saved tree' }
    $Count = 0
    [single]$Sum = 0
    foreach ($Child in $Node.branch.children) {
        if ($Child.weight -isnot [ValueType] -or $Child.weight -is [bool] -or [single]$Child.weight -le 0 -or -not [single]::IsFinite([single]$Child.weight)) { throw 'layout: invalid saved weight' }
        $Sum += [single]$Child.weight
        $Count += Saved-Leaves $Child.node $Node.branch.orientation
    }
    if (-not [single]::IsFinite($Sum)) { throw 'layout: invalid saved weight sum' }
    return $Count
}
function Set-LaunchEnvironment($Start, [string]$Profile, [string]$Control) {
    foreach ($Name in @($Start.Environment.Keys)) {
        if ($Name -match '^(XDG_|MEASURE_|IVY_)' -or $Name -in @('SHELL', 'PANEL_ID', 'PANEL_DIR')) {
            [void]$Start.Environment.Remove($Name)
        }
    }
    $Start.Environment['APPDATA'] = $Profile
    if ($Control -eq 'shell') {
        # Neither default resolution nor fallback can start a shell in this negative trial.
        $Start.Environment['PATH'] = $Profile
        $Start.Environment['ComSpec'] = Join-Path $Profile 'missing-cmd.exe'
    }
}
$Start = [Diagnostics.ProcessStartInfo]::new($Executable)
$Start.UseShellExecute = $false
$Start.WorkingDirectory = $EvidenceDir
$Start.RedirectStandardOutput = $true
$Start.RedirectStandardError = $true
Set-LaunchEnvironment $Start $Profile $Control
$Expected = if ($Control -eq 'shell') { $Start.Environment['ComSpec'] } else { Expected-Shell }
$Process = $null
$Children = @()
# Every trial starts in 'startup': the shell control only counts once its window exists, so an
# application that never got that far cannot pass it.
$Stage = 'startup'
$Clock = [Diagnostics.Stopwatch]::StartNew()
function Check-Alive {
    if ($Process.HasExited) { throw "${Stage}: Ivy exited before normal close ($($Process.ExitCode))" }
    if ([IvyLaunchNative]::Windows($Process.Id) | Where-Object { $_.Class -eq '#32770' }) { throw "${Stage}: unexpected modal dialog" }
    if ($Clock.Elapsed.TotalSeconds -ge $TimeoutSeconds) {
        if ($Stage -eq 'shell') { throw 'shell: no working default console before the deadline' }
        throw "deadline: $Stage exceeded ${TimeoutSeconds}s"
    }
}
try {
    if ($Control -eq 'layout') { New-Item -ItemType Directory $Layout -Force | Out-Null }
    $Process = [Diagnostics.Process]::Start($Start)
    $Stdout = $Process.StandardOutput.ReadToEndAsync()
    $Stderr = $Process.StandardError.ReadToEndAsync()
    if ($Control -eq 'crash') {
        $Process.Kill($true)
        if (-not $Process.WaitForExit(5000)) { throw 'control: startup termination did not complete' }
    }
    $Window = $null
    while ($null -eq $Window) {
        Check-Alive
        $Windows = @([IvyLaunchNative]::Windows($Process.Id))
        if ($Windows | Where-Object { $_.Class -eq '#32770' }) { throw "${Stage}: unexpected modal dialog" }
        $Window = $Windows | Where-Object { $_.Title -match 'Ivy' -and $_.Class -ne '#32770' } | Select-Object -First 1
        if ($null -eq $Window) { Start-Sleep -Milliseconds 50 }
    }
    $Stage = 'shell'
    $Shell = $null
    while ($null -eq $Shell) {
        Check-Alive
        $Children = @(Descendants $Process.Id)
        Check-Alive
        $Shell = $Children | Where-Object { $_.ExecutablePath -ieq $Expected } | Select-Object -First 1
        if ($null -eq $Shell) { Start-Sleep -Milliseconds 250 }
    }
    $Shell | Select-Object ProcessId, ParentProcessId, ExecutablePath, CommandLine | ConvertTo-Json | Set-Content (Join-Path $EvidenceDir 'shell.json')
    $ShellProcess = [Diagnostics.Process]::GetProcessById($Shell.ProcessId)
    $Activated = [IvyLaunchNative]::SetForegroundWindow($Window.Handle)
    if (-not $Activated -and [IvyLaunchNative]::GetForegroundWindow() -ne $Window.Handle) { throw 'shell: could not focus the application window' }
    while ([IvyLaunchNative]::GetForegroundWindow() -ne $Window.Handle) {
        Check-Alive
        Start-Sleep -Milliseconds 50
    }
    if ([IO.Path]::GetFileName($Expected) -ieq 'cmd.exe') {
        $Command = "echo $Token>`"$Witness`""
    } else {
        $Escaped = $Witness.Replace("'", "''")
        $Command = "[IO.File]::WriteAllText('$Escaped', '$Token|' + `$PID)"
    }
    [IvyLaunchNative]::Type($Command)
    $Required = if ([IO.Path]::GetFileName($Expected) -ieq 'cmd.exe') { $Token } else { "$Token|$($Shell.ProcessId)" }
    while ($true) {
        if ($ShellProcess.HasExited) { throw 'shell: console program exited without executing the input witness' }
        Check-Alive
        if (Test-Path $Witness -PathType Leaf) {
            try { $Text = [IO.File]::ReadAllText($Witness).Trim() } catch [IO.IOException] { $Text = '' }
            if ($Text -eq $Required) { break }
        }
        Start-Sleep -Milliseconds 50
    }
    # A window nobody rearranged writes no layout (opening.rs records the first mirror as seen), so
    # the trial splits the console the way a person would and waits for the second shell.
    $Stage = 'split'
    [IvyLaunchNative]::Chord(0x11, 0x10, 0x44)
    while ($true) {
        Check-Alive
        $Children = @(Descendants $Process.Id)
        if (@($Children | Where-Object { $_.ExecutablePath -ieq $Expected }).Count -ge 2) { break }
        Start-Sleep -Milliseconds 250
    }
    $Stage = 'close'
    if ($Control -ne 'deadline') {
        if (-not [IvyLaunchNative]::PostMessage($Window.Handle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) { throw 'close: WM_CLOSE failed' }
    }
    while (-not $Process.HasExited) {
        if ($Clock.Elapsed.TotalSeconds -ge $TimeoutSeconds) { throw 'deadline: normal shutdown did not finish' }
        Start-Sleep -Milliseconds 50
    }
    if ($Process.ExitCode -ne 0) { throw "close: Ivy exited $($Process.ExitCode)" }
    $Known = @($Children | ForEach-Object { [int]$_.ProcessId })
    while ($true) {
        if ($Clock.Elapsed.TotalSeconds -ge $TimeoutSeconds) { throw 'close: descendant processes survived normal shutdown' }
        if (@(Descendants $Process.Id $Known).Count -eq 0) { break }
        Start-Sleep -Milliseconds 250
    }
    $Stage = 'layout'
    if (-not (Test-Path $Layout -PathType Leaf)) { throw 'layout: no saved layout' }
    $Document = Get-Content $Layout -Raw | ConvertFrom-Json
    if ($Document.v -ne 2 -or @($Document.tabs).Count -eq 0) { throw 'layout: empty or unsupported document' }
    $Leaves = 0
    foreach ($Tab in $Document.tabs) { $Leaves += Saved-Leaves $Tab.root }
    if ($Leaves -ne 2) { throw "layout: saved $Leaves console(s), expected the two of the split" }
    Write-Output "Installed launch passed: native window, $Expected, real console input, $Leaves saved console(s), normal exit 0."
} catch {
    $_ | Out-String | Set-Content (Join-Path $EvidenceDir 'failure.txt')
    if ($null -ne $Process) {
        [IvyLaunchNative]::Windows($Process.Id) | Select-Object Title, Class, Handle | ConvertTo-Json | Set-Content (Join-Path $EvidenceDir 'windows.json')
        @(Descendants $Process.Id) | Select-Object ProcessId, ParentProcessId, ExecutablePath, CommandLine | ConvertTo-Json | Set-Content (Join-Path $EvidenceDir 'processes.json')
    }
    throw
} finally {
    if ($null -ne $Process) {
        $Remaining = @(Descendants $Process.Id @($Children | ForEach-Object { [int]$_.ProcessId }))
        if (-not $Process.HasExited) {
            try { $Process.Kill($true) } catch { if (-not $Process.HasExited) { throw } }
            if (-not $Process.WaitForExit(5000)) { throw 'cleanup: Ivy did not terminate' }
        }
        foreach ($Child in $Remaining) { Stop-Process -Id $Child.ProcessId -Force -ErrorAction SilentlyContinue }
        foreach ($Log in @(@($Stdout, 'stdout.txt'), @($Stderr, 'stderr.txt'))) {
            if ($null -ne $Log[0]) {
                $Text = if ($Log[0].Wait(5000)) { $Log[0].GetAwaiter().GetResult() } else { 'Output pipe did not close after cleanup.' }
                $Text | Set-Content (Join-Path $EvidenceDir $Log[1])
            }
        }
        $Process.Dispose()
    }
}
