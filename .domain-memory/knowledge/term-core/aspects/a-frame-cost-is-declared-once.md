---
id: asp_a-frame-cost-is-declared-once_dzpy7b
slug: a-frame-cost-is-declared-once
name: >-
  Declaring a frame cost twice subtracts it twice — and a name whose absence
  would make a refusal a lie is acquired BEFORE the resource it names
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T18:28:16.753Z'
updated_at: '2026-09-16T18:28:16.753Z'
last_verified: '2026-09-16T18:28:16.753Z'
file_paths:
  - crates/app/src/control.rs
  - crates/app/src/closing.rs
  - crates/panel-control/src/wire.rs
symbols:
  - 'Window::close_a_pane_called'
  - 'Window::answer_the_windows_requests'
  - opening_for_a_connection
  - MAX_PROGRAM
  - MAX_ARGS
content_hashes:
  crates/app/src/control.rs: 'sha256:953bd85dc730a278480bdb6e760773a2191539f835c733250f370faf7eb13542'
  crates/panel-control/src/wire.rs: 'sha256:6515e4305c93b63c85de343c220f2c139c4eff83c715c61e82210a70534f5cae'
tags:
  - D13
  - measurement
  - frame-cost
  - ordering
  - execve
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two traps from one delivery about the frame's cost, which is split by subtraction (D13).**

**One: declaring the same work twice subtracts the same microseconds twice.** `Window::close_a_pane_called` added `its_cost + before.elapsed()` to `timed_separately`, and its caller `Window::answer_the_windows_requests` times the whole drain and adds that too. The work was declared twice, so the framework's figure — the one D3 judges egui by — was understated by it. That is the exact mirror of the failure the rule exists to prevent (undeclared work published as the framework's), and neither the report nor any test shows either direction. **The rule that comes out: declare a piece of work once, at the boundary of the frame path that runs it — and when a helper is called from a frame path that already times itself, the helper must not time itself.** The chord's close keeps its own timing because nothing else times it.

**Two: a name a refusal must be truthful about is acquired BEFORE the thing it names.** The 16-random-byte handle for `split_open` was obtained after `Pane::launching_with_args` and after the graft, so a `getrandom` failure would have answered «nothing was opened» while a live console was on screen. The fix moved the handle to the top of `opening_for_a_connection`, before the name is spent and before the program is launched: failing there spends nothing and opens nothing, and the refusal is true. The same shape applies to any token, name or id whose absence makes an answer a lie.

**Also shipped in the same review round, same family:** `program` and `args` had no bound where `source` had `MAX_SOURCE`, so the transport's 256 KiB ceiling admitted a single argument that reaches `execve` and comes back `E2BIG` — a launch failure the caller cannot read as its own mistake. `MAX_PROGRAM` (1 KiB) and `MAX_ARGS` (16 KiB) are measured on the encoded form and refused as malformed.

## Where it lives

`crates/app/src/closing.rs::close_a_pane_called` (no timing of its own) and `crates/app/src/control.rs::Window::answer_the_windows_requests` (the single declaration). Ordering: `crates/app/src/control.rs::opening_for_a_connection` (the handle before the launch). Bounds: `crates/panel-control/src/wire.rs` (`MAX_PROGRAM`, `MAX_ARGS` in `split_open`). Related cards: whoever-does-not-declare-their-work-attributes-it-to-someone-else, seeding-can-be-pinned-timing-not-always.
