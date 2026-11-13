# Context 57

## Ticket
Found while reviewing #55 (#51). Pre-existing, not introduced there.

## What

Where a console's directory lives under a shared temporary directory — macOS by default, a Linux session without `XDG_RUNTIME_DIR`, or since #55 a relative one that falls back — a closed console's `ivy-<id>` is removed. A tmux or agent session started there keeps `PANEL_ID`/`PANEL_DIR` (eight hex digits readable by any process of the same user, and guessable by others). Another local user can create `/tmp/ivy-<id>/control.sock` in the meantime; the stale identity is then whole, the bridge connects, and the agent streams its panel documents — task state, questions — to that user's socket.

## Fix to consider

Before connecting, check that `PANEL_DIR` is owned by the current user and has mode `0700` (as `PanelDir` creates it); refuse otherwise, naming the directory. One `stat`, on the bridge and on the session helper alike.

## Why it matters

A same-machine information leak of whatever agents write on their panels. Low likelihood (needs a stale identity and a local attacker), but the fix is cheap.

Related: #55.

<!-- flow-study -->
## What and why
A client that connects with an inherited console identity refuses when `PANEL_DIR` is not a private directory of this user, so a stale identity can no longer stream panel documents to a socket another local user planted after the console closed.

## Acceptance criteria
- AC1 — A genuine console (`PanelDir::create`): the MCP bridge, the session helper and the opencode plugin connect exactly as before.
- AC2 — `PANEL_DIR` with any group or other bit (e.g. `0755`): the bridge and the helper refuse without connecting; the error contains the directory path and `not a private directory of this user`.
- AC3 — `PANEL_DIR` owned by another uid: refused (covered by injecting the expected uid, no root needed).
- AC4 — `PANEL_DIR` is a symlink, even to a genuine console directory of the same user: refused.
- AC5 — `PANEL_DIR` does not exist: refused (fail closed), the directory named and the text saying it is gone — not `not a private directory`.
- AC6 — The opencode plugin with a `PANEL_DIR` that fails the check opens no connection, and retries with a fresh check every second.

## Design in brief
- One function in `crates/panel-control/src/endpoint.rs`, `private_to_this_user(dir)`: `lstat`, must be a directory, owner = effective uid, `mode & 0o077 == 0`. No-op on Windows (no channel binds there until F2.7).
- Called by the two inherited-identity doors: the bridge (`Producer::of_this_pane`) and the session helper (`exchange`); not by `Producer::of` or `endpoint::connect`. `ivy-session` stops discarding the helper's error text.
- The opencode plugin repeats the same check in JS inside `connect()`, on every 1 s reconnect.
- Out on purpose: a peer-uid check on the connected socket, a root exception, and moving the helper onto `whole_identity` (D88, deferred).
- D7 amended in `docs/02-decisions.md`; security table in `docs/03-architecture.md`.

## MR/PR plan
One PR: the check, its two Rust call sites, the opencode plugin, tests and docs.

## Schema changes
none

## Risks
- The file transport (`panel.json` written by scripts) can still write into a planted directory — accepted, the app cannot enforce arbitrary producers.
- A check-then-connect race needs the user's own console to close inside a microsecond window under a non-sticky shared temp — accepted.
- An agent under `sudo -E` with an inherited identity is now refused; the refusal names the directory.
<!-- end flow-study -->


## Decided in the ticket thread
Empty thread (orchestrator supplied comments: []).

## Relevant domain knowledge
No configured knowledge search; repository evidence is recorded in the design.

## Repo state at start
Branch: main; HEAD acdc474dc4a2702f68c17c554c2ead300161e63b. Clean checkout.

## Decisions clarified at start
No open questions. Existing flow-study block supplies concrete scope and criteria. kind null routes as feat. why=study forbids code changes.

## Estimated size: M
Multiple Rust/JS modules and a security boundary; a single atomic PR is expected.

## Effective configuration
Listed FLOW.claude.md, FLOW.codex.md, FLOW.opencode.md; read only active FLOW.codex.md. No FLOW.md; FLOW_CONTEXT=unattended, FLOW.unattended.md absent. autonomy.mode=unattended, orchestrator_cmd=ai, worktree=off, models.agents/workers=gpt-6-luna from FLOW.codex.md. Publication language English per user. CORE version 0.116.1; installed plugin manifest absent, version comparison unavailable.
