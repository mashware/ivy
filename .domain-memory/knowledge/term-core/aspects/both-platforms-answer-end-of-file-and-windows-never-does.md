---
id: asp_both-platforms-answer-end-of-file-and-windows-never-does_ffvgz0
slug: both-platforms-answer-end-of-file-and-windows-never-does
name: >-
  An ordinary exit reaches the reader as end of file on Unix AND on Windows —
  and on Windows no end of file arrives at all while the console lives
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T15:22:18.645Z'
updated_at: '2026-09-10T15:22:18.645Z'
last_verified: '2026-09-10T15:22:18.645Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - 'PtyFd::read'
  - try_clone_reader
  - 'Pty::spawn_with_env'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:b7de9a4a0bba550e68d3d1b6eaf727844d63add6175941ed90dc26363c969abd'
tags:
  - portable-pty
  - F2.7
  - platform
  - dependencies
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two facts about the same read, and the first one killed a risk while the second replaced it with a worse one.**

### 1. The translation is not Unix-only, and a whole risk rested on believing it was

Read in the registry, in the order the read travels:

- **Unix**: `portable-pty-0.9.0/src/unix.rs` — `impl Read for PtyFd` answers `Ok(0)` when `raw_os_error() == Some(libc::EIO)`, with its own comment saying `EIO` means the slave has been closed. `try_clone_reader` hands back exactly that `PtyFd`.
- **Windows**: `portable-pty-0.9.0/src/win/conpty.rs` — `openpty` builds two **anonymous pipes** and keeps `stdout.read` as the readable end; `try_clone_reader` hands back a clone of that **`filedescriptor::FileDescriptor`**, not anything of `portable-pty`'s own. And `filedescriptor-0.8.3/src/windows.rs` — its `io::Read` does `if err.kind() == BrokenPipe { Ok(0) }`.

> **So both platforms reach «the program finished» through the same `Ok(0)` arm.** A design that says «an ordinary exit says nothing new» does **not** rest on one platform's mapping, and a risk that says it might narrate every normal exit on Windows is refutable by reading. `win/mod.rs` — the file such a risk naturally points a reader at — has **no read path at all**; it is only the child handle.

### 2. What is actually different, and it is the opposite problem

On Windows the pipe's **write end is owned by the pseudoconsole** and shared with the master through one `Arc` (`conpty.rs`, `Inner { con, readable, writable, .. }`). So `drop(pair.slave)` in `Pty::spawn_with_env` — the line whose whole purpose on Unix is to let the read reach end of file when the child dies — **closes nothing there**, and **no end of file can arrive while the `Pty` is alive**, whether or not the program exited.

That is the survivor case of [[closing-hot-hangs-the-window]] and **D32** arriving as a **platform default** rather than as somebody's background job: not «when a shell leaves a `sleep &` behind» but «always». Whoever compiles on Windows meets it first, before any of the rest.

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn_with_env`'s `drop(pair.slave)` and the comment above it, which states the Unix reasoning; the reading loop that turns `Ok(0)` into an ordinary ending. The dependency sources are in cargo's registry, `portable-pty-0.9.0` and `filedescriptor-0.8.3`.

Cases 17 and 18 for [[someone-elses-api-is-read-not-assumed]], and the second one is carried in the F2.7 row of `docs/05-roadmap.md`, where it moved when #150 deleted `docs/DEBT.md`.
