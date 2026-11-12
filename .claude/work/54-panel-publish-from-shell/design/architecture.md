> **Working draft, superseded where it differs by `03-design.md`.** In particular: the reply comes once the publication is queued, not after the window (no 5 s wait); the hold is recorded once by the window in a shared cell; actions survive a hook only while it still says `wait`; "nobody holds" exits 0 with `"code":"unheld"`.

# Architecture #54: an attested hook publishes over its holder's panel

Paths are relative to the worktree.

## 1. Holder root
- **Today the window does not know the holder's peer.** `Holder` stores only `connection`, `source` and `file_refused` (`crates/app/src/panel/driving.rs:208-223`).
- The listener does capture a `Peer` for every connection when it accepts it (`crates/panel-control/src/listener.rs:390-393`). It hands that peer only to the session handler (`listener.rs:533-541`). `answer()` never receives it (`listener.rs:589`).
- **Smallest change:**
  - Add `attest: Option<Attester>` to `Services`. `Attester` is `Arc<dyn Fn(&Peer) -> Option<ClientRoot>>`, and `ClientRoot` is `{pid: u32, birth: u64}`, `Copy + Eq`, in `session/peer.rs`.
  - The app builds it in `opening.rs:644-646`, next to `route.callback()`, from `pty.foreground_scope()` (`term-core/src/lib.rs:450`). The scope is its own descriptor, so a second one does not touch recovery.
  - It reuses `verify::root` (`ai_sessions/verify.rs:15`) unchanged and returns `root.process.{pid,birth}`. Programs are discovered once per pane, on first use.
- **Do not reuse the session handler.** It exists only in `StartupRestore::Sessions` (`restoration.rs:220-223`, `resume.rs:148-153`) and refuses while disabled (`ai_sessions/mod.rs:119`).
- **When the root is resolved:** at every holder `panel_set`, on the holder's connection thread. It travels as `What::Publish{source, panel, root: Option<ClientRoot>}`, and `deliver` stores it in `Holder.root`.
- It cannot be resolved at publish time. The holder's `Peer` lives only on the holder's thread (`listener.rs:405-412`), and reaching it would need a registry shared between threads.
- **No liveness guard.** A dead root takes its `panel-mcp` with it, so the hold ends. A `(pid, birth)` pair cannot be reborn.
- `root: None` (a script, Windows, an unrecognised client) still holds the pane, as D43 says. Hook publications into that pane are refused.

## 2. The request
- **It is a new `wire.rs` tool, not a `session.rs` `Request`:**
  - Session requests never reach the window (`session.rs:21`).
  - Session requests are capped at 16 KiB (`session.rs:15`), while a document may be up to 256 KiB (`panel-contract/src/panel.rs:45`).
- `interpret` gets one new arm next to `panel_set` (`wire.rs:355`).
- The line is `{"tool":"panel_publish","document":{…}}\n`.
- **Bounds:**
  - The helper reads stdin with `take(MAX_BYTES+1)` and refuses anything larger.
  - It parses the input as a JSON object and re-serialises it compactly, because a pretty-printed document would break newline framing. Compact output is never wider than the input.
  - The envelope adds 36 bytes, which stays under `ENVELOPE` (`wire.rs:52,60`). The line therefore never becomes `Unrecoverable` (`wire.rs:200-206`). Even if it did, only the helper's own connection would drop.
- **Attestation runs on the publisher's connection thread.** If it fails, the refusal is sent before anything is queued, so it costs no frame.
- `panel_publish` does not set `used_panel_protocol` (`listener.rs:556`). It holds nothing, so it has nothing to release.
- **Replies:**

| Case | Reply |
|---|---|
| Taken | `{"ok":true}` |
| Publisher not attested | `{"ok":false,"error":"this publication does not come from an attested agent client in this console's foreground"}` |
| Nobody holds | `{"ok":false,"error":"nobody holds this console's panel: a hook publishes only over its holder's"}` |
| Holder not attested | `{"ok":false,"error":"this panel is held by \"SOURCE\", which is not an attested agent client"}` |
| Holder has a different root | the existing `held_by_somebody_else` sentence (`driving.rs:30`) |
| Window did not answer in time | `{"ok":false,"error":"the window did not answer within five seconds"}` (`listener.rs:833`); the publication is still applied later (`listener.rs:88-91`) |

## 3. Overlay rule
- **Replaced by the hook's document:** `lines`, `attention`, `phase`, `updated_at`, `version` and `stale_after`. `written_at` is set to now.
- **Kept from the committed document:** `actions` and `header`. `header` is kept so the hook cannot hide where the painting came from. Field list from `panel-contract/src/panel.rs:84-131`.
- **No second copy of the document.** The composed panel is committed to `last_good`. A hook only ever copies the holder's actions, so `last_good.actions` always belongs to the holder.
- **Identity row:** `lay_out` receives `"{source} · hook"` instead of `source` (`panel/cache.rs:31`).
- **D80 / D18:** `effective_attention = max(panel, signal)` (`agent_signals.rs:228-235`) does not change. The overlay changes only the panel's half, and its staleness backing works as it does for a held panel (`panel.rs:503-519`).
- **State machine** (`painted ∈ {Holder, Hook}`):

```
Unheld        --panel_set(c,r)------------> Held{c,src,r,Holder}
Held{c}       --panel_set(c,r')-----------> Held{c,src,r',Holder}       whole replace
Held{c}       --panel_set(c2)-------------> unchanged, refused
Held{c}       --panel_patch(c)------------> unchanged painted; ops apply to last_good (overlay included, driving.rs:393)
Held{r=Some(x)} --publish(root x)--------> Held{..,Hook}; commit(hook ⊕ held actions/header)
Held{r=Some(x)} --publish(root y≠x)------> refused (held_by_somebody_else)
Held{r=None}  --publish------------------> refused (holder not attested)
Unheld|Retired --publish------------------> refused (nobody holds)
Held{c}       --Closed(c)-----------------> Unheld, last_good kept (driving.rs:466-476)
Held{c}       --panel_retire(c)-----------> Retired, cleared (driving.rs:443-448)
any           --pane close----------------> gone
```

- The overlay never clears `file_refused`.

## 4. The command's contract
- **`ivy-session publish`** reads the document from stdin.
- **Exit codes:**
  - `0`: taken. Stdout carries the reply line.
  - `0`: outside Ivy, with nothing on stdout or stderr. That means `IVY_SESSION_HELPER` is set but `PANEL_ID`/`PANEL_DIR` is not whole (use `client::whole_identity`, `client.rs:30`), or `endpoint::BINDS` is false (`endpoint.rs:143`).
  - `1`: the window refused the publication, including the time-out. Stdout carries the reply. Stderr carries `ivy-session: <error>`.
  - `2`: a local failure: bad usage, input that is not JSON or is over the ceiling, or the endpoint cannot be reached. Stderr carries `ivy-session: <reason>`.
- **The exchange deadline must exceed 5 s.** The existing `exchange` gives up after 2 s (`session/helper.rs:84`), which is earlier than the window's `ANSWER_DEADLINE` (`listener.rs:41`).
- **Risk:** a minimised window runs no pass (`control/applying.rs:10-11`), so every hook would wait 5 s. Measure this in validate.
- **The git-ignored FLOW line:** `panel.publish_cmd: exec "${IVY_SESSION_HELPER:-true}" publish`. In any other terminal it runs `true publish`, which exits 0. The variable is always injected inside Ivy (`opening.rs:608-612`).

## 5. Frame cost (D13)
- **The only new frame work** is one new arm in `PaneControl::apply` (`control/applying.rs:32`). It compares two roots, builds one panel and commits it, which is the same class of work as `Publish`.
- **Declare it** as `did.overlaid` and `did.overlays_refused` in the control report, beside `applied` and `patched` (`applying.rs:35,66`).
- Attestation and program discovery run on connection threads.
- No batch exercises this path. That is a declared gap.

## 6. Decisions
- **D43:** a held pane also takes a one-shot publication whose attested client root equals the holder's. That publication overlays the document without taking the hold.
- **D47:** a patch applies to the committed document, which may be an overlay, and it is still holder-only.
- **D50:** a command hook may publish over the holder's pane, but it still never claims, releases or retires.
- **D80:** a one-shot hook can now update Flow's held panel through the attested route. The signal register and the attention priority do not change.
- **D84:** the attestation now also serves panel publication in every restore mode. `publish` succeeds silently outside Ivy, unlike the other subcommands.
- **New decision:** «An attested hook publishes over the hold it works for».
