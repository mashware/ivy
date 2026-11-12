# Context #57

## Ticket
- Found while reviewing #55 (#51); pre-existing, not introduced there.
- Where a console's directory lives under a shared temporary directory (macOS by default, Linux
  without `XDG_RUNTIME_DIR`, or a relative one that falls back since #55), closing a console removes
  its `ivy-<id>` directory.
- A tmux or agent session started there keeps `PANEL_ID`/`PANEL_DIR`. Another local user can create
  `/tmp/ivy-<id>/control.sock` meanwhile; the stale identity is then whole, the bridge connects, and
  the agent streams its panel documents (task state, questions) to that user's socket.
- Fix to consider: before connecting, check that `PANEL_DIR` is owned by the current user and has
  mode `0700` (as `PanelDir` creates it); refuse otherwise, naming the directory. One `stat`, on the
  bridge and on the session helper alike.
- Why: same-machine information leak of whatever agents write on their panels. Low likelihood, cheap fix.

## Acceptance criteria (provisional — promoted to first-class criteria in the design phase)
- A connection to a console's channel is refused when `PANEL_DIR` is not owned by the current user
  or its mode is not `0700`, and the refusal names the directory.
- The check applies to every client that connects with an inherited identity: the MCP bridge and the
  session helper alike.
- A genuine console directory (created by `PanelDir`) still connects as before.

## Decided in the ticket thread
empty thread

## Relevant domain knowledge
- `PANEL_ID` is a name, not a credential: inherited by every descendant, readable via
  `/proc/<pid>/environ` by the same uid, 32 bits sized for collisions not secrecy.
- Every reader of `PANEL_ID`/`PANEL_DIR` must use one rule, `client::whole_identity`; the session
  helper still has its own rule (deferred) — a natural place for the owner/mode check to converge (D88 card).
- `PanelDir::create` adds a single path component under the base dir so nothing intermediate can be
  pre-planted in a shared temp; removal is non-recursive on purpose.
- The closing pane removes `PanelDir` before the acceptor ends; the endpoint is the crate's.
- Other connectors seen in code: the opencode plugin (`resources/agent-integrations/opencode/ivy-opencode.tsx`)
  opens the socket path directly — design must decide whether it is in scope.

## Repo state at start
- Branch: 57-check-panel-dir-owner (worktree `.worktrees/57-check-panel-dir-owner`, linked to #57)
- Last commit: 9e30f7a 51 Agents in an Ivy console are asked to publish their panel [minor] (#55)

## Decisions clarified at start
no open questions

## Estimated size: S
One owner/mode check shared by the clients that connect with an inherited identity (bridge, session
helper, possibly the opencode plugin); no contract or schema change. Design reclassifies if the JS
integrations and the cross-platform story (macOS, Windows) widen it.

Roadmap: not linked (auto mode; no roadmap item names #57).
Config read: `FLOW.md`, `FLOW.claude.md` (main checkout); `FLOW_CONTEXT` unset.
