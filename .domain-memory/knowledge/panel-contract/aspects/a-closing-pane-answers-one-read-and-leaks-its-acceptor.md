---
id: asp_a-closing-pane-answers-one-read-and-leaks-its-acceptor_7v8c2b
slug: a-closing-pane-answers-one-read-and-leaks-its-acceptor
name: >-
  A closing pane's connection answers one read and then vanishes; its acceptor
  is never woken, and that leak hides a false watcher line (#293, #294)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T22:38:22.067Z'
updated_at: '2026-09-26T22:38:22.067Z'
last_verified: '2026-09-26T22:38:22.067Z'
file_paths:
  - crates/panel-control/src/listener.rs
  - crates/app/src/lib.rs
  - crates/panel-contract/src/lib.rs
  - crates/app/src/watcher.rs
symbols:
  - converse
  - Drop for Channel
  - PanelDir
  - lost_a_watched_dir
content_hashes:
  crates/panel-control/src/listener.rs: 'sha256:61294a07fa0ecc5bdc1008e877c3994eb5ae8acba1b1da0d996040e2ac298e68'
  crates/app/src/lib.rs: 'sha256:cdc5459384a2f25a97423e26d6f64a76cf40afaf1715e852bbe88ca5ce6d7a1c'
  crates/panel-contract/src/lib.rs: 'sha256:9ab0d8898f2610de4c4737d0447eb64f9480ccee0b09f66f1616af7cf6616ed2'
  crates/app/src/watcher.rs: 'sha256:c6fd561c46c58a536845d23d75d13f97ba6bda3949206d095866f3f98e6d28a0'
tags:
  - control-channel
  - pane-close
  - race
  - leak
  - watcher
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Measured on Linux 7.0.0-34, 27 September 2026.**

**What a producer gets from a pane that is closing.** A connection's thread reads with a 200 ms timeout and leaves at the first read that **begins** with the stop flag raised. One `panel_get` it reads once the flag is up is answered (the admitted document or «this pane is gone»); a second is not. A request that arrives after the thread has gone gets `EPIPE` on write; bytes still unread when the socket closes get `ECONNRESET` on the peer's next read (Linux AF_UNIX: `unix_release_sock` sets `sk_err`). A test that asks a pane after closing it is therefore racing a timer: it passed on almost every run and flaked under load (#293). The admission rule only covers what the thread still reads, which is now pinned by `one_read_is_answered_after_the_pane_closes_and_no_more` in `crates/panel-control/src/listener.rs`.

**The acceptor of a closed pane is never woken (#294, open).** `Pane` drops `panel` (the `PanelDir`, which unlinks `control.sock` and the directory) before `control`, whose `Drop for Channel` wakes the acceptor by connecting to that socket path. The knock finds no file, fails silently, and the acceptor stays in `accept` for the life of the process: one thread and one listening fd per closed pane (seen in `/proc/self/task/*/comm`). The same wrong order is in the control test harness, listener.rs's test `Pane`, and `attach_a_channel`.

**That leak hides a false line.** The kernel reports a watched directory deleted (`IN_DELETE_SELF`) only once nothing holds it, and a bound AF_UNIX socket pins its path. With the acceptor woken, the watcher being replaced on close prints «the panels will be reread on every frame» for the closed pane's directory, which is false: the window re-arms a fresh watcher. A `strong_count`-on-the-flag guard was tried and rejected in review (two closes in one pass bring it back; it makes the re-arm guard test unable to fail; a live pane could lose lost-directory detection). And once the knock reaches a listener it is a blocking `connect` on the interface thread that a full backlog could hold. So #294 needs a design: a wake that does not go through the socket file, and an explicit unwatch or a replaced watcher that ignores notices.

**Method lesson**: a leak masked a second defect, the same shape as [[moving-a-value-into-a-thread-moves-its-drop]]. Measure what changes downstream of a fix before claiming it: here the first reading, «the window degrades», was false, and only a check of the real watcher showed it.

## Where it lives

crates/panel-control/src/listener.rs (`converse`, `Drop for Channel`, the test); crates/app/src/lib.rs (`Pane` field order, the final-report test); crates/panel-contract/src/lib.rs (`remove`); crates/app/src/watcher.rs (`lost_a_watched_dir`); issue #294
