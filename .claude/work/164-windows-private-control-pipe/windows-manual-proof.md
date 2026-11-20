# Windows owner proof before marking ready

No native desktop, second-user token or network-capable Windows host was available to this run.
The windows-2025 job must pass the committed transport, DACL, PID, cancellation and lifecycle tests.
These remaining checks require an owner observation; none is claimed passed here.

## Intended-pane painting and resting-window wake (AC6)

1. Build this exact revision on Windows: `cargo build --locked -p app -p panel-control --bins`.
2. Start `target\debug\ivy.exe`, open two PowerShell consoles inside Ivy and leave the app resting
   (no mouse/keyboard interaction during delivery). In each console confirm `$env:PANEL_ID`,
   `$env:PANEL_DIR`, `$env:PANEL_SERVER_PID` differ only where expected (PID is the same app).
3. In console A run the following, using the built bridge's absolute path. In B replace `pane A`
   with `pane B`. The pipeline closes standard input so this test publisher exits after its reply.

```powershell
@'
{"jsonrpc":"2.0","id":1,"method":"initialize"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"panel_set","arguments":{"source":"windows-proof","document":{"lines":["pane A"]}}}}
'@ | & 'C:\path\to\ivy\target\debug\panel-mcp.exe'
```

4. Observe instructions in initialize and a successful tool reply. Confirm the distinct status
   paints only beside the intended console without interacting with the window to wake it.
5. Repeat while minimized, restore and verify latest status; use panel_get while minimized to
   verify committed reads do not require a frame. Capture before/after screenshots and the exact
   revision, Windows version and results. Closing the test publisher should release its claim.

## Other-user denial (remaining AC2 behavioral proof)

1. Keep an Ivy console open under user A. Note its ephemeral PANEL_ID without persisting config.
2. Run PowerShell as a distinct ordinary user B (`runas /user:<user-B> powershell.exe`). Do not use
   elevation, impersonate A or change ACLs. Set `$pipeName` below to `ivy-` plus that noted ID.
3. Run:

```powershell
$pipeName = 'ivy-<ephemeral-PANEL_ID>'
$pipe = [IO.Pipes.NamedPipeClientStream]::new('.', $pipeName, [IO.Pipes.PipeDirection]::InOut)
try { $pipe.Connect(2000); throw 'FAIL: another user connected' }
catch [UnauthorizedAccessException] { 'PASS: distinct user denied' }
finally { $pipe.Dispose() }
```

A timeout or a missing endpoint is inconclusive: verify A can still connect to that same live pane
and capture the actual denial exception and `whoami /user` for both users.

## Remote-client rejection (remaining AC2 behavioral proof)

From a second network-capable Windows machine, with authenticated SMB access to the Ivy host and
a live endpoint, use the same NamedPipeClientStream check with the first constructor argument set
to the Ivy host name instead of `.`. Use an identity authorized by the DACL (same domain user), so
the test isolates remote-client rejection. It must be refused. Prove the network/IPC path works
independently; firewall, unreachable SMB, missing endpoint or timeout alone does not prove rejection.
Record both hosts, user SID, exception and exact revision. Do not relax the pipe flags or ACLs.
