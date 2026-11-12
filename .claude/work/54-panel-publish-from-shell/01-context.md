# Context #54

## Ticket
- flow's `PostToolUse` hook re-renders the panel (plan ticks, record changes, an answered question) but cannot call MCP tools; it reaches a terminal only through `panel.publish_cmd`, a shell command reading the document on stdin.
- Ivy offers no such command, so «waiting for you» stays on after the question is answered (18 min measured in one session) and ticked plan steps arrive only when the agent republishes.
- Copying Bellwether's `bellwether-panel-mcp publish` (mashware/bellwether@94118ea) collides with two rules here:
  1. The hold compares **connections**, not sources (`crates/app/src/panel/driving.rs`, D43/D44): a second connection is refused even if it calls itself `flow`, and a held pane's `panel.json` is not read. Relaxing to «same source name» lets any process in the console overwrite the panel.
  2. The hook's render carries no `actions`: if it replaces the agent's document, the buttons vanish after every tool call.

## Acceptance criteria (provisional — promoted to first-class criteria in the design phase)
The ticket lists decisions, not criteria:
- Decide who may publish into a held pane from outside the holding connection, and how it is proven (`PANEL_ID` is an identity, never a credential — D44).
- Decide whether a shell publication replaces the document or merges into it keeping the holder's `actions`.
- Outside an Ivy console the command does nothing and succeeds, so one git-ignored FLOW file works in any terminal.
- Not included: any change to flow itself.

## Decided in the ticket thread
empty thread

## Relevant domain knowledge
- `panel-id-is-a-name-not-a-credential`: every descendant inherits `PANEL_ID` and any same-uid process can read it from `/proc/<pid>/environ`; authenticating with it authenticates nothing.
- `mcp-instructions-reach-subagents` (D88): subagents share the connection that holds the pane; any handshake text must tell an agent working for another agent to leave the panel alone.
- `the-transport-is-not-the-contract` (D17): the document is the contract, `panel.json` one transport; the source of truth is the state the app holds per pane; two producers are never settled by chance.
- `a-bound-counts-the-encoded-width-not-the-decoded-value`: wire bounds count encoded bytes; a line over `MAX_REQUEST` is `Unrecoverable` and drops the connection — and the hold with it.
- `a-frame-cost-is-declared-once`: any new frame work is declared once, at the boundary that runs it (D13).
- Decisions to read in design: D17, D43 (connection holds a pane), D44 (MCP scope, opaque handles), the `panel_patch` decision (~line 6900 of `docs/02-decisions.md`), `panel_retire` (~7203), D88.

## Repo state at start
- Branch: 54-panel-publish-from-shell (worktree `.worktrees/54-panel-publish-from-shell`, from origin/main)
- Last commit: 9e30f7a 51 Agents in an Ivy console are asked to publish their panel [minor] (#55)

## Decisions clarified at start
no open questions — the ticket's three «to decide» points are the design's job, not gaps in the ticket.
- Config read: `FLOW.md` + `FLOW.claude.md` (main checkout); `FLOW_CONTEXT` unset.
- Roadmap: no item names this ticket (the roadmap's `#54` is F2.1 in the archived repo) → not linked (auto).
- Linked branch skipped: a `--study` work is handed off, never merged from this branch.
- Tracker transition: `tracker.start_cmd` empty → nothing done.

## Estimated size: M
Touches the trust boundary (`crates/panel-control/**`, a sensitive path) and adds a new decision next to D43/D44; likely a new CLI entry point plus a merge rule for the document.
Recorded in auto mode; design reclassifies.
