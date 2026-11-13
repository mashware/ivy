# Context 69

## Ticket
Typing `codex` in an ordinary Ivy console starts an independent client with `--no-daemon`. This preserves console-local panel routing, but prevents the same live conversation from being opened through Codex remote control on a phone.

## Observed behavior
On Linux on 13 November 2026:
1. Start Codex by typing only `codex` in an Ivy console.
2. Enable remote control with `codex remote-control start --json`; the server reports `connected`.
3. The computer and conversation appear in the Android client.
4. Opening the conversation fails with a message that its messages could not be loaded.

The daemon log records Android `thread/resume` failing for the active local conversation:

```text
failed to initialize thread persistence: thread-store conflict: thread <id> already has an active writer
Failed to create session: thread-store conflict: thread <id> already has an active writer
```

After exiting and starting again through the usual `codex` command, the process again has `--no-daemon`, and the failure persists.

## Why Ivy is involved
The console-local launcher (`ivy-codex`) deliberately injects `--no-daemon` for interactive starts, resumes and forks when supported. This was introduced by #43 / PR #46 because shared Codex sessions lost `PANEL_ID` / `PANEL_DIR`, breaking independent panel routing.

The current README documents this behavior. Remote arguments bypass injection, but the ordinary startup path still creates a separate writer from the remote-control server.

This host also has `features.daemon_auto_start = false` in its personal Codex configuration. Ivy does not write that setting; investigate it separately from the launcher argument.

## Expected behavior
A person can type `codex` in an Ivy console and access the same live conversation from their phone, while panel publications remain scoped to the correct console across multiple simultaneous sessions.

## Acceptance criteria
- Investigate a supported way to combine Codex remote control with console-specific MCP identity; do not simply remove the independent-launch safeguard and reintroduce cross-console routing failures.
- Verify ordinary start and resume with two Ivy consoles: each client updates only its own panel.
- Verify from a real phone that the active conversation loads messages and accepts input through remote control without an active-writer conflict.
- Preserve existing aliases/functions, explicit remote launches and user Codex configuration.
- If the installed Codex version cannot support both functions, explain the limitation and supported choices in the Connect guide and README; do not report a successful remote connection as proof that a live conversation can be opened.

## Scope
This report does not establish whether Codex itself needs an upstream change. The observed conflict and Ivy's injected argument are confirmed; the compatible implementation still needs investigation.

## Decided in the ticket thread
Empty thread (orchestrator returned comments=[]).

## Relevant domain knowledge
No knowledge search role configured. Repository decisions will be inspected.

## Contracts received
Codex is an external dependency; no upstream change or sibling repository work is authorized.

## Repo state at start
Clean main at afef5c75e0f94cc9156501df613bafbaa7986217.

## Decisions clarified at start
No product questions: the ticket explicitly permits documenting a limitation when joint support cannot be established. Never infer phone success from a connected server.

## Effective configuration
Found FLOW.claude.md, FLOW.codex.md, FLOW.opencode.md; read only FLOW.codex.md. Base FLOW.md absent; FLOW_CONTEXT=unattended, FLOW.unattended.md absent. autonomy.mode=unattended, orchestrator_cmd=ai, git.worktree=off from FLOW.codex.md. Shared core 0.116.1; installed plugin.json unavailable, so version comparison unavailable. Publications in English per run instruction.

## Estimated size: M (Recommended)
Launch policy, MCP environment and recovery span modules; no database or new public API is planned. Reclassify after investigation if the safe result is documentation only.
