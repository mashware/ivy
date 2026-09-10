---
id: asp_someone-elses-api-is-read-not-assumed_hnvyp8
slug: someone-elses-api-is-read-not-assumed
name: >-
  Someone else's API is read, not assumed: in this project assuming it has
  already cost nineteen times
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-09T20:07:25.783Z'
updated_at: '2026-08-25T11:40:00.000Z'
last_verified: '2026-08-11T19:50:20.710Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
  - crates/app/src/painter.rs
  - crates/term-core/src/lib.rs
symbols:
  - 'Painter::text'
  - 'IntegrationInfo::cpu_usage'
  - 'RawInput::time'
  - 'Pty::spawn'
  - 'Sense::click'
  - 'Context::run_ui'
  - 'Rect::distance_sq_to_pos'
  - 'Frame::_new_kittest'
content_hashes:
  crates/app/src/lib.rs: 'sha256:c7f2d67be30a71cbe392f9c446013656f0c3443fb6f7d7035e8575837d3f7a1c'
  crates/app/src/measure.rs: 'sha256:b55a4357bfb9ee09793b07a352ac7f74bdcfbbd12c25527fe829182eaadcec3c'
  crates/app/src/painter.rs: 'sha256:531c8fb585188fd0f14e2bfb6fe0d70a2a2be51c64f5cc5d8848f293ccb18ef8'
  crates/term-core/src/lib.rs: 'sha256:efa66ec48f1e9c4ba5f5d05657492ca563960b543264d613c06cd094b0818c73'
tags:
  - method
  - verification
  - egui
  - eframe
  - f1.3
  - f1.4
feature_id: feat_term-core_nx0g1r
---

## What it does

**The pattern, with its count.** **Sixteen** times in this project a third-party library has done
something other than what its name promised, and **all nineteen were settled by reading its source
code**, never by reasoning about it. It is not bad luck: it is what happens when a terminal is built
on top of a VT engine, a pseudoterminal and an immediate-mode UI framework, three layers whose
abstractions were designed for something else.

**The cases, so that the count is not an impression:**

1. `App::update` **does not exist** in eframe 0.36 — the method is `ui`, and there is a `logic` that
   runs with the window hidden as well. It changed the whole shape of the loop.
2. `Receiver` is `!Sync`: lending it does not cross threads, moving it does. It changed the contract
   of `Pty::spawn`, which went on to return the channel.
3. `RawInput::time` is documented as «monotonic, **relative to whatever**» and key events carry no
   stamp of their own. That is why there is no number called «key → pixel».
4. **`Painter::rect_filled` and `Painter::text` do not paint: they stack shapes and return.**
   Tessellating happens after the pass. The bench published that the frame spent 14 % of its slot;
   it spent 33 %.
5. `IntegrationInfo::cpu_usage` times **the whole pass**, not just the drawing. The latency
   breakdown called «tessellate and draw» a residue that included half a dozen other things.
6. `viewport_rect` is **10 000 × 10 000 points** while the window does not report a size yet. With
   no ceiling, that is hundreds of thousands of cells and an absurd size notice to the program.
7. `max_passes` is **2**: one frame can run the pass twice over the same input, and the same key
   would be written twice to the pseudoterminal.
8. `egui-winit` turns Ctrl+C into a «copy» event **before** emitting the key. Without picking it up
   at that door, **Ctrl+C does not interrupt**, which is the most basic thing a terminal does.
9. `portable-pty` copies the parent's whole environment to the child, and has no parameter to avoid
   it.
10. Feeding the parser a single repeated character walks its fast path and **gives a number that
    lies** — the same lesson, twice, in two different instruments.
11. **`Sense::click()` includes `FOCUSABLE`** (`sense.rs:60`), that is, the widget *asks for* egui's
    keyboard focus. In an application whose keyboard routing is **geographic and its own**, that
    focus competes with the application's. The source itself says so: *«Use `Sense::CLICK` if you
    don't want the widget to be focusable»* (`sense.rs:58`). Caught **before** writing, by reading
    the source.
12. **`Context::run` does not exist in 0.36.1: it is `run_ui`** (`context.rs:793`), and it hands
    over a `&mut Ui` directly, with no need to set up a panel. Caught by the compiler, which was
    cheap; but the same slip about a method that **did** exist with different semantics would have
    been of type 4 or 5.
13. **`Rect::distance_sq_to_pos` gives zero for a point *on* the border**, not only inside. So **two
    rectangles that touch each other both get the direct hit**: `hit_test.rs:205-211` looks for them
    with radius 0.0 and `find_closest_within` compares with `<=` (`hit_test.rs:414-433`). Real
    consequence: in the two border columns of the draggable divider, one and the same gesture
    **drags the divider and gives the focus to the console next to it** — `hit_test.rs:377-386`
    returns both hits on purpose. A comment in the code claimed that not widening the grab zone
    avoided that; it was false, and the review uncovered it by reading the source. The behavior was
    left as it was (pressing on the border of a console and giving it the focus is what you would
    expect); what was taken out was the invented guarantee.
14. **`eframe::Frame` can indeed be built in a test: `Frame::_new_kittest()`** (`epi.rs:720`),
    public, with no compilation condition whatsoever, and with the documentation saying it exists in
    order to test applications. And `App::ui` **does not touch the `Frame`** — its parameter is
    called `_frame`. This case is different from the first twelve and that is why it is the most
    uncomfortable: **what a call did was not assumed, what was assumed was that the call did not
    exist**, and it was written in a review artifact that testing the UI pass was «a real limitation
    of the framework». Five tests that covered mutilations able to leave the window mute went
    unwritten because of that.

15. **The VT engine's three mouse modes REPLACE one another, they do not stack.** Asking for the
    drag (`?1002h`) **turns off** the clicks-only one (`?1000h`), and a program that turns the three
    on one after another ends up with one. The opposite intuition —that they are independent flags
    that add up— was written into a test of the public surface **and the test failed**, which is how
    it was discovered. It matters because whoever reads those flags decides with a «does it want it
    for anything?» that looks at all of them: if they stacked, turning off the drag would leave the
    clicks on and the console would not get the selection back.
16. **`ALTERNATE_SCROLL` comes ON in the engine's default mode** (`TermMode::default()`), just as
    `xterm` does. So what really decides whether the wheel is translated into arrows is **the
    alternate screen**, not that flag. The first test was written assuming the opposite —«both are
    needed»— and it failed on the first run. The flag is still asked about because a program **can
    turn it off**, and `vim` does so as soon as it wants the mouse for itself.

17. **`portable-pty`'s Windows reader is not `portable-pty`'s at all**, and a whole risk rested on
    assuming its `EIO → Ok(0)` translation was Unix-only. On Windows `openpty` builds two anonymous
    pipes and `try_clone_reader` hands back a **`filedescriptor::FileDescriptor`**, whose own
    `io::Read` answers `Ok(0)` when the error is a broken pipe — the same arm, reached the same way.
    And `win/mod.rs`, the file the risk pointed a reader at, has **no read path at all**. The risk
    had already been written into a design, a decision record and a pull-request body before a
    skeptic read the source.
18. **On Windows the pipe's write end is owned by the pseudoconsole**, shared with the master through
    one `Arc`, so `drop(pair.slave)` — the line whose whole purpose on Unix is to let the read reach
    end of file when the child dies — **closes nothing there**, and no end of file can arrive while
    the console lives. Found in the same read as case 17, and it is the opposite risk: not «an
    ordinary exit may look like a failure» but «no ordinary exit is ever seen».
19. **`std::io::ErrorKind` is a coarse bucket, not a description**, and the errors a pseudoterminal
    produces fall in the one that says nothing: errno 5, 70 and 75 all come out as the variant whose
    entire printed form is one word, which is **unnameable in user code**. `raw_os_error()` keeps the
    number. A value declared as a contract carried only the kind, and a blinded auditor asked what
    the sentence built from it would say.

**Where the alarm goes off.** The expensive case is not the one that does not compile: it is the one
where **the label of a number describes what one believes the call does**. Cases 4 and 5 are the
same error with opposite signs — calling queueing «painting», and calling a residue «tessellating»—
and both published an optimistic number with every appearance of being the right one, in the
milestone whose product **is** the number. Neither of the two would have been detected by a test:
both numbers were internally coherent.

**Three patterns, and the third one is new:**

- **Convenience constructors that package a decision** (cases 11 and 12). `Sense::click()` is not
  «sense clicks», it is «sense clicks *and on top of that* ask for focus». When an API offers a
  short constructor and an explicit one, **the short one carries inside a choice somebody made for
  another use case** — look at what it is before accepting it. It happened again with
  `Sense::drag()`, which is `DRAG | FOCUSABLE` (`sense.rs:68`), although that time it was already
  known what to look for.
- **Geometry that does not behave the way intuition says** (case 13). A shared edge is not a border
  that separates: for the hit-test engine it belongs to both sides.
- **Assuming that some flags add up** (cases 15 and 16). A set of modes with similar names looks
  like a set of independent switches, and sometimes it is a **state machine**: turning one on turns
  another off. And a mode can come **on from the factory**, with which the condition one believes to
  be checking is always met already. Both times it was uncovered by **a test written from intuition,
  which failed** — which is the cheap way of discovering it, and the reason for writing the test
  before trusting.
- **Assuming an absence, not a semantics** (case 14). «This cannot be done» is a claim about someone
  else's code just as «this does X» is, and it needs the same proof. A limit declared honestly is
  worth more than a contorted test — but only if it has first been checked that it is a limit.

**What to do, concretely.** The sources are in
`~/.cargo/registry/src/index.crates.io-*/<caja>-<versión>/`, and reading them costs one search. They
are read **before** leaning on the behavior, not after the number comes out odd, whenever: a number
that is going to be published depends on when something happens; a contract depends on what exactly
a call does; one is about to write in a comment «this does X» about someone else's code; one is
going to use a **convenience constructor** of an API that also offers the explicit version; or one
is about to **declare that something cannot be done**.

**And who catches it when it slips through:** in **eight of the nineteen** cases it was uncovered by a
review that went and read the source instead of opining about the diff. It is the strongest argument
in favor of asking whoever reviews to **verify against the library's code**, not against the
author's reasoning.

## Where it lives

The cases lived through are spread across `crates/app/` —`lib.rs` (the loop, the keyboard, focus by
click, the draggable divider and the tests of the UI pass), `medida.rs` (the stopwatches),
`pintor.rs` (the grid measurement)— and `crates/term-core/src/lib.rs` (the pseudoterminal and its
channel). Cases 1 to 10 came out in F1.3; 11 to 14 in F1.4, when putting two consoles with focus by
click and a draggable divider. The account of each one, with the source line that proves it, is in
the review records of those milestones. Cases 15 and 16 came out in F2.6, when asking the engine
what the program has asked for about the mouse: they live in `crates/term-core/src/screen.rs` and
their decision is **D23**. Cases 17 to 19 came out in F2.13's first pull request, when giving a
console's ending a vocabulary: 17 and 18 in `portable-pty`'s Windows side and `filedescriptor`,
found by a skeptic **refuting a risk this project had already published**, and 19 in the standard
library, found by an auditor blinded to the design. Their decision is **D38**, whose «what it leaves
open» carries the withdrawal of the risk cases 17 and 18 refuted.
