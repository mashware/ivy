# Linux console scope architecture

## Decision

Use an opt-in, Linux-only re-exec helper and let Ivy's parent own the user-manager D-Bus connection. `portable-pty` 0.9 installs its own `pre_exec` hook for session setup and closes every fd above 2, so term-core cannot add a competing hook or pass an inherited control fd. The helper is the PTY child: it keeps that session and controlling TTY, waits before running the user's program, then `exec`s in place. This preserves process-group shutdown and makes the actual console process the PID systemd places in the scope.

## Flow and ownership

`app::main` checks a private helper argv mode before `app::run`. On Linux, `opening.rs` and `resume.rs` pass the current Ivy executable as an optional helper path to a Linux-specific `Pty` spawn entry point; ordinary callers and other platforms keep the current path. `term-core` creates a private Unix socket endpoint, starts the helper through the existing PTY slave command path with the target argv/env/cwd, and exposes the child PID plus endpoint to an app-owned scope worker. The helper connects by socket path after exec (no inherited fd), sends a ready message, then blocks on a gate.

The worker gets the helper PID from the authenticated local peer, calls user-manager `StartTransientUnit` for a unique `.scope` with `PIDs=[pid]`, and waits for the returned start job's completion before releasing the gate. It calls `RefUnit` before release so the scope's `Result` remains queryable after termination. If no bus/manager exists or scope startup fails, it releases the helper in direct-run mode; the helper then execs the original command. The target's environment removes the private helper variables. An exec-error message distinguishes failed `exec` from successful `exec` (CLOEXEC socket EOF); existing child status/output paths remain authoritative for exit and terminal drain.

When the child exits, the parent worker reads the retained scope `Result`; `oom-kill` is positive evidence that systemd-oomd or kernel OOM caused the unit kill. (The cgroup `memory.events` counter only observes kernel OOM.) It then `UnrefUnit`s. If manager observation is unavailable or inconclusive, preserve unknown SIGKILL rather than claim OOM.

## Why not `systemd-run --user --scope`

`systemd-run` is a valid simpler launch wrapper: systemd documents scope commands as running synchronously in the caller's environment. But it adds a runtime CLI dependency to a feature whose contract is graceful fallback, and its wrapper status/exec error path does not give Ivy a retained unit reference for post-exit OOM attribution. The helper uses systemd's documented `StartTransientUnit` scope path directly and can keep the observation alive through the ending notice.

## Main risk

The gate and D-Bus job completion must be race-free: never release user code before successful PID placement; on any failed setup, release exactly once in direct mode. Socket loss must not strand a console: helper should report or fall back to direct exec if the parent disappears.
