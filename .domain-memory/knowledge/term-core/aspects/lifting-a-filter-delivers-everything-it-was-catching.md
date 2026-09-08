---
id: asp_lifting-a-filter-delivers-everything-it-was-catching_xezp9n
slug: lifting-a-filter-delivers-everything-it-was-catching
name: >-
  Lifting a filter upstream delivers everything that filter was catching, and
  the second thing is the one nobody looks for
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-08T09:13:27.818Z'
updated_at: '2026-09-08T09:13:27.818Z'
last_verified: '2026-09-08T09:13:27.818Z'
file_paths:
  - crates/app/src/keyboard.rs
  - vendor/egui-winit/src/lib.rs
symbols:
  - bytes
  - key_bytes
  - is_paste_command
  - 'Chord::from_written'
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:345522591ba5b695d69c974a854c72542d5148a73ae1aa0cc9f95cda935ce0c8'
tags:
  - design
  - keyboard
  - review
  - D11
  - D13
  - D33
feature_id: feat_term-core_nx0g1r
---

## What it does

**Removing a swallow in a dependency does not deliver one thing — it delivers everything that swallow was catching.** Found in review of #203, not in use.

The patch stops `egui-winit` discarding the paste chord when the clipboard holds no text. But its predicate is `modifiers.command && keycode == V` and **does not look at Shift**, so that one branch was swallowing **two** chords: `Ctrl+V`, which this terminal wants delivered, and `Ctrl+Shift+V`, **which is the terminal's own paste shortcut**.

So with a screenshot on the clipboard, pressing *paste* sent `\x16` into the hosted program: the keymap has no `V` binding, control falls through to `key_bytes`, and `key_bytes` lets Shift through **on purpose** — Shift does not change a control code, which is what `xterm` does. Press the paste shortcut expecting a paste and `vim` opens visual block. Before the patch that chord did nothing at all. **A chord of the application reaching the program inside is the one thing D11 and D13 both refuse.**

**The fix is one arm in `keyboard::bytes`**: silence for `Ctrl+Shift+V`, and deliberately **not** for `Ctrl+V`, whose byte is the entire point of the delivery. That asymmetry *is* testable in the ordinary way — it is our own translation, not the delivery — and both halves belong in one test, because asserting only the silence stays green if the byte stops coming out for the bare chord too.

> **The criterion: when a filter upstream is lifted, enumerate what that filter was catching, not what you wanted from it.** The predicate that decides the swallow is the place to read. Here, that it ignores Shift **was already written down** — in [[the-two-paste-shortcuts-are-the-same-event]] and in D11 — and it still took a reviewer to connect «the two chords are indistinguishable there» with «so both of them fall through now». The store had the answer and the connection was not made; see [[the-store-had-the-answer-and-the-search-returned-it]].

**A second consequence of the same shape, and it changed a written justification rather than any code.** `Chord::from_written` refuses to bind `ctrl+v` because «it never arrives as a key». It now sometimes does. **The refusal is still right, for the opposite reason**: the binding would fire **only when the clipboard happened to hold no text**, and a shortcut that works depending on what somebody copied last is worse than one that never works — and cannot be explained in a settings file. A guard whose behaviour survives while its stated reason dies is worth hunting for after any change like this: the code review found two such comments in this delivery, both at the exact doorway the patch changes, and **neither was in the diff**.

## Where it lives

The arm is in `keyboard::bytes` (`crates/app/src/keyboard.rs`), next to the `Event::Paste` one; the predicate it compensates for is `is_paste_command` in `vendor/egui-winit/src/lib.rs`. D33 records it as the fix's own side effect.
