---
id: asp_capturing-stderr-lives-in-main_amyxqh
slug: capturing-stderr-lives-in-main
name: >-
  Anything that captures the window's standard error has to live in main, not in
  run: the startup error is printed after run returns
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T18:32:32.148Z'
updated_at: '2026-11-21T18:32:32.148Z'
last_verified: '2026-11-21T18:32:32.148Z'
file_paths:
  - crates/app/src/main.rs
  - crates/app/src/diagnostics_log.rs
symbols:
  - main
  - finish
content_hashes:
  crates/app/src/main.rs: 'sha256:f2eddad4ab805f53378775aae7005bbd6ef651a12a10b59f1b12c75e83336b75'
tags:
  - stderr
  - log
  - startup
feature_id: feat_term-core_nx0g1r
---

## What it does

`main` returns `eframe::Result`, so a window that cannot open is printed by std's
`Termination` as `Error: ...` AFTER `app::run()` returns. A capture scoped to `run()` is
already gone by then; if fd 2 were still a pipe whose reader had ended, that line would be
lost from the log and from the terminal. D114 keeps the capture in `main` and has `main`
print the error itself while the capture stands (`diagnostics_log::finish`).

Second trap of the same piece: `std::io::pipe()` ends are close-on-exec on both platforms,
raw `libc::pipe`/`dup` are not, and the app starts git with inherited descriptors
(viewer/load.rs, board/branches.rs, project.rs). A leaked write end keeps a copying thread
from ever seeing the end of the pipe.

Evidence: crates/app/src/main.rs at origin/main 81521ec; found by the design challenger of
#139; `a_window_that_cannot_open_says_so_in_the_file_and_on_the_original` and the real
binary run without a display (exit 1, `Error:` in both).

## Where it lives

crates/app/src/main.rs, crates/app/src/diagnostics_log.rs (finish, capture)
