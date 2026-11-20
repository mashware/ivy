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
Choice recorded (unattended); revised size L: the approved F1 response requires a cross-module Windows service and UI-wake boundary as well as native I/O. Estimates are advisory.

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
| AC5 | Dropping a Windows channel stops pending accept and active I/O and leaves zero transport workers when it returns, within ANSWER_DEADLINE + wire::REPLY_DEADLINE + RECV_TIMEOUT (currently 10.2 seconds); a saturated inbox cannot retain a release sender, and no arbitrary callback runs on a Windows channel worker. | test |
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
- Track Windows worker completion/handles, including acceptor refusals and spawn failure paths. Stop admissions before snapshotting the registry; join acceptor first, then all registered connection handles outside registry locks. Cancel transport waits at stop; window-answer waits retain the finite ANSWER_DEADLINE (no need for a new answer-wait event). Windows workers use only the concrete bounded service operations below. Preserve max-eight active connections and release reliability. No success reply for an operation cancelled before admission.
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
3. Add the callback-free Windows service capability, committed document/plan snapshots, concrete native UI wake and independent release slots described below; migrate Windows app and test constructors. Wire client authentication, discovery and owned Windows shutdown.
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
| Lifecycle F1 | Channel callbacks run synchronously (listener.rs:675 and request handlers); Closed delivery blocks on a bounded inbox (listener.rs:744-753). Cancelling pipe I/O and joining cannot bound arbitrary callback completion. AC5's zero-workers-at-return promise is not yet justified. | missing | high | Answered by the orchestrator: Apply the recommended response to each (id recorded below). Reopened lifecycle design: Windows callback-free service capabilities, nonblocking UI wake, independent release slots and joined owned workers as specified below. Reclassified L. The no-worker-at-return contract is preserved; no unsafe thread termination or detached callback executor. |
| Public API F2 | endpoint::bind/connect are public and expose interprocess types; a private replacement changes Rust API. | missing | medium | Make the platform Listener/Stream wrappers public wherever returned by existing public functions. Record the pre-1.0 source API change and migrate all in-repo callers/test imports in this PR; wire unchanged. |
| Environment F3 | Two-slot identity arrays and no-pane clearing are intentional fixed contracts. | missing | medium | Keep ENV_NAMES and PanelDir::env as the existing two-variable identity on all platforms. Add a separate Windows launch-boundary PANEL_SERVER_PID entry and explicit no-pane removal; client reads it separately. Update forwarding lists and tests. This supersedes proposals to expand the identity arrays. |
| Security F4 | Flag inspection alone does not prove actual remote-client denial. | missing | medium | Required network-capable Windows remote connection proof; if unavailable AC2 remains partially unproven, not passed. |
| Security F5 | ACL masks/flags were underspecified. | missing | low | Exact allowed ACE tuples and protection now specified in internal behavioral contracts. |
| ADR | No additional false dichotomy or unnecessary runtime found. | idiom | low | No change. |

F1 was answered by the orchestrator; its implementation is specified in the lifecycle revision below. Independent revision challenge completed: lifecycle-final-challenge.md reports no high blocker; revised-security-audit.md reports no actionable high/medium security finding.

## Answer applied to design challenge

ID: 164-windows-private-control-pipe:design:design_challenge:137fd34:ec05b26

```text
Orchestrator answer: Apply the recommended response to each
```

F1 response: Reopen the lifecycle design to include bounded, cancellation-aware Windows handler completion and release delivery. Preserve zero workers at return; reassess scope and size before code. This answer decides F1 only.

## Lifecycle revision implementing the answered F1

This section replaces callback-based Windows service wiring in the initial draft. It preserves the original AC5. Requiring callback authors to promise cancellation is insufficient: a closure can ignore its token, and putting that closure on another thread leaves a worker behind. Windows channel-owned threads will execute no supplied closures, trait plugins or user-owned destructors. Unix callback APIs and behavior remain as today.

### Concrete Windows service boundary

- Add a public Windows-only constructor `Channel::open_windows(dir, id, inbox, services: WindowsServices)`. `WindowsServices` is a concrete final struct defined by panel-control, with private fields: document snapshot reader, plan snapshot reader, native wake capability and release-slot registry. Expose concrete constructors/publishers, no generic callbacks or implementation traits. Legacy callback-bearing constructors remain Unix APIs; Windows call sites migrate explicitly rather than compiling and ignoring services. The public Rust API is pre-1.0; record the Windows source API change in release notes.
- Snapshots contain only owned bounded strings and primitive status values, never captured app objects. Cells contain `Mutex<Snapshot>` and an atomic alive flag; workers check stop/alive, use `try_lock`, clone the bounded value and release immediately. Contention returns the existing `Reply::Failed` shape with message `this pane's snapshot is busy; try again`; a closed pane returns `this pane is gone`. Poisoned state fails closed, never blocks. No spin, retry loop or sleeping on a cell. Counters used by the app are atomic, not arbitrary callbacks or `ReadActivity`'s blocking mutex.
- The UI publishes canonical serialized documents at every existing `Committed::replace`/`clear` boundary in `app/src/panel/document.rs`; Windows reads do not acquire app locks or serialize a retained Panel graph. Retirement publishes the existing nothing-published error, later commit updates the same capability, pane close revokes it before dropping the channel. The plan publisher updates at bank commit/retire and project identity changes in `app/src/board/plans.rs`; project unknown and no-plan responses retain existing text. Readers keep working off-frame while minimized. Use panel-contract's existing document/plan ceilings; reject oversized snapshot publication rather than retain unbounded strings.
- Native session and overlay authorization remain unsupported on Windows. Return existing disabled/unauthorized responses before executing session/publisher/prepare-holder callbacks. Publication origin is the constant Unknown. `ai_sessions::callback` already returns unsupported outside Linux; no Windows trust mechanism is added by this work. Ordinary panel_set/patch/retire, signals, plan mutations and splits retain the existing UI delivery path. No new worker is created for callbacks.

### UI wake is also part of the boundary

The app supplies a concrete revocable HWND capability obtained on the UI thread from its native window handle at startup (`app/src/opening.rs` CreationContext). The Windows channel posts `WM_PAINT` with zero pointer arguments using `PostMessageW`, so the worker never calls egui's arbitrary repaint closure. The HWND remains owned by the UI; close all channels before destroying it. A post failure is diagnosed, never spun on or retried via `SendMessageW`. If a delivery was already admitted, report failure/uncertain application honestly: do not claim that it was rolled back, and retain its release-slot accounting until UI completion or pane destruction. A queued paint after pane closure carries no pane pointer and cannot dereference freed state. UI/native tests must prove a publication wakes a resting window and reaches the normal control drain. Native test constructors may use a real test message window with a paused UI thread to prove posts do not await dispatch.

Evidence: [PostMessageW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postmessagew) documents that posting returns without waiting for processing. [winit Window::request_redraw](https://docs.rs/winit/latest/winit/window/struct.Window.html#method.request_redraw) documents WM_PAINT -> RedrawRequested on Windows. Integrating that redraw into eframe's normal control drain is a planned native integration test, not an already proven fact. If the pinned eframe/winit pair filters a posted paint, implement a UI-owned native message hook that calls request_repaint on the UI thread; the channel-facing operation remains the same nonblocking post with no user pointer. The HWND lifecycle and wake failure test are mandatory in either implementation.

### Close delivery cannot wait behind publication traffic

Do not send Windows `What::Closed` through the full `SyncSender`. Use a fixed-capacity registry of release slots, each holding atomic connection id, closed state and outstanding-delivery count. Reserve a slot before accepting a connection for service; a slot is not reusable until its closed state is consumed by the UI AND all admitted deliveries (including deferred split operations) are completed/discarded. The capacity is a fixed constructor constant at least MAX_CONNECTIONS, independent of client churn; exhaustion returns the existing busy/refusal response without admitting a holder. Each connection admits at most one outstanding UI request; an answer timeout does not allow another queued mutation until the prior delivery completes. This bounds retained release state even if the UI is stopped indefinitely.

Increment pending before a successful queue admission and undo it on failure; the UI decrements after applying/discarding a request, including deferred window work. Worker exit marks closed with release ordering and posts the native wake. The UI reads closed with acquire ordering after draining eligible deliveries; it applies the same holder/signal/owned-split cleanup as What::Closed only after pending reaches zero. Thus a close cannot overtake its accepted publication or be forgotten when the inbox is full. On pane destruction, revoke the registry: workers mark closure but need no UI acknowledgement, because the pane and its holds are being destroyed. Slot state is concrete owned atomics; no worker waits for a mutex, free slot or frame. Registry synchronization and reuse need dedicated interleaving tests.

### Shutdown sequence and proof

1. Revoke snapshots/registry and prevent new admissions; set shared stop and signal the private stop event.
2. All pending native operations observe stop, CancelIoEx and drain completion before OVERLAPPED storage/handle destruction. Acceptor's busy refusal writes share the same stop-aware I/O implementation. Finite transient-error waits must be stop-event waits rather than uninterruptible new callbacks.
3. Join acceptor so no later worker can enter the registry, then join all connection handles outside the registry lock. Worker cleanup touches only its bounded concrete resources; no arbitrary captured destructor can block shutdown. Panic completion uses a guard to maintain the alive witness and still joins.
4. Return only with zero channel-owned threads and zero in-flight OVERLAPPED operations. Tests compare against the aggregate 10.2-second envelope, never multiply the bound by eight workers; the stop is broadcast concurrently. This is an ordinary scheduling/deadline guarantee, not a hard-real-time OS promise.

Required F1 tests (AC5/AC7): a deliberately held snapshot lock yields busy and drop joins; an undrained native UI message queue cannot retain workers; a full live inbox retains exact close state until its prior publication is processed; repeated connect/disconnect cannot grow release memory; timeout then late UI application cannot reacquire a released hold; deferred split completion and slot reuse remain ordered; stop racing worker registration, panic and spawn failure leaves zero workers. All request variants and callbacks are audited by name, including inspector, wake, plan reader and prepare_holder. Existing Unix tests remain unchanged in meaning. Native off-frame tests prove committed reads while UI dispatch is stopped. No existing callback is silently ignored by a constructor.

### Size and review decision

Reclassified M -> L (recorded unattended): native secure transport plus a Windows service API, UI wake, snapshot publication and release lifecycle span multiple app modules. The route remains design -> plan -> build -> review -> validate -> ship, with full review appropriate to the actual diff. Estimate ~2,400 added/modified lines across ~32 files, advisory only. No DB/schema objects and no sibling repository. Scope expansion is the lifecycle work explicitly requested by the answered F1, not unrelated Windows porting. Design review is recorded accepted only after the independent revision challenge closes; no acceptance test has run during this study.

## Design review close
Accepted as a study design (recorded unattended), not implementation approval. F1 is resolved by the authorized lifecycle revision. Independent lifecycle and security reviewers found no remaining high blocker. AC1–AC7 remain the implementation validation gates; native wake integration is explicitly unproven until tested. No source code changed.
