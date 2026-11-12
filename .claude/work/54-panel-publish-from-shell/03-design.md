# Design #54

Approach panel run in auto mode: three advisors (minimum, reframe, operations — the last because the
change sits on the trust boundary `crates/panel-control/**`), 3/3 answered. Files in `approaches/`.

## Approaches considered

### A. Attested hook publish bound to the holder's client (operations lens)
- **What:** `ivy-session publish` (the D84 helper, located by `IVY_SESSION_HELPER`) reads a document on stdin and sends it as a new request on the pane's endpoint. The window accepts it only when the peer's attested native client root (D84: `SO_PEERPIDFD` peer + `ForegroundScope::inspect_descendant`, `crates/app/src/ai_sessions/verify.rs::root`) is **the same root (pid + birth)** as the root of the connection holding the pane. Then it lays the hook's document over the held one, keeping the holder's `actions`.
- **Modules:** `panel-control` (`session.rs` request, `wire.rs` bounds, `bin/ivy-session.rs`), `app` (`ai_sessions/verify.rs`, `panel/driving.rs` overlay + confession).
- **Main risk:** the attestation is Linux-only (`term-core/src/foreground.rs`: every other OS returns `Unsupported`), so on macOS the publish is always refused. Client-specific: a Claude version that reparents hooks degrades to refusal.
- **Why it could be bad:** the overlay is a second writer to a held document (D17 "never settled by chance"), so it needs a precise rule for who wins.

### B. Attested overlay, not bound to the holder (minimum lens)
- **What:** the same as A, but any recognised client in the foreground may publish. The overlay replaces `lines` and `attention`, and the holder's `actions` always win at paint time. The overlay is dropped on the holder's next `panel_set` or `panel_retire`, or on disconnect.
- **Main risk:** the same Linux-only attestation; whether Claude's `PostToolUse` hook ancestry passes it is unproven.
- **Why it could be bad:** without the root binding, a second recognised client in the same console (a worker, D88) can overlay a panel it does not hold, which is the collision D43 exists to stop.

### C. The holder forwards the file (reframe lens)
- **What:** no new verb. `panel.publish_cmd` is a one-liner that writes `$PANEL_DIR/panel.json` atomically. `panel-mcp`, the holder, watches the file and republishes it on its own connection, merging in its last `actions`.
- **Modules:** only `bin/panel-mcp.rs` (a watcher thread and a merge), plus a decision.
- **Main risk:** any same-uid process that writes `panel.json` now reaches a held pane through the holder. D43 part 2 (the file is not read while held) stays true in letter and false in effect.
- **Why it could be bad:**
  - It works only while the bridge lives.
  - An old hook render can race a newer `panel_set`.
  - It adds a watcher to a bridge that is quiet by design.
  - It works on macOS, but by giving up the boundary.

### Ranking for this case
1. **A** — the only option that keeps D43's "one voice" with a proof that is not a name. It reuses D84's attestation as is and adds one comparison.
2. **B** — the same machinery, but it lets a worker overlay its parent's panel, which D88 explicitly tells workers not to do.
3. **C** — the cheapest and the only one that works off Linux today, but it hands the hold's protection to whoever can write a file, which is exactly what D43 discarded.

**Panel consensus / disagreement:**
- Consensus:
  - No bearer token (D50 discarded it).
  - `PANEL_ID` is never the proof.
  - The holder's `actions` survive the hook's document.
  - The command exits 0 and does nothing outside Ivy.
- Disagreement:
  - Whether the proof should be D84's process attestation (A, B) or no new authority at all (C).
  - Whether a publish must be bound to *this* holder (A) or to any attested client (B).
- **Fact checked after the panel:** `ForegroundScope` is Linux-only, so A and B cover no macOS console today, although Ivy ships a DMG and the owner runs it (D66, #34).

## Decisions clarified while choosing the approach
- **macOS coverage** — asked: Linux only now · weak route everywhere · safe on Linux and weak on macOS. The owner rejected that framing and asked why doing it properly on macOS was not offered. Re-asked with macOS attestation as an option, and the owner (12 Nov 2026) chose **«Bien en los dos»**: this work includes a macOS backend for the process attestation (`LOCAL_PEERTOKEN`, `proc_pidinfo`, `proc_pidpath`, `kqueue`) in its own PR before the command. It is not claimed working on macOS until it is confirmed on the owner's Mac.
- **Approach chosen:** A, the attested publish bound to the holder's client root. Recorded in `meta.json.notes`.
- **Size:** M → **L**. A second platform backend for the attestation, used by session recovery too, plus the overlay rule in the window.
- **Codex on macOS** — first taken in auto as `KERN_PROCARGS2`, then cut by the challenge: the publish path attests identity with `check_mode` off, so no process arguments are read on any platform, and Codex is attested on macOS by its native executable.
- **Buttons after a hook** (owner, 12 Nov 2026): the holder's buttons survive a hook only while the hook's document still says `wait`.
- **Who may repaint** (owner, 12 Nov 2026): any descendant of the holder's client. No opt-in switch.

## Additional domain context
- `panel-id-is-a-name-not-a-credential` — `PANEL_ID` authenticates nothing. Every descendant inherits it, and any process of the same user can read it.
- `mcp-instructions-reach-subagents` (D88) — subagents share the holder's connection, and a worker in the same console must leave the panel alone. Binding the publish to the holder's *root* keeps a worker with a client root of its own out.
- `a-bound-counts-the-encoded-width-not-the-decoded-value` — a wire line over `MAX_REQUEST` is `Unrecoverable`. The helper therefore re-serialises compactly and checks the encoded bytes.
- `a-frame-cost-is-declared-once` — the new apply arm is declared once, in the control report (D13).
- `the-transport-is-not-the-contract` (D17) — two producers are never settled by chance. Here the overlay has a written precedence: the holder's `panel_set` always wins whole.

## What already exists (inventory)
Full list in `inventory.md`. The pieces this design builds on:
- **The per-pane endpoint and listener** — `crates/panel-control/src/listener.rs` captures a kernel `Peer` for every accepted connection (`:390-393`).
- **The D84 attestation** — `crates/app/src/ai_sessions/verify.rs::root`, on top of `term-core` `ForegroundScope::inspect_descendant`. Linux-only today.
- **The helper** — `ivy-session` (`crates/panel-control/src/bin/ivy-session.rs`), located through the absolute path in `IVY_SESSION_HELPER`, which Ivy injects (`crates/app/src/opening.rs:608-612`).
- **The hold** — `Holder` in `crates/app/src/panel/driving.rs:208-223`, plus `deliver`, `retire` and the confession on the app's row (D43).
- **The document** — `panel_contract::parse_document`, `MAX_BYTES` (256 KiB), and `last_good` in the window's panel state.
- **What is new:** a wire tool `panel_publish`, the holder's root, the overlay commit, the helper subcommand, and the macOS backend of `ForegroundScope` and `Peer`.

## Executive summary
- **The new command:** `ivy-session publish` reads a panel document on stdin and sends it to the console's own endpoint as `panel_publish`. Flow's git-ignored line is `exec "${IVY_SESSION_HELPER:-true}" publish`. Outside Ivy it exits 0 and does nothing.
- **Who may publish:** only a process the kernel shows to descend from **the same native client root** (pid + birth) as the connection holding the panel, and that root must be in the console's foreground. `PANEL_ID` is never the proof, and no token exists (D44, D50).
- **What the hook replaces:** its document replaces the panel's `lines`, `attention`, `phase` and timing. The holder's `header` stays, and its `actions` stay only while the hook still says `wait`. The identity row reads `<source> · hook`. The holder's own next `panel_set` replaces everything, as today.
- **The hook never waits for a window pass.** The window publishes its hold (connection plus root) to a per-pane shared cell, so the publisher's connection thread checks it off-frame, queues the publication and answers. A minimised window cannot stall every tool call by five seconds.
- **macOS:** the attestation gets a macOS backend (`LOCAL_PEERTOKEN`, `proc_pidinfo`, `proc_pidpath`, `kqueue`, plus Mach-O recognition in `programs.rs`) in its own PR. It is claimed working only after the check on a real Mac.

## Acceptance criteria

| ID | Given / When / Then | Proof |
|----|---------------------|-------|
| AC1 | Given a Linux Ivy console where `claude`'s `panel-mcp` holds the panel with `attention: "wait"` and a `Decision` line, when a hook of that `claude` runs `"$IVY_SESSION_HELPER" publish` with a document without `attention`, then the command exits 0 printing `{"ok":true}`, the panel shows the hook's lines, the pane and its tab no longer claim `wait`, the holder's buttons are gone, and the identity row reads `flow · hook`. When the hook's document still carries `attention: "wait"`, the holder's buttons stay painted and pressable | test + manual |
| AC2 | Given the same held panel, when `publish` runs from a process that does not descend from the holder's client root — the pane's shell after the client quit, another console with the first one's `PANEL_ID`/`PANEL_DIR` exported, or a second recognised client started inside the first one's console — then it exits 1 with stderr `ivy-session: <reason>`, and the panel is unchanged | test |
| AC3 | Given a pane nobody holds, or one held by a connection whose root is not an attested client (a script), when `publish` runs from an attested client, then nobody holding exits **0** with `{"ok":false,"error":"nobody holds this console's panel: a hook publishes only over its holder's"}` on stdout and nothing on stderr (it is the normal state before the agent's first publication), and an unattested holder exits 1 with `this panel is held by "<source>", which is not an attested agent client`; nothing changes | test |
| AC4 | Given a process with no whole identity (`PANEL_ID` or `PANEL_DIR` missing or empty), or a platform where no endpoint binds, when `ivy-session publish` runs, then it exits 0 and writes nothing to stdout or stderr. And `exec "${IVY_SESSION_HELPER:-true}" publish` with the variable unset exits 0 | test |
| AC5 | Given a hook overlay on screen: when the holder publishes with `panel_set`, its document replaces everything and the row drops `· hook`; when the holder patches, the ops apply over what is on screen; when the holder's connection dies, the overlay stays on screen and the file transport takes over at its next publication (D43); when the holder retires (`panel_retire`), everything clears (D50) | test |
| AC6 | Given a window whose passes are not running (minimised, or a test with nobody draining the inbox), when an attested hook publishes, then the reply arrives in under 1 s, not after `ANSWER_DEADLINE` (5 s), and the document is painted at the next pass | test |
| AC7 | Given stdin over `panel_contract::MAX_BYTES`, or not a JSON object, when `publish` runs, then it exits 2 before connecting, with stderr naming the reason. A pretty-printed document under the ceiling is accepted — it is re-serialised onto one line | test |
| AC8 | Given a measured run, the control report declares `overlaid` and `overlays_refused` beside `applied`/`patched`, so the new apply arm is never published as framework cost (D13) | test |
| AC9 | Given a real Mac with the macOS backend, AC1 and AC2 hold for `claude`, `codex` and `opencode`, and `cargo test -p term-core foreground`, `-p panel-control session` and `-p app ai_sessions` pass there | manual |

## Modules/layers affected
- **`crates/term-core/src/foreground.rs`, `foreground/linux.rs`, new `foreground/macos.rs`:**
  - The ancestry walk moves up into `foreground.rs`.
  - The platform files keep only `snapshot`, `executable`, `group` and `lifetime`.
  - `arguments.rs` moves up beside the walk.
  - A shared helper, `lifetime_ended(&OwnedFd)`, replaces the direct `libc::poll` calls.
- **`crates/panel-control/src/session/peer.rs`:** a macOS branch. It reads `LOCAL_PEERTOKEN`, then `pid` and `pidversion`, then a `proc_pidinfo` snapshot whose `p_idversion` must match. `birth` is `p_uniqueid`, and the lifetime is a `kqueue`.
- **`crates/panel-control/src/wire.rs`:** a new tool, `panel_publish`, carrying only `document`.
- **`crates/panel-control/src/listener.rs`:**
  - `Services.attest: Option<Attester>`.
  - Per pane, a shared cell `Arc<Mutex<Option<(Connection, Option<ClientRoot>)>>>` that only the window writes: when `deliver` takes a hold, refreshes it, or ends it by close or retire. Listener threads only read it. One record of the hold, not two (challenge row).
  - `panel_publish`: attest the peer → compare with the holder's root → `try_send` → answer at once.
- **`crates/panel-control/src/bin/ivy-session.rs`:** the `publish` subcommand: whole-identity check, bounded stdin, compact re-serialisation, exchange, and exit codes.
- **`crates/app/src/opening.rs`:** builds the `Attester` from the pane's `ForegroundScope` and `verify::root`, independently of the restore mode.
- **`crates/app/src/panel/driving.rs`:**
  - `What::Overlay{root, panel}` re-checks the root against `Holder.root`, because the holder may have changed between the queue and the pass.
  - It commits `hook ⊕ {header, actions-while-wait}` (Internal behavioral contracts), and `Holder.painted = Hook`.
- **`crates/app/src/panel/cache.rs`:** `<source> · hook` on the identity row.
- **`crates/app/src/control/applying.rs` and the report:** `did.overlaid`, `did.overlays_refused`.
- **`crates/app/src/ai_sessions/verify.rs` and `programs.rs`:** `root` gains an identity-only mode (`check_mode` off), and `programs.rs` recognises Mach-O and the darwin Codex targets. Only these two compile on macOS. Session recovery stays Linux-only (F4).
- **`docs/02-decisions.md`:** the new decision, plus amendments to D43, D47, D50, D80 and D84.
- **`docs/04-panel-contract.md`:** the third transport, and what it may replace.
- **`docs/08-configuration.md`, or the agents guide:** the flow line.

## Data model
- New / modified entities: `ClientRoot {pid: u32, birth: u64}` (new — nothing equivalent; `NativeProcess` carries more than the comparison needs, and the root is what `verify::Root` already names); `Holder.root: Option<ClientRoot>`, `Holder.painted: Holder | Hook`.
- Migrations: **none** — no database. The only on-disk format touched is none (`panel.json` unchanged, layout unchanged).
- Indexes: none.

### Access paths
N/A — no data access.

## CQRS / Commands and Queries (if applicable)
Not applicable (no CQRS here). The wire verbs are the commands: `panel_publish` is new, a write that never claims.

## API / HTTP (if applicable)
Not applicable — the surfaces are the control socket and a CLI, declared below as contracts.

## Identified risks
- **Performance:**
  - The apply arm runs in the frame, and AC8 declares it.
  - Attestation walks `/proc` or `proc_pidinfo` on a connection thread, bounded at 64 hops as today.
  - Program discovery runs once per pane.
  - **F1:** no measurement batch exercises a hook publication, so its frame cost is declared but never measured. Logged as a bench gap.
- **Security:**
  - The proof guarantees that the publisher is a live descendant of the holder's client root, and that the root is in the console's foreground.
  - It does not guarantee that the code in that root is the vendor's. Same-uid `ptrace`, `DYLD_INSERT_LIBRARIES` or an edited hooks file all defeat it. That is the same limit D84 accepts, and it is written in the decision.
  - **F2:** a hook can replace the panel's `attention`, so a hook can hide a `wait` the agent raised. That is the purpose, but the agent's own hooks are as trusted as the agent.
- **Compatibility:**
  - An unattested holder — a script, or Windows — gets AC3's refusal, never a silent pass.
  - flow's existing behaviour outside Ivy is unchanged (AC4).
  - Session recovery keeps its behaviour on Linux. The refactor that hoists the walk is covered by the existing tests.
- **Platform:**
  - **F3:** that BSD ptys forward `TIOCGPGRP` on the master is reasoned, not observed. AC9 on a real Mac are the proof. Until then the PR says macOS is unverified (D53, D54).
- **Online migrations:** none.

## External contracts

### Contract 1: wire tool `panel_publish`
- **Type:** control-socket request and reply, one JSON line each.
- **Literal shape:**
  ```
  → {"tool":"panel_publish","document":{"lines":["…"],"phase":"build"}}\n
  ← {"ok":true}
  ← {"ok":false,"error":"this publication does not come from an attested agent client in this console's foreground"}
  ← {"ok":false,"code":"unheld","error":"nobody holds this console's panel: a hook publishes only over its holder's"}   (the helper keys its exit 0 on "code", never on the text; the publisher is attested before the cell is read)
  ← {"ok":false,"error":"this panel is held by \"<source>\", which is not an attested agent client"}
  ← {"ok":false,"error":"this panel is held by \"<source>\""}          (a different root: the existing held-by sentence, driving.rs:30)
  ← {"ok":false,"error":"the window is not taking publications right now"}   (inbox full on try_send)
  ```
  No `source` field: the publication paints under the holder's source, suffixed `· hook`.
- **Known consumer:** `ivy-session publish` (this repo).
- **Pattern deviation:** **`panel_set` answers after the window decides; this verb answers once it is queued.** The reason is that a hook runs after every tool call, and a minimised window runs no pass, so waiting would cost 5 s per call (`control/applying.rs:10-11`). The window still re-checks the root at apply time. A publication refused there is confessed on the app's row, as D43 confesses a refused file.

### Contract 2: `ivy-session publish` (CLI)
- **Type:** a command, document on stdin.
- **Literal shape:**
  ```
  usage:   ivy-session publish < document.json
  exit 0   {"ok":true} on stdout                        — queued for the window
  exit 0   nothing on stdout or stderr                  — outside Ivy (no whole identity, or no endpoint binds)
  exit 0   the reply line on stdout, nothing on stderr       — nobody holds the panel (normal before the agent's first publication)
  exit 1   the reply line on stdout; "ivy-session: <error>" on stderr — refused: a different root, or an unattested publisher or holder
  exit 2   "ivy-session: <reason>" on stderr            — usage, input not a JSON object, over 262144 bytes, endpoint unreachable
  ```
- **Known consumer:** flow's `panel.publish_cmd`, set by the person in a git-ignored FLOW file: `panel.publish_cmd: exec "${IVY_SESSION_HELPER:-true}" publish`.
- **Pattern deviation:** the other subcommands answer `{"ok":false,"error":"outside_ivy"}` with exit 1 outside Ivy. `publish` exits 0 silently, because the ticket requires the same FLOW line to work in every terminal.

## Internal behavioral contracts
- **Composition:** `commit(hook, held) = hook` with `header := held.header`, `written_at := now`, and `actions := if hook.attention == wait { held.actions } else { [] }`. Every other document field comes from `hook`. The hook's own `actions` and `header`, if present, are ignored, never merged.
  - Example: held `{attention: wait, actions: [Approve]}`, then a hook `{attention: absent}` gives a panel with no buttons and no claim.
  - Example: held `{attention: wait, actions: [Approve]}`, then a hook `{attention: wait}` keeps `[Approve]`.
- **Root rule:** `publish(peer)` is taken ⇔ `attest(peer) == Some(r)` ∧ `holder == Some((_, Some(r)))`. `None == None` never matches.
- **Precedence:**
  - The holder's `panel_set` replaces the composed document whole and sets `painted := Holder`.
  - A holder `panel_patch` applies to `last_good`, whatever painted it, and leaves `painted` as it is.
  - A hook publication sets `painted := Hook`.
- **Lifetime:**
  - When the holder's connection closes, the hold ends, `last_good` stays (D43) and `painted` is forgotten.
  - `panel_retire` clears everything (D50).
  - A hook never claims, releases or retires anything, and it never touches `file_refused`.
- **Edge:** a publication queued before the holder changed, then applied after the change, is refused at apply time and confessed. It is never applied over the new holder.

## Depends on other tickets
none. The pieces it needs (D84's attestation, the helper, the per-pane endpoint, `panel_patch`) are on `origin/main` 9e30f7a.

Not dependencies (either order works): none recorded.

## Defensive mechanisms and their justification

| Mechanism | Real scenario that justifies it (with evidence) | Needed now? |
|-----------|--------------------------------------------------|-------------|
| Root re-check at apply time in the window | The listener answers before the pass. Between the queue and the pass the holder can disconnect, and another client can claim the pane (D43 releases on close, `driving.rs:466-476`) | yes |
| Answering once queued instead of waiting for the window | A minimised window runs no pass (`control/applying.rs:10-11`), and flow's hook runs after every tool call that changed the plan or the record | yes |
| `pidversion == p_idversion` check when the macOS peer is captured | A PID is reused between `accept` and the snapshot. `LOCAL_PEERPID` alone would attest the newcomer | yes |
| Snapshot before and after on macOS | PID reuse between the PID-keyed reads (`getsid`, `proc_pidpath`). Linux does the same at `foreground/linux.rs:88-91` | yes |
| Compact re-serialisation in the helper | flow prints JSON on one line today, but the contract is "a document on stdin", and a pretty-printed one would split into several wire requests | yes |
| Re-checking that the holder's root is alive | — A dead root takes `panel-mcp` with it, so the hold ends on its own. Cut | no |

## Implementation plan (order)
1. **term-core:**
   - Hoist the ancestry walk and `arguments.rs` into `foreground.rs`.
   - Add `lifetime_ended`.
   - Make the tests call a neutral `native::birth`.
   - No behaviour change on Linux, and the existing suite proves it.
2. **term-core + panel-control:**
   - Add `foreground/macos.rs`: `snapshot`, `executable`, `group`, and `lifetime` via `kqueue`. In `programs.rs`, Mach-O recognition and the darwin Codex targets. Peer capture is gated on «session handler or attester».
   - Add the macOS branch of `Peer`.
   - Widen the `cfg` gates in `ai_sessions`.
   - Prove it with `cargo check --target aarch64-apple-darwin`, then on a real Mac.
3. **panel-control:**
   - Add `ClientRoot`, `Services.attest`, the listener's record of the holder root, and the `panel_publish` tool with its replies.
   - Add `ivy-session publish`.
4. **app:**
   - Build the `Attester` in `opening.rs`.
   - Add the `What::Overlay` arm in `driving.rs`, with the re-check and the confession.
   - Add `· hook` to the identity row.
   - Add `did.overlaid` and `did.overlays_refused` to the report.
5. **docs:** the new decision, the five amendments, the contract document, and the flow line in the agents guide and configuration.

## Planned tests
- **Unit:**
  - **AC1 (composition):** the hook's lines and attention win, the holder's actions and header stay, the row reads `· hook`.
  - **AC5 (state machine):** the holder's `panel_set` wins whole; its patch applies over the overlay; a close keeps the document; retire clears.
  - **AC2/AC3 (root rule):** `None` never matches `None`; a different root is refused; nobody holding is refused.
  - **AC7:** the helper's stdin bound, input that is not an object, and a pretty-printed document becoming one line.
  - **AC4:** the helper outside Ivy exits 0 and writes nothing.
  - **AC8:** the report declares both counts.
- **Integration (panel-control and app, real socket):**
  - **AC1:** a fixture client root with `panel-mcp`-like and hook-like descendants, built like `ai_sessions/tests.rs::detached_reporters_require_their_own_current_foreground_client_root`.
  - **AC2:** a descendant of another root, and a process outside the pane.
  - **AC6:** nobody drains the inbox, and the reply arrives in under 1 s.
  - **The apply-time re-check:** the holder changes between the queue and the pass, and the publication is refused and confessed.
- **Functional:** **AC1 and AC9** — a real `claude` in an Ivy console, flow's hook configured with the line, a question answered and the `wait` gone. On Linux, and on the owner's Mac for `claude`, `codex` and `opencode`.
- **Mutation, per the conventions:**
  - Break the root comparison, so that it always matches: AC2 must go red.
  - Drop the actions carry-over: AC1 must go red.
  - Remove the apply-time re-check: the integration test must go red.
  - Make the listener wait for the window: AC6 must go red.

## Decisions (ADR-light)

| Decision | Discarded alternative | Why |
|----------|-----------------------|-----|
| The proof is D84's process attestation with `check_mode` off, bound to the holder's own client root | A bearer token handed to hooks; `PANEL_ID`; any attested client, whoever holds; an opt-in switch per agent | D50 discarded tokens, which are a second authority system. `PANEL_ID` is a name (D44). Without the binding, a hook would overlay a pane held by a non-client producer such as a CI script (AC3). The switch was declined by the owner: every descendant of the agent speaks for it, and none can change a button's command |
| The hook overlays; the holder's `header` survives, and its `actions` survive only while the hook still says `wait` | Keep the buttons always; drop them always; ignore hooks while held; let the holder forward `panel.json` (approach C) | Owner's decision: buttons belong to a question. Kept always, they outlive their question; dropped always, they vanish after every tool call. Ignoring hooks is the 18-minute stale `wait`. Forwarding the file opens the pane to any same-uid writer (D43) |
| The answer comes once the publication is queued, and the window re-checks | Answer after the window applies, as `panel_set` does | A minimised window would charge 5 s to every hook call, and that happens on every tool call that changes the plan |
| A new wire tool on the panel connection path | A `session.rs` request | Session requests never reach the window and are capped at 16 KiB, while a document can be 256 KiB |
| A macOS backend for the attestation in this work | Linux only, with macOS later; a weaker route on macOS | Owner's decision, 12 Nov 2026 |
| `birth` on macOS is `p_uniqueid`, and the peer is matched by `pidversion` | `LOCAL_PEERPID`; start time; `pidversion` as the birth | `LOCAL_PEERPID` carries no version. Start time has microsecond resolution and can collide. `pidversion` changes on `exec`, and ancestors have no token |
| Outside Ivy, `publish` exits 0 silently | The other subcommands' `outside_ivy`, exit 1 | The ticket requires one git-ignored FLOW line to work in every terminal |

## Design challenges

Two challengers: need/assumptions (`design/challenge.md`) and trust boundary (`design/challenge-security.md`).

| Angle | Finding | Type | Severity | Response |
|-------|---------|------|----------|----------|
| Fit/need | Reusing `verify::root` "unchanged" pulls in Codex's `check_mode`, which accepts only resumable argument shapes (`ai_sessions/verify.rs:55,104`). `codex --model x` would get no root, and every hook would be refused. That check is also the only reason for `KERN_PROCARGS2` | unnecessary | high | **Cut.** The publish path attests identity with `check_mode` off: a variant of `root` that selects the client and checks the foreground, without matching resume arguments. `KERN_PROCARGS2` leaves this work, which also removes the environment copy (security row below). npm-wrapped Codex still passes on its native child |
| Assumption | On macOS no client is ever found: `programs.rs` accepts only ELF (`:215-219`) and knows only Linux Codex targets (`:247-250`) | missing | high | **Incorporate.** `programs.rs` joins the macOS PR: Mach-O recognition and the darwin Codex targets. Added to modules and plan |
| Assumption | The listener records the holder only on the window's `Taken`. A `panel_set` that times out while the window is minimised leaves the record empty, so hooks get "nobody holds" in exactly AC6's scenario | missing | high | **Incorporate, with the simplification below.** The hold is recorded once, by the window, in `Holder` (connection plus root). The window publishes it to a per-pane shared cell that listener threads read. A hold the window has not applied yet does not exist for a hook either; that race only affects the first publication of a minimised window, and is accepted and documented |
| Simplification | The hold is recorded twice, in the listener and in the window | unnecessary | medium | **Adopted.** It now has one writer, the window, through the shared cell above. The apply-time re-check compares the connection and the root, both already in `Holder` |
| Abuse | The proof attests every live descendant of the client root, not only its hooks: third-party MCP servers and tool children the agent runs can set the panel's `lines` and `attention`. D43 refused all of them until now | missing | medium | **Owner, 12 Nov 2026: «Todo lo del agente».** Any descendant of the holder's client may repaint text and attention, and none may change a button's command. Written in the decision: what the agent runs speaks for the agent. No opt-in switch |
| Composition | The overlay keeps the holder's buttons while it replaces their question: AC1 removes the `Decision` line and its buttons stay pressable, so a press answers whatever the program asks next. Both challengers flagged it | missing | medium | **Owner, 12 Nov 2026: «Se van si ya no espera».** The holder's `actions` survive a hook only while the hook's document still says `attention: "wait"`. A hook without `wait` drops them, and the agent puts them back at its next stop |
| Order | The holder's `panel_set` and a hook arrive on two connections, so the last to arrive wins | missing | medium | **Assume and document.** Both are rendered by the same CLI from the same `panel-input.json` and `meta.json`, so a late hook repaints the same content, or a newer answered copy. A hook rendered from stale files is the case the holder's next `panel_set` corrects. Written in the decision |
| Assumption | After an overlay, the holder's own patches by key can fail with "no line carries the key" (`panel-contract/src/patch.rs:455-457`) | missing | medium | **Assume and document.** flow patches only when its CLI is unavailable, and then the hook is not rendering either. A refused patch says so to the agent, which republishes; it is not silent |
| Operations | "Nobody holds" is the normal state before the agent's first publication, yet it exits 1 on every tool call | missing | medium | **Adopted.** "Nobody holds" exits 0 with the reply on stdout and nothing on stderr. Exit 1 is kept for a refusal that means something is wrong: a different root, or an unattested publisher or holder |
| Fit/need | Widening the `ai_sessions` gates turns on session recovery and its relaunch on macOS, which the ticket did not ask for | unnecessary | medium | **Cut.** Only `verify` and `programs`, plus `term-core` and `Peer`, compile on macOS. AC10 is removed. Session recovery on macOS is follow-up F4 |
| Decision idiom | ADR row 1 rejects "any attested client" for a reason that cannot happen: a nested worker client is already denied (`verify.rs:37-46`) | idiom | medium | **Corrected.** The real reason is AC3: a pane held by a non-client producer, for example a CI script |
| macOS | `KERN_PROCARGS2` copies Codex's environment, which holds tokens, into Ivy on every publish, and it was decided in auto | missing | medium | **Resolved by the first row.** The call leaves this work |
| Aftermath | When the hold ends, `· hook` is forgotten and the hook's lines stay under the bare source | missing | low | **Adopted.** `painted` survives the release and is cleared by the next publication from any transport |
| Assumption | Clients are matched by exact executable path and discovered once per pane. A self-updating client can move, and npm-installed Claude is never attested (`programs.rs:43-44`) | missing | low | **F5, risk recorded.** That is how D84 recognises clients today, and this work inherits it without changing it |
| Plan | Peer capture is gated on a session handler existing (`listener.rs:390-393`) | missing | low | **Incorporated.** The gate becomes "session handler or attester" |
| Idiom | `architecture.md` §2/§4 is stale (it waits 5 s; compact output "never wider") | idiom | low | **Superseded** by this document. The helper checks the encoded width after re-serialising, and refuses when it is over the ceiling |
