# #57 — Check the owner of PANEL_DIR before connecting

- What: a client with an inherited identity refuses to connect when `PANEL_DIR` is not a private directory of this user, so a stale identity cannot stream panels to another user's socket.
- Size: S · branch `57-check-panel-dir-owner` (worktree) · study for handoff (`ai-study/#57`).
- Decisions: one `private_to_this_user(dir)` in `panel-control/src/endpoint.rs` — `lstat`, a directory, owner = euid, `mode & 0o077 == 0`; fail closed, `NotFound` said as «gone».
- Called from the MCP bridge (`of_this_pane`) and the session helper (`exchange`), not from `Producer::of`/`endpoint::connect`; `ivy-session` keeps the error text.
- opencode plugin repeats the check in JS on every 1 s reconnect. Windows: no-op until F2.7.
- Out: peer-uid socket check, root exception, helper's relative-path rule (D88 deferred).
- Contracts: none. Depends on: none. Criteria AC1–AC6 in `03-design.md`.
- Next: handoff of the study; the builder opens `03-design.md` whole.
