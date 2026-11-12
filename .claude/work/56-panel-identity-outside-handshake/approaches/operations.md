**Name:** Verify the caller, release on done, say why.

**What it is:** every failure becomes a named, visible refusal instead of a silent mis-publish. Where the platform can prove who is calling, the app checks it. Where it cannot, it says "unverified" rather than guessing.

**Item 1 (borrowed identity).**
- On the first publish, the app reads the connecting peer's pid with `session/peer.rs` (`Peer::capture`; Linux today, macOS needs `LOCAL_PEERPID`).
- It checks that pid descends from the pane's child pid (`term-core` `process_id()`).
- A reparented daemon or multiplexer (tmux server, daemonized Codex) fails the check and is refused. The tool result and the app row both say "this process is not running in this console".
- This is a fact about the connection, not about the agent's state, so D6 is untouched.
- Off Linux, or when the walk fails, it falls back to today's behaviour. Nothing blocks, but the panel row says "caller unverified".

**Item 2 (unreachable endpoint).**
- `panel-mcp` does a `stat` of `PANEL_DIR/control.sock` and a `sun_path` length check via `endpoint::name_of` before it promises a panel.
- A failure withholds the instructions and adds a fifth named refusal cause in `Producer::of_this_pane`.
- Windows skips the stat because `BINDS` is false there.

**Item 3 (hold outlives done).**
- `attention: done` in `PanelState::deliver` (`app/panel/driving.rs`) drops the hold and keeps the document.
- This amends D43. The new signal is the measured refusal of a later file producer.
- A later agent publish reclaims the hold. A file write then lands on the done document, and any refusal is still confessed.

**Items 4–6.**
- `session/helper.rs` uses `client::whole_identity`.
- `shell_integration.rs` uses `panel-contract::base_dir`.
- The Connect guide in `chrome/connection.rs` appends `{PROMPT}` only for clients that ignore server instructions.

**Modules:** `panel-control` (`client`, `endpoint`, `session/peer`, `panel-mcp`), `app` (`panel/driving`, `shell_integration`, `chrome/connection`), `term-core` (pid exposure), `docs/02-decisions.md` (D43 and D88 amendments).

**Main risk:**
- The ancestry check refuses legitimate agents that run in the console but were reparented (`setsid`, `systemd-run`, a container or pid namespace).
- It cannot catch a daemon that was not reparented.

**Why it could be a bad idea:**
- It is Linux-first, so macOS and Windows behave differently, which is a divergence from D1.
- It reverses D88's discarded probing, since the stat is new. The new signal is the ticket's evidence.
- It adds a peer-credentials path to a sensitive hot path.
- A false refusal is worse than a wrong panel.
