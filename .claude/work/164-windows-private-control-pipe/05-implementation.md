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
- [x] 1. Private native security and directory creation — panel-contract and endpoint security — ACL/directory tests (AC2/AC3).
- [x] 2. Cancellable overlapped transport and client authentication — panel-control endpoint/client — bind, PID, race and timeout tests (AC1/AC4/AC5).
- [x] 3. Callback-free Windows services and app lifecycle — listener, snapshots, UI wake and releases — ordering/shutdown tests (AC5/AC6).
- [x] 4. Native test job and forwarding/discovery integration — workflow and launchers — native Windows tests and Unix regressions (AC6/AC7).
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

## Proof location clarification
Owner ticket comment 2026-11-20T21:12:18Z assigns native proof to windows-2025 CI; no native executor is requested here. Remaining second-user, remote rejection and intended-pane painting steps travel in the draft body.

## Running implementation log
- Delegated native security/transport, concrete Windows worker lifecycle and app-side integration as independent writers, per the L build procedure. Effective model setting: `models.agents: gpt-6-luna` from FLOW.codex.md; no agent role definitions are configured.
- Client PID parsing is strict, nonzero decimal u32. `Producer::of_server` accepts explicit expected identity; `Producer::of` reads the inherited expected PID on Windows. Session helpers use the same authenticated native transport. Both peer witness modules now consume the platform stream facade; native Windows session authorization stays unsupported.
- MCP guidance requires complete expected Windows PID; no connection probe occurs during initialize. Shared daemon launch explicitly removes the extra PID variable.
- Added windows-2025 transport/MCP and app lifecycle test steps. Added native integration cases for wrong-PID rejection before bytes and bridge panel_set round trip.
- Initial Linux panel-control suite exposed request-before-identity validation in the session helper adaptation. Restored inherited identity validation before encoding the request; the complete crate suite subsequently passed (all 7 session-helper tests included).
- Native manual procedure is recorded in windows-manual-proof.md and will be included in the draft body. Native tests execute in CI; a red native job blocks readiness.
- Flow CORE reports 0.118.0; installed plugin manifest `~/.claude/flow/.claude-plugin/plugin.json` is absent. CLI pinned to core's 0.118.0; panel command succeeded. No separate installed version can be compared.

## Platform test API migration
Unix callback fixture tests remain on Unix because those constructors are now Unix APIs by the approved lifetime design. Portable identity and reply parser assertions still compile on Windows. The Windows job runs separate concrete-service lifetime, transport and MCP integration tests, with required test-name checks to prevent empty suites.

## Additional build checks
- Packaging Python suite: 23 passed. Session recovery JavaScript suite: 19 passed.
- Both Windows Rust target test sets for panel-contract and panel-control compile; native execution is pending CI, never claimed here.
- Initial app cross check found missing MinGW C compiler for ring. Downloaded official Debian compiler packages and extracted them under /tmp/164-mingw without changing system/repo configuration. App test cross compilation now reaches app source/tests; integration fixes are in progress.
- Build coverage audit identified required late-completion/worker-failure/off-frame native tests; those are implementation steps, not excused by lack of local Windows execution.

## Size thermometer
The actual file count exceeds the 32+2 warning threshold. Recorded unattended: continue the atomic delivery rather than split the security boundary from its lifecycle/native tests. Estimates remain advisory.

## Contract verification
- Pipe name is exactly `\\.\pipe\ivy-<PANEL_ID>`; native transport emits no new wire identity fields. Existing newline JSON request/reply keys remain unchanged.
- `ENV_NAMES` and `PanelDir::env` remain two-entry identity arrays. Windows launches append `PANEL_SERVER_PID` separately and clear it for no-pane launches. Client parsing requires nonzero decimal u32; explicit Producer::of_server and inherited Producer::of authenticate before request writes.
- No schema, database, new protocol verb or cross-repository surface.

## Validation repairs during build
The full Linux suite exposed an introduced deferred-request bug: mem::replace removed What before queueing the same Delivery, so split/retirement requests arrived as Closed. Restore the exact deferred request before enqueueing; keep the existing assertions. The connection guide also changed Unix wording required by its visible scrolling test; restore the original Unix text and retain platform-specific Windows guidance.
The split test also contained a known fragile 200ms arrival assumption. Replace it with an inbox arrival barrier and one apply pass; preserve queue type/count and untouched-panel assertions. Its repair receives its own commit as required. No tests are deleted, retried, loosened or given longer sleeps.

## Completed build steps
- 22d9919 · Repair the fixed-window split-delivery assertion with an arrival barrier · 1 file +16 -1.
- 3b4fbbc · Native private directory/pipe security, authenticated platform client, joined concrete Windows services, app snapshot/release/wake integration and CI/native tests · 41 files +3790 -202.
- Step 5 native intended-pane observation remains not-verified-unattended per the owner: exact Windows steps travel in the draft body. This is validation deferred explicitly by the owner, not missing implementation.
- Actual branch: 41 files +3806 -203; both file and line overrun thresholds exceeded. Continue the one atomic security/lifecycle delivery (recorded unattended).
- GNU Windows app --tests cross check and relevant Linux control tests passed after integration fixes; final comprehensive checks follow review.

## Review and validation repairs
- Independent review found the public raw Windows connector bypass and late release admission after revocation. Both fixed in d251b2d; native regressions and required CI test names added. Follow-up review passed.
- Explicit Unix callback fixture gates align app tests with the removed Windows callback API; native concrete-service tests remain required and compiled. Fixed misplaced/transitive cfg gates after GNU test compilation caught them.
- Full source guards found three missing module rows and an unlisted external environment name. Added exact entries without weakening either assertion.
- App size guards required the HWND bridge/wake test to live in control/channel.rs and document publication policy in document.rs free functions. Pane's release drain is also a free control function. Module/type ceilings were preserved; the measured PanelState ratchet decreased from 356 to 353.
- Final native proof location is Windows CI; desktop, second-user and remote checks are not claimed passed. The earlier environment preflight stop above is historical and superseded by the owner's proof-location clarification.

- Final wait audit: declared the six native waits, restored adjacency of the plan-worker recv explanation, and expressed test-only platform modules with equivalent separate cfg attributes. The pinned declaration equality is measured 21. Independent delta review found no runtime change.
