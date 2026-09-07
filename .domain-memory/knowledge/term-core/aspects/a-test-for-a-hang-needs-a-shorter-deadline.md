---
id: asp_a-test-for-a-hang-needs-a-shorter-deadline_y5xtgw
slug: a-test-for-a-hang-needs-a-shorter-deadline
name: >-
  A test for a hang hangs the suite instead of failing it — and it needs a
  FLOOR, a premise proved by whoever holds it, and a script that is a guard:
  seven measured false greens
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T15:16:28.874Z'
updated_at: '2026-09-07T20:24:05.554Z'
last_verified: '2026-09-07T20:24:05.554Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/closing.rs
  - crates/app/src/lib.rs
symbols:
  - drain_until_closed
  - wait_for_marker
  - READER_GIVE_UP
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21899fbd18b28515f01ca0c0c025ca2b260981459a259167fbc21b961f6070c7'
  crates/app/src/lib.rs: 'sha256:a0c2bb67292fa890b3b484429675a8747e96cdbaaa36ef25625f7e2c98fb897b'
tags:
  - tests
  - mutation
  - false-green
  - pty
  - D30
  - D32
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

> The two tests that still carried the pattern — the ones holding **D23** up — were closed by **D30**, which added the mode probe and a guard over the source in both crates.

### Three more rules, from the hang on closing (D32, September 2026) — and each one had passed green over a live bug first

**1. The premise is proved by WHOEVER HOLDS IT, not by the process that launched it.** The test needed a survivor holding the slave end, and it launched one with `{ setsid … & }` and then waited for the **shell's** marker. A backgrounded list **exits 0 whatever happens inside it** — verified: `sh -c '{ nosuchcmd & } && echo REACHED'` prints REACHED — so with `setsid(1)` missing, which is every macOS, the chain reached its `exec` with **nobody holding anything** and the test passed measuring the easy case. The fix: the survivor is a shell of its own that prints **its own** marker, and the test waits for that. Seeing it come out of the master proves the survivor exists *and* that it can write through the descriptor it inherited. Chained last, it also proves the `stty` before it exited 0.

**2. A «did it finish?» test needs a FLOOR, or every accidental rescue passes.** Only an upper bound was asserted, so the two ways the hang gets rescued by accident — the echo left on, no survivor at all — came back in hundredths of a second and passed. Asserting `took >= READER_GIVE_UP` turns both into red tests, **and the bound is the constant itself** so a delivery that makes the mechanism cheaper moves it instead of going red.

**3. The other half of the claim needs its own test.** The hang test says closing comes back when a survivor holds the descriptor. It says nothing about the healthy close, so the whole timed wait could be **deleted — or made unconditional — with the suite green** while every closed console cost a tenth of a second more, once per pane of the tab. Three reviewers found that independently. A second test asserts the healthy close does *not* spend the deadline.

**And in a measuring harness: the label of a number comes from the SCENARIO, never from the clock.** The reproduction picked each row's explanation from timing bands, so an ordinary close at 130 ms on a busy machine would have printed «somebody survived» — and if the library's immediate first poll ever reaped first, the hang row would have printed the library's poll, **publishing this crate's own deadline as somebody else's**. Now each row declares what it measures, the elapsed time is *checked* against the band that claim implies, and a row outside its band says so and makes the run exit non-zero.

Same family as [[a-test-that-pins-the-helper-does-not-pin-the-path]] and [[what-breaks-without-anything-failing]]: green suite, green guards, nothing held. And see [[a-mutation-only-tests-what-its-author-imagined]] for why two rounds of mutation still missed rule 1.

## Where it lives

`crates/term-core/src/lib.rs`, `mod tests`: `the_write_returns_even_though_the_program_never_reads_its_input`, `nothing_handed_over_is_lost_when_the_program_reads_late`, `closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal` (which carries its own mutation table in its doc comment) and `closing_an_ordinary_console_does_not_spend_the_abandon_deadline`; the precedent is `drain_until_closed`. The measuring harness with the declared bands is `crates/term-core/examples/closing.rs`.
