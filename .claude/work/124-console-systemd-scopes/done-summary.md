<!-- flow-study -->
## What and why
On Linux, Ivy consoles currently share the window's cgroup, so one memory-heavy build can cause systemd-oomd to kill unrelated consoles and the GUI. Give each console a separate optional scope and report confirmed memory-pressure termination. This is a study only: no production code changed.

## Acceptance criteria
| ID | Observable behavior | Proof |
|----|---------------------|-------|
| AC1 | With a Linux user manager, two console roots and their immediately forked descendants have different `ivy-console-*.scope` cgroups in app.slice; neither scope contains the Ivy GUI PID. | Linux integration |
| AC2 | No user manager, denied creation, unsupported optional OOM properties or a timed-out setup still execute the requested program exactly once with the original cwd, environment and argv. A successful basic scope is retained when only OOM attribution is unsupported. | injected failures + Linux integration |
| AC3 | A scope observed with `Result=oom-kill` produces `This console was stopped because of memory pressure.` in its console notice, retains existing scrollback, and leaves another console and the GUI alive under normal leaf-victim policy. | state/UI tests + isolated Linux integration |
| AC4 | A manually signaled child with no OOM evidence is never labeled as memory pressure. A normal `exit 0` remains an ordinary ending. Missing/late/unavailable scope evidence stays unknown, with a signal notice when the wait status supplies one. | status/notice tests |
| AC5 | Normal launch, restored launch, launch fallback and takeover all use isolation; failed exec still reaches the existing spawn-error/fallback path. PTY controlling terminal, resize, input and process-group close semantics survive. Several simultaneous stalled-manager startups do not block GUI event processing; closing a pending launch cannot resurrect its console. | actual executable integration |
| AC6 | macOS and Windows use today's direct launch, require no systemd library or executable, and compile without Linux backend dependencies. | cross-platform checks |


## Design in brief
Use a Linux-only internal re-exec launcher that waits before executing the requested program. An Ivy worker creates a distinct transient user-manager scope in app.slice containing that PID, then releases the program in place. If setup is unavailable or fails, run the original program once without isolation. Preserve cwd, environment, arguments, PTY and process-group close behavior.

Keep Linux startup asynchronous so slow manager calls do not freeze opening/restoring consoles. Cover normal, restored, fallback and takeover launches, including pending-launch cancellation. macOS and Windows retain direct launching; there is no extra installed binary or required systemd executable.

Retain the scope result and distinguish `Result=oom-kill` from unknown signals. A confirmed OOM notice says `This console was stopped because of memory pressure.` Keep scrollback and the existing dismissible notice behavior. Startup uses a one-second worker deadline; final attribution uses 500 ms from confirmed root exit. Late or unavailable evidence becomes unknown, never guessed OOM.

Independent design challenge found no high-severity blockers. Three medium findings were addressed and independently rechecked: remove redundant peer authentication, specify asynchronous startup/cancellation, and bound final-result observation.

Dependencies on other tickets: none.

## MR/PR plan
Wave 1: PR #1 — Isolate Linux consoles in systemd scopes and explain OOM exits.
One coherent PR covers launch isolation, fallback, exit evidence and notices, with tests. No preceding PR or release required. Advisory estimate: about 1200 added/modified lines across 18 files, including tests and source inventories. No implementation has started.

## Schema changes
none

## Risks
- The helper gate must preserve exactly-once execution and real spawn errors; original program descendants must not run before scope placement.
- Older systemd versions can support isolation without reliable managed-OOM attribution; the notice stays generic when unconfirmed. The 500 ms cutoff can miss late evidence.
- Common-ancestor policies and global kernel OOM are outside the protection separate leaf scopes can guarantee.
- Required proof includes real helper/PTY/cgroup integration, controlled OOM and sibling survival, asynchronous cancellation, and platform regressions. Kernel-OOM simulation alone does not prove systemd-oomd.
- Runtime verification was not performed: this study container has no cargo/rustc/systemctl or running systemd manager. Implementation must run the workspace checks and controlled Linux tests in a suitable environment.

Study artifacts: 01-context.md, 03-design.md, 04-mr-plan.md, design-challenge.md and challenge-resolution.md in the work folder.
Configuration: FLOW.codex.md only; FLOW_CONTEXT=unattended has no FLOW.unattended.md. Core is 0.117.0; installed plugin manifest is absent, so version comparison was unavailable. Panel rendered successfully with flow-workflows 0.117.0.
<!-- end flow-study -->
