param([Parameter(Mandatory=$true)][string]$PanelId,
      [Parameter(Mandatory=$true)][uint32]$ServerPid,
      [Parameter(Mandatory=$true)][string]$Text,
      [Parameter(Mandatory=$true)][string]$Ack)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Publication witness requires the disposable CI desktop.' }
if ($PanelId -notmatch '^[a-fA-F0-9]+$') { throw 'Invalid pane identity' }
if (-not ('IvyWitnessPipe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class IvyWitnessPipe {
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern bool GetNamedPipeServerProcessId(IntPtr pipe, out uint pid);
}
'@
}
function Open-WitnessPipe([string]$Name) {
    $Pipe = [IO.Pipes.NamedPipeClientStream]::new('.', $Name, [IO.Pipes.PipeDirection]::InOut, [IO.Pipes.PipeOptions]::Asynchronous)
    try {
        $Pipe.Connect(2000)
        [uint32]$Owner = 0
        if (-not [IvyWitnessPipe]::GetNamedPipeServerProcessId($Pipe.SafePipeHandle.DangerousGetHandle(), [ref]$Owner) -or $Owner -ne $ServerPid) {
            throw 'Witness pipe is not owned by this Ivy process'
        }
        return $Pipe
    } catch { $Pipe.Dispose(); throw }
}
function Read-WitnessFrame($Reader) {
    $Read = $Reader.ReadLineAsync()
    if (-not $Read.Wait(2000)) { throw 'Witness reply exceeded its deadline' }
    $Line = $Read.GetAwaiter().GetResult()
    if ($null -eq $Line -or $Line.Length -gt 1024) { throw 'Invalid witness reply' }
    return $Line | ConvertFrom-Json
}
$Bootstrap = $null; $Fresh = $null
$BootstrapReader = $null; $BootstrapWriter = $null; $Reader = $null; $Writer = $null
try {
    $Utf8 = [Text.UTF8Encoding]::new($false)
    $Bootstrap = Open-WitnessPipe "ivy-$PanelId"
    $BootstrapReader = [IO.StreamReader]::new($Bootstrap, $Utf8, $false, 1024, $true)
    $BootstrapWriter = [IO.StreamWriter]::new($Bootstrap, $Utf8, 1024, $true)
    $BootstrapWriter.NewLine = "`n"; $BootstrapWriter.AutoFlush = $true
    $BootstrapWriter.WriteLine('{"tool":"peer_hello","v":1}')
    $Redirect = Read-WitnessFrame $BootstrapReader
    if ($Redirect.ok -ne $true -or $Redirect.peer.v -ne 1 -or $Redirect.peer.mode -cne 'reconnect' -or $Redirect.peer.name -cnotmatch '^ivy-peer-[a-f0-9]{32}\z') {
        throw 'Expected an exact verified-peer redirect'
    }
    $Fresh = Open-WitnessPipe $Redirect.peer.name
    $Reader = [IO.StreamReader]::new($Fresh, $Utf8, $false, 1024, $true)
    $Writer = [IO.StreamWriter]::new($Fresh, $Utf8, 1024, $true)
    $Writer.NewLine = "`n"; $Writer.AutoFlush = $true
    $Ready = Read-WitnessFrame $Reader
    if ($Ready.ok -ne $true -or $Ready.peer.v -ne 1 -or $Ready.peer.mode -cne 'verified') { throw 'Expected verified peer acknowledgement' }
    $Request = @{ tool='panel_set'; source='desktop witness'; document=@{ header=$false; lines=@($Text) } } | ConvertTo-Json -Depth 4 -Compress
    $Writer.WriteLine($Request)
    $Reply = Read-WitnessFrame $Reader
    if ($Reply.ok -ne $true) { throw 'Native desktop publication was refused' }
} finally {
    foreach ($Owned in @($Writer, $Reader, $BootstrapWriter, $BootstrapReader, $Fresh, $Bootstrap)) {
        if ($null -ne $Owned) { $Owned.Dispose() }
    }
}
[IO.File]::WriteAllText($Ack, 'published')
