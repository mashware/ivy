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
updated_at: '2026-11-07T21:05:59.155Z'
last_verified: '2026-11-07T21:05:59.155Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/injected/links.rs
  - crates/app/src/links/console.rs
  - crates/app/tests/batches.rs
  - crates/host-load/src/lib.rs
  - crates/app/src/settings/editor/script.rs
  - docs/02-decisions.md
symbols:
  - ForTheBench
  - 'InjectedDrags::links_were_painted'
content_hashes:
  scripts/measure-frame.sh: 'sha256:045ddf68ef22071e604578015988572f8f9580abc21713abcebfd5064e76ffeb'
  crates/app/src/injected/links.rs: 'sha256:3f5cbd8674fdd30b55d82eb8622434e1f451d26222fb4557a68448d3288c51be'
  crates/app/src/links/console.rs: 'sha256:6a05543f47a874397988814fc79df4e8061c0b3279ae720ae6f516cadb609251'
  crates/app/tests/batches.rs: 'sha256:2c937d3be9c8cc05ff0693f82ba46702e79292df8f6ede04bf514f840385dcd5'
  crates/host-load/src/lib.rs: 'sha256:03d8066b569ac30919c7a2b1abe5a34e1a750c367183a251f7dfab4e0a603c5a'
  crates/app/src/settings/editor/script.rs: 'sha256:903f13d79e1161b9d59cdc3bf4e9cc672e10b8bed64dcbca77970dbafd840829'
  docs/02-decisions.md: 'sha256:a3202b61ac7b1185d28bd3c2b83ca16fd72337fe23e9489f7c490ba578b24845'
tags:
  - bench
  - measurement
  - D13
  - batch-W
  - F2.15
  - D63
feature_id: feat_term-core_nx0g1r
---

## What it does

Found in the review of #257 before batch `W` ever ran, and each one would have invalidated or mislabelled every run:

- **The target rows must sit OUTSIDE the scroll region the dump runs in.** The ordinary load scrolls the whole grid; links written at the top were carried away between 20 ms rewrites, so stale points missed and the presses fell into focus, selection-clear or panel clicks. `W` sets `\033[3;r` and re-asserts it on every rewrite inside the saved cursor, because a resize resets alacritty's margins.
- **Clear of host-load's own counter**: the load paints ` {i} ` at the top-left (`\x1b7\x1b[H\x1b[7m {i} \x1b[0m\x1b8`, about nine columns), so the targets start at column 20 after a space — a stray character glued to `https` ends a plain link.
- **The injector must use points from the LATEST painting only**, reset each frame. The routing resolves a press against the painting before it, and the panel is republished every second, so its next painting is unsettled and unreachable; a point kept from further back lands on nothing.
- **Only then can the guard demand one opened link per completed gesture** (`LINK_PLAIN + LINK_OSC + LINK_PANEL < INJECTED` invalidates). Counting «at least one activation per source» let a run of mostly-missed presses publish as the gesture's cost. `INJECTED` counts completed gestures; a press in flight at close makes the sum one higher, never lower.

And **the script does not build**: it only checks the release binaries exist, so a stale one runs and its report wording no longer matches the new anchors — the empty-capture guard is what caught it. Build with `cargo build --release -p app -p host-load` first. Result on #257: two valid runs, 860/860 and 504/503.

### Anchored settings dialogs: a missed click can masquerade as free discovery

F2.15 (#322, PR #323) exposed the same false-measurement risk outside a scrolling console. Opening an anchored dialog, and then loading its font inventory, can move its controls on the next layout pass. A click at a rectangle retained before that change may miss the intended control; a zero enumeration cost would then describe an action that never happened, not an inexpensive font search.

The settings-fonts scenario therefore leaves a quiet layout frame after opening and after the first inventory request, uses the current painted control rectangle, and requires **two accepted font requests but exactly one enumeration**. This jointly proves that the first explicit lookup occurred and the repeated lookup reused the inventory. Merely counting injected clicks cannot establish either property. Two valid release runs witnessed two accepted requests, one enumeration and 259 installed family names each.

Keep the evidence boundary explicit: this scenario does not expand the family dropdown, so it does not measure painting the full inventory. Its configuration I/O is vetoed, and Xvfb/software-renderer timings do not prove native keyboard, clipboard or real-file Save latency. D63 records the measured spans and those limits.

## Where it lives

`scripts/measure-frame.sh` (batch W and settings variants, accepted-action/effect guards), `crates/app/src/injected/links.rs` (`links_were_painted`), `crates/app/src/links/console.rs` (`ForTheBench`), `crates/app/src/settings/editor/script.rs` (anchored dialog settling), `crates/app/tests/batches.rs` and D63 in `docs/02-decisions.md`.
