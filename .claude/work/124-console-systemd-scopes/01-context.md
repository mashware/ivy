# Context 124

## Ticket
- Linux systemd-oomd killed an entire Ivy window because its consoles shared the application scope.
- Give every console child a distinct transient systemd user scope when available.
- Preserve terminal startup on systems without systemd.
- Explain memory-pressure termination in the ending notice when it can be identified.
- macOS and Windows behavior stays unchanged.

## Acceptance criteria (provisional)
- Linux console processes and descendants use separate scopes, apart from the GUI.
- Unavailable user manager or scope setup failure does not prevent a console starting.
- A killed console retains its output and explains its termination without mislabeling normal exits.
- Existing input, resize, close, shell integration and program/cwd handling keep working.

## Decided in the ticket thread
Empty thread (orchestrator returned comments: []).

## Relevant domain knowledge
No configured knowledge search. Repository and upstream sources will be examined in design.

## Repo state at start
- Branch: main; clean.
- Last commit: 161c80fdef8a2cbc0868a3b7948e809f1097d979.

## Decisions clarified at start
No open product questions; attribution must distinguish confirmed OOM from an otherwise unknown SIGKILL.
Only this repository is affected. No roadmap found. No ticket dependencies identified.

## Estimated size: M
New Linux process containment and exit reporting cross the terminal core and application UI, with an external systemd integration. Size is advisory, not a line budget.

## Effective configuration
Only FLOW.codex.md applies. FLOW.md absent; FLOW_CONTEXT=unattended, FLOW.unattended.md absent. FLOW.claude.md and FLOW.opencode.md listed and intentionally inactive.
Autonomy unattended, orchestrator ai, worktree off, proportional review (FLOW.codex.md).
Core reports 0.117.0; installed plugin manifest is missing, so its version cannot be compared.
