Closes #164

Windows panes can publish panels and use the MCP control tools through a private named pipe. Clients verify the expected Ivy app PID before sending requests. Closing a pane cancels native I/O and joins its channel workers; committed reads use bounded snapshots while the UI is resting.

The pipe and pane directory have protected current-user/SYSTEM DACLs. The listener reserves the first pipe instance and rejects remote clients. Windows launchers forward `PANEL_SERVER_PID` separately from the stable `PANEL_ID`/`PANEL_DIR` address pair and clear it for no-pane launches.

## Brief

- Users can publish to their intended Windows pane and use its panel, plan, signal and split operations.
- Other users and remote clients are excluded by the native security boundary.
- Clients authenticate the app process and pane shutdown owns its worker lifetime.
- Native Windows session/overlay authorization, a separate runtime/service and unrelated porting are excluded.

Prepared by an unattended run; the implementation brief was recorded rather than separately confirmed.

<details>
<summary>Technical compatibility</summary>

The newline JSON wire protocol is unchanged. The pre-1.0 Rust API exposes platform Listener/Stream types. Windows apps use `Channel::open_windows` with concrete services; callback constructors remain Unix APIs. `Producer::of_server` takes an explicit expected Windows PID; `Producer::of` reads it from the inherited environment. Existing Unix behavior and the two-entry identity arrays are preserved.

</details>

## Validation

Local validation on `4ed9487a44b152a773109cf83a7a55b3c0417b47`:

- `cargo test --locked --workspace`: 2691 passed, 0 failed, 16 ignored.
- `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo doc-check --locked` passed.
- `cargo check --locked -p app --tests --target x86_64-pc-windows-gnu` passed with a local MinGW C cross-toolchain.
- `cargo check --locked -p panel-contract -p panel-control --tests --target x86_64-pc-windows-msvc` passed.
- Portable packaging tests: 23 passed. Session-recovery JavaScript tests: 19 passed.

The `windows-2025` job must execute and pass the committed 31 native unit and 3 native integration tests, plus existing startup/path checks. Required-test-name assertions prevent an empty filtered suite from passing. The local Windows checks above establish compilation; native runtime proof remains pending. Existing macOS CI also remains required before readiness.

## Evidence

Final Linux app unit result (the full workspace aggregate is above):

```text
test result: ok. 2011 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 104.14s
```

The source guards also pass: 5 script tests, 14 size tests and 13 wait tests. All local commands above exited 0. Native Windows behavior and screenshots are pending the CI/owner procedures below.

## Also fixed

`a_split_request_waits_for_the_window_instead_of_being_applied` used a 200ms scheduling assumption; an explicit request-arrival barrier now preserves the original application and reply assertions. This repair is in its own commit.

Source guards now include the three new transport modules and the external PID environment name. App ownership refactoring preserves module/type ceilings and lowers the measured PanelState ratchet 356→353; no limit was raised.


## Not verified — needs a human

No native desktop, second-user token or network-capable Windows host was available to this run.
The windows-2025 job must pass the committed transport, DACL, PID, cancellation and lifecycle tests.
These remaining checks require an owner observation; none is claimed passed here. The directory suite asserts the fresh owner SID and rejects unsafe ACLs/reparse points; a negative foreign-owner directory fixture was not exercised. Include that fixture in the second-user validation if available.

### Intended-pane painting and resting-window wake (AC6)

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

### Other-user denial (remaining AC2 behavioral proof)

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

### Remote-client rejection (remaining AC2 behavioral proof)

From a second network-capable Windows machine, with authenticated SMB access to the Ivy host and
a live endpoint, use the same NamedPipeClientStream check with the first constructor argument set
to the Ivy host name instead of `.`. Use an identity authorized by the DACL (same domain user), so
the test isolates remote-client rejection. It must be refused. Prove the network/IPC path works
independently; firewall, unreachable SMB, missing endpoint or timeout alone does not prove rejection.
Record both hosts, user SID, exception and exact revision. Do not relax the pipe flags or ACLs.
