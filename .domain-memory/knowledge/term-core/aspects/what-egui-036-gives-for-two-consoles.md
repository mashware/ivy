---
id: asp_what-egui-036-gives-for-two-consoles_673gqh
slug: what-egui-036-gives-for-two-consoles
name: >-
  What egui 0.36 gives and does not give for putting two consoles in one window
  with a divider between them
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-11T16:28:57.321Z'
updated_at: '2026-09-26T21:28:48.854Z'
last_verified: '2026-09-26T21:28:48.854Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/painter.rs
  - crates/app/src/tree.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:0955cb278e25d124ab132882b82e670f9f54c4579dcbc3627c5326cc35ffafc1'
  crates/app/src/painter.rs: 'sha256:e750df07817b291b8bfe7f96868f65d62008899db1084da8fbf48750b312d1b4'
  crates/app/src/tree.rs: 'sha256:cf3e53de34e076347665537f1c1f6d014536435b28403e146f40ba67ae9ffb16'
tags:
  - egui
  - eframe
  - panes
  - foreign-api
feature_id: feat_term-core_nx0g1r
---

## What it does

Read in the source of egui/eframe **0.36.1** in cargo's local registry, not assumed, and since F1.4 **verified in use as well**.

**`SidePanel` no longer exists.** It was merged into `Panel::left/right/top/bottom` (`containers/panel.rs:206-258`). Whoever looks for the old name in old documentation will not find it.

**And the panel is no good for fan-out by fraction, which is the trap that matters.** Its size is `default_outer_size: Option<f32>` and `outer_size_range: Rangef`: **points, not fraction**. With `.resizable(true)` it brings the draggable separator ready made, and that is why it looks like the obvious option — but on resizing the window the panel keeps its width in points and **the whole difference falls on the other side**, so the proportion breaks without anyone touching the divider. For a tiling by fraction the rectangles have to be computed by hand, and that is what was done.

**Clipping to a rectangle exists, is cheap and is hybrid.** `Ui::painter_at(rect)` (`ui.rs:619`) is `painter().with_clip_rect(rect)`: a `clone` with an intersection of clips, **zero cost per cell**. Tessellation only does coarse pruning —and for text, **per whole line, not per glyph**—; the exact pixel cut is done by the GPU's *scissor test*. It matters because this project's painter **deliberately does not clip glyphs to their cell**, so a wide character at the edge would invade the neighbouring console if nobody clipped from above. The only thing it adds is a **fixed** cost: four clip groups per frame instead of one.

**egui's «focus» is not geographic — it is the name trap that costs the most.** `Memory::request_focus` is keyboard focus **per widget**, for Tab and the arrows. It is **no** good for deciding which pseudoterminal a key goes to: that state is carried by the application. From that it follows that when detecting the click one has to use **`Sense::CLICK` and not `Sense::click()`**, because the second includes `FOCUSABLE` and the widget *would ask* for that focus, competing with the routing of our own (`sense.rs:58`).

## The drag, already verified in use

- **`Sense::DRAG`, not `Sense::drag()`** — the same trap as the click: `drag()` is `DRAG | FOCUSABLE` (`sense.rs:68`).
- **Not sensing clicks is an advantage, not a shortcoming.** A widget that senses click *and* drag pays a **decision window**: egui does not know whether the pressed button is a click or a drag until the pointer moves (`sense.rs:74-79`, `interaction.rs:196-217`). Being drag only, one enters the `else` branch (`interaction.rs:218-221`) and it is marked as dragged **in the very frame of the press**. For a divider that is exactly what is wanted.
- **`Response::interact_pointer_pos()` gives the CURRENT position of the pointer, not that of the press.** `context.rs:1534-1543` fills it in from `input.pointer.interact_pos()`, and `input_state/mod.rs:1119-1120` reassigns that field on **every** `PointerMoved`. The position of the press is **another** field, `press_origin` (`input_state/mod.rs:1031`). It is what lets the divider chase the mouse.
- **The drag is tracked by identifier, not by rectangle**, and that is why a divider that moves underneath the pointer does not lose it: `interaction.rs:134` starts from `prev_snapshot.dragged` and the search for a new candidate is inside `if dragged.is_none()` (`:191`). Only releasing the button or Escape cut it off.
- **The candidate to be dragged comes out of where the pointer was ON PRESSING** (`interaction.rs:169-171`), and `hits` is computed once per pass. Consequence for the tests: **three passes are needed** — register, press, move.
- **`on_hover_and_drag_cursor` versus `on_hover_cursor`**: during the drag `hovered()` is true **wherever the pointer is** (`interaction.rs:257-261`). What the variant with `drag` does cover is the frame of the **release**.
- **`interact_radius` (5.0, `style.rs:1482`) does NOT widen the grip.** Direct hits are looked for with radius 0.0, and with the pointer inside a console egui deliberately discards the nearby drag (`hit_test.rs:340-357`).
- **But the border IS shared**: see case 13 of [[someone-elses-api-is-read-not-assumed]].
- **Unverified, and it depends on another crate**: if the pointer leaves the window with the button pressed, the divider might freeze until release (`interaction.rs:233-236`). Checked by hand on Linux/winit: **it does not happen**.

**The divider's fraction comes from the pointer's absolute position, not from accumulating `drag_delta()`** — first for precision, and second as a **defence**. **Corrected 26 September 2026 (#290):** this used to say the pass runs «twice over the same input» and the delta would be applied twice. In egui 0.36.1 a repeated pass receives no events, so its drag delta is zero; an egui that replayed a frame's events would double it, and the absolute position is immune either way. See [[egui-hands-a-frame-s-events-to-its-first-pass-only]].

## Testing the interface without opening a window — the whole of it, not in pieces

- `Context::run_ui(RawInput, |ui| ...)` (`context.rs:793`) hands over a `&mut Ui` directly, without setting up a panel. **Two passes are needed** for a click, because egui resolves the interaction against what existed in the previous one.
- **And the COMPLETE ui pass can also be run**: `eframe::Frame::_new_kittest()` (`epi.rs:720`) builds an empty `Frame`, and `App::ui` does not touch it. The area comes in through `RawInput::screen_rect`.
- `run_ui` repeats the pass **only** if someone asks to discard (`context.rs:863-865`), so in a test it runs once. The repeated pass would not see the key anyway — events reach the first pass only.
- The fonts exist from the first pass (`context.rs:1120`), so `CellMetrics::measure` can be called **from outside** the pass.
- The underscore of `_new_kittest` is a warning: eframe may remove it in a minor version; losing it breaks only the tests.

**When it would stop holding**: on moving up an egui version. All of this is from 0.36.1.

## Where it lives

The by-hand tiling in crates/app/src/tree.rs and lib.rs; per-console clipping in the painting pass via `painter_at`; the click and console drag in `Window::handle_the_mouse` (crates/app/src/pointer.rs); the divider drag in crates/app/src/tree.rs; UI-pass tests in the test module of crates/app/src/lib.rs; the painter's no-clip precondition in crates/app/src/painter.rs
