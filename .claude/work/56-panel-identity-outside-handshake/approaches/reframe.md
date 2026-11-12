# Approach: "Fix it in the window and the launcher, not the bridge"

**What it is.** Treat the bridge as correct and move each fix to where the knowledge lives: the window knows the hold, the launcher knows the identity. Declare item 1 a non-defect of the bridge.

**Per item**
1. Borrowed identity: no bridge change. Process ancestry cannot tell it apart, because a daemon started from A makes B's bridge a descendant of A's shell; D88 is right. Instead: Ivy-side launchers (`ivy-codex` already passes `--no-daemon`) get the fix, and the docs name the borrowed case. `Peer::capture` is not used: it proves who connected, not which console they sit in.
2. Unreachable endpoint: split. A stale id keeps failing at first call, naming the directory (D88). The `sun_path` overflow is Ivy's own fault, so check it where the pane dir is created (`panel-contract::base_dir`, pane spawn) and warn once there, so the console never carries an identity Ivy cannot serve.
3. Hold outlives `done`: in `app/src/panel/driving.rs`, when the holder's document is `attention: done`, drop the hold (not the document) and let `panel.json` be read again. It is the producer's own declaration (D18), not a deduction (D6), and not the discarded release message. The agent can claim again with its next `panel_set`.
4. and 5. One helper in `panel-contract` (absolute-dir, lossless-id rule) used by `session/helper.rs`, `client::whole_identity` and `shell_integration.rs`.
6. Connect guide: drop `{PROMPT}` in `chrome/connection.rs` for clients that honour instructions; keep it for the rest.

**Modules.** `app/panel/driving.rs`, `panel-contract` (`base_dir`), `session/helper.rs`, `app/shell_integration.rs`, `app/chrome/connection.rs`, `docs/02-decisions.md` (amend D43, D88 note).

**Main risk.** Item 3 changes D43. New signal: a person met a file producer ignored after the agent finished. After `done`, the agent's late `panel_patch` could re-claim and overwrite the file producer's panel.

**Why it could be a bad idea.**
- Item 1 stays open for editors and multiplexers. This is a documented limitation, not a fix.
- Releasing on `done` is racy when subagents or a restart publish `done` and then continue.
- The window would need a rule for a re-claim over a live file source.
