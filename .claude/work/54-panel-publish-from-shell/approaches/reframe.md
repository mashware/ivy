**Name:** The holder forwards the file.

**What it is:** No second publisher and no new CLI. `panel.publish_cmd` becomes a shell one-liner that atomically writes the rendered document into `$PANEL_DIR/panel.json` and does nothing if `PANEL_DIR` is unset. The pane's holder, `panel-mcp`, notices the file and republishes it on its own connection.

**Layers affected:** only `crates/panel-control/src/bin/panel-mcp.rs`. It needs a watcher thread (today it is a blocking stdin loop with a lazily opened `Producer`) and a merge step. The window (`app/src/panel/driving.rs`) and the wire are untouched. Plus one new decision, and documentation of the one-liner.

**Authority:** none is granted. The only voice into a held pane stays the holding connection, so D43 holds (the window still never reads `panel.json` while held), D44 holds (no credential, `PANEL_ID` stays a name), D50 holds (no token, nothing destructive reachable) and D80 holds (the signal register is untouched). Without a holder the file is the existing unheld transport. D80 cannot clear `wait` by itself: its text says a signal cannot withdraw the panel's claim. So the stale `wait` is only fixed by republishing the document.

**Replace vs merge:** the holder merges. Everything the hook rendered replaces the body and `attention`, and the last `actions` the agent published are kept unless the hook's document carries its own.

**Main risk:** the file becomes a de facto second authority. Any same-uid process can write it, but the same process could already write it when the pane is unheld. This amends the spirit of D43 (one voice), not its letter. The new signal is the measured 18 minutes of stale `wait`.

**Why it could be a bad idea:**
- It only works while the agent's bridge is alive. If the agent exits, the stale panel returns.
- An older hook render can race a newer agent `panel_set` and overwrite it.
- A watcher in a bridge that is quiet by design costs a thread and introduces a new failure surface (cross-platform watching until F2.7).
- Subagents sharing the connection also share the forwarding.
