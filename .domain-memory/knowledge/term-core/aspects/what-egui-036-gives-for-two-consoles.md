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
updated_at: '2026-08-11T19:51:22.263Z'
last_verified: '2026-08-11T19:51:22.263Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/painter.rs
symbols:
  - 'Window::ui'
  - 'Window::handle_the_mouse'
  - 'Window::drag_the_divider'
  - place
  - 'painter::paint'
content_hashes:
  crates/app/src/lib.rs: 'sha256:c7f2d67be30a71cbe392f9c446013656f0c3443fb6f7d7035e8575837d3f7a1c'
  crates/app/src/painter.rs: 'sha256:531c8fb585188fd0f14e2bfb6fe0d70a2a2be51c64f5cc5d8848f293ccb18ef8'
tags:
  - egui
  - eframe
  - f1.4
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
- **`Response::interact_pointer_pos()` gives the CURRENT position of the pointer, not that of the press.** Verified along the chain: `context.rs:1534-1543` fills it in from `input.pointer.interact_pos()`, and `input_state/mod.rs:1119-1120` reassigns that field on **every** `PointerMoved`. The position of the press exists but is **another** field, `press_origin` (`input_state/mod.rs:1031`). It is what lets the divider chase the mouse.
- **The drag is tracked by identifier, not by rectangle**, and that is why a divider that moves underneath the pointer does not lose it: `interaction.rs:134` starts from `prev_snapshot.dragged` and the search for a new candidate is inside `if dragged.is_none()` (`:191`). egui itself comments that a dragged widget may be «in the air» and nothing happens. Only releasing the button or Escape cut it off.
- **The candidate to be dragged comes out of where the pointer was ON PRESSING** (`interaction.rs:169-171`), and `hits` is computed once per pass. Consequence for the tests: **three passes are needed** — register, press, move— because a press-and-move in the same pass looks for the candidate at the destination, where the divider no longer is.
- **`on_hover_and_drag_cursor` versus `on_hover_cursor`**: during the drag `hovered()` is true **wherever the pointer is** (`interaction.rs:257-261` puts the dragged one in the «over» list without looking at the position), so the reason that looks obvious —«the pointer leaves the line»— is false. What the variant with `drag` does cover is the frame of the **release**, where the drag has already finished.
- **`interact_radius` (5.0, `style.rs:1482`) does NOT widen the grip.** It only populates the list of nearby candidates; direct hits are looked for with radius 0.0, and with the pointer inside a console egui deliberately discards the nearby drag (`hit_test.rs:340-357`).
- **But the border IS shared**: see case 13 of [[someone-elses-api-is-read-not-assumed]]. In the two border columns of the divider, the gesture drags **and** gives the focus.
- **Unverified, and it depends on another crate**: if the pointer leaves the window with the button pressed, the source leaves it open that the divider might freeze until release (`interaction.rs:233-236` clears `potential_drag_id` but not `dragged`). Checked by hand on Linux/winit: **it does not happen**. On another platform it would have to be looked at again.

**And a design consequence that is not egui's but is imposed by egui:** since the pass can run **twice over the same input** (`max_passes` = 2), the divider's fraction **has** to be computed from the absolute position of the pointer and not by accumulating `drag_delta()`. With the delta, the second pass would apply it again and the divider would move twice as far. It is the same trap that forced **consuming** the keyboard events instead of reading them.

## Testing the interface without opening a window — the whole of it, not in pieces

- `Context::run_ui(RawInput, |ui| ...)` (`context.rs:793`) hands over a `&mut Ui` directly, without setting up a panel. **Two passes are needed** for a click, because egui resolves the interaction against what existed in the previous one.
- **And the COMPLETE ui pass can also be run**, which is what was believed impossible: `eframe::Frame::_new_kittest()` (`epi.rs:720`) builds an empty `Frame`, is public and has no `cfg`, and `App::ui` does not touch it. The area comes in through `RawInput::screen_rect`, which is exactly where `ui.max_rect()` comes from in the application (`context.rs:801-807` → `input_state/mod.rs:526`). With that, **the wiring** is tested, not just the pieces: that the pass tiles with the fraction from the state, that it calls the keyboard and the click, and that `logic` applies what the pass measured.
- `run_ui` repeats the pass **only** if someone asks to discard (`context.rs:863-865`), so in a test it runs once — it matters because the keyboard events are consumed and a second round would not see the key.
- The fonts exist from the first pass (`context.rs:1120`), so `CellMetrics::measure` can be called **from outside** the pass to compute what is expected, without running an extra pass that would dirty the interaction state of a half-finished drag.
- The underscore of `_new_kittest` is a warning: it is a name eframe reserves for itself and it may disappear in a minor version. It is accepted because the price of losing it is a compilation error **in the tests**, never in what is delivered.

**When it would stop holding**: on moving up an egui version. All of this is from 0.36.1 and this library renames and merges types between versions — `SidePanel` is the proof, and `Context::run` → `run_ui` the second one.

## Where it lives

The by-hand tiling is the `tile` function in `crates/app/src/lib.rs`; the per-console clipping is in `Window::paint_pane`, which calls `painter_at`. The click lives in `Window::handle_the_mouse` —which since F2.3 also handles the dragging of one console over another; before that it was called `atender_clics`— and the dragging of the divider in `Window::drag_the_divider`, both kept apart from the painting pass precisely so they can be tested with `run_ui`. The tests that run the **whole** pass use the `ui_pass` helper of the test module of that same file. The precondition that the painter does not clip glyphs is documented in `crates/app/src/painter.rs`.
