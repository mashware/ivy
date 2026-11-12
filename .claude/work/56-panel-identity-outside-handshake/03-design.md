# Design #56

Paths are relative to the worktree `.worktrees/56-panel-identity-outside-handshake`. Panel run in
`auto` (flow mechanics, not asked): approach panel of 3 lenses, then architecture and security passes.
Detail behind each section: `approaches/*.md`, `design-architecture.md`, `design-security.md`.

## Approaches considered

**A. Minimum — release on done, stat before promising, one identity rule; item 1 left out.**
Items 2–6 as below; item 1 recorded as a follow-up because nothing the bridge sees tells a borrowed
identity apart (D88). Risk: the headline symptom (a wrong console's panel changing) stays unfixed.

**B. Reframe — fix it in the window and the launcher, not the bridge.** Item 1 declared a launcher
problem (Ivy's `ivy-codex` already passes `--no-daemon`) and documented; item 2 reduced to a
`sun_path` check where the pane directory is created; items 3–6 as A. Risk: item 1 stays open for
tmux, editors and other daemons; its claim that ancestry cannot tell a daemon apart holds only for a
daemon that did not reparent.

**C. Operations — verify the caller, release on done, say why.** On Linux, the window checks
whether the connecting process descends from the pane's own child; a reparented caller (a detached
daemon, a tmux server) is refused with a named reason; elsewhere «unverified». Items 2–6 as A. Risk:
false refusals — a person's single agent inside tmux in its own console loses its panel, which D88
rejected.

**Panel consensus / disagreement.** All three agree on item 3 (end the hold on the producer's own
`done`, amend D43) and on items 4–6. They disagree on item 1 (leave out · document · refuse) and on
item 2 (stat in the bridge · `sun_path` at directory creation).

**Ranking for this case:** C's mechanism with A's restraint — *label, never refuse* — then A, then B.
The ancestry check is the only signal the platform gives that is not the inherited name, and as a
label it costs no false refusal.

**Chosen:** C's ancestry check, applied as a **label** (user decision), on **Linux and macOS** (user
decision); item 3 released **to the file only** (security pass); A's items 2, 4, 5, 6.

## Decisions clarified while choosing the approach

- *How should Ivy treat a caller that does not run inside the console whose identity it carries?*
  → **Allow it, label it**: the panel updates and the application row says it was published from a
  process outside this console. Not a refusal. (user, 2026-11-12)
- *Build the label on macOS too?* → **Linux and macOS**: each platform gets its own module for the
  peer pid and the parent walk; proved on a real Mac. Windows has no channel until F2.7. (user,
  2026-11-12)

## Additional domain context

- `mcp-instructions-reach-subagents` — subagents share the connection that holds the pane; every
  reader of the identity must use `client::whole_identity` (the session helper is the known
  exception — item 4).
- `panel-id-is-a-name-not-a-credential` — the id authenticates nothing: the label is information,
  and nothing (actions, the hold, MCP scope) may be gated on its absence.
- `the-contract-removes-what-the-contract-puts-there`, `a-closing-pane-answers-one-read-and-leaks-its-acceptor`
  — closing removes the pane directory before the channel, so a stale identity meets a missing
  `control.sock`.

## What already exists (inventory)

See `inventory.md`. Key reuse:
- `client::whole_identity` (`crates/panel-control/src/client.rs:30`) — the identity rule.
- `Peer::capture` (`crates/panel-control/src/session/peer.rs:52`) — Linux peer pid + pidfd + birth;
  today captured only when a session handler exists (`listener.rs:390`).
- `ForegroundScope::inspect_descendant` (`crates/term-core/src/foreground.rs:78`,
  `foreground/linux.rs:66`) — already walks from a pid up to the pane's child checking birth on both
  ends; it folds «outside» and «cannot tell» into one `PermissionDenied`.
- `PanelState::deliver` / `patch` / `retire` / `release` (`crates/app/src/panel/driving.rs`).
- `panel_contract::base_dir` (`crates/panel-contract/src/lib.rs:213`) — the absolute runtime/temp
  rule with its one warning.
- `resume::Route` (`crates/app/src/resume.rs:7`) — the swappable-cell shape for state that changes
  when a pane's shell restarts.

## Executive summary

- **A borrowed identity is visible.** On Linux and macOS, the first publication of a connection is
  checked against the pane's own process tree; a caller outside it still publishes, and the panel's
  application row says so. Undecidable → no label.
- **A dead console is not promised a panel.** The bridge withholds its handshake instructions when
  `PANEL_DIR/control.sock` is not there (a stat, never a connect).
- **`done` gives the panel back to its file.** A holder that publishes or patches `attention: done`
  stops blocking `panel.json`; it may publish again itself; other connections stay refused until it
  disconnects; it can still retire what it published.
- **One rule per thing.** The session helper uses `whole_identity`; the shell-integration rc dir uses
  `base_dir`; the Connect guide asks for the manual publishing prompt only where the client did not
  pass the server's instructions, and the prompt carries the subagent rule.

## Acceptance criteria

| ID | Given / When / Then | Proof |
|----|---------------------|-------|
| AC1 | Given a pane on Linux or macOS, when a process that descends from that pane's child connects and publishes, then the panel shows the document and the application row carries **no** outside label. | test (Linux) · manual (macOS) |
| AC2 | Given a pane on Linux or macOS, when a process that does **not** descend from that pane's child (e.g. the test runner itself, or a detached daemon) publishes, then the document is shown **and** the application row reads `published from a process outside this console`. | test (Linux) · manual (macOS) |
| AC3 | Given an outside-labelled document, when its connection disconnects, the label stays with the document; when a newer `panel.json` is painted, the label goes. | test |
| AC4 | Given the check cannot decide (peer capture unsupported, the pane's program exited, a birth mismatch), then no label is painted and the publication is applied. | test |
| AC5 | Given `PANEL_ID`/`PANEL_DIR` whole but `PANEL_DIR/control.sock` absent, when `panel-mcp` answers `initialize`, then the reply has **no** `instructions` key; with a bound listener at that path it has them. | test |
| AC6 | Given connection C holds a pane, when C publishes (or patches to) `attention: done`, then a `panel.json` written afterwards is read and painted. | test |
| AC7 | Given AC6's state, when a second connection D publishes, then D is refused naming C's source; when C publishes again, C's document is painted and the file is no longer read. | test |
| AC8 | Given AC6's state, when C calls `panel_retire`, then the retirement succeeds (its auxiliary consoles close, D50) — the hold survives `done`, so retirement needs no exception. Given AC6's state and a newer file painted, C's `panel_patch` is refused (it would patch the file's document); C's `panel_set` still wins. | test |
| AC9 | Given a file written while C held the pane *before* `done`, when C publishes `done`, then that older file is not resurrected; only a file newer than C's last publication is read. | test |
| AC10 | Given `ivy-session` runs with a relative `PANEL_DIR` or a non-UTF-8 `PANEL_ID`, then it reports `outside_ivy` (not a helper error) and connects to nothing. | test |
| AC11 | Given a relative `TMPDIR` and no `XDG_RUNTIME_DIR`, when a console starts with shell integration, then its rc directory is absolute (under `/tmp`) and the one `base_dir` warning names `TMPDIR`. | test |
| AC12 | The Connect guide's copied prompt asks for the manual publishing prompt only «if this client did not pass you the bridge's server instructions», and that prompt (and the README quote) carries D88's subagent rule. | test |

## Modules/layers affected

- `crates/term-core` — `foreground.rs`: a sibling of `inspect_descendant`, `descends(pid, birth) ->
  io::Result<bool>` (Err = cannot tell); `foreground/linux.rs` as today; **new `foreground/macos.rs`**
  (`proc_pidinfo` `PROC_PIDTBSDINFO`: `pbi_ppid`, `pbi_start_tvsec/usec`).
- `crates/panel-control` — `session/peer.rs` split into `peer/linux.rs` (today's code) and
  `peer/macos.rs` (`getsockopt(SOL_LOCAL, LOCAL_PEERPID)` + start time, which must precede the accept
  since macOS has no pidfd); captured for every connection; `listener.rs`: `Services.origin` closure, verdict
  computed once on the connection thread at its first `PanelSet`, carried in `What::Publish { source,
  panel, outside }`; `endpoint.rs`: `socket_present(dir)`; `bin/panel-mcp.rs`: `instructions_for` filtered by
  it; `session/helper.rs`, `bin/ivy-session.rs`: `whole_identity`.
- **No new recording of the pane child.** `Pty::foreground_scope()` (`term-core/src/lib.rs:450`)
  already returns a `ForegroundScope` holding the child's pid, birth, a pidfd and the pty's stop flag
  (`foreground/linux.rs:11-43`), and `resume.rs:7-31,148` already routes one through a swappable cell
  to a connection thread. The label takes **its own scope instance** through that same shape (today
  one is built only while session capture is on, `resume.rs:132-150`). Pid reuse is covered by the
  birth check (plus the pidfd on Linux) — **not** by the child being unreaped: `Pty::exit_success`
  reaps it with `try_wait` (`lib.rs:455-462`).
- **macOS port, whole.** `ForegroundScope`'s only field is `cfg(linux)` (`foreground.rs:11-15`) and
  `Peer::is_alive` answers `false` off Linux (`peer.rs:47-50`): reusing either as-is on macOS turns
  every verdict into «cannot tell», the label never shows, and nothing fails. So the macOS module
  gives the scope its own fields (pid + `pbi_start` of the child) and `peer/macos.rs` never routes
  through `is_alive`; every other target keeps a fallback that compiles and answers «cannot tell».
- `crates/app` — `control/channel.rs`, `opening.rs:644`, `resume.rs:272`: the label's scope cell, set
  at spawn and replaced on restart, reusing `resume::Route`'s shape; `panel/driving.rs`: `published_by`, `Holder.done` set by
  `deliver`/`patch`, `refresh_file` reading through `read_if_newer` while `done`, `patch` requiring
  the current document to be this connection's; the held-only warning and the backed-claim rule become
  «held and not done»; `pane.rs:561` `warning()`: the label; `shell_integration.rs:70`: `base_dir`;
  `chrome/connection.rs`: conditional prompt step + subagent rule; README quote.
- `crates/panel-contract` — `base_dir` made public.
- `docs/02-decisions.md` — D43, D88, D7 amended (below). D50 untouched: the hold survives `done`.

## Data model

- New / modified entities: `PanelState.published_by: Option<(Connection, Outside)>` — no equivalent
  (`Holder` cannot carry it: the hold ends at `done` while the document stays). `Holder` gains a
  `done` state (released to the file, still the only connection admitted).
- Migrations: none. On-disk formats: none (`panel.json` contract unchanged; the label is the app's own
  row).
- Indexes: N/A.

### Access paths
N/A — no data access.

## CQRS / Commands and Queries
N/A.

## API / HTTP
N/A. The MCP tool surface does not change; only the `initialize` reply's `instructions` key appears in
fewer cases (AC5).

## Identified risks

- Performance: the ancestry walk runs on the connection thread, once per connection, never in the
  frame; `/proc` reads (Linux) or `proc_pidinfo` (macOS) per hop — bounded by process depth. No frame
  cost, so nothing new to declare under D13.
- Security: the label is information only — nothing is gated on its absence (handled: design rule,
  stated in D7). A same-uid process with ptrace (Yama scope 0) could make an outside caller look
  inside; accepted, because the same process can write `panel.json`, which gets no label either.
- Accepted limit (a trade-off the user chose, not deferred work): a borrowing process still parented
  inside console A (an editor or a non-daemonised server started from A) reads as A's and gets no
  label; and tmux started *inside* console A is labelled «outside» for its own agent — a label, never
  a refusal. Written into D7/D88.
- Ordering: `control.sock` is bound at `opening.rs:646`, after the program is spawned at
  `opening.rs:621`. An agent launched as the pane's own program (`resume.rs:114`) whose bridge answers
  `initialize` within that sub-millisecond window loses the instructions for its session (its tool
  calls still work). Accepted and named in D88; moving the bind before the spawn is not this ticket.
- Compatibility: Windows unchanged (no channel until F2.7). An agent that publishes `done` and keeps
  working re-claims with its next publish (AC7).
- Online migrations: none.

## External contracts

- **MCP `initialize` reply** — `result.instructions` is now absent also when `PANEL_DIR/control.sock`
  does not exist. Literal: present → `"instructions": "This session runs in an Ivy console…"`; absent →
  no key. Known consumer: any MCP client (harnesses). Pattern deviation: none.
- **Panel application row (painted, not a wire contract)** — literal text
  `published from a process outside this console`. Consumer: the person.
- No change to `panel.json`, the socket wire, or the tool schemas.

## Internal behavioral contracts

- `descends(pid, birth)`: pid is the pane child or a descendant whose chain reaches it with matching
  birth → `Ok(true)`; the chain reaches pid 1 / launchd without meeting it → `Ok(false)`; the pane's
  program exited, a birth mismatch, or an unreadable hop → `Err` (no label).
- Hold states for a pane: `None` → (publish by C) → `Held(C)` → (C publishes/patches with
  `attention: done`) → `Done(C)`: file read again (only if newer than C's last publication), C may
  publish (→ `Held(C)`) or retire, every other connection refused naming C's source → (C disconnects)
  → `None`.
- `instructions_for(id, dir, binds, socket_present)`: `None` unless `binds && whole_identity(id, dir)
  && socket_present` — named for what a stat proves (the path exists), not «reachable»: after an Ivy
  crash the socket file can remain and connects fail; the first tool call still says so (D88).

## Depends on other tickets

none.

Not dependencies (either order works):
- #57 (check the owner of `PANEL_DIR` before connecting) — touches the same client path; whichever
  merges second rebases a few lines, no premise of this design rests on it.
- #54 (publish a panel from a shell command for hooks) — a new producer; it inherits the done-release
  and the label without needing them.

## Defensive mechanisms and their justification

| Mechanism | Real scenario that justifies it (with evidence) | Needed now? |
|-----------|--------------------------------------------------|-------------|
| Birth/start-time check on each ancestry hop and at the pane child | pid reuse after the pane's shell exits and is reaped (`Pty::exit_success` reaps, `lib.rs:455-462`) makes an outside tree read as inside; reused from the existing walk | yes |
| Peer pid read between two pidfd liveness checks (Linux) | already the pattern in `peer.rs:72-78`; the peer may exit while `/proc` is read | yes (reused) |
| «Cannot tell» → no label | the label must never lie; an unsupported platform or an exited program is not evidence of «outside» | yes |
| `Done(C)` still refuses other connections | agents publish `done` at a stop and keep working — the bridge's own instructions say «republish without it as soon as it stops being true» (`panel-mcp.rs:263-265`) — so C must be able to re-claim; a second producer taking the pane between C's stops would refuse C at every later stop (AC7) | yes |
| File read after `done` only if newer than C's last publication | **not new**: `refresh_file` already reads through `read_if_newer(dir, last_good.written_at)` (`panel.rs:390-392`) and `deliver`/`patch` stamp `written_at = now`; it follows once `Done` passes the holder early-return (`panel.rs:377-381`). Kept as the AC9 regression test | — (existing) |
| `sun_path` length check in the bridge | Ivy binds the same absolute path, so an over-long path never gets a socket and the stat already withholds | **no — cut** |

## Implementation plan (order)

1. term-core: `descends` on Linux + macOS module; tests with a pty-born setsid grandchild.
2. panel-control: peer capture for every connection (+ macOS `LOCAL_PEERPID`), `Services.origin`,
   `What::Publish.outside`; `endpoint::socket_present` + `instructions_for`.
3. app: pane-child cell (spawn + restart), `published_by`, the label in `warning()`; `Done(C)` hold
   state and retirement after done.
4. Items 4–6 (helper, rc dir, Connect guide + README).
5. Decisions D7, D43, D88 amended.

## Planned tests

- Unit: `descends` true/false/Err (AC1, AC2, AC4); `instructions_for` with/without socket (AC5);
  `PanelState` done-release, refusal of D, re-claim, retire, stale file (AC6–AC9).
- Integration: app test with a process inside the pty publishing over the socket vs the test process
  publishing (AC1–AC3); `panel_mcp_handshake.rs` no-socket case (AC5); `session_helper.rs` relative
  dir / non-UTF-8 id (AC10); shell-integration re-run with relative `TMPDIR` (AC11);
  `integrated_chrome.rs` prompt text (AC12).
- Manual: AC1/AC2 on a real Mac.

## Decisions (ADR-light)

| Decision | Discarded alternative | Why |
|----------|-----------------------|-----|
| Label an outside caller | Refuse it · leave it out | User decision: a refusal costs a tmux user their panel (D88's trade-off); leaving it out keeps the wrong panel changing silently |
| Linux and macOS | Linux only | User decision; Ivy ships a macOS DMG its owner runs |
| Verdict on the connection thread, once | In the frame · per publication | D13: no new frame work; the peer of a connection does not change |
| Flag on the document (`published_by`) | On `Holder` | The hold ends at `done` while the document — and its origin — stays on screen |
| `done` releases to the file only | Full release · keep D43 as is | Full release lets a second connection hold the pane while C keeps working past its `done`, refusing C at every later stop (AC7); keeping D43 refuses a later file producer until the bridge exits (the ticket's item 3) |
| Stat `control.sock` before promising | Connect probe (D88 discarded) · nothing | A stat is not the connect D88 discarded; a closed console's agent stops being told to publish into nothing |
| `sun_path` check cut | Check length in the bridge | Ivy cannot bind an over-long path, so the stat already covers it |

## Design challenges

Challenger: one `general-purpose` agent (`design-challenge.md`). No high finding.

| Angle | Finding | Type | Severity | Response |
|-------|---------|------|----------|----------|
| Fit | A new pane-child cell and start-time recording duplicate `ForegroundScope` and `resume::Route` | unnecessary | medium | Adopted: the label reuses both; no new recording (Modules) |
| Fit | The read_if_newer gate already exists (`panel.rs:390-392`) | unnecessary | low | Adopted: kept only as AC9's regression test |
| Fragile | «The child is unreaped while `Pty` owns it» is false (`lib.rs:455-462` reaps) | idiom | medium | Adopted: premise removed; birth check (+pidfd on Linux) is the guard. Must not reappear in a comment |
| Fragile | The stat can race the bind (`opening.rs:621` spawn before `:646` bind) | missing | low | Assume and document: sub-millisecond window, tool calls unaffected; named in D88 |
| Production | macOS: `ForegroundScope` field is `cfg(linux)`, `Peer::is_alive` false off Linux → label silently never shows | missing | medium | Adopted: macOS gets its own scope fields and never routes through `is_alive`; other targets keep a compiling «cannot tell» fallback; AC1/AC2 proved on a real Mac |
| Idiom | `endpoint::reachable` names what a stat does not test | idiom | low | Adopted: `socket_present` |
| Idiom | `Done(C)`'s rationale (borrowed agent) is undone by the file being readable | idiom | low | Adopted: rationale is AC7 — agents publish `done` at a stop and keep working, so C must re-claim |
