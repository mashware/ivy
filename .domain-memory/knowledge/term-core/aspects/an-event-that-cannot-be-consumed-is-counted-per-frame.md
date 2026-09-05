---
id: asp_an-event-that-cannot-be-consumed-is-counted-per-frame_e1crwv
slug: an-event-that-cannot-be-consumed-is-counted-per-frame
name: >-
  An egui event that is ACTED upon but cannot be consumed demands keeping the
  frame count: without it, the repeated pass sends the click twice
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T09:23:01.083Z'
updated_at: '2026-08-25T09:23:01.083Z'
last_verified: '2026-08-25T09:23:01.083Z'
file_paths:
  - crates/app/src/lib.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:6ffd89344e0593f6d7362dccb0c76cc2ac211086a11fe9f50adb258313184450'
tags:
  - egui
  - mouse
  - f2.6
  - events
  - repeated-pass
feature_id: feat_term-core_nx0g1r
---

## What it does

**The rule, and it comes out of putting together two things this project already had on file
separately.** egui can run the interface pass **twice over the same input**, and that is why the
keyboard and the wheel **consume** their events with `retain`: reading them and leaving them there would
write the same key twice to the pseudoterminal.

**But there are events that cannot be consumed.** The pointer button ones are **what egui needs** in
order to decide who is being dragged: whoever takes them away leaves the gesture never starting. The
project already knew this for **looking** —the chord of the press is read without consuming— and that
note said «here we only look».

> **The moment somebody ACTS on one of those events, the note stops being enough.** F2.6 hands out the
> presses and releases to send them to the program inside, and in a frame with the repeated pass the
> program was receiving **two presses where the user gave one**: a double click nobody made, which
> inside an editor selects a word or opens whatever is under the pointer.

**The way out is the one the batches' gesture injector already uses: keep the frame count.** And the
right number matters —`cumulative_frame_nr` goes up **once per frame**; `cumulative_pass_nr` goes up
**per pass** and would filter nothing, which is the mistake this project already made once in the
injector and documented in the code itself.

**The summary as a criterion, so as not to reason it out again:**

| About the event | What is needed |
|---|---|
| Only looking at it | Nothing: reading it twice comes to the same |
| Acting and it **can** be consumed | Consume it (`retain`), like the keyboard and the wheel |
| Acting and it **cannot** be consumed | Keep the **frame** count, not the pass count |

**And there is a side effect worth knowing about**: a guard of the «do not repeat what has already been
counted» kind —like the one that avoids reporting the same cell twice while dragging— **covers this
problem up on its way past without meaning to**, because on the repeated pass the datum is the same.
That is what makes the defect show up on one path and not on the one next to it, and therefore what
makes it get looked for in the wrong place.

## Where it lives

The handing out of the buttons and their frame count are in `crates/app/src/lib.rs`, in the mouse
handling. The two paths that do consume —keyboard and wheel— are in the same file. The precedent of the
wrong number (`pass` instead of `frame`) is commented in the gesture injector of the measurement
batches.
