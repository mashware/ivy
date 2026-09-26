---
id: asp_non-blocking-pty-traps-macos-and-spinning_7vwspk
slug: non-blocking-pty-traps-macos-and-spinning
name: >-
  Making the pty non-blocking: macOS's poll does not wait on a pty, and a
  spinning wait passes every functional test — only processor time catches it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T19:31:39.259Z'
updated_at: '2026-09-26T19:31:39.259Z'
last_verified: '2026-09-26T19:31:39.259Z'
file_paths:
  - crates/term-core/src/nonblocking.rs
  - crates/term-core/tests/idle_console_cpu.rs
  - crates/term-core/src/lib.rs
symbols:
  - Readable
  - 'Writable::wait'
  - write_chunk
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:598307ef482b8847d279ad556b074bb929e86396d7284e83bd995094bd17b1e8'
tags:
  - macos
  - poll
  - testing
  - cpu
  - F2.7
feature_id: feat_term-core_nx0g1r
---

## What it does

**Found reviewing #159 (26 September 2026).** Three traps for anyone who waits on the pseudoterminal with `poll`.

**1. macOS's `poll(2)` does not wait on a pty: it answers `POLLNVAL` at once.** The filedescriptor crate beneath `portable-pty` says so (`filedescriptor-0.8.3/src/unix.rs:463`, «macOS has a broken poll(2) implementation») and switches to `select` there. A poll-based reader would spin an idle console at 100 % CPU. A poll-based writer would read a full buffer as «program gone» and go deaf on a live program. So #159's non-blocking path is `cfg(target_os = "linux")` only. macOS and Windows keep the blocking write and the leak, and F2.7 has to choose with `select` or a measurement of its own. The first version of the fix said «Unix», was only measured on Linux, and the review caught it.

**2. A spinning wait is invisible to every functional test**: the bytes still arrive. Measured, all 117 term-core tests stayed green with the reader's `poll` removed, and again with the writer's wait turned into a zero-timeout `poll` that answers «room». **The only binding assertion is processor time.** Use `CLOCK_PROCESS_CPUTIME_ID` over a 500 ms idle window with a 50 ms ceiling, in an integration-test file with a single `#[test]`, so the process clock counts nobody else; run the measurements in sequence inside that one test. A spinner measured 499.9 ms. A loaded machine can only make it pass wrongly (a spinner gets less CPU), never fail wrongly.

**3. A `poll` that wakes with neither the awaited event nor `POLLHUP` (`POLLERR`, `POLLNVAL`) must fail, not retry** — the next read or write may say «would block» again for ever. It is testable without a pty: `poll` for input on the writing end of a pipe whose reading end is closed answers only an error flag.

## Where it lives

`crates/term-core/src/nonblocking.rs` (module doc, `Readable::read`, `Writable::wait`, its unit test), `crates/term-core/tests/idle_console_cpu.rs`, the non-Linux `write_chunk` in `crates/term-core/src/lib.rs`, D29's amendment.
