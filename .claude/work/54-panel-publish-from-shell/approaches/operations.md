# Approach: operations / security

**Name:** Attested hook publish (a verb on the session service, not on the panel hold).

**What it is:** `ivy-session publish` (same helper as D84, found via `IVY_SESSION_HELPER`) reads the document on stdin and sends it over the pane's existing endpoint; the window accepts it only if the peer is attested as working for the holder, then merges it onto the held document.

**Layers:** `crates/panel-control` (`session.rs` request kind, `wire.rs` bounds, `bin/ivy-session.rs` subcommand); `crates/app` (`ai_sessions/verify.rs` attestation, `panel/driving.rs` merge + confession).

**Authority proof (reuse):** kernel peer credentials (`Peer` pid + birth, same uid) plus `scope.inspect_descendant(peer.pid(), peer.birth())`, which already proves a process descends from a recognized native client in the pane's foreground group/SID/tty. Add one comparison: the publisher's attested client root must be the same root (pid+birth) as the holding `panel-mcp` connection's. That is "works for the holder"; `PANEL_ID` is never read or sent (D44). Unattested (Windows, remote, unknown client) means refused, never guessed.

**Replace vs merge:** merge. Hook lines, attention and meta replace the holder's; `actions` stay the holder's. Overlay is volatile: the holder's next `panel_set` wins whole; hold released means overlay dropped. No hold: the publish becomes an ordinary file-less publication from a non-holder source.

**Outside Ivy:** no PANEL_ID/endpoint, or `endpoint::BINDS` false: silent exit 0. Inside Ivy but refused: stderr one line, non-zero exit, plus a confession on the app row naming "hook publication refused: not attested" (D43 style).

**Decisions:** keeps D43 (hold untouched, connection never swapped), D44, D88; amends D80/D50 wording («one-shot hook cannot patch», «command hook does not pretend») by adding a narrowly attested route; no bearer token (D50 discarded it). New signal needed: the measured 18-minute stale «waiting» and flow's fixed `publish_cmd`.

**Main risk:** attestation is client-specific; a Claude version change that reparents hooks silently degrades to refusal. Also merge semantics add a second writer to D17's "never settled by chance".

**Bad idea if:** the root-equality check proves too brittle on subagent/hook topologies, or the merge rule is judged a second authority system.
