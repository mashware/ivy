# Native Gentle dialog experiment

Two independent Gentle Shell 5.1.0 processes on Linux received identical selection dialogs. Each session accepted its current dialog ID. Sending another session's ID, a completed dialog's ID or an ID from a terminated process did not settle the new dialog. Explicit cancellation resolved the original dialog as cancelled. The probe sends a response, drops the process before observing completion, and verifies that the old response cannot answer the replacement session. The original delivery outcome remains unknown.

## Reproduce

Use Linux with pidfd support, Python 3.11+, npm and a compatible Node runtime (tested with Node 24.14.1 and Pi 1.6.1). From this directory run:

```sh
probe_dir=$(mktemp -d)
npm install --prefix "$probe_dir" --ignore-scripts --no-audit --no-fund gentle-pi@5.1.0 @earendil-works/pi-coding-agent@1.6.1
python3 probe.py "$probe_dir"
```

Each session gets a fresh temporary home, project and private stdio channel.
The first command exercises the dialog relay in the parent.
The second command uses Gentle's installed task runner to spawn a real Pi child in each session.
A deterministic extension asks the same question in each child, without contacting a model.
After the host responds, only the resumed child emits its answer over the private child-process channel.
The host verifies the choice in one session and cancellation in the other.
A Linux process handle separately verifies that the fixture child exits after its waiting parent is killed.
The fixture installs its own disconnect handler for this check.
The npm installation stays in the supplied directory.

## What Ivy can display

The observed selection request contains `type`, `id`, `method`, `title` and `options`. The parent process and dialog ID identify the reply destination. The dialog contains no structured helper ID. In Gentle 5.1.0 the task runner knows the helper ID, but the registered UI callback drops it before opening the dialog. Task-mode helpers can ask through this path. The runner cancels background-mode dialogs. [Parent relay](https://github.com/Gentleman-Programming/gentle-shell/blob/v5.1.0/extensions/gentle-agents.ts) · [Task runner](https://github.com/Gentleman-Programming/gentle-shell/blob/v5.1.0/lib/agents-runner.ts)

An initial Ivy dialog can safely name the owning session. Showing a specific helper requires an explicit association from the runtime. A title or a nearby activity event cannot establish it when helpers overlap. The test notification is an observation inside the fixture. Production must not treat writing the reply to stdin as proof of continuation.

## Desktop acceptance

Task-mode routing is verified at the process boundary with a scripted child command. The exact helper name requires an upstream correlation field. The Linux desktop check used **New Gentle task** in two projects. While project A showed the second hunk of its diff, choosing an option for B produced a receipt only in B; cancelling A then produced its own receipt. The visible diff row remained at the same pixel through both interactions. This experiment is separate from Ivy startup and contains no production settings or UI changes. [Task #286](https://github.com/mashware/ivy/issues/286)

## Stop a delegated helper

With the same installation, run `python3 stop.py "$probe_dir"` on Linux. A deterministic HTTP model bound to loopback asks each parent to call Gentle's built-in `subagent_run` tool and holds both child requests open. Each isolated home provides its own model configuration. The probe exercises Gentle's production desktop RPC, task store and runner.

While both parents awaited their children, B refused A's helper ID with `not_found`. A accepted the same ID and initially reported `running`. A later `list_subagents` returned `cancelled` for that ID and the child's held model connection closed. B remained `running` and completed when its own model replied. The parent can continue after its helper exits. The list describes the owning session's bounded helper projection. It does not establish that the root prompt or a separate background job has stopped. [Desktop protocol](https://github.com/Gentleman-Programming/gentle-shell/blob/v5.1.0/docs/desktop-rpc.md)

Add `--background` to hold the parent in a separate turn after its background delegation returns. An acknowledged parent `abort` leaves A’s helper and its HTTP stream active. Only the subsequent helper cancellation settles that helper. The same run starts a real background shell job in each session, verifies that stopping A’s job leaves B’s running, and completes a follow-up in A under the original session ID with the initial request still present in its message history.

In a separate Linux check, the launcher received `SIGTERM`, followed by a group `SIGKILL` after the 750 ms grace period. With one owned background helper, the launcher exited, the detached Pi child’s pidfd became readable, its model connection closed, and B continued to completion. This observation covers that installed client’s shutdown path. Reaping the launcher alone cannot confirm what happened to every helper.
