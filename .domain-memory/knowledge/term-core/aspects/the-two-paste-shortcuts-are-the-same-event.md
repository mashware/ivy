---
id: asp_the-two-paste-shortcuts-are-the-same-event_861653
slug: the-two-paste-shortcuts-are-the-same-event
name: >-
  `Ctrl+V` and `Ctrl+Shift+V` arrive as the same event with no modifiers, and
  the field egui publishes does not tell them apart
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T16:42:11.261Z'
updated_at: '2026-08-13T16:42:11.261Z'
last_verified: '2026-08-13T16:42:11.261Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/keyboard.rs
symbols:
  - 'Pane::send_keys'
  - 'Window::modifiers'
content_hashes:
  crates/app/src/lib.rs: 'sha256:a05a4309b46dd644b5b6a2389c026f7fef94d9e1a7cb1fcde40b75eb39607291'
  crates/app/src/keyboard.rs: 'sha256:fac665b7c9d92b5de4f79218612a98ba1d6c97b26242a606fd4149487b51b79c'
tags:
  - egui
  - keyboard
  - clipboard
  - D11
feature_id: feat_term-core_nx0g1r
---

## What it does

**The desktop resolves these shortcuts before they arrive, and leaves them indistinguishable.** `is_paste_command` is `command && key == V` and **does not look at Shift**; after emitting the paste event it does `return`, so **the key event never arrives**. The two shortcuts produce exactly the same thing.

> **And reading `InputState::modifiers` does NOT tell them apart.** That field is **a single one per pass**: egui walks the batch and keeps the last change notice, so it describes how the keys stood **at the end**, not when the paste arrived. Releasing the chord before the frame is collected would give «none»; pressing Shift after pasting would give «yes». A comment of this project took it for granted for two milestones that looking at it «in the loop» was enough: it got the **where** right and the **what** wrong.

**What does work is rebuilding them event by event**, which is what egui does internally: walking the queue in order carrying an accumulator of one's own, which the change notice updates. Three things that are needed and are not obvious:

1. **Seeding it with the value from the previous pass.** A chord can begin in one frame and be used in the next — egui rebuilds the event list in each one.
2. **Resetting it on losing focus.** It is the safeguard egui has in its loop, with its reason written down: when the window leaves, the notice that the keys were released does not arrive. Without it, going away with a desktop shortcut and releasing outside leaves Shift on forever, and the next `Ctrl+V` dumps the clipboard.
3. **Leaving the change notice in the queue**, because it is not ours.

**Why the trouble is worth it, and why not keep `Ctrl+V` for pasting:** that byte has an owner, and it is not the terminal. It is vim's visual block and the shell's *quoted-insert*. Verified with Tilix in front: there `Ctrl+V` does not paste, but **inside that same window** Claude does paste with `Ctrl+V`, because it receives the byte and decides that for it that is what it means. **Who pastes with `Ctrl+V` is each program's decision.**

**An edge that cannot be fixed from here:** with an **empty clipboard** the desktop recognizes the shortcut, finds nothing and does `return` **without emitting any event**. So `Ctrl+V` sends not even the control byte, and vim's visual block does not open in a session with nothing copied.

## Where it lives

`Pane::send_keys` in `crates/app/src/lib.rs` rebuilds the modifiers; the seed lives in `Window` because they belong to the keyboard, not to a console. Read in `egui-winit-0.36.1/src/lib.rs:1027-1035,1430` and `egui-0.36.1/src/input_state/mod.rs:396,433,442`.
