---
id: asp_the-heartbeat-was-armed-a-frame-late_qn6jme
slug: the-heartbeat-was-armed-a-frame-late
name: >-
  The heartbeat was armed with the previous frame's state, and in an idle window
  «a frame late» means «never»
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:22:53.529Z'
updated_at: '2026-08-13T13:22:53.529Z'
last_verified: '2026-08-13T13:22:53.529Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/panel.rs
symbols:
  - set_the_alarm
  - how_long_until_warning
  - until_the_next_step
content_hashes:
  crates/app/src/lib.rs: 'sha256:1e1fccb7584128dc015d57b4a85b7afba61479eddebd8cffdbabeba928cc19be'
  crates/app/src/panel.rs: 'sha256:568e67aba57fd9f60e9ca802161c515776057b5d550f6074bf1af4cf561d98a8'
tags:
  - egui
  - eframe
  - window-loop
  - repaint
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The failure.** In eframe 0.36.1 the `logic` half runs **before** `ui` in the same pass, and the panels are re-read in `ui`. Arming the alarm only in `logic`, it was computed with the **previous** frame's state: on the frame in which a panel is read for the first time there is still no panel, so there is nothing to schedule. Measured with a probe: the published delay was `Duration::MAX` and **the window went to sleep forever**. It happens on the session's first publication and on **every** publication by a producer slower than its own deadline.

> **In an immediate-mode loop split into two halves, anything computed out of state that the other half mutates arrives a frame late. And in a reactive window «a frame late» is «never»: there is no next frame if nobody asks for one.**

**Why both calls are needed, and moving the single one to the end is not enough.** `Context::run_logic` —read in the source— **starts by setting `repaint_delay = Duration::MAX`**, and it is the only half that runs with the window **minimized or occluded**. Without the call in `logic`, every round of a covered window erases the alarm and nobody re-arms it: the notice about a dead producer would not come back **even on un-minimizing**. Without the one in `ui`, the freshly read panel is not seen. There is one test for each, and the second one had to be added during validation: removing the one in `logic` left the 286 green.

**The little brother of the same failure: the frozen number.** «no news for 5 min» stayed at 5 for hours, because once the deadline was met nobody asked for a repaint any more. An old panel has to **keep** asking for an alarm for the moment when its number is going to change — at the next minute, or at the next hour once the notice is already counted in hours.

**Cost bounded by construction:** one wake-up when a panel meets its deadline, and after that one per minute for as long as it stays old. With the window idle and no old panel, the window keeps sleeping.

## Where it lives

`Window::set_the_alarm` in `crates/app/src/lib.rs`, called from **both** halves of the eframe 0.36.1 loop (`logic` and the end of `ui`). The number comes from `PanelState::how_long_until_warning` in `crates/app/src/panel.rs`.
