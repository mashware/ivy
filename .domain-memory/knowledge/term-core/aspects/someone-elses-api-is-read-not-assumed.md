---
id: asp_someone-elses-api-is-read-not-assumed_hnvyp8
slug: someone-elses-api-is-read-not-assumed
name: >-
  Someone else's API is read, not assumed: in this project assuming it has
  already cost twenty times
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-09T20:07:25.783Z'
updated_at: '2026-09-26T21:28:25.346Z'
last_verified: '2026-09-26T21:28:25.346Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/keyboard.rs
  - crates/app/src/measure.rs
  - crates/app/src/painter.rs
  - crates/term-core/src/lib.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:0955cb278e25d124ab132882b82e670f9f54c4579dcbc3627c5326cc35ffafc1'
  crates/app/src/keyboard.rs: 'sha256:8de7e63e699188ed4ad7cddaf074fac66eff1f5376c704896c71e05e26b679d7'
  crates/app/src/measure.rs: 'sha256:66e58d69d879e7333dee6c1ec93cf2dc6213f2c5dc0ad947600a6d8d0855d9e1'
  crates/app/src/painter.rs: 'sha256:e750df07817b291b8bfe7f96868f65d62008899db1084da8fbf48750b312d1b4'
  crates/term-core/src/lib.rs: 'sha256:a9ffaf445eb42e55e3727ab8952a28d8012b37eaedc1a7bc9bc1e21335d03f16'
tags:
  - method
  - verification
  - egui
  - eframe
feature_id: feat_term-core_nx0g1r
---

## What it does

**The pattern, with its count.** **Twenty** times in this project a third-party library has done something other than what its name promised, and **all twenty were settled by reading its source code**, never by reasoning about it. It is not bad luck: it is what happens when a terminal is built on top of a VT engine, a pseudoterminal and an immediate-mode UI framework, three layers whose abstractions were designed for something else.

**The cases, so that the count is not an impression:**

1. `App::update` **does not exist** in eframe 0.36 — the method is `ui`, and there is a `logic` that runs with the window hidden as well. It changed the whole shape of the loop.
2. `Receiver` is `!Sync`: lending it does not cross threads, moving it does. It changed the contract of `Pty::spawn`, which went on to return the channel.
3. `RawInput::time` is documented as «monotonic, **relative to whatever**» and key events carry no stamp of their own. That is why there is no number called «key → pixel».
4. **`Painter::rect_filled` and `Painter::text` do not paint: they stack shapes and return.** Tessellating happens after the pass. The bench published that the frame spent 14 % of its slot; it spent 33 %.
5. `IntegrationInfo::cpu_usage` times **the whole pass**, not just the drawing. The latency breakdown called «tessellate and draw» a residue that included half a dozen other things.
6. `viewport_rect` is **10 000 × 10 000 points** while the window does not report a size yet. With no ceiling, that is hundreds of thousands of cells and an absurd size notice to the program.
7. `max_passes` is **2**: one frame can run the pass twice. **Corrected 26 September 2026 (#290):** what repeats is the closure (pump, paint, frame close), not the input — `RawInput::take` moves the events out before each pass, so the repeated pass sees no key. The keyboard's consumption is still needed, for a different reason: egui's own `end_pass` reads the same queue for its quit and zoom shortcuts. See case 20 and [[egui-hands-a-frame-s-events-to-its-first-pass-only]].
8. `egui-winit` turns Ctrl+C into a «copy» event **before** emitting the key. Without picking it up at that door, **Ctrl+C does not interrupt**, which is the most basic thing a terminal does.
9. `portable-pty` copies the parent's whole environment to the child, and has no parameter to avoid it.
10. Feeding the parser a single repeated character walks its fast path and **gives a number that lies** — the same lesson, twice, in two different instruments.
11. **`Sense::click()` includes `FOCUSABLE`** (`sense.rs:60`), that is, the widget *asks for* egui's keyboard focus. In an application whose keyboard routing is **geographic and its own**, that focus competes with the application's. Caught **before** writing, by reading the source.
12. **`Context::run` does not exist in 0.36.1: it is `run_ui`** (`context.rs:793`), and it hands over a `&mut Ui` directly. Caught by the compiler, which was cheap.
13. **`Rect::distance_sq_to_pos` gives zero for a point *on* the border**, not only inside. So **two rectangles that touch each other both get the direct hit** (`hit_test.rs:205-211`, `hit_test.rs:414-433`): in the two border columns of the draggable divider, one gesture **drags the divider and gives the focus to the console next to it**. A comment claimed otherwise; the review uncovered it by reading the source.
14. **`eframe::Frame` can indeed be built in a test: `Frame::_new_kittest()`** (`epi.rs:720`). What was assumed was that the call did not exist, and five tests went unwritten because of it.
15. **The VT engine's three mouse modes REPLACE one another, they do not stack.** Asking for the drag (`?1002h`) **turns off** the clicks-only one (`?1000h`). Discovered by a test written from intuition, which failed.
16. **`ALTERNATE_SCROLL` comes ON in the engine's default mode**, just as `xterm` does, so what decides whether the wheel becomes arrows is **the alternate screen**.
17. **`portable-pty`'s Windows reader is `filedescriptor::FileDescriptor`**, whose `io::Read` answers `Ok(0)` on a broken pipe — a risk published on the assumption it was Unix-only was refuted by a skeptic reading the source.
18. **On Windows the pipe's write end is owned by the pseudoconsole**, so `drop(pair.slave)` closes nothing there and no end of file arrives while the console lives.
19. **`std::io::ErrorKind` is a coarse bucket**: errno 5, 70 and 75 all come out as an unnameable variant; `raw_os_error()` keeps the number.
20. **egui 0.36.1 hands a frame's events to its first pass only, and forty comments said otherwise** (#200, #290). `RawInput::take` (`data/input/raw_input.rs:137`, called at `context.rs:854`) moves them out; measured `[(1, true), (0, false)]`. Correcting it found two present-day truths the belief had been hiding: keyboard consumption is load-bearing because `Context::end_pass` (`context.rs:2442-2450`) consumes Ctrl+Q and the zoom keys from the same queue, and eframe calls `raw_input_hook` and `logic` on logic-only ticks while the window is hidden (`epi_integration.rs:323-362`) without advancing `cumulative_frame_nr`. Found by three review rounds that each went back to the source.

**Where the alarm goes off.** The expensive case is not the one that does not compile: it is the one where **the label of a number describes what one believes the call does**. Cases 4 and 5 published an optimistic number with every appearance of being the right one. Case 20 is the same shape in prose: a belief repeated so often that nobody thought to read it.

**Patterns:** convenience constructors that package a decision (11, 12); geometry that does not behave as intuition says (13); assuming some flags add up (15, 16); assuming an absence, not a semantics (14); and a belief multiplied by copying (20) — the more places say it, the less anyone checks it.

**What to do, concretely.** The sources are in `~/.cargo/registry/src/index.crates.io-*/<crate>-<version>/`, and reading them costs one search. They are read **before** leaning on the behaviour whenever: a published number depends on when something happens; a contract depends on what a call does; a comment is about to say «this does X» about someone else's code; a convenience constructor is about to be used; or something is about to be declared impossible.

**Who catches it when it slips through:** in **nine of the twenty** cases, a review that read the source instead of opining about the diff.

## Where it lives

crates/app/src/lib.rs (loop, keyboard, focus, divider, UI-pass tests), crates/app/src/keyboard.rs and pointer.rs (case 20), measure.rs, painter.rs, crates/term-core/src/lib.rs and screen.rs (cases 15-16, D23), portable-pty/filedescriptor on Windows (17-19, D38)
