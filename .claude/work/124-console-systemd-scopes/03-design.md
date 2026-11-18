# Design 124

## What already exists (inventory)
- `crates/term-core/src/lib.rs:512`: `Pty::spawn_with_env_in` owns PTY setup, cwd validation, environment overrides and direct child lifetime. `exit_success` (455) reads a confirmed process status; `Ending` describes the reader, not the child.
- `crates/term-core/src/lib.rs:1064`: process-group hangup, grace and eventual kill already implement bounded close. Preserve them.
- `crates/app/src/opening.rs:598`: common normal/restored program launch, including fallback shell, shell integration and panel identity.
- `crates/app/src/resume.rs:240`: takeover starts a replacement shell separately; it also needs scope handling.
- `crates/app/src/pty_output.rs:360`: `Silences` and `QuietConsole` carry reader problems to notices. Existing ordinary EOF must stay distinct from process termination.
- `crates/app/src/main.rs`: tiny executable entry point, suitable for dispatching an internal launcher before GUI initialization.
- `crates/app/tests/{shape,prose,glossary,scripts,waits}.rs`: source census, module ceilings and bounded-wait conventions. Update applicable inventories when adding modules, without raising existing ceilings.
- Existing dependencies include portable-pty 0.9 and Unix libc, but no D-Bus client. `term-core/tests/api_without_deps.rs` requires dependency-free public types.

## Additional domain context
The local term-core knowledge page says backend types stay behind the facade so a future GUI replacement does not inherit them; confirmed against the existing API.
Exact portable-pty 0.9.0 source was downloaded from crates.io into `/tmp/ivy-124-portable-pty/`: its command builder exposes no pre-exec callback and its Unix spawn closes descriptors above stderr. Its public status preserves a signal description. Upstream main was consulted first, then the locked version verified.

Sources checked on 2026-11-18:
- [systemd scope semantics](https://www.freedesktop.org/software/systemd/man/latest/systemd.scope.html): scopes group externally spawned processes; scope OOMPolicy was added in 253. Scope exit result is not a substitute for collecting child exit status.
- [systemd scope implementation](https://github.com/systemd/systemd/blob/main/src/core/scope.c): managed OOM notification produces `oom-kill` when OOMPolicy is not continue. This covers systemd-oomd, unlike kernel-only memory.events counters.
- [systemd manager API](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.systemd1.html): StartTransientUnit, JobRemoved, RefUnit, UnrefUnit and ResetFailedUnit support creation and lifecycle observation.
- [VTE spawn contract](https://gnome.pages.gitlab.gnome.org/vte/gtk3/method.Pty.spawn_with_fds_async.html): Linux child scopes are optional by default; this supports the ticket's compatibility requirement.

## Approaches considered
1. **Gated launcher, user-manager D-Bus (chosen).** A fresh helper executable waits before exec; Ivy places its PID in a scope and then releases it. Touches term-core, app startup and spawn callers. Main cost: internal startup IPC and a Linux-only D-Bus client. It preserves descendant placement, requested program identity and one direct child.
2. **systemd-run --user --scope wrapper.** Reuses the installed CLI and handles placement before execution. Touches command building and result observation. Its startup failure cannot safely trigger an unconditional second launch: the program might already have run. Extra wrapper behavior complicates terminal/process identity and exact spawn-error fallback. Not selected.
3. **Attach after spawn, or only adjust OOM scores.** Smallest code change, but descendants may fork before placement; OOM score changes do not separate oomd cgroup victims. Rejected because neither fulfills isolation.
Panel consensus / disagreement: both advisors identify placement before user code and truthful attribution as essential; minimum favors the gate, reframe favors CLI reuse. Ranked above on preservation of the existing launch contract. Their answers are in `approaches/`.
Choice recorded (unattended): approach 1. Size M remains appropriate; estimates never constrain correct implementation.

## Executive summary
- Give Linux Ivy console programs and their descendants their own sibling scope in the user manager's app.slice.
- Launch a small internal helper through the existing PTY, place it before any user code executes, then exec the original program in place.
- Scope setup is optional and bounded; failure runs the same program once without isolation.
- Keep process exit, reader EOF and confirmed OOM facts separate. Show a confirmed memory-pressure notice using scope Result=oom-kill.
- Leave non-Linux launching untouched; cover normal, restored, fallback and takeover launches.

## Acceptance criteria
| ID | Observable behavior | Proof |
|----|---------------------|-------|
| AC1 | With a Linux user manager, two console roots and their immediately forked descendants have different `ivy-console-*.scope` cgroups in app.slice; neither scope contains the Ivy GUI PID. | Linux integration |
| AC2 | No user manager, denied creation, unsupported optional OOM properties or a timed-out setup still execute the requested program exactly once with the original cwd, environment and argv. A successful basic scope is retained when only OOM attribution is unsupported. | injected failures + Linux integration |
| AC3 | A scope observed with `Result=oom-kill` produces `This console was stopped because of memory pressure.` in its console notice, retains existing scrollback, and leaves another console and the GUI alive under normal leaf-victim policy. | state/UI tests + isolated Linux integration |
| AC4 | A manually signaled child with no OOM evidence is never labeled as memory pressure. A normal `exit 0` remains an ordinary ending. Missing/late/unavailable scope evidence stays unknown, with a signal notice when the wait status supplies one. | status/notice tests |
| AC5 | Normal launch, restored launch, launch fallback and takeover all use isolation; failed exec still reaches the existing spawn-error/fallback path. PTY controlling terminal, resize, input and process-group close semantics survive. Several simultaneous stalled-manager startups do not block GUI event processing; closing a pending launch cannot resurrect its console. | actual executable integration |
| AC6 | macOS and Windows use today's direct launch, require no systemd library or executable, and compile without Linux backend dependencies. | cross-platform checks |

## Modules/layers affected
- New small Linux modules in term-core: launch gate/IPC, user-manager scope backend and cached termination state. Keep backend types private.
- term-core facade: additive launcher option and a bespoke read-only process termination snapshot; preserve existing spawn methods for consumers that do not supply a launcher, and preserve exit_success behavior.
- App entry point: recognize exact private launcher mode before app::run, dispatch to a term-core helper routine, then exit. It must never load GUI/session/settings/panel state in that mode.
- opening.rs and resume.rs, plus an app launch-completion module: make the Linux isolation path asynchronous and pass an explicit supported launcher path and console identity through one shared launch adapter. Do not infer helper support from the current test binary; tests opt in using the real Ivy executable.
- pty_output.rs and existing console notice/auto-close integration: merge termination snapshot into notice selection; preserve the existing producer-owned status panel.
- Linux target Cargo dependency and lockfile; module/wait inventories and focused docs where behavior changes. No new distributed executable or packaging location is required because Ivy re-execs itself.

## Data model
No persisted state, database objects, migrations or indexes.
Access paths: N/A — no database access.

## External contracts
No new public HTTP/MCP/configuration or on-disk contract. No cross-repository consumers.
The only external dependency is the session user's systemd manager over D-Bus; never use the system manager or elevation.

## Internal behavioral contracts
### Startup ordering
The normal PTY setup execs the installed Ivy executable in private launcher mode, with original OS-string argv. Cwd and the final environment are already inherited. The helper connects to a fresh private Unix socket and waits; it cannot fork user work before release. The socket is opened by the helper after its first exec, so portable-pty's fd cleanup cannot close it.
A parent-owned scope controller uses `StartTransientUnit(name, "fail", properties, [])`, with `PIDs=[helper_pid]`, `Slice="app.slice"`, and a human-readable description containing window/console identity. A process-specific random namespace plus a sequence makes `ivy-console-<namespace>-<sequence>.scope` unique across windows and restarts. No program arguments or secrets go into names.
Subscribe before creation; wait for the matching job completion and confirm placement before release. Retain a reference while the helper is alive, before it may exec, so the unit's final result survives early exit. If `OOMPolicy="kill"` is supported, set it to retain an explicit OOM failure result. If that exact optional property is rejected as unknown, create the basic scope without it; do not drop isolation solely for old systemd. Other failures release unscoped after the attempted job is settled/cancelled. No late background placement after fallback release.
All bus operations execute in an owned worker with a finite operation deadline; helper gate and parent handshake share a finite startup deadline (initial target 1 second). Socket reads/writes, job wait, connection and cleanup must all obey it, not just the final reply wait. The current app spawn calls are synchronous, so the Linux isolated path must move the entire launch/handshake into an app launch worker. Reserve console identity/order while pending, wake on completion, install the completed Pane on the UI thread, and route exec errors through the same existing fallback or failure behavior. A closed/cancelled pending pane must drop any late-created child through normal shutdown rather than resurrect it. Non-Linux launch stays direct. Never join the startup worker or wait for the manager in a frame; opening several restored consoles must not multiply UI stalls.
Release means the helper calls exec directly, without a shell. Successful exec closes a CLOEXEC control connection; an exec failure writes the OS error and exits. EOF is only success following the expected release/exec stage, never before handshake. Keep this failure visible to the original fallback behavior; never execute the original twice after a scope failure. Unexpected helper death is a launch failure. Missing helper means use the direct path; a failing supported helper must not silently turn an unknown partial launch into a second execution.

### IPC and lifetime
Use a private 0700 temporary/runtime directory and a Unix stream, not inherited fd numbers. The owned Child PID is the scope PID; there is no separate same-UID authentication mechanism. Send a bounded versioned control exchange; v1 control values are `ready`, `exec`, and `exec-error` plus an OS error number. Original program/args are passed as argv entries, never joined shell text. Remove the private socket/directory on every terminal startup path. CLOEXEC and channel closure prevent a control descriptor leaking into the requested program.
Helper PID remains the actual program PID after exec, preserving session leader, controlling tty and process-group cleanup. Retain the exact original CommandBuilder resolution contract, including PATH overrides and relative executables; regression-test failed exec and cwd behavior. D-Bus work must never occur between fork and exec in the multithreaded GUI.
Existing HUP/grace/KILL remains authoritative on console close. Release references and stop observation on drop without waiting for the bus on the UI thread; reset only this feature's own failed unit after preserving its result. Active scopes remain manager-owned until their remaining processes finish. Do not stop arbitrary units or alter global OOM settings.

### Exit and notices
Cache the direct child's collected status separately from terminal-reader Ending. Preserve existing repeated exit_success callers in resume/restoration. A live scope observer retains explicit `org.freedesktop.systemd1.Scope.Result == "oom-kill"`; hold the unit reference until classification is captured or observation is conclusively unavailable. A one-off read immediately after waitpid is insufficient because manager notification can arrive later.
On EOF, drain pending output normally. Pending classification requests a repaint when it resolves; do not let ordinary-EOF auto-close hide a console awaiting an OOM decision. A 500 ms observation window starts at confirmed root exit and finishes with unknown if the manager disappears or never provides evidence, even if descendants keep the scope active. Positive oom-kill may complete it early. Release the retained reference after classification; the active scope itself can outlive the root. Unknown plus a signaled wait status may say `This console was terminated by a signal.`; never infer OOM from code 137, a translated signal string, memory usage or EOF. An OOM notice uses the existing dismissible console-notice surface, does not overwrite the producer panel and does not discard scrollback. A retained OOM outcome prevents automatic window closure until the console is handled like an existing abnormal ending.
Older systemd may provide isolation without reliable managed-OOM attribution: document this explicitly instead of claiming every SIGKILL was oomd. Kernel memory.events may supplement evidence only if actually observed for this console; it is not the primary managed-OOM proof.

## Identified risks
- Startup gate or connection failure can hang a pane unless every stage shares a real deadline; tests must stall the actual boundary.
- Placing only the root after user exec would leak immediate descendants; integration must fork before emitting its ready marker.
- Failed unit GC or disconnect races can erase attribution; capture Result before releasing the parent reference and test delayed notification.
- Manager versions before scope OOMPolicy support can isolate but cannot promise the same notice accuracy.
- Ancestor policy and global kernel OOM can still kill larger groups; separate leaf scopes improve victim granularity, not immunity from every memory policy.
- CLI fallback, login argv, PATH resolution and restored-shell takeover are compatibility traps; exercise real launch paths.
- The current container has no cargo/systemctl command or running systemd user manager; study can inspect contracts but cannot prove runtime behavior here.

## Defensive mechanisms and their justification
| Mechanism | Present scenario/evidence | Needed now? |
|-----------|---------------------------|-------------|
| Gated helper before original exec | lib.rs:580 launches executable before caller gets PID; a shell can fork immediately | yes |
| Direct fallback for absent/failed manager | Explicit ticket requirement for containers and other init systems | yes |
| Shared deadline, cancel/settle before fallback | User bus may exist without an answering manager; waits.rs requires bounded waits | yes |
| Private unique socket directory | A helper must connect after first exec because portable-pty closes inherited descriptors | yes |
| Parent-held unit reference and cached evidence | Fast commands may exit before UI polls, and portable-pty's reader EOF carries no process cause | yes |
| Separate unknown/ordinary/OOM state | kill -9 and systemd-oomd both use SIGKILL; normal EOF is currently silent | yes |
No runtime policy switches, resource limits, retries of user programs or monitoring services are introduced.

## Implementation plan (order)
1. Implement Linux scope backend and gate protocol with injected manager outcomes and controlled helper tests.
2. Add explicit supported launcher setup in the Ivy entry point and common asynchronous Linux app spawn adapter; preserve pending identity/order, cancellation, errors and fallback; cover opening and takeover.
3. Add cached process/scope outcome and connect it to the existing notice, wake and auto-close behavior.
4. Exercise actual Linux user-manager integration in a disposable session, then workspace and platform checks. Record missing-environment proof separately.

## Planned tests
- AC1/AC5: actual Ivy helper executable, PTY child that forks immediately; inspect /proc/<pid>/cgroup for both children and GUI, verify distinct scopes and original PID/tty.
- AC2: injected no bus, access denied, unknown OOMPolicy, failed job and stalled bus; assert exactly one program-side marker, preserved argv/env/cwd, successful fallback, and deadline release.
- AC3/AC4: state/notice tests for delayed `oom-kill`, ordinary success, kill -9, normal nonzero exit, bus disconnect and missing final result. Assert retained output, dismissibility, repaint and auto-close selection.
- AC5: existing PTY input/resize/close tests plus PATH, login shell argv and fallback/takeover exercised through the real helper, not just a model. Hold several manager startups pending and prove the UI continues processing events; cancel one and verify no late pane or orphan child appears.
- AC3: disposable Linux integration uses a bounded per-scope memory limit to prove kernel-OOM result and sibling survival; this does not by itself prove systemd-oomd. Separately run controlled systemd-oomd pressure in a disposable VM/user session and record selected scope and resulting notice; never stress the host session.
- AC6: non-Linux dependency graph/build checks and existing platform release tests.
- Required validation when implementing: cargo fmt --all --check; cargo clippy --locked --workspace --all-targets -- -D warnings; cargo test --locked --workspace; cargo doc-check --locked; python3 -m unittest scripts/test_release.py scripts/test_macos.py scripts/test_windows.py -v. No code changed in this study, so these are planned, not claimed as passed.

## Decisions (ADR-light)
| Decision | Discarded alternative | Why |
|----------|----------------------|-----|
| D-Bus and a fresh gated helper | Post-spawn move; systemd-run wrapper | Root moves before it can fork, and exec preserves the PID already owned by Pty; CLI startup failures blur whether user work ran |
| Re-exec existing Ivy binary | Separate installed helper; vendoring portable-pty; raw Linux PTY fork implementation | Avoids new packaging paths and backend duplication; current entry point is tiny; portable-pty has no callback |
| Parent retains unit observation | Read memory.events only; infer SIGKILL | systemd scope source explicitly receives managed_oom; kernel counters alone cannot prove userspace oomd |
| Keep old direct-spawn API, opt in Ivy | Globally assume current_exe supports helper mode | term-core tests/examples and other hosts are not the Ivy executable |
| One coherent PR | Separate unused launcher infrastructure and UI follow-up | Isolation, fallback and truthful notice are one ticket; either half alone leaves compatibility or explanation incomplete |

## Depends on other tickets
None. All existing launch/notice infrastructure is present on main at 161c80fdef8a2cbc0868a3b7948e809f1097d979. No sibling repository contract required.

## Design challenges
Independent challenger: `design-challenge.md`; no high findings.

| Angle | Finding | Type | Severity | Response |
|-------|---------|------|----------|----------|
| Fit/YAGNI | Same-UID peer authentication adds a boundary the private socket does not promise | unnecessary | medium | Remove authentication/SO_PEERCRED. Keep the 0700 directory and unique endpoint for routing; helper PID is taken from the owned child. This is internal coordination, not a same-user security boundary. |
| Operation | A synchronous one-second scope deadline per pane can accumulate during layout restore | missing | medium | Make Linux isolated startup asynchronous at the app boundary, including error completion and cancellation; scope deadline remains one second in a worker. No bus wait or join on the GUI thread. Add a many-console stalled-manager test. |
| Attribution | A bounded observation window needs a concrete end condition when descendants remain | missing | medium | On confirmed root exit start one 500 ms final-result deadline, query retained state and accept earlier positive oom-kill immediately. At expiry finish unknown if evidence is absent, even if descendants remain. Release the reference after classification; active scope lifetime is still manager-owned. Tests control clock and delayed manager replies. |

Design review accepted as an unattended study draft for owner approval. These responses are technical refinements, not answers to a product gate.

