---
id: asp_macos-titlebar-flags-and-window-buttons_4ynevx
slug: macos-titlebar-flags-and-window-buttons
name: >-
  On macOS a transparent titlebar still draws the title, and nothing in winit
  0.30 or eframe 0.36 moves the window buttons
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-10T12:13:41.908Z'
updated_at: '2026-11-10T12:13:41.908Z'
last_verified: '2026-11-10T12:13:41.908Z'
file_paths:
  - crates/app/src/opening.rs
  - crates/app/src/chrome/traffic_lights.rs
  - crates/app/src/chrome/connection.rs
  - vendor/egui-winit/src/lib.rs
symbols:
  - chrome_viewport
  - with_title_shown
  - with_titlebar_shown
  - 'traffic_lights::center'
content_hashes:
  crates/app/src/opening.rs: 'sha256:c2dedd52dfd5d454471308f2a59e7629057ddba772f99036b8f657316a48a2fa'
  crates/app/src/chrome/connection.rs: 'sha256:6c33f262b5eb32a23a1d9dd090ff9c8eebad43c8982f9e4a41b06e5d586be3be'
  vendor/egui-winit/src/lib.rs: 'sha256:ad3e91014f9a6f9bbb679bc7eabacbb79ac696a2bfa4c874cc01e240f3c5d1e7'
tags:
  - macos
  - appkit
  - window-chrome
  - egui
  - winit
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two egui `ViewportBuilder` flags that read like one.** `with_titlebar_shown(false)` maps (vendored egui-winit `lib.rs:2203-2206` → winit `window_delegate.rs:597-602`) to `setTitlebarAppearsTransparent` only: the titlebar is see-through but **AppKit still draws the window title** over whatever the app paints there. Hiding the text is a separate flag, `with_title_shown(false)` (`titleVisibility = Hidden`). The first sight of the integrated header on a real Mac (#34, November 2026) showed two «Ivy» on top of each other for exactly this reason.

**The close / minimize / zoom buttons cannot be moved through winit 0.30.13 or eframe 0.36.1.** winit's macOS window setup (`window_delegate.rs:535-618`) never positions them; eframe only *measures* them (`eframe::WindowChromeMetrics`, `native/macos.rs:39-58`). AppKit centres them in its own ~28 pt titlebar, so a taller custom header leaves them ~10 pt above its icons. What works is the Electron `trafficLightPosition` / Zed approach: grow `closeButton.superview.superview` (a private container) and `setFrameOrigin` each button — reached from `eframe::Frame`'s `HasWindowHandle` (AppKit `ns_view` → `window()`), with the objc2 0.6 / objc2-app-kit 0.3 versions eframe already pulls in. AppKit re-lays that container out on its own, so it has to be compared and re-applied, and full screen left alone.

**Discarded, with its reason:** a unified `NSToolbar` gives a ~52 pt titlebar with the buttons centred for free, but its view hit-tests above the content and takes the clicks meant for controls painted underneath (and macOS 26 adds a glass container that blocks them too).

**Still unverified at the time of writing**: whether the grown container passes clicks through to the content in its 28–49 pt band, whether a double-click there zooms twice (AppKit's titlebar plus the header's own Maximize), and whether the buttons' hover glyphs follow them — see D66's amendment.

## Where it lives

`crates/app/src/opening.rs` `chrome_viewport_on` (title flag); `crates/app/src/chrome/traffic_lights.rs` (`center`, `container`, `button_y`); called from `chrome::connection::begin_pass` with the eframe `Frame`; decision in docs/02-decisions.md D66 (10 November 2026 amendment).
