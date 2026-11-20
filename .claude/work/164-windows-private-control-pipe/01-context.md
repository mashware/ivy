# Context 164

## Ticket
- Enable the Windows pane control channel and its MCP consumers.
- Bind a user-only named pipe with explicit current-user and SYSTEM access, reject remote clients and first-instance squatting.
- Authenticate the server PID against an app PID inherited in the environment.
- Use overlapped accept plus a private stop event; verify inherited directory owner and DACL.
- Ticket explicitly depends on #161.

## Acceptance criteria (provisional)
- BINDS is true and binding_is_what_binds_says passes on Windows.
- Another user cannot connect; assert the actual pipe DACL.
- Drop meets its bound and leaves no worker.
- panel-mcp supplies instructions and panel_set paints.

## Decided in the ticket thread
Empty thread (ai ticket supplied comments: []).

## Relevant domain knowledge
No configured knowledge search.

## Repo state at start
- Clean main at 137fd348165866dcd3382608a811d0c594119f42.
- Configuration listing: FLOW.claude.md, FLOW.codex.md, FLOW.opencode.md; only FLOW.codex.md applies. FLOW_CONTEXT=unattended; FLOW.unattended.md absent. No FLOW.md base.
- autonomy.mode=unattended and orchestrator_cmd=ai from FLOW.codex.md.

## Decisions clarified at start
No product questions; security and shutdown requirements are explicit.

## Estimated size: M (Recommended)
Native transport and security logic spans endpoint, streams, environment propagation and MCP integration; no schema changes.

## Size reassessment after F1 answer
L (Recommended): the authorized bounded Windows lifecycle requires concrete service capabilities, app snapshot publication, native UI wake and release accounting across modules. Estimates are advisory.
