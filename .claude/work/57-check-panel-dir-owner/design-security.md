# Security review #57: PANEL_DIR owner+mode check

**Verdict:** the check closes the ticket's attack. Five things need fixing in the design, and five risks remain to state.

## Findings

- **[medium] The opencode plugin reconnects every 1 s forever** (`ivy-opencode.tsx:74`). A check made once at startup misses an attacker directory planted later.
  - Mitigation: run the check inside `connect()` on every attempt (`fs.lstatSync`, `process.geteuid()`).
  - Rust clients reconnect per call, so this applies only to the plugin.
- **[low] Use `lstat`, not `stat`, and require a real directory.**
  - On Linux, `protected_symlinks=1` already refuses an attacker symlink in sticky `/tmp` (measured: 1 on this machine).
  - macOS has no such protection. A followed symlink can point at another live console's directory of the same user. The data stays with the user, but it goes to the wrong pane.
  - Mitigation: `symlink_metadata`, check `is_dir`, and only the last component is checked. macOS `/var` is a symlink in a middle component, so it stays allowed.
- **[low] Compare `mode & 0o077 == 0`, not `mode == 0o700`.**
  - The mode the directory is created with passes through the umask, and setgid can be inherited. An exact compare can falsely refuse a genuine directory.
  - The security property is "no group or other bits".
- **[low] Race between check and connect.**
  - Sticky `/tmp` means only the owner can rename or unlink a user-owned entry. So a swap needs the user's own console to close inside the window and the attacker to recreate the name. The window is microseconds.
  - Under a non-sticky world-writable `TMPDIR`, the attacker can swap at will.
  - Mitigation, optional for the Rust clients: before the first write, check the listener's uid on the connected socket (`SO_PEERCRED` on Linux, `getpeereid` on macOS). Node has no equivalent.
- **[low] Relative `PANEL_DIR` is still accepted** by the session helper (`helper.rs:76`) and by opencode.
  - Mitigation: apply the "must be absolute" rule there, as `whole_identity` does.

## No finding

- **Attacker-owned directory:** refused by the owner check.
- **User-owned directory with the wrong mode:** another user cannot create it, since there is no chown without root.
- **Symlinked `control.sock` inside a genuine 0700 directory:** nobody else can create entries there.
- **The user's stale directory holding another user's socket:** impossible for the same reason.
- **Per-user macOS `$TMPDIR`:** it is 0700, so the attack cannot reach it.

## Root / sudo

- `sudo` resets the environment by default, so the identity only arrives with `-E` or `env_keep`.
- In that case euid 0 does not match the directory's owner, so the client refuses where root used to connect.
- **YAGNI:** do not add a root exception. "Root trusts any owner" would accept the attacker's directory. Naming the directory in the refusal is enough.

## Accepted risks (not closed)

- **File transport:** producers that write `$PANEL_DIR/panel.json` directly can still write into an attacker's directory. This fix cannot enforce anything for arbitrary scripts.
- **Same-uid processes are not a boundary:** `PANEL_ID` is a name, not a credential.
- **Race in the opencode plugin and the Rust clients** if the console closes inside the check window and no peer-uid check is added.
- **Windows:** no check, and no channel is bound until F2.7.
- **Linux without `protected_symlinks`:** the `lstat` check still holds.
