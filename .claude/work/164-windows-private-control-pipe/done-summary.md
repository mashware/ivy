<!-- flow-study -->
## What and why
Enable Ivy's Windows control channel so agents can publish panels and use MCP tools through a user-private named pipe. The answered shutdown challenge keeps bounded teardown with zero workers remaining and adds the service lifecycle changes needed to make that requirement credible. Size revised M -> L; the wider lifecycle is part of this ticket.

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

## Design in brief
Use native Windows overlapped named pipes with a protected current-user/SYSTEM DACL, remote-client rejection, first-instance ownership, private stop event and cancellation/completion draining. Verify the inherited app server PID before sending any request; create pane directories privately and validate inherited owner/DACL through a no-follow handle.

Windows workers use concrete bounded document/plan snapshots, a nonblocking native UI wake and fixed-capacity connection-release slots. They execute no arbitrary app callbacks. The UI publishes snapshots at commit/retire/project transitions; reads remain available while minimized. Snapshot contention yields an honest busy error. Native session/overlay authorization remains unsupported. Drop stops admissions and joins every owned worker after native operation completion; accepted mutations retain close ordering under inbox saturation.

The Windows Rust constructor/API changes and app snapshot/wake/close wiring ship together. Existing newline JSON and two-variable pane identity contract remain, with a separate Windows PANEL_SERVER_PID launch variable. Unix retains its current callbacks and teardown behavior.

No outstanding ticket dependencies: #161 is already on origin/main at 8380c4e (ancestor check passed at HEAD 137fd348165866dcd3382608a811d0c594119f42).

## MR/PR plan
Wave 1: PR #1 — Enable the private Windows control channel. One atomic delivery includes secure pipe/directory creation, PID forwarding/authentication, Windows service API, snapshots, native UI wake, close accounting, owned worker shutdown, MCP discovery, tests and native Windows CI. No earlier PR is required. Advisory estimate: ~2,400 added/modified lines across ~32 files; estimates do not constrain correct implementation.

## Schema changes
none

## Risks
- Native Windows ACL/access checks, other-user denial, remote-client rejection and cancellation races need execution on Windows. Flag inspection or cross-compilation is insufficient proof.
- Native UI wake must prove that the pinned eframe 0.36.1/winit 0.30.13 pair drains control traffic when resting; the concrete native-message hook fallback remains callback-free on worker threads.
- Release-slot reuse, deferred UI requests, snapshot retirement and concurrent stop/registration require explicit interleaving tests.
- Correct-pane painting requires native observation; retain manual proof status honestly if unavailable. Native tests and GUI evidence are planned, not executed during this study.
- Study validation was source inspection, dependency ancestry verification and independent lifecycle/planning/security review. No production code, executable tests or PR publication occurred.
<!-- end flow-study -->
