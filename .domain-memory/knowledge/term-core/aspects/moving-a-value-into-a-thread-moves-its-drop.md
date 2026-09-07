---
id: asp_moving-a-value-into-a-thread-moves-its-drop_exsy3c
slug: moving-a-value-into-a-thread-moves-its-drop
name: >-
  Moving a value from a struct field into a thread moves WHEN its Drop runs —
  and it hid a month-old hang, which is worse than causing one
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-07T20:24:35.526Z'
updated_at: '2026-09-07T20:24:35.526Z'
last_verified: '2026-09-07T20:24:35.526Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - Pty
  - Drop
  - UnixMasterWriter
  - writer
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21899fbd18b28515f01ca0c0c025ca2b260981459a259167fbc21b961f6070c7'
tags:
  - drop-order
  - rust
  - regression
  - review-question
  - D29
  - D32
feature_id: feat_term-core_nx0g1r
---

## What it does

**A fix for one bug made a different, month-old bug unreachable — through destructor ordering alone, with nobody on either side of the review having reason to look.** Nothing was broken by it. Something was **hidden** by it, and that turned a reproducible hang into one that needs `stty -echo` to see.

## The mechanism, in one sentence

**A struct's fields drop AFTER its `Drop::drop` returns; a thread's locals drop when the thread ends.** So moving a value out of a field and into a thread moves its destructor from «after the whole body» to «whenever that thread finishes» — which, for a thread whose loop ends as soon as its channel closes, is *the first thing the body does*.

## The case, measured

**Before #157** (`03803eb^`, 6 September 2026), `Pty` held the pseudoterminal's master writer as a **field**: `writer: Box<dyn Write + Send>`. `Drop::drop` never touched it, so `UnixMasterWriter::drop` — which writes `\n` + the terminal's `VEOF` into the master — ran **after `reader.join()`** and could rescue nothing.

**#157 gave writing a thread of its own** (D29, the fix for the blocking write). The writer moved **into that thread**, where it is dropped the moment the sender closes — and closing the sender is the **first** thing `Drop for Pty` does. From that commit on, the write lands *before* the kill and the join, the line discipline echoes it back out of the master, and a reader parked in `read` is woken by it.

| Row, same example, seven of them | `03803eb^` | after #157 |
|---|---|---|
| survivor holding the slave, **echo on** | **no return** | 50.72 ms |
| survivor holding the slave, echo off | **no return** | **no return** |

So the hang stopped being reachable in the common case, and **that is why four attempts on `main` could not reproduce what the issue had measured in August**. The bug was still there, still one `stty -echo` away, and `stty -echo` is the first thing every TUI does. See [[closing-hot-hangs-the-window]].

## Why no review caught it

Nothing in #157's review was wrong. **The diff was about writing**, and destructor ordering is not visible in it: no line moved in `Drop::drop`, no line moved in the reader. What is missing is a question, and it is cheap enough to ask every time:

> **Does this change when anything drops relative to anything else?** Specifically: did a value move between a struct field and a thread, a closure, or a local?

## The rule that generalises past drop order

During the investigation the sentence *«that commit did not touch the reader or the join»* was written down as evidence that #157 was not involved. It was **true and materially misleading**, and it nearly closed the investigation on «not a regression, nothing to see». An adversarial reviewer overturned it and named the experiment that settled it.

> **A non-regression argument has to be about REACHABILITY, not about which lines were edited.** «It did not touch X» answers a question nobody asked. The question is whether the change moved the state the defect needs — nearer, or out of reach.

The second form is the one that bites here, because **hiding a defect reads as fixing it**: the symptom stops appearing, no test goes red, and the next person to look concludes the report was wrong.

## Where it lives

`Drop for Pty` and the write thread's set-up in `crates/term-core/src/lib.rs`. The commit is #157 (D29, `03803eb`); the hang it hid is #62 (D32). The reproduction that shows both trees is `crates/term-core/examples/closing.rs`.
