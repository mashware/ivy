# Implementation 164

## Brief MR/PR #1: Enable the private Windows control channel
Recorded (unattended).

After this PR:
- Windows pane users can publish panels and use MCP tools through their own private channel.
- Other Windows users and remote clients cannot reach the channel.
- Clients verify they reached the app that launched their pane before sending requests.
- Closing a pane stops its channel workers within the documented bound.

This PR does NOT include:
- Native Windows session or overlay authorization.
- A separate runtime or service.
- Other Windows porting tickets.

## Contracts to respect (copied verbatim from 03-design.md)
Existing pipe address: `\\.\pipe\ivy-<PANEL_ID>`. The wire remains newline-delimited JSON with unchanged verbs, payloads, document ceilings and replies. No additional pane identifier or PID is sent on the wire.
Windows inherited environment adds `PANEL_SERVER_PID`: decimal nonzero PID of the app process that owns the listener. PANEL_ID and absolute PANEL_DIR remain mandatory. Generate the PID at the app's pane launch boundary; never derive the expected PID from the connected pipe. All Windows producer entry points must receive this expectation explicitly or read the inherited value; direct Producer::of must not bypass authentication. Preserve the two-slot ENV_NAMES/PanelDir::env identity arrays and Unix behavior. Add PANEL_SERVER_PID separately at the Windows app launch boundary, remove it explicitly for no-pane launches, and update every Windows forwarding list; the client reads PID separately from whole_identity. No other repository consumes a new surface.
No database, HTTP or new public protocol verb.


## Plan MR/PR #1
- [ ] 1. Private native security and directory creation — panel-contract and endpoint security — ACL/directory tests (AC2/AC3).
- [ ] 2. Cancellable overlapped transport and client authentication — panel-control endpoint/client — bind, PID, race and timeout tests (AC1/AC4/AC5).
- [ ] 3. Callback-free Windows services and app lifecycle — listener, snapshots, UI wake and releases — ordering/shutdown tests (AC5/AC6).
- [ ] 4. Native test job and forwarding/discovery integration — workflow and launchers — native Windows tests and Unix regressions (AC6/AC7).
- [ ] 5. Native intended-pane observation — Windows running app — painting evidence (AC6).

Point of no return: none; all source changes are reversible.
Out of this plan: unrelated porting and native authorization.
Plan accepted (unattended). Estimates are advisory, never a budget.

## Steps

## Decisions made during implementation
- Environment preflight: Rust is installed under /usr/local/cargo/bin but omitted from PATH. Use that toolchain explicitly.
- Native Windows validation remains required; this Linux host cannot establish Windows ACL, remote denial or GUI painting evidence.

## Pending
- Native Windows execution access for AC1–AC6 and native app wake/painting.

## Environment preflight stop
No implementation step started. Native Windows security, lifecycle and GUI execution is unavailable; see environment-preflight.md. No acceptance criterion is marked proven. Resume the first unchecked step once native execution is available.
