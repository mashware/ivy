---
id: asp_an-event-that-cannot-be-consumed-is-counted-per-frame_e1crwv
slug: an-event-that-cannot-be-consumed-is-counted-per-frame
name: >-
  An egui event that is ACTED upon but cannot be consumed demands keeping the
  frame count: without it, the repeated pass sends the click twice — and, worse,
  can act twice
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T09:23:01.083Z'
updated_at: '2026-09-18T18:05:38.857Z'
last_verified: '2026-09-18T18:05:38.857Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pointer.rs
symbols:
  - whose_button_was_pressed
  - route_the_buttons
  - last_frame_with_buttons
content_hashes:
  crates/app/src/lib.rs: 'sha256:50752558d0523d169433c027e12e14d723fd9c05be34294033087ed320bff01d'
  crates/app/src/pointer.rs: 'sha256:78bdcf4b9e74451cde588a56ecbd693850d038e1adb4420f122caf64b7e3e570'
tags:
  - egui
  - mouse
  - f2.6
  - events
  - repeated-pass
  - f2.11
feature_id: feat_term-core_nx0g1r
---

## What it does

**The rule, and it comes out of putting together two things this project already had on file separately.** egui can run the interface pass **twice over the same input**, and that is why the keyboard and the wheel **consume** their events with `retain`: reading them and leaving them there would write the same key twice to the pseudoterminal.

**But there are events that cannot be consumed.** The pointer button ones are what egui needs in order to decide who is being dragged: whoever takes them away leaves the gesture never starting. The project already knew this for LOOKING — the chord of the press is read without consuming — and that note said «here we only look».

> **The moment somebody ACTS on one of those events, the note stops being enough.** F2.6 hands out the presses and releases to send them to the program inside, and in a frame with the repeated pass the program was receiving two presses where the user gave one: a double click nobody made.

**NEW ANGLE (F2.11, September 2026): the worst consequence is not a doubled keystroke, it is a DESTRUCTIVE action done twice.** The close button read the frame's press queue OUTSIDE `route_the_buttons`'s `last_frame_with_buttons != Some(cumulative_frame_nr)` gate, and a repeated pass closed a SECOND console with the same click — two programs killed, no confirmation, no way back. Two panes hide it entirely: the survivor of the first close is alone in its tab and carries no button, so every test written with two panes is green. THREE in a row is what shows it, because the siblings refill the area and the new rightmost strip puts its button on the SAME abscissa the press is still sitting at. Pinned by `a_repeated_pass_does_not_close_a_second_console_with_the_same_click`, which closes two consoles with the guard removed and one with it. Aggravating detail: this delivery's own knowledge search RETURNED this very card at its start and the delivery missed it anyway — see [[the-store-had-the-answer-and-the-search-returned-it]].

**The transferable rule, widened by this second case: anything that ACTS on a pointer event — not only what writes to the program — goes inside the frame gate, and the test for it needs THREE panes, not two**, because two panes hides both «focus stolen from a third console» and «a repeated pass closes a second one» behind «the survivor was the only one left anyway».

**The way out is the one the batches' gesture injector already uses: keep the frame count.** The right number matters — `cumulative_frame_nr` goes up once per frame; `cumulative_pass_nr` goes up per pass and would filter nothing, the mistake this project already made once in the injector.

**The summary as a criterion, so as not to reason it out again:**

| About the event | What is needed |
|---|---|
| Only looking at it | Nothing: reading it twice comes to the same |
| Acting and it can be consumed | Consume it (`retain`), like the keyboard and the wheel |
| Acting and it cannot be consumed | Keep the frame count, not the pass count |

**And there is a side effect worth knowing about**: a guard of the «do not repeat what has already been counted» kind — like the one that avoids reporting the same cell twice while dragging — covers this problem up on its way past without meaning to, because on the repeated pass the datum is the same. That is what makes the defect show up on one path and not on the one next to it, and therefore what makes it get looked for in the wrong place.

## Where it lives

The handing out of the buttons and their frame count are in `crates/app/src/lib.rs`, in the mouse handling. The two paths that do consume — keyboard and wheel — are in the same file. The precedent of the wrong number (`pass` instead of `frame`) is commented in the gesture injector of the measurement batches. F2.11's close-button case: `crates/app/src/pointer.rs` (`whose_button_was_pressed`, `route_the_buttons`, `last_frame_with_buttons`).
