param([Parameter(Mandatory=$true)][string]$Executable,
      [Parameter(Mandatory=$true)][string]$EvidenceDir,
      [ValidateRange(1, 60)][int]$TimeoutSeconds = 30)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:IVY_DISPOSABLE_RUNNER -ne 'yes') {
    throw 'Graphics failure checks require a disposable GitHub Windows runner.'
}
$Executable = (Resolve-Path $Executable).Path
$EvidenceDir = [IO.Path]::GetFullPath($EvidenceDir)
if (Test-Path $EvidenceDir) { throw 'Graphics evidence directory must be fresh.' }
New-Item -ItemType Directory $EvidenceDir | Out-Null
if (-not ('IvyGraphicsTrial' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class IvyGraphicsTrial {
    public delegate bool EnumProc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr p);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr h, EnumProc f, IntPtr p);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    public static IntPtr Dialog(int pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, unused) => {
            uint owner; GetWindowThreadProcessId(h, out owner);
            var title = new StringBuilder(512); var name = new StringBuilder(256);
            GetWindowText(h, title, title.Capacity); GetClassName(h, name, name.Capacity);
            if (owner == pid && IsWindowVisible(h) && name.ToString() == "#32770" && title.ToString() == "Ivy could not start")
                found = h;
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static string Text(IntPtr dialog) {
        var parts = new List<string>();
        bool timedOut = false;
        EnumChildWindows(dialog, (h, unused) => {
            var text = new StringBuilder(8192);
            IntPtr result;
            if (SendMessageTimeout(h, 0x000D, (IntPtr)text.Capacity, text, 2, 1000, out result) == IntPtr.Zero) {
                timedOut = true; return false;
            }
            if (text.Length != 0) parts.Add(text.ToString());
            return true;
        }, IntPtr.Zero);
        if (timedOut) throw new Exception("Native dialog text did not respond within its deadline.");
        return string.Join("\n", parts);
    }
}
'@
}
$Start = [Diagnostics.ProcessStartInfo]::new($Executable)
$Start.UseShellExecute = $false
$Start.CreateNoWindow = $true
$Start.WorkingDirectory = $EvidenceDir
$Start.RedirectStandardOutput = $true
$Start.RedirectStandardError = $true
foreach ($Name in @($Start.Environment.Keys)) {
    if ($Name -match '^(XDG_|MEASURE_|IVY_)' -or $Name -in @('PANEL_ID', 'PANEL_DIR')) {
        [void]$Start.Environment.Remove($Name)
    }
}
$Start.Environment['XDG_CONFIG_HOME'] = Join-Path $EvidenceDir 'config'
$Start.Environment['XDG_STATE_HOME'] = Join-Path $EvidenceDir 'state'
$Process = $null
$Stderr = $null
$Stdout = $null
try {
    $Process = [Diagnostics.Process]::Start($Start)
    $Stderr = $Process.StandardError.ReadToEndAsync()
    $Stdout = $Process.StandardOutput.ReadToEndAsync()
    $Clock = [Diagnostics.Stopwatch]::StartNew()
    $Dialog = [IntPtr]::Zero
    while ($Dialog -eq [IntPtr]::Zero) {
        if ($Process.HasExited) { throw "Graphics failure had no visible notice; exit $($Process.ExitCode)" }
        if ($Clock.Elapsed.TotalSeconds -ge $TimeoutSeconds) { throw 'Expected the hosted runner without software OpenGL to show a graphics failure notice.' }
        $Dialog = [IvyGraphicsTrial]::Dialog($Process.Id)
        if ($Dialog -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 50 }
    }
    $Text = [IvyGraphicsTrial]::Text($Dialog)
    if ($Text -cnotmatch 'Ivy could not initialize graphics\.' -or
        $Text -cnotmatch 'Ivy requires OpenGL 2\.0 or later\.' -or
        $Text -cnotmatch 'Detected OpenGL version: (unavailable|[0-9][^\r\n]*)' -or
        $Text -cnotmatch 'Details:') { throw "Incomplete native graphics notice: $Text" }
    Set-Content (Join-Path $EvidenceDir 'dialog.txt') $Text -Encoding utf8NoBOM
    $Button = [IvyGraphicsTrial]::GetDlgItem($Dialog, 1)
    if ($Button -eq [IntPtr]::Zero) { throw 'The native graphics notice has no OK button.' }
    if (-not [IvyGraphicsTrial]::SetForegroundWindow($Dialog) -and [IvyGraphicsTrial]::GetForegroundWindow() -ne $Dialog) {
        throw 'Could not activate the native graphics notice.'
    }
    Add-Type -AssemblyName System.Drawing
    $Bounds = [IvyGraphicsTrial+Rect]::new()
    if (-not [IvyGraphicsTrial]::GetWindowRect($Dialog, [ref]$Bounds)) { throw 'Could not read native dialog bounds.' }
    $Bitmap = [Drawing.Bitmap]::new($Bounds.Right - $Bounds.Left, $Bounds.Bottom - $Bounds.Top)
    try {
        $Graphics = [Drawing.Graphics]::FromImage($Bitmap)
        try { $Graphics.CopyFromScreen($Bounds.Left, $Bounds.Top, 0, 0, $Bitmap.Size) }
        finally { $Graphics.Dispose() }
        $Bitmap.Save((Join-Path $EvidenceDir 'dialog.png'), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $Bitmap.Dispose() }
    # Activate the actual OK control so its own notification, with its control handle,
    # reaches the dialog.
    if (-not [IvyGraphicsTrial]::PostMessage($Button, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)) { throw 'Could not click the native OK button.' }
    if (-not $Process.WaitForExit(5000)) {
        if ([IvyGraphicsTrial]::Dialog($Process.Id) -ne [IntPtr]::Zero) { throw 'The native graphics notice stayed open after OK button activation.' }
        throw 'The native notice closed, but Ivy did not exit.'
    }
    if ($Process.ExitCode -ne 1) { throw "Expected graphics failure exit 1, got $($Process.ExitCode)." }
    if (-not $Stderr.Wait(5000) -or $Stderr.Result -notmatch 'Error: (OpenGL|Glutin|NoGlutinConfigs)') {
        throw 'Original typed graphics error was not preserved on standard error.'
    }
    Write-Output 'Native graphics notice, requirement/version text, screenshot, dismissal and failure exit passed.'
} finally {
    if ($Process -and -not $Process.HasExited) {
        $Process.Kill($true)
        [void]$Process.WaitForExit(5000)
    }
    if ($Stderr -and $Stderr.IsCompletedSuccessfully) { Set-Content (Join-Path $EvidenceDir 'stderr.txt') $Stderr.Result -Encoding utf8NoBOM }
    if ($Stdout -and $Stdout.IsCompletedSuccessfully) { Set-Content (Join-Path $EvidenceDir 'stdout.txt') $Stdout.Result -Encoding utf8NoBOM }
    if ($Process) { $Process.Dispose() }
}
