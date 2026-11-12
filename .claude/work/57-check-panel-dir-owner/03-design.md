# Design #57

## Approaches considered
Not run at this size (S).

## Additional domain context
- `PANEL_ID` is a name, not a credential (card `panel-id-is-a-name-not-a-credential`): the `0700`
  directory is what does the work, so a stale identity pointing at a directory that is no longer
  ours removes the only boundary there is.
- D88 card: every reader of the identity should use `client::whole_identity`; the session helper
  keeps its own rule, deferred. Not reopened here.
- `PanelDir::create` adds a single component under the base dir, `0700` set explicitly (D7,
  `docs/02-decisions.md:711-714`); the directory is removed non-recursively when the console closes.

## What already exists (inventory)
Full list in `inventory.md`. What matters:
- `crates/panel-control/src/client.rs:30` `whole_identity`; `:66-108` `Producer::of_this_pane` (the
  MCP bridge's only door, wraps failures as «could not be reached at <dir>»); `:113` `Producer::of`
  (tests and the app, pair taken as given).
- `crates/panel-control/src/endpoint.rs:124` `name_of`, `:143` `BINDS` (false on Windows), `:183` `connect`.
- `crates/panel-control/src/session/helper.rs:70-91` `exchange` — own identity rule, connects with a 2 s timeout.
- `crates/panel-control/src/bin/ivy-session.rs:19` — discards the helper's error text.
- `resources/agent-integrations/opencode/ivy-opencode.tsx:27-76` — connects to `join(dir,"control.sock")`
  itself and reconnects every 1 s forever.
- `crates/panel-contract/src/lib.rs:171` `create_dir` (mode `0o700`, unix only), `:213,256` base dir.
- panel-control already depends on `libc` (unix); panel-contract does not.
- No client checks owner or mode today. No uid check anywhere in the repo.

## Executive summary
- One function in `endpoint.rs`, `private_to_this_user(dir)`: `lstat` the directory; it must be a
  directory (not a symlink), owned by the effective uid, with no group/other bits. A directory
  that is gone is said as gone (the console closed), not as «not private».
- Called by the two Rust doors that use an inherited identity: the bridge (`of_this_pane`) and the
  session helper (`exchange`). Not by `Producer::of` or `endpoint::connect`.
- The opencode plugin repeats the same check in JS on **every** connect attempt.
- A refusal names the directory and says why; `ivy-session` stops discarding that text.
- Windows: no-op (no channel binds there until F2.7). macOS runs the unix code.

## Acceptance criteria

| ID | Given / When / Then | Proof |
|----|---------------------|-------|
| AC1 | Given a genuine console (`PanelDir::create`), when the MCP bridge, the session helper or the opencode plugin connects, then it connects exactly as before | test |
| AC2 | Given `PANEL_DIR` with any group or other permission bit (e.g. `0755`), when the bridge or the helper connects, then it refuses without connecting, and the error contains the directory path and `not a private directory of this user` | test |
| AC3 | Given `PANEL_DIR` owned by another uid, when the check runs, then it refuses (covered by injecting the expected uid, no root needed) | test |
| AC4 | Given `PANEL_DIR` is a symlink (even to a genuine console directory of the same user), when the check runs, then it refuses | test |
| AC5 | Given `PANEL_DIR` does not exist, when the bridge or the helper connects, then it refuses (fail closed) with the directory named and the text saying it is gone — not `not a private directory` | test |
| AC6 | Given the opencode plugin with a `PANEL_DIR` that fails the check, then no connection is opened, and it retries with a fresh check every second | test |

## Modules/layers affected
- `crates/panel-control/src/endpoint.rs` — new `private_to_this_user` + `private_to(dir, uid)`.
- `crates/panel-control/src/client.rs` — `of_this_pane` calls it before `Self::of`, inside the existing `map_err`.
- `crates/panel-control/src/session/helper.rs` — `exchange` calls it before `name_of`, error wrapped naming the directory.
- `crates/panel-control/src/bin/ivy-session.rs` — keeps the helper's error text in `message`.
- `resources/agent-integrations/opencode/ivy-opencode.tsx` — same check inside `connect()`.
- `docs/02-decisions.md` (D7 amended), `docs/03-architecture.md` security table.

## Data model
N/A — no schema. Migrations: none.

### Access paths
N/A — no data access.

## External contracts
None changes shape. The helper's error `message` text and the bridge's refusal text gain the
reason; both are free text read by people, not parsed.

## Internal behavioral contracts
- **Rule**: `private_to(dir, uid)` is `Ok` iff `symlink_metadata(dir)` succeeds, `file_type().is_dir()`,
  `st_uid == uid` and `mode & 0o077 == 0`. Anything else → `Err` whose text says
  `not a private directory of this user` plus the observed owner uid and mode.
- **Edge**: mode `0o700` passes; `0o750`, `0o705`, `0o755` fail.
- **Edge**: lstat failing → `Err`, never `Ok`; `NotFound` keeps its kind and reads as «the directory
  is gone (the console closed)», any other lstat error is passed through as is.
- **Invariant**: on Windows `private_to_this_user` returns `Ok(())` and is never the reason a connection fails.

## Depends on other tickets
none

## Defensive mechanisms and their justification

| Mechanism | Real scenario that justifies it (with evidence) | Needed now? |
|-----------|--------------------------------------------------|-------------|
| Owner + mode check before connect | Ticket #57: a stale identity under shared `/tmp` reconnects to a socket another local user planted after the console closed | yes |
| `lstat` + must be a directory | Not a cross-user threat (a foreign symlink already fails the uid clause; Linux `protected_symlinks`, macOS per-user `$TMPDIR`): a symlink is simply never what `PanelDir::create` makes, so accepting one would accept a directory that is not a console's | yes — same call as `stat`, no extra cost |
| Check on every opencode connect attempt | The plugin reconnects every 1 s forever (`ivy-opencode.tsx:74`), the exact window when the directory can be replanted | yes |
| Peer-uid check on the connected socket (`SO_PEERCRED`/`getpeereid`) | Only a race where the user's own console closes inside a microsecond window under a non-sticky world-writable temp | **out** — no present scenario |
| Root exception | `sudo` resets the environment; a root exception would accept the attacker's directory | **out** |

## Implementation plan (order)
1. `endpoint.rs`: `private_to(dir, uid)` (unix) + `private_to_this_user(dir)` with `libc::geteuid()`; non-unix returns `Ok(())`. Unit tests.
2. `client.rs` `of_this_pane`: call it after the `BINDS` check, inside the existing «could not be reached at <dir>» wrapper. Handshake integration test.
3. `helper.rs` `exchange`: call it before `name_of`, wrap the error naming the directory; `ivy-session.rs` keeps the text. Session helper test.
4. `ivy-opencode.tsx` `connect()`: `fs.lstatSync(dir)`, `isDirectory()`, `uid === process.geteuid()`, `(mode & 0o077) === 0`, else treat as a failed connect (retry in 1 s). `plugin.test.mjs`.
5. Docs: amend D7 with the client-side check and what was discarded; security table row.

## Planned tests
- Unit (`endpoint.rs`, unix): real `PanelDir` passes (AC1); `0o750`/`0o705` refused (AC2 — kills a dropped or one-sided mask); `private_to(dir, geteuid()+1)` refused (AC3 — kills a dropped uid clause); a user-owned symlink to a real `PanelDir` refused (AC4 — kills `stat` for `lstat`); a missing dir refused (AC5 — kills fail-open).
- Integration: `panel_mcp_handshake` — a live channel on a `PanelDir` chmod'ed `0755` makes a tool call refuse with the directory and the reason (AC2 — kills removing the call in `client.rs`). Existing `nowhere()` tests unaffected (the handshake never opens the directory).
- Integration: `session_helper` (linux) — same `0755` setup, helper fails, the service callback never runs, output names the directory (AC2 — kills removing the call in `exchange`).
- JS: `plugin.test.mjs` — a `0755` `mkdtemp` dir and a stubbed `process.geteuid` each leave `connections === 0` after a pause (AC6); the existing `0700` test still connects (AC1).

## Identified risks
- Security: the file transport (`$PANEL_DIR/panel.json` written by scripts) can still write into a planted directory; this fix covers the socket clients only. Accepted — the app cannot enforce anything for arbitrary producers.
- Security: check-then-connect race (see the mechanisms table). Accepted.
- Compatibility: an agent run under `sudo -E` with an inherited identity is now refused where it used to connect. Accepted; the refusal names the directory.
- Compatibility: the session helper still accepts a relative `PANEL_DIR` — D88's deferred convergence, not reopened.
- Performance: one `lstat` per connection. Nothing to measure; not on the frame path.

## Decisions (ADR-light)
| Decision | Discarded alternative | Why |
|----------|-----------------------|-----|
| Check in `endpoint.rs`, called from `of_this_pane` and `exchange` | Inside `endpoint::connect` / `Producer::of` | Those take a pair as given (tests, the app's own panes); the threat is only an *inherited* identity, and the helper does not go through `connect` |
| `mode & 0o077 == 0` | `mode == 0o700` | The property that closes the attack is «group and others have nothing»; the owner's own bits protect nobody, and testing them would refuse for a reason unrelated to the threat |
| `lstat`, must be a directory | `stat` | A symlink is never what `PanelDir::create` makes; following it reaches another directory |
| `NotFound` said as «gone» | One «not private» text for every failure | A closed console is the common case; «not private» there sends the reader looking for an attack |
| Fail closed on lstat error | Skip the check when the directory is unreadable | A missing directory is exactly the stale-identity case |
| Same check duplicated in JS | Route opencode through the `ivy-session` helper | The plugin streams documents over a long-lived socket; the helper is a one-shot request/reply process |

## Design challenges
Challenger verdict: the design is tight — no high or medium findings. Full table in `design-challenge.md`.

| Angle | Finding | Type | Severity | Response |
|-------|---------|------|----------|----------|
| Fit/need | `lstat` row justified by a cross-user threat that does not exist | idiom | low | Rationale rewritten: a symlink is never what `PanelDir::create` makes |
| Assumption | A missing directory would read «not a private directory», which looks like an attack | missing | low | `NotFound` said as «gone»; AC5 and the internal contract updated |
| Assumption | «macOS by default» uses a shared temp is false for GUI launches (per-user `TMPDIR`) | idiom | low | D7's amendment states the threat as Linux without `XDG_RUNTIME_DIR` or an explicitly shared `TMPDIR` |
| Operation | `ivy-session` now forwards the helper error text (directory path, uid) | — | low | Accepted; no consumer parses `message` |
| Decision idiom | `& 0o077` justified by umask, which is not checkable | idiom | low | Rationale rewritten to the property itself; `0o500` edge dropped |
