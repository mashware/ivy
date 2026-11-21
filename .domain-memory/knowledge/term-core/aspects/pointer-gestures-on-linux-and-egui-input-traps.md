---
id: asp_pointer-gestures-on-linux-and-egui-input-traps_t9caxh
slug: pointer-gestures-on-linux-and-egui-input-traps
name: >-
  No pinch on Linux, GNOME blocks the touchpad while Space is held, and
  layer_id_at deadlocks inside ui.input
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T23:24:45.695Z'
updated_at: '2026-11-20T23:24:45.695Z'
last_verified: '2026-11-20T23:24:45.695Z'
file_paths:
  - crates/app/src/viewer/image.rs
symbols:
  - layer_id_at
  - zoom_delta
content_hashes: {}
tags:
  - egui
  - winit
  - linux
  - trap
feature_id: feat_term-core_nx0g1r
---

## What it does

Met making the drawer's image zoom and pan (#178):

- winit 0.30.13 emits `PinchGesture` only on macOS and iOS (grep of its src: only macos/view.rs and
  ios/view.rs construct it), and `PanGesture` only on iOS. On Linux a touchpad pinch never reaches
  egui as `Event::Zoom`; touchpad zoom is Ctrl + two-finger scroll (egui turns Ctrl+wheel into
  `zoom_delta`, input_state/mod.rs:461-469). A touch screen gives `multi_touch` zoom but no wheel,
  so it cannot pan through `smooth_scroll_delta`.
- GNOME's touchpad «disable while typing» (default true) drops touchpad clicks while a non-modifier
  key such as Space is held, so Space + drag only pans with a mouse there. Ctrl and Shift are
  exempt. Verified on the owner's desktop (gsettings disable-while-typing = true).
- `Context::layer_id_at` takes the same context lock as `ui.input`, so calling it inside an input
  closure deadlocks. egui's `PointerEvent` is `pub(crate)`: a press position per button is read
  from `InputState::events`. `press_origin` is cleared by any button release in the frame.
- `contains_pointer()` keeps lower layers within `interact_radius` of a popup's border; a gesture
  owned by a layer needs `layer_id_at(pos) == ui.layer_id()` plus the current-frame rect.
- A 2400x1600 PNG takes 1.5-4 s to decode in a debug build under the full suite's load: a 5 s test
  wait failed 19 tests and looked like a regression; such waits are hang guards (30 s), not speed
  claims.

## Where it lives

crates/app/src/viewer/image.rs; vendor/egui-winit; winit 0.30.13
