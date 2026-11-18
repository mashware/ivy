---
id: asp_capturing-the-window-on-gnome-wayland_gej18e
slug: capturing-the-window-on-gnome-wayland
name: >-
  An agent can capture the real window on GNOME Wayland through XWayland and
  GDK, and drive it with the bench gestures
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-18T23:35:35.527Z'
updated_at: '2026-11-18T23:35:35.527Z'
last_verified: '2026-11-18T23:35:35.527Z'
file_paths:
  - crates/app/src/injected.rs
  - crates/app/src/injected/search.rs
  - scripts/measure-frame.sh
symbols:
  - GESTURE_VARIABLE
  - DRAGS_VARIABLE
content_hashes:
  crates/app/src/injected.rs: 'sha256:399be8e44d9d9646a2eec26cf4709531b7ee726db955a6327bf4d1fa6a522d6b'
  crates/app/src/injected/search.rs: 'sha256:62241a19223a0e21368991173f4db328184032ec29fb3b28eba6c1945a51aad8'
  scripts/measure-frame.sh: 'sha256:2183c098370f198ef0f53ce5281528eb4f1447226ea174656877aca15d4daa6f'
tags:
  - validation
  - wayland
  - screenshots
feature_id: feat_term-core_nx0g1r
---

## What it does

On the development machine (GNOME, Wayland, no xdotool, ydotool or grim) an agent can still see the
real app window. GNOME's `org.gnome.Shell.Screenshot` answers AccessDenied, and PIL's ImageGrab of
XWayland's root fails (rootless, BadMatch). What works: launch the app with `WAYLAND_DISPLAY` unset
so it runs on XWayland, find its xid with `xwininfo -display :0 -root -tree`, and capture it with
`Gdk.pixbuf_get_from_window(GdkX11.X11Window.foreign_new_for_display(display, xid), ...)` from
python3-gi; HiDPI comes out at 2x.

Input cannot be injected, but the bench gestures drive the window: `MEASURE_GESTURE=search` with
`MEASURE_DRAGS=100000` (without DRAGS nothing is injected), `MEASURE_CHROME=1` for the integrated
titlebar and `MEASURE_HEADER=1` for console headers, a temporary `XDG_CONFIG_HOME`, and `SHELL`
pointing at a script that prints sample text and then execs bash. Capture several frames and pick
one by colour (the raised surface, the match highlight).

Evidence: #120 validation, evidence `ac1-ac5-box-with-matches.png`.

## Where it lives

crates/app/src/injected.rs (`GESTURE_VARIABLE`, `DRAGS_VARIABLE`), crates/app/src/injected/search.rs
(the search gesture), scripts/measure-frame.sh (the variables a batch exports).
