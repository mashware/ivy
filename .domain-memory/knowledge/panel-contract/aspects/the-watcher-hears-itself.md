---
id: asp_the-watcher-hears-itself_9chdbq
slug: the-watcher-hears-itself
name: >-
  A directory watcher hands you back your own reads: the application heard
  itself and spun at 60 frames with no visible cause
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:21:52.516Z'
updated_at: '2026-08-13T13:21:52.516Z'
last_verified: '2026-08-13T13:21:52.516Z'
file_paths:
  - crates/app/src/watcher.rs
  - crates/app/src/panel.rs
  - docs/04-contrato-panel.md
symbols:
  - Watcher
  - Flag
  - is_nothing_new
content_hashes:
  crates/app/src/watcher.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
  crates/app/src/panel.rs: 'sha256:568e67aba57fd9f60e9ca802161c515776057b5d550f6074bf1af4cf561d98a8'
  docs/04-contrato-panel.md: 'sha256:74a34b90ad96f4314965ff99fe2a644715d2ae032f5914bdfe890ef4d0608c4d'
tags:
  - inotify
  - notify
  - watcher
  - performance
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The failure, and it is one of those you do not see by looking at the screen.** The mask with which `notify` registers a directory includes `OPEN`. That is: **opening the `panel.json` to read it raises the flag that says «the `panel.json` has to be read»**. With a valid file the loop cuts itself off, because the comparison of dates avoids reopening it; but **with a file that is not understood there is no good date to store**, so it is reopened every time. Measured: **123 reads in two seconds against the 2 expected**, the window never sleeping, and on screen absolutely nothing moves.

> **Before arming a watcher, the question is not «which events interest me» but «which of its events do I produce myself».** A watcher on the directory you also read from is a closed loop by construction, and its symptom is consumption, not an error.

**The fix, and why it was not the simple one.** The access notices are discarded **except close-after-write** (`Access(Close(Write))`). Filtering them all —the obvious proposal— would have left out the producer that writes **without renaming**: for that one, closing after writing is the only signal that it has finished, and without it the panel would be read half written.

**The rename's echo, measured and discarded.** Publishing the way the contract recommends —writing aside and renaming on top— cost **1.88 frames** instead of one: the rename delivers the destination and **immediately afterwards** a second notice with origin and destination together. Verified in `notify`'s source that the second one never arrives without the first one in front of it, so it is discarded without losing anything and it goes down to **1.00**.

**What was decided NOT to fix, with its number.** Attending to the «I have touched it» notices would mean reading mid-write, and a producer that writes in pieces would cost **~42 frames per publication**. Keeping only «I have finished» would leave out the producer that keeps the file open and rewrites it in place. It stays as it is and the published contract gains a bullet —«publish in one go»— with the measured figure, which is the honest way of passing the cost on to whoever can avoid it.

## Where it lives

`crates/app/src/watcher.rs` (crate `app`), over `notify` 8.2, which on Linux is inotify. The watcher looks at each pane's **directory**, not its `panel.json`, and raises one flag per console.
