---
id: asp_what-the-window-hands-a-painter-is-tested-through-the-window_t4gf08
slug: what-the-window-hands-a-painter-is-tested-through-the-window
name: >-
  What the window decides per console (where the keys go) is only visible
  through `App::ui`, and a renamed item's broken doc link passes the whole test
  suite
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T14:21:34.766Z'
updated_at: '2026-09-26T14:21:34.766Z'
last_verified: '2026-09-26T14:21:34.766Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/src/lib.rs
  - .cargo/config.toml
symbols:
  - paint_a_console
  - Keys
  - Blink
  - one_frame_of_the_window
  - text_of_the_two_consoles
content_hashes:
  crates/app/src/painter.rs: 'sha256:86310d51e47691a817336e50660b43133ea20d2136f7978825b2ab34f3279d28'
  crates/app/src/lib.rs: 'sha256:def487034867d993c3bc3caa084d5e0a5ef05e2cb9e7f6d55ea2195164e51e51'
  .cargo/config.toml: 'sha256:6b47a7067095b8cbc418a72997ae7d23f64f0c4b7a239178ca654d13f1b840bb'
tags:
  - test-disguise
  - method
  - painter
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two traps met in #211, both invisible to a green `cargo test --workspace`.**

**1. The test-only `painter::paint_a_console` takes an ink, and knows nothing of the window.** It has no desktop focus and no idea which console the window hands its keys to. It maps an outlined ink to «elsewhere» and anything else to «here». So a test that reads a notice through it cannot see anything the window decides per console. For example, a held-paste notice in an alt-tabbed window must say «come back and Ctrl+Enter sends it», which is `painter::Keys::OnceTheWindowIsBack`. Such tests must paint through `eframe::App::ui` with `RawInput.focused` set. The shared helper is `one_frame_of_the_window`, with shapes attributed to consoles by clip. `text_of_the_two_consoles` and the cursor helpers read from it.

Why it mattered: the first fix kept a second reading of the focus (a mark the mouse pass wrote on the paste) beside the cursor's. The regression test could not tell the cursor's value from the bare window focus, because the old mark hid the console half. The honest shape is **one value both marks paint from**, built in the frame loop from the window's focus and the console's name (`Keys::of(window, this_console, with_the_keys)`). The mutation `Keys::of(window, _, _)` is then red on both notice tests.

**2. The intra-doc link guard is not part of the suite.** Renaming `Blink::focused` to `keys` left [`Blink::focused`] in `HowToPaint`'s docblock. Every test, clippy and fmt stayed green. Only `cargo doc-check -p app` fails on it: the alias in `.cargo/config.toml`, with `deny(rustdoc::broken_intra_doc_links)`. Its ~20 warnings about links to private items were there before. **Run it after renaming or removing any documented item.**

## Where it lives

`paint_a_console` and `Keys` in `crates/app/src/painter.rs`. `one_frame_of_the_window` and `text_of_the_two_consoles` are in `crates/app/src/lib.rs` tests. The `doc-check` alias is in `.cargo/config.toml`.
