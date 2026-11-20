# Design 164

## What already exists (inventory)
- `panel-control/src/endpoint.rs`: name_of already produces the Windows namespaced ivy-<id> pipe; Unix bind, acceptor readiness and private wake pair; Windows stubs and BINDS=false.
- `panel-control/src/listener.rs`: bounded eight connection workers, 200ms receive deadline, five-second answer deadline, reply deadline, stop flag and test alive counter. Channel drop currently detaches rather than joins; Windows cannot inherit its lifetime proof blindly.
- `panel-control/src/client.rs`: Producer::of_this_pane validates the two-variable identity and directory; Producer::of takes an explicit pane and calls endpoint::connect.
- `panel-contract/src/lib.rs`: PanelDir creates unique directories, mode 0700 only on Unix, and provides a two-entry environment array.
- `panel-control/src/diagnostic_peer.rs`, session/peer.rs: transport peer witnesses; preserve unsupported authorization semantics rather than granting Windows native session authority by enabling transport.
- `panel-mcp`, `ivy-session`, app control/channel.rs: consumers share endpoint capabilities; handshake uses BINDS and socket_present.
- integrations and app connection setup text forward PANEL_ID/PANEL_DIR; all explicit forwarding lists must be audited for the added PID.
- Existing Windows startup checks in `.github/workflows/ivy-prerelease.yml` provide a native Windows runner.

## Approaches considered
Parallel panel (three advisors, models.workers=gpt-6-luna from FLOW.codex.md).
1. Native Windows endpoint adapter, preserving protocol and Unix transport (Recommended). Raw Win32 is necessary for overlapped cancellation and auditable creation security. Touch endpoint/transport, client, directory/environment and consumers. Risk: handle and OVERLAPPED lifetimes. Bad if native I/O logic spreads into protocol handlers.
2. Extend interprocess's synchronous named pipe listener with security configuration and external cancellation. Smaller apparent API delta, but a synchronous accept does not provide the private event-based cancellation contract; converting raw overlapped handles into synchronous streams needs explicit library support. Reject unless the pinned library proves both requirements without unsafe mode mismatches.
3. Separate Windows service/runtime or an alternate AF_UNIX channel. Extra process/runtime lifetime and incompatible Node named-pipe connection semantics; does not solve today's requested transport simply. Rejected.
Panel consensus: native creation and cancellation are needed; disagreement: retain detached workers/timeouts versus explicitly own Windows worker teardown. Chosen: native endpoint plus owned Windows cancellation/worker completion; the latter is required by the ticket's no-worker-left criterion.
Choice recorded (unattended); size remains M.

## Additional domain context
Microsoft documents explicit pipe security descriptors, first-instance rejection, byte mode, overlapped I/O, and remote-client rejection:
- https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew
- https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights
- https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid
The pipe DACL controls client and additional-instance access; default descriptors expose read access beyond this user. Keep the pipe in byte mode for the existing newline JSON and Node clients. PID is an inherited witness, not a credential against same-user processes.

## Executive summary
- Introduce a small platform transport boundary; Unix continues to delegate to interprocess.
- Windows creates secure named pipe instances and runs overlapped connect/read/write with owned cancellation and completion.
- Create private Windows pane directories at creation, validate inherited owner/DACL and authenticate the connected server PID before any protocol bytes.
- Enable Windows capability and MCP discovery together with native security, teardown and handshake tests in one PR.

## Acceptance criteria
| ID | Assertion | Proof |
|---|---|---|
| AC1 | On Windows BINDS=true and binding_is_what_binds_says succeeds; two pane IDs never cross-connect; duplicate first bind fails. | test |
| AC2 | The bound pipe's actual protected DACL permits only the current token user SID and LocalSystem; an access check with a different user token denies connect. Remote clients are rejected. | test |
| AC3 | An inherited directory with the wrong owner, null/unprotected broad DACL or a reparse-point substitution is refused; freshly created PanelDir is accepted. | test |
| AC4 | Before sending bytes, a Windows client requires nonzero PANEL_SERVER_PID and GetNamedPipeServerProcessId equality; absent, malformed and mismatched PID fail closed. | test |
| AC5 | Dropping a Windows channel stops pending accept and active I/O and leaves zero transport workers when it returns, within the existing answer+reply+receive deadline envelope; a saturated inbox cannot retain a release sender. | test |
| AC6 | panel-mcp initialization inside a live Windows pane gives its instructions; panel_set succeeds and the intended pane displays the submitted status. | test + manual |
| AC7 | Existing Linux/macOS endpoint, framing, timeout and handshake tests remain green; no Windows transport success is treated as native session authorization. | test |

## Modules/layers affected
- endpoint.rs plus Windows transport/security modules: public platform Listener/Stream return types with private native implementation, accept/stop and read/write deadlines.
- client.rs, listener.rs, diagnostic_peer.rs and transport imports in tests: route through the common transport; preserve existing Unix descriptor access internally.
- panel-contract: native directory creation with explicit descriptor, environment contract.
- app process environment and setup guides/forwarding lists, panel-mcp and ivy-session discovery: carry server PID and capability without storing identity values.
- Windows PR job and control integration tests: execute native tests, not merely cross-check compilation.

## Data model / schema changes
None. No tables, columns, indexes, constraints or data copies.
### Access paths
N/A — no database access.

## External contracts
Existing pipe address: `\\.\pipe\ivy-<PANEL_ID>`. The wire remains newline-delimited JSON with unchanged verbs, payloads, document ceilings and replies. No additional pane identifier or PID is sent on the wire.
Windows inherited environment adds `PANEL_SERVER_PID`: decimal nonzero PID of the app process that owns the listener. PANEL_ID and absolute PANEL_DIR remain mandatory. Generate the PID at the app's pane launch boundary; never derive the expected PID from the connected pipe. All Windows producer entry points must receive this expectation explicitly or read the inherited value; direct Producer::of must not bypass authentication. Preserve the two-slot ENV_NAMES/PanelDir::env identity arrays and Unix behavior. Add PANEL_SERVER_PID separately at the Windows app launch boundary, remove it explicitly for no-pane launches, and update every Windows forwarding list; the client reads PID separately from whole_identity. No other repository consumes a new surface.
No database, HTTP or new public protocol verb.

## Internal behavioral contracts
- Native pipe security is explicit, non-inheritable handle security with protected current-user+SYSTEM DACL. Pipe DACL: exactly two ACCESS_ALLOWED_ACE entries (one if the current SID is SYSTEM), masks FILE_ALL_ACCESS, AceFlags=0; no inherited or object/callback ACEs. Directory DACL uses the same principals/masks with OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE so child panel files stay private; directory owner is the current user, SE_DACL_PROTECTED is set, and null/absent DACLs are rejected. Validator and tests compare semantic ACE tuples, allowing order changes but no additional grant. First creation uses FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_REJECT_REMOTE_CLIENTS, FILE_FLAG_OVERLAPPED, PIPE_TYPE_BYTE and duplex access. Never rebind with insecure defaults after failure.
- Only the initial instance uses FIRST_PIPE_INSTANCE. Keep a server instance handle alive while preparing its successor so another process cannot capture the name between accepts. Apply the same security and remote rejection on every instance.
- Windows Listener/Stream implement the subset already used: accept readiness, Read/Write (including borrowed streams), receive/send timeout, and diagnostic raw-handle access. Do not wrap an overlapped handle as a synchronous stream unless the pinned API explicitly guarantees it. No runtime dependency.
- Each outstanding operation owns stable OVERLAPPED storage and an event until completion is drained. Accept handles synchronous success, ERROR_IO_PENDING and ERROR_PIPE_CONNECTED distinctly. Stop wins races; CancelIoEx is followed by completion observation before freeing storage/handles.
- Stop event is private and non-inheritable. Drop signals it; no polling loop, synthetic connection, or FlushFileBuffers waiting on a non-reading peer. Active read/write use cancellation-aware bounded waits.
- Track Windows worker completion/handles. Cancel transport waits at stop, interrupt window-answer waits, and make Windows shutdown release delivery nonblocking/cancellation-aware once receiver lifetime ends. Join outside any worker registry lock. Preserve ordinary live-pane release reliability and existing max-eight admission cap. No success reply for an operation that was cancelled before admission.
- Windows drop must return with no transport thread left within ANSWER_DEADLINE + wire::REPLY_DEADLINE + RECV_TIMEOUT (currently 10.2 seconds). Tests derive the bound from these constants and observe worker completion; no blind sleep as proof. Unix's existing nonjoining drop semantics stay unchanged.
- Create PanelDir via CreateDirectoryW with protected inheritable-to-children user+SYSTEM ACL at birth (do not create permissively then tighten). Inherited lookup checks owner SID, actual DACL and directory type through a no-follow handle; reject reparse points and unsafe grants. Prefer handle-based GetSecurityInfo to a check that follows a replacement path; any required GetNamedSecurityInfoW lookup must be paired with no-follow identity checks.
- Windows socket_present cannot use Unix metadata or connect to probe: report that the platform can offer the named transport when inherited identity/PID are complete, and let the authenticated operation give reachability errors. Adapt call signature if needed so it has ID/PID context. Do not claim a successful connection from this hint.
- Session/recovery native authority remains unverified on unsupported Windows hosts; enabling MCP transport does not enable unrelated foreground-process attestation.

## Depends on other tickets
None outstanding. Ticket #161 is already delivered on the fetched main base: `8380c4e` (Fix Ivy closing on launch on Windows), with decision D107 and Windows startup checks. It is an antecedent, not an unmet dependency; ticket_deps=[].
### Not dependencies (either order works)
Windows packaged GUI smoke automation (#162) can land separately; native transport tests belong to this delivery and GUI proof can be manual.

## Defensive mechanisms and justification
| Mechanism | Present scenario / evidence | Needed now? |
|---|---|---|
| Protected pipe DACL and remote rejection | Pipe namespace is machine-wide and default ACL grants other users reads; explicit ticket requirement. | yes |
| First instance and persistent name ownership | Named pipe name can already be occupied or reclaimed between instances. | yes |
| PID comparison | A stale pane identity may connect to a replacement server; directory permissions do not authenticate a pipe. | yes |
| Private directory creation and owner/DACL checks | panel-contract currently sets privacy only on Unix; file transport and inherited dir are shared surfaces. | yes |
| Overlapped stop/cancel/drain and worker ownership | Current Windows stubs cannot wake accept; current drop detaches workers; idle/blocked clients otherwise survive. | yes |
No new retries, caches, polling service, authentication token or runtime.

## Implementation plan (order)
1. Add target-scoped windows-sys features and private security helpers at their owning crate boundaries; private directory creation and environment forwarding.
2. Add platform Listener/Stream facade and Windows overlapped implementation, acceptor cancellation and all operation lifetimes.
3. Wire client authentication, discovery and owned Windows shutdown, keeping existing protocol and Unix behavior.
4. Add native ACL/access-check, wrong-PID, squatting, cancellation/backpressure, pane-isolation and handshake tests; execute on Windows CI and existing Unix suites.
5. Verify intended-pane rendering on Windows and document any remaining manual criterion honestly before publication.

## Planned tests
- AC1: native bind/duplicate-bind, repeated connect and parallel pane isolation; port current cfg(not(windows)) tests where transport-independent.
- AC2: GetSecurityInfo on the actual pipe; enumerate ACEs, descriptor protection and current/SYSTEM identities; AccessCheck with a synthetic valid other-user token or controlled restricted token that lacks allowed SIDs. If token construction cannot demonstrate a distinct user, retain DACL assertion and require an actual second-user native test; do not label same-user denial as proof. Assert remote-rejection mode on actual construction and a required native remote connection test on a network-capable Windows host. If remote access cannot be exercised, AC2 remains partially unproven and needs explicit manual evidence; flag inspection alone never satisfies that behavior.
- AC3: private new directory, permissive/null ACL, foreign-owner fixture where available, reparse/link substitution, regular file and missing directory.
- AC4: matching server subprocess, bogus PID, missing/nondecimal/zero/overflow PID; prove refusal precedes any request bytes. Explicit API and inherited API both covered.
- AC5: accept with no clients, connect-before-wait race, partial line, idle client, client not reading replies, saturated inbox and simultaneous drop/connect; zero alive counter and joined workers after return. Derive deadlines from production constants and use channels/events as readiness barriers.
- AC6: native panel-mcp initialize and panel_set round trip; Windows pane screenshot/manual observation proves painting and correct pane.
- AC7: cargo fmt --all --check; cargo clippy --locked --workspace --all-targets -- -D warnings; cargo test --locked --workspace; native Windows cargo test --locked -p panel-contract -p panel-control (including handshake integrations); app Windows startup checks. Run relevant integration JS tests after forwarding changes.
Study verification is source inspection only; tests above are planned, not claimed executed.

## Decisions (ADR-light)
| Decision | Discarded alternative | Why |
|---|---|---|
| Native overlapped Windows endpoint under shared facade | Blind interprocess handle conversion / runtime / AF_UNIX | Explicit creation flags and private event wait are required; Node clients use pipes; Unix remains reusable. |
| One atomic delivery | Enable BINDS first then add ACL/authentication | Every usable Windows endpoint must have access control and server verification from its first published version. |
| Explicit PID environment | Trust directory alone / trust connected server's self-report | Pipe namespace is independent of directory and wire identity is not an authentication witness. |
| Windows owned teardown | Copy detached Unix implementation | Ticket requires no surviving workers at drop; cancellable I/O and tracked ownership provide a measurable completion witness. |

## Design challenges
Independent general and security challenges completed; source reports: challenge.md and security-challenge.md.

| Angle | Finding | Type | Severity | Response |
|---|---|---|---|---|
| Lifecycle F1 | Channel callbacks run synchronously (listener.rs:675 and request handlers); Closed delivery blocks on a bounded inbox (listener.rs:744-753). Cancelling pipe I/O and joining cannot bound arbitrary callback completion. AC5's zero-workers-at-return promise is not yet justified. | missing | high | OPEN — orchestrator question design_challenge. Recommended: reopen the design to include cancellation-aware, bounded Windows request-handler completion and release delivery, then reassess size and scope before code. Alternative: owner explicitly changes AC5 to nonblocking drop with eventual completion after callbacks return. |
| Public API F2 | endpoint::bind/connect are public and expose interprocess types; a private replacement changes Rust API. | missing | medium | Make the platform Listener/Stream wrappers public wherever returned by existing public functions. Record the pre-1.0 source API change and migrate all in-repo callers/test imports in this PR; wire unchanged. |
| Environment F3 | Two-slot identity arrays and no-pane clearing are intentional fixed contracts. | missing | medium | Keep ENV_NAMES and PanelDir::env as the existing two-variable identity on all platforms. Add a separate Windows launch-boundary PANEL_SERVER_PID entry and explicit no-pane removal; client reads it separately. Update forwarding lists and tests. This supersedes proposals to expand the identity arrays. |
| Security F4 | Flag inspection alone does not prove actual remote-client denial. | missing | medium | Required network-capable Windows remote connection proof; if unavailable AC2 remains partially unproven, not passed. |
| Security F5 | ACL masks/flags were underspecified. | missing | low | Exact allowed ACE tuples and protection now specified in internal behavioral contracts. |
| ADR | No additional false dichotomy or unnecessary runtime found. | idiom | low | No change. |

Design review remains incomplete until F1 is answered. No high finding has been accepted on the owner's behalf.
