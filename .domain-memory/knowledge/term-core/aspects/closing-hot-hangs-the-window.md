---
id: asp_closing-hot-hangs-the-window_s1ca6d
slug: closing-hot-hangs-the-window
name: >-
  Closing a pseudoterminal hot freezes the window for 101 ms and can hang it
  forever: the signal goes to the pid and the grandchild keeps the slave end
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:06.689Z'
updated_at: '2026-08-15T21:24:06.689Z'
last_verified: '2026-08-15T21:24:06.689Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
symbols:
  - Pty
  - PtyOutput
  - Drop
  - stop
  - try_clone_reader
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
  crates/app/src/lib.rs: 'sha256:b3ca4328c99c0b94b314ea9a6c6d1ab4f6afe0dad22ef3415700f289bc9af411'
tags:
  - measured
  - closing
  - threads
  - pseudoterminal
  - F2.1
feature_id: feat_term-core_nx0g1r
---

## What it does

**Measured on this machine against `term-core`, not reasoned.** `Drop for Pty` does three blocking
things and **none of them has a deadline**: `child.kill()`, `child.wait()` and `reader.join()`. While
that only ran when closing the application, a hang was mistaken for «it takes a while to close». Since
F2.1 a tab can be closed **hot**, and that same path runs **inside the interface pass**.

**It freezes the window for ~101 ms per tab closed.** `child.kill()` of `portable-pty` **does not send
SIGKILL: it sends SIGHUP**, and afterwards grants grace by polling with 50 ms waits. The first poll goes
right after the signal, so in the normal case —the program does die, but does not get the time before the
first poll— a whole cycle is paid for. Ten samples with a live shell inside: median **50.72 ms**,
deviation **below the millisecond** — this is not noise, it is the fixed `sleep`. With the program
already dead, **0.23 ms**. Two consoles per tab in series → ~101 ms, some 17 frames of the budget D3
publishes. With a program that ignores SIGHUP, **404 ms**.

**And it can hang the window indefinitely.** Three facts that separately are all fine:

1. The signal goes **to the child's pid**, not to its process group: whatever the shell launched that
   outlives it receives nothing.
2. That grandchild **inherits the descriptors of the slave end**. As long as somebody has it open,
   `read()` never reaches end of file. The comment that says «the only one holding it is the child»
   **stops being true the moment the shell launches something that outlives it — which is what a shell
   does**.
3. The `parar` flag **is only consulted when trying to put something into the channel**. A thread blocked
   reading bytes that never arrive never looks at it.

Result: `reader.join()` does not return and **the whole window is left frozen, with no repaint and no
keyboard**. Measured with `sh -c "setsid sleep 60 & exec sleep 60"` —what anything launched with `&`, a
`nohup` or a development server does—: **it did not finish in 8 seconds**. The same with a program that
ignores the signal, where the grace SIGKILL kills the shell and leaves the grandchild orphaned with the
pseudoterminal open.

> **[[every-wait-adds-a-closing-path]] predicted it days earlier**, in these words: «it becomes a leak
> for every pane closed at that instant **as soon as closing a pane stops closing the process**». That
> happened in F2.1, and the symptom turned out to be worse than a leak.

**Where to fix it**, in order of what resolves most: signal the **process group** (`killpg`) instead of
the pid —the child is already the session leader because `portable-pty` does `setsid()` in `pre_exec`—,
which resolves it at its source; give the reader thread a way to abandon **without end of file**, or do
not join it without a deadline (closing the master is not enough: the thread owns a descriptor of its own
from `try_clone_reader()`); and do not pay the library's polling inside the frame.

**There are no zombies:** `child.wait()` reaps the direct child, and orphaned grandchildren are adopted
by `init`.

## Where it lives

`Drop for Pty` in `crates/term-core/src/lib.rs`, invoked on dropping a `Tab` from
`Window::close_the_active_tab` in `crates/app/src/lib.rs`, inside `eframe::App::ui`. Ticket open
with the measurements: https://github.com/mashware/agent-terminal/issues/62
