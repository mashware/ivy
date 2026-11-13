# Design 57

## What already exists (inventory)
- `crates/panel-contract/src/lib.rs:88` creates a fresh random `ivy-<id>` directory; `create_dir` at line 172 uses Unix mode `0o700`, without adopting an existing name.
- `crates/panel-control/src/endpoint.rs:182` provides the low-level socket connector, and already depends on Unix libc through the crate manifest.
- `client.rs:66` validates inherited identity and platform support, then calls `Producer::of`; the MCP tool opens this producer lazily (`bin/panel-mcp.rs:736`).
- `session/helper.rs:70` builds its own bounded ConnectOptions exchange. `bin/ivy-session.rs:17` currently discards its error text.
- `resources/agent-integrations/opencode/ivy-opencode.tsx:28` has the direct socket connect and 1000 ms reconnect timer, plus cleanup that stops retries. The similarly named session-recovery adapter calls ivy-session and is covered through the helper.
- Tests: endpoint unit tests; bridge subprocess fixture `tests/panel_mcp_handshake.rs`; helper subprocess fixture `tests/session_helper.rs`; direct plugin fixture `resources/agent-integrations/opencode/plugin.test.mjs` with simulated and real Unix socket modes.

## Approaches considered
Parallel approach panel, 3/3 lenses requested, models.workers=gpt-6-luna from FLOW.codex.md. Inventory precedes proposals. See approaches/ for individual reports.
1. Shared private-directory guard at inherited entry points (minimum, recommended). Reuse endpoint.rs and both clients, mirror in JS. Small concrete fix; residual pathname replacement race remains.
2. Same guard with explicit diagnostics and reconnect lifecycle verification (operations). Incorporate these details in option 1; a separate transport/retry abstraction adds no value.
3. Enlarge random identifiers (reframe). Changes identity generation and consumers yet does not authenticate a known or observed pathname; rejected because learned stale paths remain usable.
Panel agreement: identity entropy does not replace ownership validation. Difference: prevention through authentication of directory metadata versus obscurity of its name. Select the guard because it directly rejects the reported planted directory without changing identity/wire formats.

## Additional domain context
No knowledge tool configured. D7 documents directory privacy as the boundary; endpoint.rs currently claims no supplementary access control is needed. Both must describe stale identity validation after this change. D88's whole-identity handshake behavior remains intact.

## Executive summary
- Validate the inherited directory before either Rust client opens its channel.
- Require a real directory of the effective uid with zero group/other permission bits; do not follow a final symlink.
- Preserve a named refusal through MCP and ivy-session; missing paths remain distinguishable from insecure existing paths.
- Repeat the equivalent check before every direct OpenCode connection attempt, using its existing one-second retry cadence and cleanup.
- Deliver one PR with caller-level refusal proofs, positive controls, and updated security documentation.

## Acceptance criteria
| ID | Given / When / Then | Proof |
|---|---|---|
| AC1 | Given a genuine PanelDir and a listening channel, bridge, helper and direct OpenCode plugin connect and exchange their existing frames successfully. | test |
| AC2 | Given any group/other mode bit (including 0755), bridge and helper refuse before connecting. Refusal contains the actual directory and `not a private directory of this user`. | test |
| AC3 | Given metadata whose uid differs from the expected effective uid, the private-directory check refuses; inject the expected uid in tests without chown/root. Mirror this decision in the JS checker fixture. | test |
| AC4 | Given PANEL_DIR naming a symlink to a genuine private directory, clients refuse before connecting. | test |
| AC5 | Given PANEL_DIR absent from the filesystem, clients fail closed. Rust refusals name the directory and say it is gone, without `not a private directory`. | test |
| AC6 | Given invalid PANEL_DIR, the direct plugin opens no socket, schedules its next attempt at 1000 ms, and checks metadata afresh. Repairing the directory allows a new connection; disposal cancels further attempts. | test |

## Modules/layers affected
- panel-control endpoint, inherited producer and session helper; ivy-session error presentation.
- Direct OpenCode TUI asset and its executable fixture (not the helper-based recovery reporter).
- docs/02-decisions.md D7 and docs/03-architecture.md Security; endpoint module's outdated access-control explanation.

## Data model
No entities, schema objects, migrations, indexes, persisted state, or wire changes.
### Access paths
N/A — no database access. One lstat per inherited connection attempt, not per frame.

## External contracts
### Refusal text
Existing JSON-RPC tool result shape is unchanged: `result.isError = true`, `result.content[0].type = "text"`; text contains the original pathname.
Existing ivy-session shape is unchanged: `{"ok":false,"error":"helper_error","message":"<detail>"}`, exit 1, stderr prefix `ivy-session: `. Keep its existing `outside_ivy` response for absent identity.
For an existing insecure path, detail contains: `<dir> is not a private directory of this user`.
For a missing path, detail contains: `this console's panel could not be reached at <dir>` and `directory is gone`; never mislabel it as an insecure directory.
Other metadata failures preserve path and system cause and fail closed. No raw metadata failure falls through to connect.
Consumers: MCP tool callers, helper adapters, and subprocess test fixtures; no sibling-repo contract.
### Direct OpenCode retry
`setTimeout(connect, 1000)` after a metadata refusal; `createConnection(...)` is called only after success. A refused directory must not trigger the connected callback or send cached frames. `close()` cancels retry and prevents further attempts.

## Internal behavioral contracts
`private_to_this_user(dir: &Path) -> io::Result<()>`:
- Unix: lstat via symlink_metadata, require `is_dir && uid == geteuid() && (mode & 0o077) == 0`.
- Final symlinks and ordinary files fail even if owner/mode are otherwise acceptable. For the metadata lookup, lexically remove terminal slash and terminal `/.` components (repeat as needed, preserve root and every `..` component), so `alias`, `alias/` and `alias/.` all inspect the alias itself. Do not canonicalize or collapse `..`, which could change traversal through an ancestor symlink. Keep the original path for diagnostics and the eventual connect. Rust and JS use the same spelling rule.
- Mode test follows the ticket's explicit mask; do not impose exact owner bits, a root exception, or special-bit policy. Genuine creation is 0700.
- Inject expected uid through a private testable helper while the public entry obtains the real effective uid; production callers cannot supply an override.
- Windows: no-op checker; Producer's existing BINDS rejection and plugin's Windows exclusion stay in force.
- Call after existing identity/platform checks and before any connect in Producer::of_this_pane and session::exchange. Keep generic Producer::of and endpoint::connect behavior.
- JS: `lstatSync`, `stat.isDirectory()`, `stat.uid === process.geteuid()`, `(stat.mode & 0o077) === 0`. Catch stat failures, suppress connection and schedule retry. Pass the inherited directory to channel (or derive it without changing socket name).
- Preserve helper's exchange failure text instead of replacing it. Retain the bridge's existing `could not be reached at` context for stale paths.

## Depends on other tickets
None. Best-effort `git fetch origin main` succeeded; main and origin/main resolve to acdc474dc4a2702f68c17c554c2ead300161e63b and contain the inspected code.
### Not dependencies (either order works)
#55 is background: current base already contains absolute runtime-directory fallback and D88 whole-identity behavior. This work does not require a pending change there.

## Defensive mechanisms and their justification
| Mechanism | Real scenario / evidence | Needed now? |
|---|---|---|
| lstat owner, type, permission guard | #57: another uid creates the directory/socket after a console closes; inherited clients currently connect without authenticating that path. | yes |
| Recheck on reconnect | Direct plugin retains its inherited path after socket close; current connect retries every second. A once-only startup check becomes stale. | yes |
| Preserve underlying refusal | ivy-session.rs discards exchange errors, preventing users from identifying the rejected directory. | yes |
No new retry policy: reuse the existing reconnect timer and disposal controls.

## Identified risks
- Files written by arbitrary external scripts into PANEL_DIR still depend on those producers; the app cannot enforce their checks. Explicit ticket limitation, not a new deliverable.
- Path validation and socket connect are separate; no claim of race-free peer authentication. Shared temporary parent assumptions still apply; a malicious writable non-sticky ancestor can undermine pathname checks. Peer-uid authentication and ancestor hardening are outside this study's requested scope.
- Final symlink spelling must not be normalized into its target by implementation. Test the inherited spelling, including terminal slash/dot forms; the metadata lookup rule above mitigates that bypass. See path-spelling-evidence.md for the confirmed Linux mechanism.
- `sudo -E` with inherited identity is rejected on effective-uid mismatch; directory is named. No privileged exception.
- Linux tests can exercise uid mismatches with injection; real different-user/chown and macOS/Windows runtime execution require appropriate hosts. Windows channel availability does not change.
- Failure to stat is a refusal, not permission to fall back to an unchecked connection or file write.
- No feature flag or migration; rollback reverts this PR, restoring prior behavior and its security gap.

## Implementation plan (order)
1. Shared Unix checker and focused tests with positive PanelDir, mode-bit matrix, owner mismatch injection, symlink, non-directory, absent path and failed stat.
2. Wire bridge and helper before connect, preserve helper diagnostics, add subprocess refusal and success controls against listeners.
3. Mirror checker in direct plugin's connect; reuse timer and disposal; extend executable fixture with no-connect and repair/retry proofs.
4. Update D7, architecture security table and endpoint explanation, then review and validate the one coherent PR.

## Planned tests
- AC1: real PanelDir positive control for both Rust binaries and direct plugin real-socket mode.
- AC2/AC4: listening endpoint behind each unsafe directory; run child client to completion and non-blocking accept must report no pending connection. Assert exact refusal substrings and original path. Restore fixture permissions for cleanup. No sleeps to infer no connection.
- AC3: checker with actual metadata and a deliberately different expected uid (choose without integer overflow); JS isolated checker/metadata fixture with a mismatched uid. Never mutate process uid or global Rust environment in parallel tests.
- AC4 spelling cases: symlink `alias`, `alias/`, `alias/.`, and repeated terminal slash/dot components all refuse; a genuine directory with equivalent trailing components passes. Do not normalize interior parent traversal.
- D88 contract: a whole identity with an insecure existing directory still gets initialize instructions, while its first tool request refuses without connecting. Initialization alone never opens a socket.
- AC5: remove/omit directory and assert named gone error with no privacy refusal; ordinary file and unreadable metadata must also fail closed.
- AC6: JS controlled timers and metadata reads, recording createConnection count: blocked attempt -> 1000 ms -> blocked again -> repair -> fresh check -> successful connect. Dispose during refusal -> no future attempts. Include symlink, unsafe mode, uid mismatch and missing path.
- Run focused `cargo test --locked -p panel-control`, both documented plugin modes (`node --test resources/agent-integrations/opencode/plugin.test.mjs` and `IVY_REAL_SOCKET=1 node --test resources/agent-integrations/opencode/plugin.test.mjs`), plus `npm test --prefix integrations/session-recovery` for helper consumers.
- Final checks: `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `cargo doc-check --locked` as current forge workflow specifies. Tests are planned, not executed during this study.

## Decisions (ADR-light)
| Decision | Discarded alternative | Why |
|---|---|---|
| Guard inherited doors | Guard every endpoint::connect / enlarge identifiers | Explicit callers manage known directories; the ticket concerns inherited stale identities. Longer names do not authenticate observed paths. |
| lstat and effective uid | stat following final symlink / real uid / root exception | Final symlink is explicitly refused; an effective uid is the identity used for filesystem access and must match JS. |
| Check on connect attempts | Stat every frame / cache result at startup | Existing connections do not re-resolve path; a new connection does, so its metadata must be fresh. |
| Keep D88 handshake identity-only | Stat during initialize / move helper to whole_identity | The socket opens on the first tool request; the handshake deliberately promises only a supported whole identity. Scope is directory validation. |
| One atomic PR | Rust and JS as separate delivery waves | Same security invariant crosses both implementations; splitting prolongs partial protection without independent ticket value. |

## Design challenges
Independent design and security challengers completed; no unresolved high findings.

| Angle | Finding | Type | Severity | Response |
|---|---|---|---|---|
| Handshake | Pair initialize behavior for insecure directory with first-call refusal. | missing | medium | Incorporated explicit D88 regression assertion; validation stays at the lazy connection boundary. |
| Symlink spelling | Raw lstat(alias/) and lstat(alias/.) can follow final symlink. | missing | medium | Confirmed locally (path-spelling-evidence.md); metadata lookup strips only terminal slash/dot components, preserves root/.. and original diagnostics, with Rust/JS fixtures. |
| Predicate/no-connect/retry/scope | Existing predicate and listener/socket-count proofs are consistent; no extra mechanism needed. | none | low | Preserve current explicit scope and cleanup; no new abstraction or retry policy. |

Architecture and security design proposals agree on shared checker placement, helper error propagation and per-attempt JS validation. General-purpose role fallback used because no architecture/security agent is configured; models.agents=gpt-6-luna from FLOW.codex.md. Approach panel 3/3 returned; design agents 2/2; challengers 2/2. Total design launches 8 including inventory, within default budget 12 and fanout 4, actual simultaneous children at most 3. Each round used a 5-minute deadline.

Design review recorded as accepted for study routing (unattended); this does not approve implementation. Size M remains appropriate for a Rust/JS security change. No cross-repo work or schema change. Knowledge remains in this artifact (no configured knowledge roles).
