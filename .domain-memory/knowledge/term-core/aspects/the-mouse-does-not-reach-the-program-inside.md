---
id: asp_the-mouse-does-not-reach-the-program-inside_x550d9
slug: the-mouse-does-not-reach-the-program-inside
name: >-
  The mouse DOES reach the program inside as of F2.6: the guard that used to
  protect against nothing now does have a scenario
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:06:38.805Z'
updated_at: '2026-08-25T09:22:30.769Z'
last_verified: '2026-08-25T09:22:30.769Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/mouse.rs
  - crates/term-core/src/screen.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:6ffd89344e0593f6d7362dccb0c76cc2ac211086a11fe9f50adb258313184450'
  crates/app/src/mouse.rs: 'sha256:94147ca1504fb20af9e9cc49d03f7f0c7169613a9eccf1febb9ce98ec557c0a4'
  crates/term-core/src/screen.rs: 'sha256:6af1919efa54066bdf18665edd3cf6316c9ff4c01f3cfbd0b9a94a029dd24ef9'
tags:
  - mouse
  - f2.6
  - d23
  - tests
feature_id: feat_term-core_nx0g1r
---

## What it does

> **Status (25 August 2026): the fact of this record CHANGED.** F2.6 implemented the forwarding, and
> this entry is kept because its closing warning came true to the letter: it said «when F2.6
> implements the mouse forwarding, this path has to be looked at again: then the guard will have a
> scenario». It has one.

**What was true up to F2.6, and why it was written down.** The keyboard chords are filtered on
purpose so that they do not reach the pseudoterminal —**D13**: «none of those chords reaches the
program inside»— and out of that came an intuition that looked obvious and was **false**: that a
mouse gesture needed the same filter. It did not need it, because **the mouse did not reach it**: in
the `app` crate there was no path at all that sent mouse events to the pseudoterminal.

> **The consequence was the one that bit: an acceptance criterion of the kind «the program inside
> does not receive the gesture» was met BY CONSTRUCTION, and its test passed without pinning
> anything.** The project's third disguise on file, with the aggravating detail that there was not
> even a guard to test.

**What is true as of F2.6 (D23).** A console whose program **has asked for** the mouse receives its
gestures as bytes through the pseudoterminal. And what used to be a mechanism with no scenario now
has **several**, all of them with their real defect found in review:

- **`Shift` gives the gesture back to the selection** even if the program claims it — without that
  way out there would be no way of copying inside `vim` or `htop`.
- **The chord that moves the pane always wins**: if a program could keep it, opening an editor would
  leave that console nailed where it is.
- **A press over the panel's columns is not the program's**, and cancelling only that frame is not
  enough — it is the mouse version of the defect the selection already paid for.

**The rule that is left, corrected:** before filtering an input «so that it does not slip through to
the program», check that that class of input reaches it today. **Keyboard yes, mouse too as of
F2.6.** And the other way round: a test asserting «the program did NOT receive this» no longer
passes by construction, so now it is worth something — but only if the test's console has a program
that **has asked for** the mouse; if it did not ask for it, it goes back to being met by
construction.

## Where it lives

The fan-out of who keeps each gesture is in `crates/app/src/lib.rs`; the encoding of the report,
in `crates/app/src/mouse.rs`; what the program has asked for, in `crates/term-core/src/screen.rs`.
The decision with the order, the table and what is left out is **D23** in `docs/02-decisions.md`.
