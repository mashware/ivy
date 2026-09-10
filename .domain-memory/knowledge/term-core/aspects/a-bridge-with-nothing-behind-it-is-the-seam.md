---
id: asp_a-bridge-with-nothing-behind-it-is-the-seam_hax8g6
slug: a-bridge-with-nothing-behind-it-is-the-seam
name: >-
  «The app cannot build that state» was wrong: a bridge with nothing behind it
  gives a console an ending that is not «ended well» — with three traps, and one
  of them empties the grid
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-10T22:24:30.771Z'
updated_at: '2026-09-10T22:24:30.771Z'
last_verified: '2026-09-10T22:24:30.771Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pty_output.rs
  - crates/term-core/src/lib.rs
  - docs/DEBT.md
symbols:
  - make_it_go_quiet
  - 'PtyOutput::new'
  - is_finished
  - 'Pane::is_finished'
  - 'Ending::NobodyListening'
  - 'Screen::to_text'
content_hashes:
  crates/app/src/lib.rs: 'sha256:171c1421de9c4a6d0646eb0c98664b64626350b32415c7e52c7c788d0838ed09'
  crates/app/src/pty_output.rs: 'sha256:e86ddf6c839cd5597c18e2a1a61ee4128cae5d001d589c146bead8f237cf3f2d'
  crates/term-core/src/lib.rs: 'sha256:5af3288f30591cf1465e35c369b5fd72b46a34799967c3c9dc421abb80bb2aaa'
  docs/DEBT.md: 'sha256:8f02899880fa7e6ec60fb602c7dedc7fcd31f94387d62d5529ab9f137d51548a'
tags:
  - d30
  - d40
  - mutation
  - measured
  - test-seam
feature_id: feat_term-core_nx0g1r
---

## What it does

**The claim was: no test in `crates/app` can produce a console whose reading did not end well, because a failed read cannot be built from outside `term-core`.** The second half is true — `Pty` has only `spawn` and `spawn_with_env`, the ending is stamped by the reader thread, and on Linux the one errno a broken pseudoterminal gives is mapped to end of file **before** the failing arm ([[both-platforms-answer-end-of-file-and-windows-never-does]]). The first half does not follow, and **two reviewers refuted it independently**.

> **The question is «did it end well», so any other ending serves — it does not have to be a failure.** Swap the pane's bridge for one with nothing behind it and the reading finds its listener gone with the program still alive:
>
> ```rust
> let (orphan, nowhere) = std::sync::mpsc::channel();
> pane.pty_output = PtyOutput::new(nowhere, || {});
> drop(orphan);
> // then wait for pane.pty.ending() to be Some — it will be NobodyListening
> ```

**Three traps, all met while writing it:**

- **The program has to keep writing.** The reading only notices its listener is gone when it next *delivers*, so `sleep 30` never ends and the wait times out. `yes` works.
- **The program must not have exited first.** The ending is a `OnceLock`: `printf hello` records `EndedWell` before the swap, and the swap can never change it.
- **After the swap the pane can never pump again**, because the new bridge has no sender. **A test that then asserts «the application's sentence is not in the grid» is asserting absences over an EMPTY STRING** — measured, `to_text()` came back with length 0, and every mutation passes it. Pump the program's own output **before** the swap, and assert the grid is not empty before asserting what is missing from it. (Also: `Screen::to_text` walks only `0..screen_lines()`, so it never reads the scrollback a promise about the scrollback names; and take the words looked for from the sentence's own constant, or a rewording leaves the test green over a leak.)

**Why the wrong claim was believed, and this is the transferable half:** the decision had been extracted into a free function precisely so it could be tested — `pty_output::is_finished(alive, queue_left, ending)`, exhaustive over every `Ending` — and the extraction *is* the right move for the arms that really are unreachable (a failure, a panic). But **extracting for testability tests the decision, never that anybody asks it**: deleting the pane's question altogether left all 840 tests green, while asking for the *wrong* ending failed two. «This state is unreachable» was a claim about the whole state space made from one corner of it.

**An extraction is not a substitute for a seam** — and the seam was already there, in a constructor that takes any `Receiver<Vec<u8>>`.

## Where it lives

The recipe lives in `make_it_go_quiet` in `crates/app/src/lib.rs`'s test module, and the mechanism it exploits is `PtyOutput::new` in `crates/app/src/pty_output.rs` taking a plain `Receiver`. The decision it exercises is `pty_output::is_finished`, cited by **D40**; the arms it still cannot reach (a failed read, a panicked reader) are why that function takes values instead of a pane. What is left uncovered is in `docs/DEBT.md` under 192.
