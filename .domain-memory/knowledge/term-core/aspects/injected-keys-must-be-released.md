---
id: asp_injected-keys-must-be-released_zx0ee2
slug: injected-keys-must-be-released
name: >-
  egui decides `repeat` itself from the keys it thinks are held: an injected key
  never released turns every later press of it into a repeat
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-10-10T22:19:34.331Z'
updated_at: '2026-10-10T22:19:34.331Z'
last_verified: '2026-10-10T22:19:34.331Z'
file_paths:
  - crates/app/src/injected/search.rs
  - crates/app/src/injected.rs
  - crates/app/src/lib.rs
symbols:
  - InjectedDrags
  - begin_pass
  - search_the_focused
content_hashes:
  crates/app/src/injected.rs: 'sha256:14bdd1471a5eca463964760f8b759ce226a20d5456bf33514e4b68558809def9'
  crates/app/src/lib.rs: 'sha256:84bd2404e27115cb897f05b1804e7d589734c17791469fb29de2598a1e4748ac'
tags:
  - egui
  - measurement-bench
  - test-disguise
  - f2.13
feature_id: feat_term-core_nx0g1r
---

## What it does

egui 0.36.1's `InputState::begin_pass` keeps `keys_down` and rewrites `Event::Key { repeat }` for every pressed key: `*repeat = !keys_down.insert(key)` (egui-0.36.1/src/input_state/mod.rs:407-412), whatever the event carried. A bench gesture that injects `pressed: true` and never the matching `pressed: false` leaves the key in `keys_down` for good, so every later press of it arrives as a repeat — and every application chord guarded by `!repeat` (new tab, split, close, the search) stops firing after the first gesture.

Measured in #177: batch E's first run opened the search once and then sent its query and Enter presses to the console's program (1 292 keystrokes); only the batch's own «no keystroke reached a program» guard caught it.

**Why the unit tests could not see it:** tests that `enqueue` events with `ctx.input_mut` bypass `begin_pass`, so the repeat rewrite never runs. The test that binds it pushes the gesture through `one_frame_with` (real passes): `the_bench_s_search_gesture_never_types_into_the_console`, red when the release is dropped. Fix: inject press and release in the same batch. See [[egui-hands-a-frame-s-events-to-its-first-pass-only]] for the other half of what `begin_pass` does to injected input.

## Where it lives

crates/app/src/injected/search.rs (`key()` returns press and release); test in crates/app/src/lib.rs.
