---
id: asp_a-revocation-test-must-drive-the-door-not-the-setter_3n2k76
slug: a-revocation-test-must-drive-the-door-not-the-setter
name: >-
  A test that calls the pane's setter instead of driving the door leaves
  deleting the door's call green — and the exclusion (a panel button does NOT
  revoke) has to be pinned in the press test
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-16T18:28:04.366Z'
updated_at: '2026-09-16T18:28:04.366Z'
last_verified: '2026-09-16T18:28:04.366Z'
file_paths:
  - crates/app/src/keyboard.rs
  - crates/app/src/mouse.rs
  - crates/app/src/pointer.rs
  - crates/app/src/control.rs
symbols:
  - 'Pane::hand_it_back_to_the_person'
  - 'Pane::send_keys'
  - 'Pane::send_mouse_to_the_program'
  - answer_a_clicked_question
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:1790d82af06d1dcb5fb2e9121e822f9aac1974863c92c390d5ca899a0d7c7a35'
  crates/app/src/mouse.rs: 'sha256:4f60e47df247304019d60071c2b89288cb20db4b51f02216e615f05bda1732af'
  crates/app/src/pointer.rs: 'sha256:20b27a0241ef42b5b4f544d50f1ba46c49792cbf7c1dc7966c3b851823d85c40'
  crates/app/src/control.rs: 'sha256:953bd85dc730a278480bdb6e760773a2191539f835c733250f370faf7eb13542'
tags:
  - tests
  - mutation
  - D30
  - input-doors
  - revocation
feature_id: feat_term-core_nx0g1r
---

## What it does

**When one pane-side flag is set from several write sites, a test that sets it through the pane's own setter covers nothing: deleting the call at every real site leaves the suite green.**

Measured on this delivery (D30's first face, one level up). The flag is `Pane.taken_over`, set by `Pane::hand_it_back_to_the_person()` from three doors the person's own input uses: `keyboard.rs` after the keystrokes/paste write, `mouse.rs` after the wheel write and after a mouse report. The first tests for the criterion called the setter **directly**, so the mutations were: delete `self.hand_it_back_to_the_person()` in `keyboard.rs` (green), delete the two in `mouse.rs` (green), delete all three (green).

**The mutation that does catch it drives the door itself.** For the keyboard: `enqueue(&ctx, vec![egui::Event::Paste("hello".to_owned())])`, then `pane.send_keys(&ctx, Instant::now(), &mut modifiers, 2, &keyboard::Keymap::default(), true)`, and assert both that `pane.keystrokes` is non-empty (the premise — the write must have happened) and that `taken_over` is set. For the mouse: `pane.screen.advance(b"\x1b[?1000h")`, assert `pane.screen.mouse_reporting().wants_mouse()` (the premise), then `pane.send_mouse_to_the_program(MouseReport::Press(PointerButton::Primary), Cell { row, col, half }, Modifiers::NONE)` must return `Some`, and only then the flag.

**And the negative must be pinned too, because the exclusion is the whole design**: the panel button's door (`pointer.rs::answer_a_clicked_question`) writes into the pane's PTY on the person's behalf and must **not** revoke, or pressing a button beside an agent's console would hand it back. That is asserted in the existing press test `a_press_on_a_button_makes_the_program_receive_its_command` (after the command reaches the program, `window.active().panes()[0].taken_over.is_none()`).

**The `Cell` needs its `half`**: `term_core::Cell` is `{ row: u16, col: u16, half: Half }`, and `mouse_reporting()` returns a `MouseReporting` struct whose predicate is `.wants_mouse()`, not a bool.

## Where it lives

Flag and setter: `crates/app/src/control.rs` (`HANDED_BACK`, `impl Pane::hand_it_back_to_the_person`). Doors: `crates/app/src/keyboard.rs` (after the pty write, which keystroke *and* paste share), `crates/app/src/mouse.rs` (the wheel and the mouse report). Excluded door: `crates/app/src/pointer.rs::answer_a_clicked_question`. Tests: `lib.rs::the_persons_own_input_at_the_two_doors_hands_the_console_back`, and the assertion inside `lib.rs::a_press_on_a_button_makes_the_program_receive_its_command`. Related cards: a-test-that-pins-the-helper-does-not-pin-the-path, a-return-value-nobody-asserts-covers-nothing.
