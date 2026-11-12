# Context #56

## Ticket
- Follow-ups of #51 / PR #55 (the MCP bridge now asks every agent in an Ivy console to publish its panel), found in review outside the code that change touched.
- **Borrowed identity** — a shared daemon (Codex without `--no-daemon`), an editor or a multiplexer started from console A carries A's `PANEL_ID`/`PANEL_DIR`; an agent later running in console B through it is now told to publish and lands in A's panel (takes A's hold, or is refused against A's own agent at every stop).
- **Unreachable endpoint promised** — a stale identity (console closed, directory gone) or a socket path over `sun_path` is still promised a panel; a `stat` of `PANEL_DIR/control.sock` before promising would catch it without connecting (D88 discarded the connect, not the stat).
- **Hold outlives `done`** — an agent keeps the pane until its bridge exits (D43); a later file producer in that console is refused meanwhile.
- **Two rules for one thing** — the session helper (`session/helper.rs`, `ivy-session`) accepts a relative `PANEL_DIR` and converts the id lossily, unlike `client::whole_identity`; the shell-integration rc dir takes a relative `TMPDIR` as written (`crates/app/src/shell_integration.rs`); the in-app Connect guide still appends the manual publishing prompt for every client (no subagent rule), while the README reserves it for clients that ignore server instructions.

## Acceptance criteria (provisional — promoted to first-class criteria in the design phase)
Not specified as criteria. From "Why it matters": items 1 (wrong console's panel changing) and 3 (file producer silently ignored) are the ones a person meets; items 2, 4, 5, 6 remove duplicated rules.

## Decided in the ticket thread
empty thread

## Relevant domain knowledge
- `mcp-instructions-reach-subagents` — handshake instructions reach subagents sharing the connection that holds the pane; D88 rule: tell a worker of another agent to leave the panel alone. Every reader of `PANEL_ID`/`PANEL_DIR` must use `client::whole_identity`; the session helper is the known exception (item 4).
- `panel-id-is-a-name-not-a-credential` — `PANEL_ID` is inherited by every descendant and readable by any same-uid process; it names a pane, it never authenticates. Relevant to item 1: there is no way to prove from the env that the caller is *in* the console the id names.
- `the-contract-removes-what-the-contract-puts-there` — the pane dir is cleaned non-recursively; `control.sock` removal lives in `panel-contract`'s `remove`. Relevant to item 2's stat.
- `a-closing-pane-answers-one-read-and-leaks-its-acceptor` — closing removes `PanelDir` before the channel; a stale identity meets a missing directory/socket.
- Hard rule D6 (the app does not deduce state) bounds item 1: detecting "borrowed" must not come from parsing another program's output.

## Repo state at start
- Branch: 56-panel-identity-outside-handshake (worktree `.worktrees/56-panel-identity-outside-handshake`, from origin/main)
- Last commit: 9e30f7a 51 Agents in an Ivy console are asked to publish their panel [minor] (#55)

## Decisions clarified at start
- `#55 --study` named a merged PR, not a ticket → asked; the user chose **#56** (study here, build elsewhere: `handoff: true`, `git.handoff_branch: ai-study/{TICKET}`).
- Roadmap: `docs/05-roadmap.md` has no item naming #56/#51 — loose ends; not linked (auto mode records `null`).
- Linked branch (`gh issue develop`) skipped at start: the PR split is unknown until `plan`.
- `tracker.start_cmd` empty → no tracker transition.

## Estimated size: M
Six items across three crates (`panel-control` bridge — a sensitive path —, the session helper, `app` shell integration and Connect guide) plus at least one new or amended decision (items 1 and 3 touch D43/D88). Items 1 and 3 need a product decision each; may split in `plan`.
