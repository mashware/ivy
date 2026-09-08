---
id: asp_the-two-paste-shortcuts-are-the-same-event_861653
slug: the-two-paste-shortcuts-are-the-same-event
name: >-
  `Ctrl+V` and `Ctrl+Shift+V` are the same event to the framework, and the edge
  this card once called unfixable was neither rare nor unfixable
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T16:42:11.261Z'
updated_at: '2026-09-08T09:11:51.304Z'
last_verified: '2026-09-08T09:11:51.304Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/keyboard.rs
  - vendor/egui-winit/src/lib.rs
symbols:
  - 'Pane::send_keys'
  - 'Window::modifiers'
  - is_paste_command
content_hashes:
  crates/app/src/lib.rs: 'sha256:60da7c8ba51c1fc4bcb6f7b7c8ae54bccc71aa8d350d4bd1e40d40ac53c693ab'
  crates/app/src/keyboard.rs: 'sha256:345522591ba5b695d69c974a854c72542d5148a73ae1aa0cc9f95cda935ce0c8'
tags:
  - egui
  - keyboard
  - clipboard
  - D11
  - D33
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

## CORRECTED, 8 September 2026 (issue #203, D33)

This card used to close with: *«An edge that cannot be fixed from here: with an empty clipboard the desktop recognizes the shortcut, finds nothing and does `return` without emitting any event.»* **Both halves of that sentence were wrong, and they kept a real defect filed as a limitation for 26 days.**

- **It is not about an EMPTY clipboard, and that is why it was dismissed.** The condition is «holds no **text**», and a clipboard holding **an image** hits the identical path. In a terminal built to host agents, handing an agent a screenshot is an ordinary gesture, not an edge — the frequency argument that made the limit acceptable was judged against the wrong population. See [[a-limit-accepted-because-it-is-rare]] if that card exists; otherwise D33 carries it.
- **It was fixable, and «cannot be fixed from here» was reasoned rather than tested.** Four routes were read in the dependency's source before the working one was found, and none of that reading had been done when this sentence was written.

**What is true now:** `egui-winit` is served from a copy in the tree (`vendor/egui-winit`) whose only change moves that `return` next to the push it belongs to, so a chord that produced no paste falls through and the key is emitted like any other. The mechanism, the four discarded routes and the upstream situation are in [[ctrl-v-is-discarded-when-the-clipboard-holds-no-text]].

**And the fall-through has a consequence this card's own first paragraph predicts:** because `is_paste_command` ignores Shift, lifting the swallow delivers **both** chords. `Ctrl+Shift+V` — the terminal's own paste shortcut — started reaching the program as `\x16`. `keyboard::bytes` now answers silence for it and deliberately not for `Ctrl+V`. See [[lifting-a-filter-delivers-everything-it-was-catching]].

## Where it lives

`Pane::send_keys` in `crates/app/src/lib.rs` rebuilds the modifiers; the seed lives in `Window` because they belong to the keyboard, not to a console. The swallow itself is at `vendor/egui-winit/src/lib.rs:1027` (patched) — read the published shape in `egui-winit-0.36.1/src/lib.rs:1027-1035,1430` and `egui-0.36.1/src/input_state/mod.rs:396,433,442`. The arm that keeps the paste shortcut out of the program is in `keyboard::bytes`.
