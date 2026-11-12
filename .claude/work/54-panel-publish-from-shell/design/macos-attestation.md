> **Working draft, superseded where it differs by `03-design.md`.** `KERN_PROCARGS2` and Codex argument matching are **cut** (the publish path attests identity with `check_mode` off); the `ai_sessions` gates are **not** widened beyond `verify` and `programs`; AC10 was removed.

# macOS process attestation — design (#54)

Paths are relative to the worktree. "Reasoned" means read from source or headers, not observed on a Mac (D54).

1. **Peer identity.** Linux: `SO_PEERCRED` + `SO_PEERPIDFD`, then birth from `/proc` (`panel-control/src/session/peer.rs:57-79`). macOS: use **`getsockopt(SOL_LOCAL, LOCAL_PEERTOKEN)`** (libc 0.2.189 `apple/mod.rs:2935,2948`). Use the token's `pid` and `pidversion`. Do not use `LOCAL_PEERPID` (`:2940`): it has no version, so a PID reused between `connect` and the read would be attested.
   - `birth` = `p_uniqueid` from `proc_pidinfo(PROC_PIDT_BSDINFOWITHUNIQID)`. That flavor returns `proc_bsdinfo` and `proc_uniqidentifierinfo` in one snapshot, and its 64-bit id is never reused within a boot.
   - Capture rule: `snapshot(pid).p_idversion == token.pidversion` holds, or the call is denied. This closes PID reuse between accept and the snapshot.
   - Why not pidversion as `birth`: ancestors have no token. Also, XNU bumps the version on `exec` (reasoned). Why not `pbi_start_tvsec/usec`: it has microsecond collisions, and the uniqueid does not.
   - FFI: libc has none of `audit_token_t`, flavor 18 or `proc_uniqidentifierinfo`. Use `#[repr(C)]` copies from `<sys/proc_info.h>`, plus `#[link(name = "bsm")] extern { audit_token_to_pid, audit_token_to_pidversion }`. No new crate.

2. **Ancestry.** `stat` (`term-core/src/foreground/linux.rs:142-164`) maps to `snapshot(pid)`:
   - `parent` = `pbi_ppid`
   - `group` = `pbi_pgid`
   - `tty` = `e_tdev`
   - `birth` = `p_uniqueid`
   - zombie = `pbi_status == SZOMB` (libc `:4221`)
   - `session` = `getsid(pid)`
   - executable = `proc_pidpath` (libc `:5013`, max 4096)

   `getsid` and `proc_pidpath` take a bare PID. So the TOCTOU pattern is `snapshot → getsid → proc_pidpath → snapshot`, and the two snapshots must be equal. That is exactly the role of `stat(pid)? != current` (`linux.rs:88-91`), and it closes PID reuse between the reads. The walk in `linux.rs:65-131` has no platform code apart from `stat`, `read_link` and `group`. Hoist it into `foreground.rs`; do not copy it.

3. **Lifetime.** Linux: a pidfd plus `poll` (`linux.rs:166-191`). macOS: an owned **`kqueue`** with `EVFILT_PROC | NOTE_EXIT` (libc `:3194,3246`).
   - After `EV_ADD`, re-snapshot and require `uniqueid == birth`. This closes the case where the PID died and was reused before the note attached (`linux.rs:177`).
   - `alive` = `kevent` with a zero timeout returns 0.
   - Do not rely on `poll` over a kqueue fd (D29 is the warning about macOS `poll`). Today `verify.rs:89-95` and `peer.rs:37-44` call `libc::poll` directly. Both should call one term-core helper, `lifetime_ended(&OwnedFd)`.
   - `Root::current` and `Peer::is_alive` keep their signatures. `Peer.lifetime` widens to `any(linux, macos)`.

4. **Foreground group.** `group()` calls `tcgetpgrp` on a duplicated master (`linux.rs:36-38,133-139`). On macOS nothing blocks it:
   - portable-pty 0.9.0 exposes `as_raw_fd` for every `cfg(unix)` (`lib.rs:113-114`, `unix.rs:366`).
   - `nonblocking::duplicate` is ungated (`nonblocking.rs:39`).
   - The BSD master forwards `TIOCGPGRP`. This is reasoned, so the Mac check must confirm it.

5. **Arguments.** These calls are on the publish path:
   - `root()` calls `runs_script` for Codex's npm wrapper (`verify.rs:23-30`, `programs.rs:74-75`).
   - `Root::current` calls `matches_arguments` when `check_mode` is set (`verify.rs:104`).

   If they return `Unsupported`, Claude and OpenCode still pass, but `unwrap_or(false)` refuses **every Codex publish**. So `Unsupported` is acceptable only if the PR declares "Codex unattested on macOS".

   Recommendation: implement `sysctl KERN_PROCARGS2` (libc `:3742`). Skip `argc` and `exec_path`, then feed a `Cursor` at argv0 to the unchanged `arguments::matches`.
   - Cost: the sysctl copies all of argv and the environment into Ivy. That gives up the streaming property in `arguments.rs:5`: bytes are copied, then dropped uninterpreted.
   - This is an owner decision, not a detail.

6. **Same uid.** The audit token is filled by the kernel at `connect`, so a peer cannot forge its pid or version.

   **Guaranteed** at check time: the connecting process is a live descendant of the same root (pid + uniqueid). That root has a recognized executable path and sits in the pane's foreground group, session and tty.

   **Not guaranteed:**
   - that the code running in that root is the vendor's. Same-uid `DYLD_INSERT_LIBRARIES` at launch, or `task_for_pid` into a non-hardened `node`, both get around it. SIP and hardened runtime are not a boundary here.
   - that the hook command itself is benign. Anyone with the same uid can edit the client's hook settings.

   Linux has the same limits under same-uid `ptrace`.

7. **Tests.**
   - All seven tests call `super::linux::stat(..).birth` (`tests.rs:24,67,91,…`). Rename that to a neutral `native::birth`.
   - After the rename, tests 1–4, 6 and 7 run unchanged.
   - `nested_descendants…` (`tests.rs:130-143`) asserts `FD_CLOEXEC` and polls a pidfd. Its macOS variant waits with `kevent`. Drop the CLOEXEC assertion: kqueues are not inherited by fork or `posix_spawn` (reasoned).
   - The tests need `/usr/bin/python3`, which on a Mac is the Command Line Tools shim.
   - From Linux, `cargo check --target aarch64-apple-darwin` proves type-checking only. Nothing else in sections 1–5 can be proved there.
   - Literal Mac check:
     1. Run `cargo test -p term-core foreground`, `cargo test -p panel-control session` and `cargo test -p app ai_sessions`.
     2. In an Ivy pane, run `claude`, then `codex`, then `opencode`. Trigger a hook that runs `"$IVY_SESSION_HELPER" publish < doc.json`. The panel must show `doc.json` with the holder's `actions` kept.
     3. Run the same command from the pane's shell after quitting the client. It must be refused.
     4. Run it from a second pane with the first pane's `PANEL_DIR` exported. It must be refused and confessed.
   - Per D53, the PR names macOS as unverified until steps 1–4 pass.

8. **Shape.**
   - New file `foreground/macos.rs` beside `linux.rs`, with `#[cfg(target_os = "macos")] mod macos;`.
   - A `use … as native` alias, so each pair of `cfg`s in `foreground.rs` (`:6-109`) becomes `any(linux, macos)`.
   - `arguments.rs` moves out of `linux.rs:8-9` into `foreground.rs`.
   - `peer.rs` gains a macOS branch.
   - The `ai_sessions/mod.rs` gates widen.
   - Estimate:
     - `macos.rs` about 230 production lines: FFI structs 70, snapshot and walk glue 60, lifetime 40, procargs 45, cwd via `PROC_PIDVNODEPATHINFO` 30.
     - Peer about 50.
     - Hoisting the walk is net zero.
     - Tests about 30.
     - Total about 350.
