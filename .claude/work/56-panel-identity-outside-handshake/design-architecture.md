# Architecture — #56 (paths relative to the worktree)

## (a) Borrowed identity: label it (Linux and macOS; Windows unchanged)
- **Ancestry walk.** Add `ForegroundScope::descends(pid, birth) -> io::Result<bool>` in `term-core/src/foreground.rs`.
  - `Ok(true)` when the walk meets the pane's child; `Ok(false)` only when it reaches pid 0/1; `Err` means no label.
  - **Linux:** reuse the walk in `linux.rs:66`. `inspect_descendant` alone cannot serve, because it returns `PermissionDenied` both for "outside" and for "can't tell".
  - **macOS:** a new `foreground/macos.rs` beside `linux.rs`. Its `Scope` holds only the child's pid and start time, and supports `is_open` and `descends`. It walks with `proc_pidinfo(PROC_PIDTBSDINFO)`, reading `pbi_ppid` and `pbi_start_tvsec/usec`. `inspect*` stay `Unsupported` there.
- **Child start time, against pid reuse.** It is recorded when the scope is built (`lib.rs:450`), right after spawn. The pid cannot be reused yet, because the child is unreaped while `Pty` owns it. A child that has already been reaped makes the scope fail, so no label.
- **Peer capture.** In `session/peer.rs`, split per platform: `peer/linux.rs` keeps today's code, and `peer/macos.rs` is new.
  - macOS reads `getsockopt(SOL_LOCAL, LOCAL_PEERPID)` plus the start time.
  - macOS has no pidfd, so the start time must be earlier than the accept. A later-born process cannot be the peer.
  - `listener.rs:390` captures the peer for every connection, not only when a session handler exists.
- **Where the check runs:** off-frame, on the connection thread.
  - Add `Services.origin: Option<Arc<dyn Fn(&Peer)->Option<bool>>>` (`listener.rs:29`, through `open_with_services` :196 and `control/channel.rs:16`).
  - `converse` (:445) computes it once, at the first `PanelSet` (:608).
  - It travels as `What::Publish{source, panel, outside}` (:101).
- **How the scope reaches it:** a swappable cell, the same shape as `resume::Route` (`resume.rs:7`), kept in `PaneControl`. It is set at `opening.rs:644-646` and replaced at `resume.rs:272` when the shell restarts.
- **Exited program:** `is_open()` is false, so `Err` and no label.
- **Where the flag lives:** on the document, as `PanelState.published_by: Option<(Connection, bool)>`.
  - Set in `deliver`.
  - Cleared on the file commits (`panel.rs:397`, `driving.rs:316`) and on `retire`.
- **Painting:** joined into `warning()` (`pane.rs:561`).
- **Known limit:** a daemon still parented inside console A reads as A's.

## (b) `done` releases the hold to the file only
- Add `Holder.done: bool`, set in `deliver` (`driving.rs:346`) and in `patch` (:414) from `attention == Some(Done)`.
  - A republish by the holder rebuilds the holder, which reclaims the pane.
  - Other connections stay refused (:340) until the holder disconnects (`release`).
- **`refresh_file`** (`panel.rs:378`): when `holder.done`, it does not decline. It falls through to `read_if_newer(dir, seen)`. `seen` is the arrival stamp of the `done` document, so nothing older than the hold is resurrected.
  - A write refused before `done` stays unread, as it does after `release`.
- **Patch:** `patch` additionally requires `published_by.0 == connection`. Once the file has committed, the holder cannot patch the file's document.
- **Retirement** is unchanged: the holder still exists, so `retirement_status` (:246) still authorizes it.
- **Painting:** the held-only arm of `warning_about_the_panel` (`pane.rs:583`) and the backed-claim rule `holder.is_some()` (`panel.rs:522`) both become "holder and not `done`". File problems are then said again.

## (c) The `sun_path` check
- `endpoint.rs` has no constant or function for it.
- None is needed: Ivy binds the same absolute path (`endpoint.rs:164`), so an over-long path has no socket.
- Add `endpoint::reachable(dir)`, a `try_exists` on the socket path (unix only). Filter `instructions_for` (`panel-mcp.rs:220`) with it.

## (d) Items 4–6
- **4:** `helper.rs:71-82` and `ivy-session.rs:7-11` use `whole_identity`. Drop the lossy id at :88.
- **5:** make `panel_contract::base_dir` (`lib.rs:213`) public and use it at `shell_integration.rs:70`.
- **6:** the guide cannot know the client, because the prompt is pasted to the agent.
  - Replace the bare `{PROMPT}` (`connection.rs:431`) with a step: «if your client did not pass you the bridge's server instructions (Claude Code does), add: …».
  - Add the subagent rule to `PROMPT` (:10) and to the README quote (:149).

## (e) Decisions
- **D43:** `done` hands the pane back to the file. Only the holder may reclaim it until it disconnects.
- **D88:** a stat withholds the instructions from a stale identity. A borrowed identity is labelled on Linux and macOS. The helper and the rc dir follow the shared rules.
- **D7:** the id names a pane. A non-descendant peer is labelled, never refused.

## (f) Tests that fail when reverted
- **1:** these run under `cfg(any(linux, macos))`.
  - `descends` returns true for a grandchild inside the pty, false for the test's own child, and `Err` for `birth+1`.
  - Python inside the pty publishes over the socket and no label is painted. The test process publishes and the label is painted. A newer file clears it.
- **2:** `instructions_for` gives `None` with no socket and `Some` with a bound listener. `panel_mcp_handshake.rs` gets a no-socket case.
- **3:** after `done` (by publish, and by patch):
  - a newer file is painted, and an older one is not;
  - a second connection is refused;
  - the holder's republish wins over the file;
  - the holder's patch after the file commits is refused.
- **4:** a relative `PANEL_DIR` or a non-UTF-8 id gives `outside_ivy`.
- **5:** re-run the test binary (`shell_integration.rs:647`) with a relative `TMPDIR`; the rc dir is absolute.
- **6:** `integrated_chrome.rs:487`: the conditional sentence comes before `PROMPT`, and `PROMPT` carries the subagent rule.
