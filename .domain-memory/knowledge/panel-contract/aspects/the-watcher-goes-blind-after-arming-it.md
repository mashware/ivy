---
id: asp_the-watcher-goes-blind-after-arming-it_mxcghx
slug: the-watcher-goes-blind-after-arming-it
name: >-
  A watcher can go blind AFTER being armed and in silence: without a fallback,
  adding it makes what there was worse
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:22:29.032Z'
updated_at: '2026-08-13T13:22:29.032Z'
last_verified: '2026-08-13T13:22:29.032Z'
file_paths:
  - crates/app/src/watcher.rs
  - crates/app/src/lib.rs
symbols:
  - Watcher
  - is_blind
  - POLL_WITHOUT_A_WATCHER
content_hashes:
  crates/app/src/watcher.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
  crates/app/src/lib.rs: 'sha256:1e1fccb7584128dc015d57b4a85b7afba61479eddebd8cffdbabeba928cc19be'
tags:
  - inotify
  - notify
  - watcher
  - degradation
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Three different ways of stopping being trustworthy, and none of them gives notice on its own:**

1. **Overflow of the kernel's queue** (`Q_OVERFLOW`). It arrives as `EventKind::Other` with `Flag::Rescan` and **with no path at all**. It is the only notice that literally means «you have missed things», and in the first version it was **the only one that was thrown away**, precisely for not carrying paths.
2. **The watched directory disappears.** The watcher died in silence.
3. **It could not be armed in the first place** — the kernel limits watchers per user.

> **The rule: adding a watcher without a degradation path MAKES what was there before WORSE.** Before this PR the panel was re-read when the console wrote; with the watcher blind and no fallback, the panel stays **frozen and saying nothing**. A mechanism that replaces another one has to know, at run time, whether it is still working.

The fix is that the watcher **knows whether it has stopped being trustworthy**, and the window asks: if it is blind, it re-reads every pane with a breather of 500 ms between passes. The three cases share that fix.

**And a written reason that fell apart when it was checked.** The row that justified the fallback said that the limit of watchers per user «runs out on a loaded desktop». Measured on the development machine: **46 out of 1024**. The mechanism stays —the real scenario is the other two, and those are measured— but the row was rewritten. An invented justification that holds up a correct mechanism is worse than none: the next person reads it, believes it, and uses it to decide something else.

**A design fact that comes from here:** the **directory** is watched and not the file, because a `watch` on a file goes on the **inode** and does not re-register — at the first rename on top of it, which is precisely the publication the contract recommends, the watcher is left looking at an orphaned inode. Measured before designing, not assumed.

## Where it lives

`crates/app/src/watcher.rs` and the loop in `crates/app/src/lib.rs`. `Watcher::is_blind()` is queried by the window on every frame; the fallback raises all the flags and asks for a repaint with a breather of 500 ms.
