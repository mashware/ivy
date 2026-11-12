**Name:** Overlay over the session service

**What it is:** a `publish` helper (a second subcommand of the existing `ivy-session` bin, or a sibling bin) reads a document on stdin and sends it as a new request on the D84 session socket. The window stores it as a per-pane overlay that never touches the holder's connection or its committed document.

**Modules:**
- `crates/panel-control/src/session.rs`: add one `Request` variant (`Publish`) with `validate` and `MAX_REQUEST` bounds counted on encoded width.
- `crates/panel-control/src/bin/ivy-session.rs`: the CLI. It does nothing and exits 0 when `PANEL_ID`, `PANEL_DIR` or the socket is absent (the `client::whole_identity` check).
- `crates/app/src/panel/driving.rs`: the overlay is a field beside `Holder`, not inside the hold rule. It is parsed with `panel_contract::parse_document` and dropped whenever the holder calls `panel_set` or `panel_retire`, or the connection dies.

**Authority:** the existing D84 attestation, unchanged. Peer credentials plus a recognised native client ancestor in the pane's foreground group, SID and terminal. `PANEL_ID` is only the endpoint locator, never a credential (D44). No bearer token, which D50 explicitly discarded.

**Replace vs merge:** the overlay replaces `lines` and `attention`. At paint time, `actions` always come from the holder's document, so buttons survive. If nothing holds the pane, the overlay is painted as is, with no actions.

**Decisions:**
- Keeps D43 and D44: the hold still compares connections.
- Keeps D50: a helper cannot retire or replace the holder, and the existing test `session_helper_cannot_release_or_replace_existing_panel_connection` stays true.
- Keeps D80's separate register and D88.
- Amends D80 (a second pane-local register, for documents) and D84 (a new verb). It contradicts nothing, so it needs no new signal.

**Main risk:** the Claude Code hook's process ancestry may not satisfy the D84 attestation. That is proven for Codex but not for flow's `PostToolUse` hook, and it must be checked first.

**Why it could be bad:** the overlay can show state that disagrees with the holder's, which is a second producer in a pane. That is exactly what D43 avoided with «two producers are never settled by chance». A «stale overlay» rule is also needed.
