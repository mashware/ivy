---
id: asp_ctrl-v-is-discarded-when-the-clipboard-holds-no-text_xeakbt
slug: ctrl-v-is-discarded-when-the-clipboard-holds-no-text
name: >-
  The paste chord is discarded whole when the clipboard holds no text — the
  press, not the release — and four ways out that do not work
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-08T09:12:21.692Z'
updated_at: '2026-09-08T09:12:21.692Z'
last_verified: '2026-09-08T09:12:21.692Z'
file_paths:
  - vendor/egui-winit/src/lib.rs
  - crates/app/src/keyboard.rs
  - Cargo.toml
symbols:
  - is_paste_command
  - raw_input_hook
  - key_bytes
  - 'Event::Key'
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:345522591ba5b695d69c974a854c72542d5148a73ae1aa0cc9f95cda935ce0c8'
  Cargo.toml: 'sha256:57469a656b43c773e275b1aa6e802bd584d0178015b7cf9760d2090cfef4649b'
tags:
  - egui
  - eframe
  - winit
  - clipboard
  - keyboard
  - D33
feature_id: feat_term-core_nx0g1r
---

## What it does

**`egui-winit` recognises the paste chord, reads the clipboard AS TEXT, and if there is none pushes nothing and does an unconditional `return`** (`egui-winit-0.36.1/src/lib.rs:1027-1035`) — so the `Event::Key` for the V is dropped along with the paste that was never built. `is_paste_command` (`:1430-1434`) is `command && key == V` and ignores Shift, so both paste chords go through that door.

**The information is not merely late — it is never written, so nothing downstream can recover it.** This is the part that decides the whole solution space:

- `eframe::App::raw_input_hook` (`eframe-0.36.1/src/epi.rs:279`, called from `native/epi_integration.rs:362`) runs **after** the input is assembled, and the assembled input carries **no trace**: `modifiers` records only that Ctrl was held, never which key, and no `Event::Text` is produced because the text branch (`egui-winit lib.rs:1047-1058`) sits **below** the `return`.
- `EpiIntegration::on_window_event` (`native/epi_integration.rs:243-263`) only forwards.
- `NativeOptions::event_loop_builder` (`epi.rs:351`, consumed at `native/run.rs:44`) hands over the builder **before the loop exists** — construction, not per-event access.
- **Turning off egui-winit's `clipboard` feature makes it worse.** That feature gates only which backend compiles — grepping its `src/` for `cfg(feature = "clipboard")` gives **zero** matches — and `Clipboard::new` is called unconditionally in `State::new` (`:166-168`). With no backend the read returns nothing **always**, so **every** paste chord would be swallowed instead of only some.

**Driving `egui_winit::State` against a hand-rolled winit loop is the only structurally clean interception**, and it means dropping `eframe::run_native` and reimplementing viewport management, surface creation, resizing and redraw scheduling — a rewrite of the application's shell for one key.

## Observed, and the observation contradicted the reading in one word

Probed with a throwaway `eframe::App` printing every event it was handed (Linux/Wayland, screenshot on the clipboard):

```
Key { key: ControlLeft, pressed: true,  modifiers: NONE }
ModifiersChanged(Modifiers { ctrl: true, command: true })
Key { key: ControlLeft, pressed: false, modifiers: { ctrl: true, command: true } }
ModifiersChanged(Modifiers::NONE)
Key { key: V, physical_key: Some(V), pressed: false, repeat: false, modifiers: NONE }
```

No `Paste` and no **pressed** V — but the **release arrives**. `is_paste_command` is tested inside the pressed branch, so the key-up falls through in both clipboard states. The source reading predicted «the key is dropped»; the observation showed «the **press** is dropped». **One word, and it is the difference between a route existing and not existing** — the complement of [[someone-elses-api-is-read-not-assumed]]: reading is necessary and it is still not observation.

**Reconstructing the chord from that release is available and is a trap**, rejected with four reasons: it fires after a *successful* paste too, so the byte would double on every ordinary paste; the release carries `modifiers: NONE` whenever the user lets go of Ctrl first, which both traces did, so the chord has to be remembered after it ended; a terminal must deliver its byte on key **down**, and `repeat` never appears on releases; and the exact shape of a release event is precisely what differs per platform.

## Two things worth knowing before touching any of this

**Probe the reader, not a neighbouring tool.** The clipboard's contents were first checked with `xclip -selection clipboard -t TARGETS -o`, which reads through XWayland, while the application reads through `smithay-clipboard`. That leaves an alternative explanation standing — an ownership or backend mismatch would give an identical silent symptom. What ruled it out was pressing the chord with **text** on the clipboard in the same session and seeing `Paste("…")` arrive.

**Nothing on the application's side was at fault.** With the key delivered, `\x16` comes out unchanged: `key_bytes` (`crates/app/src/keyboard.rs`) maps `Key::V => 22` from the event's **own** embedded modifiers, `Chord::from_written` structurally refuses to bind `ctrl+v`, and the paste door matches only `Event::Paste`.

**Upstream is not a route** (checked live, September 2026): `egui`/`eframe`/`egui-winit` top out at **0.36.1 (7 Aug 2026)**; [emilk/egui#4065](https://github.com/emilk/egui/issues/4065) — this exact bug — has been **open since 2024**; #5615, which would close it, is open and unapproved; #5618 closed without landing. The published `EventFilter` carries only tab/arrows/escape, so **no runtime opt-out exists**. `master` adds `Event::PasteImage` via `clipboard.get_image()` but is unreleased **and still returns unconditionally**.

## Where it lives

The patched copy is `vendor/egui-winit/src/lib.rs`, wired in by `[patch.crates-io]` in the workspace `Cargo.toml` and guarded by `crates/app/tests/vendor_patch.rs`. The published shape to compare against is in cargo's registry at `egui-winit-0.36.1/`. **D33** carries the decision and the seven discarded alternatives.
