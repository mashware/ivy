# Approach: "Release on done, stat before promising, one identity rule" (minimum)

**What it is:** fix the item people meet that has a clean signal (3), plus the four cheap unifications (2, 4, 5, 6), and leave item 1 out on purpose.

## Per item
1. **Borrowed identity: left out.** D88 states nothing the bridge sees tells a borrowed identity apart, and `PANEL_ID` is a name, not a credential. The only unforgeable witness is `session/peer.rs` `Peer::capture` (Linux only) compared against the pane child's session. That is a new mechanism with false positives (agents that `setsid` themselves) and no Windows/macOS equivalent before F2.7. The new signal to reopen it would be a measured case, not a guess. Ship nothing and record it as an open follow-up.
2. **Unreachable endpoint:** in `panel-mcp.rs` `instructions_for`/`main`, after `whole_identity`, `stat` `PANEL_DIR/control.sock` and check that the path fits `sun_path` (via `endpoint::name_of`). Withhold the promise if either fails. This is not a connect, so D88's discard stands. On Windows (`BINDS` false) the existing branch is unchanged.
3. **Hold outlives done:** in `app/src/panel/driving.rs`, when the holding connection's `deliver`/`patch` leaves `attention == done`, drop the `Holder` but keep the document, as `release` does today. The next publish from any producer claims it. D43's disconnect rule and the discarded explicit-release message are untouched: a crash still releases via disconnect. This amends D43 in one line.
4. **Session helper:** `session/helper.rs` `exchange` and `ivy-session.rs` call `client::whole_identity`.
5. **Relative TMPDIR:** `shell_integration.rs:70` takes the rc dir from `panel_contract::base_dir()`, not `std::env::temp_dir()`.
6. **Connect guide:** `chrome/connection.rs:431` appends `{PROMPT}` only for clients that ignore server instructions, matching the README.

## Modules
`panel-control` (`bin/panel-mcp.rs`, `session/helper.rs`, `bin/ivy-session.rs`), `app` (`panel/driving.rs`, `shell_integration.rs`, `chrome/connection.rs`), and `docs/02-decisions.md` (D43 amendment).

## Main risk
Item 3: an agent publishes `done`, loses the hold, and a file producer takes the pane. The agent's next `panel_set` then claims the hold again and the file producer's writes are refused. That is the same arbitration as today, just later. A test must cover that sequence.

## Why it could be a bad idea
It leaves the headline symptom (wrong console's panel changing) unfixed. Item 2 narrows it only for stale identities, not for live borrowed ones. Someone who hits item 1 gets nothing from this PR.
