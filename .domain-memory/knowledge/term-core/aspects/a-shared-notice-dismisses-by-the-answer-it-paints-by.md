---
id: asp_a-shared-notice-dismisses-by-the-answer-it-paints-by_y4qhvq
slug: a-shared-notice-dismisses-by-the-answer-it-paints-by
name: >-
  Two reasons sharing one notice box: a press must dismiss by the same answer
  the painting chose the sentence by, and a dismissal must be keyed on an event
  count, not on a state flag
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T14:25:15.436Z'
updated_at: '2026-09-28T14:25:15.436Z'
last_verified: '2026-09-28T14:25:15.436Z'
file_paths:
  - crates/app/src/painter/notices.rs
  - crates/app/src/pointer.rs
  - crates/app/src/pty_output.rs
  - crates/term-core/src/lib.rs
symbols:
  - Silence
  - what_the_silence_is
  - still_owes_the_refusal
  - dismiss_the_refusal
  - dismiss_the_silence
  - RefusedWrite
  - Backlog
content_hashes:
  crates/app/src/painter/notices.rs: 'sha256:00a25ae23301759a442babe1ce0f676edfca63db465d6da1f5f162497ac17746'
  crates/app/src/pointer.rs: 'sha256:ecb386ee4cd9e8d69906e98837d6c454ea2e8fbc839d8f4969f3bdabf8c4d9e6'
  crates/app/src/pty_output.rs: 'sha256:e86ddf6c839cd5597c18e2a1a61ee4128cae5d001d589c146bead8f237cf3f2d'
  crates/term-core/src/lib.rs: 'sha256:a9ffaf445eb42e55e3727ab8952a28d8012b37eaedc1a7bc9bc1e21335d03f16'
tags:
  - d39
  - d40
  - d41
  - d60
  - review
  - notices
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two rules, both found reviewing #161 (September 2026), both about the box under the held paste's question, which since D60 carries two independent reasons: the output ended (D40/D41) or the input queue refused a write.**

**1. One answer for 'which reason is on screen', asked by the painting and the press alike.** When both reasons hold, only the ended-output sentence is shown. The first shape dismissed *both* reasons on any press on the box — each dismissal was self-guarding (only acts if its own condition is true), so it looked safe. It was not: the press silenced a refusal nobody had seen. The second shape mirrored the precedence by hand in the press handler — correct, and a copy of a rule that would drift the next time a reason is added. The shape that stands: `notices::what_the_silence_is` returns a `Silence` enum; the sentence is chosen from it and the press dismisses from it. **'Each guard is individually safe' does not make calling all of them safe when they share one visible slot with a precedence.**

**2. A dismissal of something that recurs is keyed on how many times it happened, not on whether it is happening.** The refusal flag (`Backlog.refused`) stays up for as long as the queue is full, so a dismissal keyed on the flag silenced a retry refused into the same full queue — the person clicked 'Send it anyway' again and saw nothing. The first fix reset the dismissal on observing the flag down: a read with a side effect, safe only under an unchecked claim about threads, and still blind to that retry. What stands: `Backlog.refusals` (a count that only goes up); a dismissal records the count, and any later refusal brings the sentence back. The query became a pure read. Contrast `QuietConsole`, whose dismissal is sticky on purpose because an ending never recurs.

**Test hint.** With a held paste, the question and its button take the top half and this box is not drawn at all (D39), so a test that presses the refusal sentence must not also hold a paste. `make_it_refuse_a_write` (over a `stty raw -echo && ... && exec sleep 60` console) and `make_it_go_quiet` (over `exec yes`) compose on one pane to put both reasons up at once.

## Where it lives

`crates/app/src/painter/notices.rs` (`Silence`, `what_the_silence_is`, `still_says_a_write_was_refused`); `crates/app/src/pointer.rs` (the press dispatch on `boxes.silence`); `crates/app/src/pty_output.rs` (`RefusedWrite { dismissed_at }`, `still_owes_the_refusal`, `dismiss_the_refusal`); `crates/term-core/src/lib.rs` (`Backlog.refused`, `Backlog.refusals`). Regression tests in `crates/app/src/lib.rs`: `dismissing_the_ended_output_notice_does_not_silence_a_separate_refusal`, `a_retry_refused_after_the_sentence_was_dismissed_brings_the_sentence_back`. The decision is **D60**.
