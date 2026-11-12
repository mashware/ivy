# Design (architecture) #57 — check PANEL_DIR's owner and mode before connecting

Paths relative to the worktree.

## Decisions
1. **One check, in `crates/panel-control/src/endpoint.rs` (beside `name_of`:124/`connect`:174):** `pub fn private_to_this_user(dir: &Path) -> io::Result<()>` calling `fn private_to(dir, uid: u32)` with `libc::geteuid()`. libc is already a unix dep of panel-control (Cargo.toml:42); panel-contract has none.
2. **Called in the inherited-identity paths, not in `endpoint::connect`/`Producer::of`:** `client.rs:99`, after `BINDS`, as `private_to_this_user(dir).and_then(|()| Self::of(dir, id))` inside the existing `map_err`; and `session/helper.rs:83`, before `name_of`. `Producer::of` (client.rs:113) "takes the pair as given" and is used by tests and the app; the helper does not use `connect` anyway.
3. **`symlink_metadata` (lstat), and it must be `is_dir()`:** `PanelDir::create` (panel-contract lib.rs:171) never makes a symlink, and `stat` would accept a link to another private dir of the same user.
4. **`mode & 0o077 == 0`, not exact `0700`:** the property is "no group/other access"; `DirBuilder.mode(0o700)` is still subject to umask, so exact 0700 adds nothing. **Owner == euid** (mkdir's owner is the effective uid).
5. **Fail closed:** an lstat error is a refusal too (the plain case: console closed, dir gone).
6. **No TOCTOU mechanism:** in sticky `/tmp`, per-user `$TMPDIR` or `XDG_RUNTIME_DIR`, nobody else can rename a dir this user owns or plant inside a 0700 one. Not checked: the parents, `openat`/fd connects.
7. **Windows: `#[cfg(not(unix))]` returns `Ok(())`:** `name_of` ignores the dir there, and the boundary is the pipe's descriptor (D43, F2.7). **macOS:** the same unix code.
8. **opencode plugin in scope:** it reconnects every 1 s (ivy-opencode.tsx:74), which is exactly the window of the attack. Inside `connect()` (:27), `lstatSync(dirname(path))` must be a dir with `uid === process.geteuid()` and `(mode & 0o077) === 0`. Otherwise it counts as a failed attempt and retries in 1 s, checking again each time. It names nothing: the TUI has no error surface.
9. **Helper stays on its own identity rule (D88 deferral):** the threat here is a foreign owner, not a relative path.
10. **Wording:** the inner text names no dir: "it is not a private directory of this user (owner uid N, mode NNN)". The bridge already wraps it as "could not be reached at <dir> (…)" (client.rs:103). The helper gains the same wrapper, and `ivy-session.rs` stops discarding it: today `map_err(|_| "cannot reach this pane's session service")`.
11. **Docs:** amend D7 (docs/02-decisions.md:714) and the security table (docs/03-architecture.md:125), with the discarded options from 3, 4 and 6.

## Tests (each line says what mutation it kills)
- **endpoint unit (unix):**
  - A genuine `PanelDir` passes `private_to_this_user`. Kills passing a wrong uid.
  - Modes `0o750` and `0o705` are refused. Kills a dropped mask clause or a one-sided mask.
  - `private_to(dir, geteuid()+1)` is refused. Kills a dropped uid clause; this is how the uid branch is covered without root.
  - A user-owned symlink to a genuine `PanelDir` is refused. Kills `metadata` instead of lstat.
  - A missing dir is refused. Kills fail-open.
- **panel_mcp_handshake:** a live `Channel` on a `PanelDir` chmod 0755 gives a `panel_get` refusal containing the dir and "private". Kills removing the call at client.rs. The `nowhere` test is unchanged.
- **session_helper (linux):** the same 0755 setup makes the helper fail, the service callback never runs, and the output names the dir. Kills removing the call in `exchange`.
- **plugin.test.mjs:**
  - A 0755 `mkdtemp` dir gives `connections === 0` after a pause.
  - A stubbed `process.geteuid` gives the same.
  - The existing happy path still connects.
