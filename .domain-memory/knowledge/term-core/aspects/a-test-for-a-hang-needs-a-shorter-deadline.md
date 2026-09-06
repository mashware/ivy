---
id: asp_a-test-for-a-hang-needs-a-shorter-deadline_y5xtgw
slug: a-test-for-a-hang-needs-a-shorter-deadline
name: >-
  A test for a hang hangs the suite instead of failing it — and its shell script
  is a guard, not a decoration: three measured false greens
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T15:16:28.874Z'
updated_at: '2026-09-06T15:16:28.874Z'
last_verified: '2026-09-06T15:16:28.874Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
symbols:
  - drain_until_closed
  - wait_for_marker
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:36606ed55bf82b06567677e0eac02210dd9c77467ee0ebae732f83244bfae7e8'
  crates/app/src/lib.rs: 'sha256:16cf7238d33ef681a4275bd08787a1c153e6e8f5ab1c54e2d30cd5157e62f4f4'
tags:
  - tests
  - mutation
  - false-green
  - pty
feature_id: feat_term-core_nx0g1r
---

## What it does

**A test that reproduces a hang does not go red: it takes the suite with it.** That is why the bug of the blocking write had no test for thirty days — and it is not an oversight, it is the shape of the failure.

**The technique, which this crate already had a precedent for and nobody had carried across:** do the blocking call on a **spawned thread** that answers through a channel, and assert with `recv_timeout`. The precedent is `drain_until_closed`, whose own comment says *«with a deadline on purpose: a `recv()` without a deadline would turn the failure these tests hunt into an eternal hang instead of a red test»*.

**And the deadline has to be shorter than the failure's own patience.** The test program is a `sleep 6`; the deadline is **3 s**. With a longer one the write would «return» when the child died and **the bug would publish as fixed** — the project's second disguise of a test that covers nothing: picking a number where the defect and the fix give the same result. Result: red at **3.01 s**, with its message.

### The shell script of such a test is load-bearing, and `;` breaks it

Three false greens **measured** in the first version of these tests, all from one root: the script chained with `;`.

| What was wrong | What it produced |
|---|---|
| `stty raw -echo; printf MARKER; sleep 6` prints the marker **even when `stty` failed** | bug restored + `stty` missing → the regression test passed in **0.02 s over the live bug** |
| With the echo still on, the terminal hands the payload back by itself | a «nothing was lost» count completed **without the reader ever running** — 0.10 s instead of ~1.1 s. An «nothing foreign arrived» assertion cannot see it: echoed bytes are the same filler |
| Nothing checked the handover happened while nobody was reading | on a loaded machine it degrades into «a pseudoterminal delivers bytes», which nobody doubts |

**`&&` and a marker the program prints itself, never `;` and a `sleep`.** Two markers when the test needs two states — applied, and reading.

**And `stty` missing is not hypothetical: F2.7 is heading for macOS and Windows**, where what exists and how it behaves is precisely what that milestone is for. A green in 0.02 s on those platforms would be permanent.

> **Two more tests in this repository still carry the pattern** — `stty -echo -icanon; printf …; echo ready; cat -v`, twice — and they are the ones that hold **D23** up. Ticketed.

Same family as [[a-test-that-pins-the-helper-does-not-pin-the-path]] and [[what-breaks-without-anything-failing]]: green suite, green guards, nothing held.

## Where it lives

`crates/term-core/src/lib.rs`, `mod tests`: `the_write_returns_even_though_the_program_never_reads_its_input`, `nothing_handed_over_is_lost_when_the_program_reads_late`, and the precedent in `drain_until_closed`. The two that still carry the pattern are in `crates/app/src/lib.rs`, in the mouse and alternate-screen tests.
