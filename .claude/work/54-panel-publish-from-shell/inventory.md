# What already exists (inventory for #54)

Worktree: /home/vioque/Mashware/agent-terminal/.worktrees/54-panel-publish-from-shell (branch from origin/main 9e30f7a).

## Decisions (docs/02-decisions.md)
- **D17** (line 2014): the contract is the document; source of truth = state the app holds per pane; `panel.json` is one transport.
- **D43** (line 6485): a connection claims the pane on first `panel_set`; what is compared is the **connection, never the source**; while held, `panel.json` is **not read**; the hold dies with the connection; a refused file publication is confessed on the app's row naming the driver. Discarded: dates, seq numbers, explicit release, window-wide endpoint.
- **D44** (line 6561): handles for auxiliary consoles = 16 random bytes scoped to (pane, connection). `PANEL_ID` is a name, not a credential (inherited by every descendant, readable via /proc/<pid>/environ by same uid).
- **D47** (line 6840): `panel_patch` — keyed lines, all-or-nothing; only the holder patches; discarded "patch takes no hold".
- **D50** (line 7190): `panel_retire` only from the holding connection. **Discarded**: letting any new pane connection retire the holder ("PANEL_ID, PANEL_DIR, process ancestry and same-user files are names or same-user data; none proves ownership of D43's live connection"); **a bearer token for command hooks** ("creates a second authority system and makes console destruction available outside the connection that opened them"); "a command hook, fresh connection, empty-document sentinel or terminal parsing does not pretend to be that route".
- **D80** (line 8919): agent signals (client identity, activity, input-required) are a separate pane-local register, independent of the panel publisher; "a one-shot hook cannot patch the connection that already owns Flow's panel"; signal never claims/patches/retires somebody else's panel; signal's input-required claim combines with panel attention by priority; withdrawing the signal cannot withdraw the panel's claim. Claude Code 2.1.319 publishes native root snapshots (OSC7501-style) after capability negotiation.
- **D84** (line 9114): session recovery helper `ivy-session` (crates/panel-control/src/bin/ivy-session.rs), located via absolute `IVY_SESSION_HELPER`. "The endpoint scopes the pane; native peer/ancestry/foreground/birth/lifetime evidence scopes its reporter." "Official Codex command hooks start a new OS session. The reporter helper therefore needs bounded descendant attestation… Only a recognized native client ancestor in the pane's actual foreground group, SID and terminal grants authority." Attach → generation token; probe/update carry generation + sequence + challenge.
- **D88** (line 9327): `panel-mcp` sends `instructions` on initialize only with a whole identity (`client::whole_identity`: PANEL_ID non-empty text, PANEL_DIR absolute) and where `endpoint::BINDS` (false on Windows until F2.7). Subagents share the parent's connection; workers in the same console must leave the panel alone.

## Code
- `crates/panel-control/src/bin/panel-mcp.rs` — the MCP bridge; one control connection per bridge process; tools panel_set / panel_get / panel_patch / panel_retire / split_*.
- `crates/panel-control/src/wire.rs` — newline-framed JSON requests (`{"tool":…,"source":…,"document":…}`), `MAX_SOURCE`, `MAX_REQUEST` (document ceiling + envelope), over-long line = Unrecoverable (connection ends, hold with it).
- `crates/panel-control/src/listener.rs` — per-pane endpoint (Unix socket `panel_contract::CONTROL_SOCKET` inside PANEL_DIR, 0700 dir), one thread per connection, deliveries to the window tagged with `connection`.
- `crates/panel-control/src/client.rs` — `whole_identity`, client used by the bridge.
- `crates/panel-control/src/session.rs` + `session/tests.rs` — the session service on the same socket; peer credential checks ("cannot_spoof_peer"); test `session_helper_cannot_release_or_replace_existing_panel_connection` proves a helper connection does not touch the panel hold.
- `crates/app/src/panel/driving.rs` — `Holder`, `deliver`, `retire`, `snapshot`: the hold rule in the window.
- `crates/app/src/panel/document.rs`, `crates/app/src/control.rs` — committed document, readers, metrics.
- `crates/panel-contract` — the document parser (`parse_document`), `MAX_BYTES`, `actions` field.

## The consumer (outside this repo, not to be changed)
flow's PostToolUse hook renders the panel document (no `actions`) and pipes it to `panel.publish_cmd` on stdin. It fires on plan ticks, meta.json changes, panel-input.json changes, and when the user answers AskUserQuestion (it then drops `decision` and `wait`). Bellwether (Herdr) shipped `bellwether-panel-mcp publish` reading a document from stdin (mashware/bellwether@94118ea).
