---
id: asp_the-input-path-below-egui-is-out-of-reach-of-any-test_pb3c5f
slug: the-input-path-below-egui-is-out-of-reach-of-any-test
name: >-
  No test in this repository can reach the winit→egui translation, and two walls
  of winit's own make it permanent
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-08T09:12:46.772Z'
updated_at: '2026-09-08T09:12:46.772Z'
last_verified: '2026-09-08T09:12:46.772Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/tests/vendor_patch.rs
symbols:
  - enqueue
  - on_window_event
  - KeyEvent
content_hashes:
  crates/app/src/lib.rs: 'sha256:60da7c8ba51c1fc4bcb6f7b7c8ae54bccc71aa8d350d4bd1e40d40ac53c693ab'
tags:
  - testing
  - winit
  - egui
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

**A whole class of defect — everything between the operating system's event and `egui::Event` — is invisible to this suite, and it cannot be made visible cheaply.** Two independent walls, either of which is enough:

1. **The triggering event cannot be constructed from outside `winit`.** `winit-0.30.13/src/event.rs:655` — `KeyEvent` carries `pub(crate) platform_specific: platform_impl::KeyEventExtra`. Six public fields and one private one, no constructor, no `Default`, no builder. A foreign crate can *pattern-match* a `KeyEvent` (winit's own doc example does, with `..`) but cannot **build** one. **The privacy is `winit`'s, not `egui-winit`'s** — so being inside a vendored copy of `egui-winit` buys nothing at all, which is a mistake this project made in writing before checking it.
2. **The `&Window` it demands cannot exist without a live display.** `on_window_event` takes `&winit::window::Window`, and winit 0.30 creates windows only through `ActiveEventLoop::create_window`, bound to a real compositor. There is no headless window. And `on_keyboard_input`, where the branch lives, is a **private** function reachable only through that method.

`egui_winit::State::new` is **not** a wall — it takes `&dyn HasDisplayHandle` and a dummy raw handle is constructible — **which is exactly the trap**: getting a `State` looks like progress and the two walls stand behind it.

> **The consequence for this project's rules.** Every keyboard test here builds `egui::Event` by hand through `enqueue` (`crates/app/src/lib.rs`) and there is **zero** mention of `egui_winit` anywhere in `crates/`. So the suite starts *below* the translation layer: a test seeding `Event::Paste` proves the translation and never the delivery. It is D30's third disguise — the test whose world cannot produce the scenario — in the variant where the impossibility is imposed by a library, the same shape as [[a-drag-test-needs-two-passes-at-least]].

**And there is no precedent to lean on:** nothing in `crates/app/tests/` or the crate's test module is gated on `DISPLAY`/`WAYLAND_DISPLAY`, and the measurement batches do not run under `cargo test`. This repository has never had a test that needs a display.

## What is possible instead, with what each one is worth

- **A guard over source text.** `crates/app/tests/vendor_patch.rs` is the worked example: it pins the patch pointer, the shape of the patched branch, the two upstream lines that branch hangs off, **and** the copy's file list — a hash of a fixed list is silent about a file that was *added*, which a review found by adding one and watching every check stay green. It proves the patch is still there and **nothing about behaviour**, and its own doc comment has to say so.
- **A test of the trigger only** (`arboard`'s `get_text`/`get_image` against an image-only clipboard) — pins the precondition, leaves the delivery unverified.
- **A heavyweight `xvfb` + `xdotool` + `xclip` integration test** — the only route to a real `KeyEvent`, since winit itself constructs it from real platform events. Nothing like it exists here.
- **Verification by hand on the real application**, which is what D33 shipped with. For a keyboard byte the strong form is `cat -v`: `Ctrl+V` prints `^` and the next press completes it to `^V`, because `0x16` **is** the line discipline's *literal-next*. A terminal echoing a `^V` of its own would print both characters on one press — two presses, one character each, is the kernel's state machine and can only run if the real byte went through.

## Where it lives

`enqueue` and the `run_ui` sites in `crates/app/src/lib.rs`'s test module; the worked guard in `crates/app/tests/vendor_patch.rs`; the walls in `winit-0.30.13/src/event.rs:655` and `egui-winit-0.36.1/src/lib.rs:295`.
