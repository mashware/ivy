# Native Gentle dialog experiment

Two independent Gentle Shell 5.1.0 processes on Linux received identical selection dialogs. Each session accepted its current dialog ID. Sending another session's ID, a completed dialog's ID or an ID from a terminated process did not settle the new dialog. Explicit cancellation resolved the original dialog as cancelled. The probe sends a response, drops the process before observing completion, and verifies that the old response cannot answer the replacement session. The original delivery outcome remains unknown.

## Reproduce

Use Python 3.11+, npm and a compatible Node runtime (tested with Node 24.14.1 and Pi 1.6.1). From this directory run:

```sh
probe_dir=$(mktemp -d)
npm install --prefix "$probe_dir" --ignore-scripts --no-audit --no-fund gentle-pi@5.1.0 @earendil-works/pi-coding-agent@1.6.1
python3 probe.py "$probe_dir"
```

Each process gets a fresh temporary home, project and private stdio channel. The test command invokes Gentle's installed helper dialog relay and reports its returned answer to the test host. The command replaces the model-generated request. The probe exits after observing the answer from each relay and removes the temporary session homes. The npm installation stays in the supplied directory.

## What Ivy can display

The observed selection request contains `type`, `id`, `method`, `title` and `options`. The parent process and dialog ID identify the reply destination. The dialog contains no structured helper ID. In Gentle 5.1.0 the task runner knows the helper ID, but the registered UI callback drops it before opening the dialog. Task-mode helpers can ask through this path. The runner cancels background-mode dialogs. [Parent relay](https://github.com/Gentleman-Programming/gentle-shell/blob/v5.1.0/extensions/gentle-agents.ts) · [Task runner](https://github.com/Gentleman-Programming/gentle-shell/blob/v5.1.0/lib/agents-runner.ts)

An initial Ivy dialog can safely name the owning session. Showing a specific helper requires an explicit association from the runtime. A title or a nearby activity event cannot establish it when helpers overlap. The test notification is an observation inside the fixture. Production must not treat writing the reply to stdin as proof of continuation.

## Remaining acceptance

Exercise a delegated task-mode helper through the full runner, establish a supported link between dialog and helper, and confirm the child resumes. Then validate the opt-in native interaction while another pane has a diff open. This experiment is separate from Ivy startup and contains no production settings or UI changes. [Task #286](https://github.com/mashware/ivy/issues/286)
