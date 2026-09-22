---
id: asp_a-click-batch-against-a-scrolling-load_db3wfm
slug: a-click-batch-against-a-scrolling-load
name: >-
  A bench gesture that clicks something the load keeps rewriting misses unless
  the target sits outside the scroll region and the injector uses the latest
  painting only
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T19:51:41.577Z'
updated_at: '2026-09-22T19:51:41.577Z'
last_verified: '2026-09-22T19:51:41.577Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/injected/links.rs
  - crates/app/src/links/console.rs
  - crates/app/tests/batches.rs
  - crates/host-load/src/lib.rs
symbols:
  - ForTheBench
  - 'InjectedDrags::links_were_painted'
content_hashes:
  scripts/measure-frame.sh: 'sha256:db8a3f61e37be2aaaa0b1c1f944cc2c30835fd0f23f918142c9c1bf1acf53030'
  crates/app/tests/batches.rs: 'sha256:2671a3c77c38b08ba6cbcfe52a62fbd33ebf7f81465f442fa391f21963f1eb42'
  crates/host-load/src/lib.rs: 'sha256:03d8066b569ac30919c7a2b1abe5a34e1a750c367183a251f7dfab4e0a603c5a'
tags:
  - bench
  - measurement
  - D13
  - batch-W
feature_id: feat_term-core_nx0g1r
---

## What it does

Found in the review of #257 before batch `W` ever ran, and each one would have invalidated or mislabelled every run:

- **The target rows must sit OUTSIDE the scroll region the dump runs in.** The ordinary load scrolls the whole grid; links written at the top were carried away between 20 ms rewrites, so stale points missed and the presses fell into focus, selection-clear or panel clicks. `W` sets `\033[3;r` and re-asserts it on every rewrite inside the saved cursor, because a resize resets alacritty's margins.
- **Clear of host-load's own counter**: the load paints ` {i} ` at the top-left (`\x1b7\x1b[H\x1b[7m {i} \x1b[0m\x1b8`, about nine columns), so the targets start at column 20 after a space — a stray character glued to `https` ends a plain link.
- **The injector must use points from the LATEST painting only**, reset each frame. The routing resolves a press against the painting before it, and the panel is republished every second, so its next painting is unsettled and unreachable; a point kept from further back lands on nothing.
- **Only then can the guard demand one opened link per completed gesture** (`LINK_PLAIN + LINK_OSC + LINK_PANEL < INJECTED` invalidates). Counting «at least one activation per source» let a run of mostly-missed presses publish as the gesture's cost. `INJECTED` counts completed gestures; a press in flight at close makes the sum one higher, never lower.

And **the script does not build**: it only checks the release binaries exist, so a stale one runs and its report wording no longer matches the new anchors — the empty-capture guard is what caught it. Build with `cargo build --release -p app -p host-load` first. Result on #257: two valid runs, 860/860 and 504/503.

## Where it lives

`scripts/measure-frame.sh` (batch `W` load and guards), `crates/app/src/injected/links.rs` (`links_were_painted`), `crates/app/src/links/console.rs` (`ForTheBench`), `crates/app/tests/batches.rs`.
