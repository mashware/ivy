# Design security review — #56

**(a) Ancestry label — low (it only labels)**
- `crates/panel-control/src/listener.rs:390` — `Peer` is captured only when `services.session` is set. Checking a pid read later lets pid reuse slip through. Fix: on Linux, capture `Peer` for every connection at accept. Check against its pidfd (`is_alive` before and after the walk). If the capture fails, show the label. Never treat a failure as «inside».
- `crates/term-core/src/lib.rs:1039` — the pane child's pid can be reused once the shell exits, so an outside tree would read as inside. Fix: store the child's start time at spawn. On each ppid hop, require the ancestor to be no younger than its descendant, and the start time to match at the pane child.
- Forging «inside» from outside needs one of three things: ptrace (blocked by Yama scope ≥1, but scope 0 allows it), ptrace of Ivy itself, or root. A same-uid process can also just write `panel.json`, which gets no label. Fix: word the label as information. Nothing may be gated on its absence: no actions, no hold, no MCP scope.
- Dodge: a borrowing process that is still a child of A's shell (an editor, a non-daemonised server) is not labelled. Fix: state this in the decision. Only reparented processes (daemons, tmux) get caught, and tmux started *inside* A is a false «outside».

**(b) Release on `done` — medium**
- `crates/app/src/panel/driving.rs:333` — after `done`, any connection to the pane can claim it while A's agent is still alive. That includes a borrowed-identity agent, the threat in item 1. A's next publish is then refused at every stop, which today only happens after A exits. Fix: on `done`, release to the **file** only. Other connections stay refused until the holder disconnects.
- File takeover: no new capability. It is the same path as today's disconnect release (`release` at :466), and file `actions` still need visible confirmation. Fix: read with `read_if_newer` against `file_seen`, so a file older than the hold never comes back. Keep the refused-file confession.

**(c) stat of `control.sock` — low**
- `crates/panel-control/src/bin/panel-mcp.rs:41` — TOCTOU does no harm, because `connect` stays the authority. Fix: use `symlink_metadata` plus `is_socket()`. Take the length limit from `endpoint::name_of`'s error, not a second `sun_path` constant. Confirm `bind` runs before the child is spawned.

**(d) Items 4–5 — no finding**
- `whole_identity` closes the risk of a relative-dir socket in the working directory and the lossy id collision. The rc dir uses a random suffix, mode 0700 and exclusive create, so falling back to `/tmp` is safe.
