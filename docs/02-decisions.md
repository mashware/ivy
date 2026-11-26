# Decisions

Each one with its reason and what was discarded. If a decision changes, it is edited here and the
why is stated.

A decision that accepts a limit instead of fixing it writes that limit down the way **D54** says.

---

## D1 — Real cross-platform (Linux, macOS, Windows)

**Decided.** This is not a personal app for Linux: there are too many people on Mac and Windows to
write them off.

**Consequence:** it rules out GTK4 + VTE, which was the technically most comfortable option on Linux
(see D3).

**Confirmed by the landscape, August 2026.** Of the four competitors analyzed, three are **macOS
only** (Xirp, Conductor, and Superset except for an experimental AppImage). The only one that runs
on all three platforms is Warp — and it is also the only one that meets every other condition
([07-landscape.md](07-landscape.md) → E5). That being cross-platform is rare in this category does
not weaken D1: it marks it as what actually sets the strongest rival apart from the rest.

> **Corrected on 28 September 2026: that count is the landscape's first round.** Later in August
> the landscape reached eight analyses, and in its side-by-side table **three** run on all three
> platforms — Warp, Orca and Herdr ([07-landscape.md](07-landscape.md) → «The seven analyzed, side
> by side»). Being cross-platform is no longer rare in this category, and D1 stands for the reason
> it was taken, which never depended on that.

---

## D2 — The terminal engine is consumed as a library

**Decided:** the **`alacritty_terminal`** crate for the VT parsing and the state, **`portable-pty`**
for the PTY (it abstracts Unix and Windows's ConPTY).

**Why:** writing a terminal emulator is person-years — a VT parser with decades of edge cases,
scrollback, reflow, shaping, IME. But it is no longer necessary: **Zed consumes it exactly like
this** and paints on top with its own framework.

**Discarded:**
- **VTE** — GTK/Linux only; and on top of that with no font ligatures ([issue #192](https://gitlab.gnome.org/GNOME/vte/-/issues/192), open for years) and no Kitty graphics protocol.
- **Writing it ourselves** — that is what Warp and JetBrains did, both with paid teams.

---

## D3 — Rust + GPUI for the interface

**Decided:** Rust, with **GPUI** for the product and **egui** as an option for prototyping fast.

**Why:** it is Zed's path for this exact case (embedded terminal, GPU, many panels, three
platforms), and Rust is where the ecosystem of this particular piece lives.

**Risk accepted:** GPUI has an unstable API and little documentation outside Zed. If it turns into
an uphill climb, egui or Iced.

**New candidate, not decided — `warpui`, August 2026.** Warp released its client on 30 April 2026
and **the licenses are not handed out uniformly**; verbatim from its README:

> Warp's UI framework (the `warpui_core` and `warpui` crates) are licensed under the **MIT
> license**. The rest of the code in this repository is licensed under the **AGPL v3**.

Verified in the `Cargo.toml` of both crates (`license = "MIT"`). That puts on the table an interface
framework in Rust, with GPU, a permissive license and **already proven in a real cross-platform
terminal** — exactly the gap that the risk above describes for GPUI.

**It does not change the decision, and nobody should change it without measuring.** This decision is
taken with numbers —that is how Electron was ruled out and that is how egui was judged— and here
there are none yet. Two things have to be checked before this is even an option:

1. **That `warpui` can be used on its own.** It sits in the same repository as the AGPL code and it
   may drag in dependencies from those crates; if it does, the crate's MIT license is worth nothing.
2. **That nothing gets copied from the rest.** The rest of the repository is AGPLv3 and this project
   is not.

See [07-landscape.md](07-landscape.md) → E5 for the product context.

**Discarded — Electron/Tauri + xterm.js.** It was the cheapest in effort (a known stack), but it has
a ceiling right in our use case. From xterm.js's own issue
[«Support dozens of terminals on a single page»](https://github.com/xtermjs/xterm.js/issues/4379):

- Browsers limit the number of active WebGL contexts (~a dozen). They say literally that
  multiplexers need dozens of panes and **the browser does not support it**.
- Terminals compete for the main thread: **2 instances → 30 fps, 4 → 15 fps**.

That describes exactly «several agents writing at the same time». VS Code does not suffer because
there you usually have one or two active terminals, not eight.

### Measured — 8 August 2026

The xterm.js issue was borrowed reasoning. Now there is a measurement of our own, against
[Orca](https://github.com/stablyai/orca) (Electron + xterm.js with WebGL, the most mature product in
this category) and WezTerm as the native reference. Same machine, Wayland, hardware GPU confirmed in
Orca (`--ozone-platform=wayland`, `--render-node-override=/dev/dri/renderD128`, with no trace of
SwiftShader). Load: 22.7 MiB of TUI-style output —color, bold and a status-bar repaint with cursor
movement every 50 lines— dumped all at once. What is measured is how long the terminal takes to
drain the PTY.

| | WezTerm | Orca |
|---|---|---|
| 1 pane | 0.80 s · 28.4 MiB/s | 2.63 s · 8.6 MiB/s |
| 4 visible panes | 0.98 / 0.98 / 1.01 / 1.06 s | 8.44 / 12.50 / 12.72 / 12.64 s |
| Drop from 1 → 4 | ×1.26 | ×4.4 |
| Aggregate with 4 | 85.7 MiB/s | 7.1 MiB/s |

**What decides is not the 12× difference, it is the sign:** Orca with four panes moves *less* total
work than with one (7.1 against 8.6 MiB/s). It scales negatively. WezTerm goes from 28.4 to 85.7,
that is ×3. It is the signature of the shared main thread.

The method plays in its favor: it measures the draining of the PTY, so a terminal that buffers and
drops frames would come out well here and badly to the eye. Orca comes out badly already in the
draining.

**What this measurement proves and what it does not.** It proves that the ceiling exists and where
it is. It does **not** prove that an agent's load touches it: an agent's prose is tens of bytes per
second, and a `cargo build` is a few hundred KB, which even at Orca's degraded 1.8 MiB/s drain in
0.2 s. Selling the 12× as the justification for a terminal of our own would be cheating.

**The regime where it does bite is real use, not the synthetic one.** Five or six visible panes:
four with agents and **one or two following logs** from a worktree that is being tested. A `tail -f`
on a development server is a **sustained** producer, not a burst, and it lives alongside the
continuous repaint of each pane's panel. That combination —one or two sustained producers plus N
panels painting— is the load that has to be measured, and it is exactly the one that the negative
scaling says this stack cannot take. Orca does not have it: its panels sit still while the agent
thinks.

**Consequence for the product:** Orca dodges the ceiling because its product is a different one — it
fans agents out across worktrees and only mounts the terminals of the active worktree (documented
lazy mounting), so it looks at one at a time. Here the panes are visible at the same time by
definition, so it cannot be dodged the same way.

**Acceptance criterion:** with N visible panes producing at the same time, the aggregate throughput
has to **go up** with N, not down. The tool that measures it lives in the repo: `crates/host-load`,
and its documentation carries the protocol and these numbers as a reference.

**Who that criterion decides against, and who it does not.** It decides **against other people's
terminals**: Orca failed already in the draining, so there was no need to look at the painting to
rule it out. It does **not decide against ours**, and the reason is structural, not a nuance — worth
reading before taking the criterion as closed with a `host-load` measurement:

- The instrument measures what the terminal **drains from the PTY**, not what it **paints**.
- The mitigation that this very phase plans for F1.3 is *«repaint only when the PTY produces
  changes»* (see the Phase 1 risks in [05-roadmap.md](05-roadmap.md)), that is, **coalescing
  frames**. Our app would pass the test **by construction**, measuring well precisely for doing what
  the test does not look at.
- And the painting is **unobservable from the process that produces the load**: no terminal protocol
  reports a presented frame, and the round trip of `ESC[6n` only proves *consumed and parsed*.

**Pending, therefore:** the counters that would see it —bytes processed per frame, frames presented,
the lag of the last one with respect to the last input— live **inside `app`** and cannot be built
before the painting loop. That is F1.3/F1.4 work, and it is what really closes the second symptom
the roadmap asks to measure.

If with real load the ceiling is not touched, the honest justification for a terminal of our own
stops being performance and becomes control of the compositing and the integrated panel.

**It also rules out forking Orca**, for two independent reasons: it would inherit this ceiling, and
its extensibility all points at the agent (skills, MCP) — there is no plugin API for its interface,
which is exactly where the panel would have to live.

**Discarded — GTK4 + VTE.** Technically the most comfortable on Linux (title bar, buttons, dragging
panes for free) but incompatible with D1.

### Measured — 9 August 2026: egui under load, from inside

This **builds** the counters that the previous measurement left pending: the ones that see the
**painting** —and not only the draining of the pseudoterminal— already exist inside `app`, because
the painting loop already exists. It does not close that measurement's whole criterion: *«with N
visible panes producing at the same time, the aggregate has to go up with N»* is still unchecked,
because Phase 1 does not have two panes yet — that arrives in F1.4. All the numbers are from the
same machine (Wayland, a 120 Hz display), in `--release`, with the **most naive painter possible**:
one cell at a time, with no cache and no tracking of what changed.

**The first symptom the roadmap asked to watch — the full repaint with more than one pane:**

| Synthetic 24×80 consoles | 1 (1 920 cells) | 2 (3 840 cells) |
|---|---|---|
| CPU per frame | 1.52 ms (worst 2.96) | **2.98 ms** (worst 5.73) |

Two consoles cost **1.96×** what one costs: it scales linearly, not super-linearly. With two, the
frame spends **36 %** of its 8.4 ms gap —and its **worst** frame, 68 %—. Measured with **full**
screens —every cell with its own background, which forces drawing a rectangle per cell on top of the
glyph— and in continuous repaint, which is the ceiling and not normal use. The instrument is `cargo
run --release --example sweep -p app`, and it calls the real painter, not a copy.

**The size of those consoles matters and has to be said before reading the number:** they are 24×80
each, so the two together add up to 3 840 cells — **fewer than the 6 954 of a single real window**
on this machine (61×114), which is where the latency further down was taken. The bench does not
measure «two panes»; it measures **two independent grids** of the classic size. That batch's frames
per second (119.6 and 119.2) are left out of the table on purpose: they are capped by vertical sync,
so they only prove that the frame fits in its gap, not how much headroom is left over — that is what
the percentage is for.

**The second symptom — the latency:**

Taken inside the application, in a window of 61×114 cells:

| | normal use (13 keys, 32 frames) | dump of 13.5 MB (111 frames) |
|---|---|---|
| key in the frame → byte in the program | **36 µs** (72 at the 95 %) | — |
| byte available → frame paid for | **3.27 ms** (9.8 at the 95 %) | **377 ms** (671 at the 95 %) |
| … waiting its turn | 2.61 ms | 373 ms |
| … interpreting + enqueuing + the rest | 0.65 ms | 4.1 ms |

The «normal use» sample is small —thirteen keystrokes— and it serves for what it is used for here:
fixing the order of magnitude, which is microseconds. To claim more would take more.

**What these two tables say together is the verdict: the real work never goes beyond a few
milliseconds.** What is perceived as lag is, in both regimes, **waiting its turn** — and they are
two different waits, neither of them chargeable to the framework: in normal use it is the 2.6 ms of
the gap between frames of a 120 Hz display, that is, vertical sync; in the dump it is the 373 ms of
the **budget of 128 KiB per frame**, which is the price deliberately chosen so that the window does
not freeze.

**Answer to the two symptoms: neither of them appears.** The full repaint scales linearly and leaves
two thirds of the frame free with two full grids; the write latency is microseconds. Neither of the
two justifies revisiting the framework today.

And a figure that confirms the budget is well set: **interpreting 128 KiB costs 1.67 ms measured**,
against the ~1.94 ms that the absorption of 64.5 MiB/s calculated in F1.2 predicted. The estimate
back then was a good one.

**The painting escape ladder is not triggered.** It was written as cell by cell → spans per row → a
mesh with the font atlas, and executable **only if the number asked for it**. It does not ask: the
most naive rung leaves 64 % of the frame free with two consoles. It stays written for when it is
needed, untouched.

**What these numbers do NOT prove, said before anyone over-cites them:**

- **One machine, one display, one compositor.** The numbers are from this box with Wayland and 120
  Hz.
- **Two 24×80 grids, not two panes of the real window.** It is the most important caveat of them
  all: 3 840 cells against the 6 954 of the window where the latency was measured. In F1.4 two panes
  **share out** the window instead of doubling it, so the total area does not grow — what grows is
  the number of grids, of reads and of calls to the painter. That case is **not measured**, and it
  is exactly the one that F1.4 has to put through the bench again, with the real window split.
- **Four and eight consoles are not measured either.** The roadmap's symptom is written with two and
  that is where it was measured. Extrapolating the linear scale, four would come to around 6 ms on
  average and their worst frame would fall out of the gap: **the headroom does not stretch to
  everything**.
- **The worst frame with two grids already spends 68 %** of the gap. The average says there is room
  to spare; the worst says how much headroom there really is, and it is half of what the average
  suggests.
- **None of this measures the compositing or the panel**, which is what Phase 1 does not have yet.
- **There is cost attributed to egui today that is not its own.** Reading the grid materializes all
  of its cells on every pass —288 KiB per pass in a 60×240 window, which at 60 frames is 17 MiB/s—
  and any framework will carry that too. And the repaint request is asked for **once for every 8 KiB
  chunk** that arrives from the pseudoterminal, some 34 000 per second in a dump, each of them
  taking a lock that the interface thread also uses; egui on top of that turns every request into
  **two** passes. None of the three things was isolated in these measurements. If one day the number
  comes out strange, this paragraph is the first place to look.

**Two things that do bite and are not about performance.** They go here because a verdict that only
looked at the milliseconds would give a false impression of «this is ready»:

1. **egui does not use the system fonts.** It ships four and none of them covers CJK or the Private
   Use Area, so Japanese and the icons of a modern `ls` come out as empty boxes, even though the
   machine has those fonts installed. The painter does the right thing —it reserves the slot and
   asks for the glyph—; the one that does not have it is the font. Registering system fonts at
   startup is pending work, and it is what separates this from being usable daily.
2. **The output queue has no ceiling**
   ([#20](https://github.com/mashware/ivy-history-private/issues/20)): a program that writes without
   stopping —`yes`, `cat /dev/urandom`— exhausts memory. The only real brake is to stop reading the
   pseudoterminal, and that lives in `term-core`.

> **Corrected on 28 September 2026: neither of the two bites any more.** The system fonts reach the
> engine since F2.9 (**D24**), and the output queue has had a ceiling since **D8**, which stops
> reading the pseudoterminal exactly as this paragraph asked. The list is kept as what was true on
> 9 August.

**What it means for this decision.** D3 stands as it is: **egui takes what Phase 1 needs with room
to spare**, which is exactly the role this decision gave it. What these numbers add is a question
that before could not even be put with data —if the headroom is this, what does GPUI bring that
justifies its cost?— and one that is **not answered here**: it is answered when F1.4 has two real
panes and F1.6 has the panels painting at the same time, which is the load D3 says has to be
measured. Changing the underlying decision in an annex, with two synthetic consoles from a single
machine, would be exactly the kind of shortcut this decision was taken to avoid.

### Measured — 11 August 2026: two real consoles, inside the application

This is what the previous annex left pending: **the panes are no longer synthetic**. They are F1.4's
two consoles, each with its pseudoterminal, its grid and its program, measured **inside the
application** and not on a bench. Same machine as the previous measurements, `--release`, a 120 Hz
display, a grid of **61×57 per console** (window maximized, shared out in halves).

The load is a TUI-style dump —color, bold and a status-bar repaint with cursor movement—. **The
batch that is published is the one of 400 000 lines per console**, which gives between **376 and 382
frames with output** per run: two runs of the symmetric case. The asymmetric one is two runs of
120 000 lines (105-120 frames). The numbers are the mean of the means.

| Frame span | One dumping, the other still | **Both dumping at once** |
|---|---|---|
| Interpreting (both consoles) | 1.92 ms | **3.35 ms** |
| Enqueuing the paint (both) | 0.87 ms | **1.83 ms** |
| The rest of the frame (tessellating, drawing, scaffolding) | 0.61 ms | **0.74 ms** |
| **Frame cost** | **3.39 ms** | **5.92 ms** |
| Of the 120 Hz gap (8.33 ms) | 41 % | **71 %** |
| Of the 60 Hz gap (16.67 ms) | 20 % | 36 % |
| **Key in the frame → byte in the program** | not measured | **3.42 ms on average, 5.00 ms the p95** (28 keystrokes) |

**A shorter batch gives a worse number, and the why of it matters.** Three runs of 120 000 lines
(105-120 frames) gave **7.00 ms**, 84 % of the gap. The difference is not noise: with three times
the frames, the startup cost —opening the window, uploading textures, measuring the font for the
first time— weighs a third in the mean. **The number that counts is the long batch's**, and the
short one is written down here so that nobody finds it afterwards and believes there is a
contradiction.

**What these numbers prove:**

- With two real consoles **dumping at the same time**, the frame costs **5.92 ms on average and fits
  in the 120 Hz gap**, with 29 % of headroom. It is the first time that number is taken over real
  panes and not simulated ones.
- **The bulk of the frame is not spent by the interface framework: it is spent interpreting.** 3.35
  of the 5.92 ms are `alacritty_terminal` swallowing the 128 KiB per frame **of each console** — the
  terminal's work, and exactly what the per-frame budget exists to bound. What is left for drawing
  is 2.57 ms, **31 %** of the gap.
- **The window keeps responding to the keyboard while the double dump lasts, and now with a
  figure:** from the key in the frame to the byte written into the program, **3.42 ms on average and
  5.00 ms at the 95th percentile**, over 28 keystrokes typed by hand during the flood. In F1.3, with
  no dump, that span gave **36 µs**: it goes up almost ×100, and **the cause is identified, it is
  not a mystery** — the stopwatch starts when the frame starts and ahead of writing the key runs
  interpreting, which is those same 3.35 ms. The key waits for the frame to finish swallowing. It is
  still imperceptible: 5 ms in the worst case is less than a 60 Hz frame.
- **The instrument shares out correctly.** The rest of the frame also discounts the painting of a
  console that received no output in that frame, which was an attribution defect found in review. It
  shows in the report itself: the frames counted in «the rest» (105-120) equal or exceed those of
  the console with output (107-115), because they include the ones in which only the still console
  was painted.

**What they do NOT prove. This list is half the value of the annex, and it is long on purpose:**

- **This 31 % does NOT confirm the 36 % of the 9 August bench, because they do not measure the same
  thing.** That one measured **full screens and continuous repaint** —a ceiling, and it said so—
  over 3 840 cells. This one measures a TUI-style dump with repaint on demand over **6 954 cells**,
  almost double, and in a regime where the painter skips most of the cells for carrying the default
  background. That the two percentages come out similar is a **coincidence of two opposing effects**
  —almost double the cells against a much cheaper fill—, not a confirmation. The warning that the
  instrument itself carries written down (`crates/app/src/measure.rs`) says exactly this: comparing
  spans across measurements demands looking at the grid, not just at the span.
- **There is no baseline of one console inside the application**, so **there is no way to state how
  the painting scales** from one to two. And it is not an avoidable oversight: since F1.4 the window
  opens **always two** consoles, so the 1-against-2 comparison stopped being measurable from inside.
  The instrument that would give it is the bench with real tiling, which is not built
  ([#33](https://github.com/mashware/ivy-history-private/issues/33)).
- **«Enqueuing» is not egui's cost: it is this project's painter**, which is still the most naive
  one possible —one cell at a time, with no cache and no tracking of what changed— calling
  `rect_filled` and `text`. Of the 3.21 ms of drawing, the only thing attributable to the framework
  beyond argument is **the rest of the frame: 0.74 ms, 9 % of the gap**. Saying «the framework
  spends 31 %» would be charging it with code of our own that would be rewritten the same way with
  any other.
- **The worst case has not been re-measured with two panes.** This scenario is demanding but it is
  not the ceiling: the ceiling is full screens and continuous repaint, which is what the bench
  measured. So this annex **does not close** the acceptance criterion that asked for the worst case
  to be measured with real panes.
- **The 95th percentile of the symmetric case falls outside the 120 Hz gap** (~9.4 ms adding up the
  p95s of the long batch; and adding up the percentiles of spans does not give the frame's
  percentile, so that number is indicative, not a measurement). It fits in the 60 Hz one with room
  to spare. There are occasional stutters during a double dump.
- **Nothing about four consoles.** Multiplying by two gives 11.8 ms, but that **contradicts this
  annex's own model**: if with four each grid is narrower, enqueuing per console goes down and only
  interpreting scales linearly, which would come out somewhat closer to 9 ms. Both sums are
  arithmetic, not measurement, and that is why the roadmap's warning still stands.
- **Nothing about the panel painting at the same time**, which is F1.6 and the other half of the
  load this decision says has to be measured.
- **The «byte available → frame paid for» series is only interpretable in the symmetric case.** It
  includes the whole frame —it is presented indivisible— so in the asymmetric one the quiet
  console's series is degraded by the neighbor's dump and does not describe it. There its own spans
  are what counts, not that total.

**What it means for this decision.** **D3 does not change, and it still cannot be closed.** What
this annex contributes is not the confirmation it seemed to be: it is an **order of magnitude taken
over real panes** —the frame fits in the 120 Hz gap with two consoles dumping— and, above all, **a
change of suspect**. The interface framework takes **0.74 demonstrable ms out of 8.33**; the rest is
spent by the terminal's parser and by a painter of our own that nobody has optimized yet. Before
reopening whether GPUI is worth its cost, there are two things that would cost less and would give
more headroom: **writing a painter that does not repaint every cell every frame**, and trying the
rotating turns of the budget that F1.4's design left noted down. And to really close the criterion
three measurements are missing: the worst case with two panes, the baseline of a single one inside
the application, and F1.6 with the panels on top.

### Measured — 13 August 2026: with both panels painting

The third of those three. Same machine and same `--release`, a grid of **61 rows** per console and
the window untouched; the load is `host-load`'s deterministic dump —color, bold and a status-bar
repaint with cursor movement—, **44 MiB per console**, started at the same time with their alignment
to the wall clock. Each batch is **two runs** and the numbers are the mean of the means, over
**357-358 frames with output per console** in all four. It is taken with
`scripts/measure-frame.sh`, which ships in the repo: the 11 August batch left no script behind and
reconstructing it cost more than taking it.

**Three regimes, and the first is the one that makes the other two legible:**

| | **A** with no panel | **B** panel still | **C** panel publishing 1×s |
|---|---|---|---|
| Columns per console | **57** | **21** | **21** |
| Interpreting (both) | 4.35 ms | 4.51 ms | 4.35 ms |
| Enqueuing the paint (both) | 1.80 ms | 0.75 ms | 0.74 ms |
| **The panel (both)** | **0.77 µs** | **12.8 µs** | **13.6 µs** |
| The rest of the frame | 0.86 ms | 0.50 ms | 0.46 ms |
| **Frame cost** | **7.01 ms** | **5.78 ms** | **5.56 ms** |
| Of the 120 Hz gap | 84 % | 69 % | 67 % |
| Frames of the window | 368 and 369 | 372 and 371 | **472 and 471** |

> **With the panels painting the frame comes out cheaper, and that does NOT mean the panels come
> free.** The panel takes **36 of the 57 columns** of its console, so the console paints 63 % fewer
> cells. **The span that goes down because of that is `enqueue`** —1.80 to 0.75 ms, a −58 %—, and it
> goes down because there are fewer cells to stack, not because anything has improved. What the
> table shows is a trade —less console in exchange for a panel—, not a gain. **Reading «−18 % of
> cost» as an improvement is the mistake this control batch exists to prevent**, and without column
> A the annex would have made it.

**And one that comes out the opposite way round from what was expected: wrapping is not
noticeable.** At 57 columns the load's lines —~50 visible characters— do not wrap; at 21 they wrap
into three rows. It was reasonable to expect interpreting to get more expensive, because the wrap
width is precisely what it depends on. **It does not happen**: 4.35 / 4.51 / 4.35 ms, flat within
the noise between runs. The cost of putting bytes into the grid is dominated by the amount of bytes,
not by the way they are shared out into rows.

**What does belong to the panel, isolated.** The subtraction that does count is the one of the
**panel span**, because its floor is the same in the three columns: in A that span is not zero —the
directory is always created, so the flag has to be picked up, the time asked for and the width
shared out— and it measures **0.77 µs**. On top of that floor:

| | Mean (both) | The 95 %, left console |
|---|---|---|
| A — with no panel | 0.77 µs | 1.4 µs |
| B — panel still | **12.8 µs** | 19.3 µs |
| C — panel publishing | **13.6 µs** | 21.4 µs |

Reading and painting two panels costs **around 13 µs per frame**, **0.2 % of the frame** and **0.15
% of the 120 Hz gap**. The panel, as a per-frame cost, **is not noticeable**.

**What publishing adds cannot be separated from the noise, and saying so matters.** The table shows
12.8 µs still and 13.6 publishing, that is 0.8 of difference. But **batch D is a valid replicate of
C** —once it is shown below that where the temporary file lands makes no difference—, and the four
runs of the «publishing» regime spread between **13.5 and 15.3 µs**: the variation **inside** the
same regime doubles the effect attributed to publishing. With two runs it looked separated; with the
four comparable ones, it is not. What does hold up is the qualitative part: **publishing once per
second is not noticeable**.

**What is noticeable about the talkative panel is the frames, not its cost.** Each console produces
in 357-358 frames in all four batches, and yet the window counts **368-369 in A** and **471-472 in
C**. The union grows with the addends staying equal, so **there is less overlap**: in A the two
consoles almost always produce in the same frame —358 + 357 − 368 = **347 shared**, 97 %— and in C
considerably less: 358 + 357 − 472 = **243**, 68 %. The same work is shared out over **28 % more
frames**, each of them cheaper. That is why **C's «frame cost» is not compared with B's**: they are
frames that carry different amounts of work inside them.

> **The arithmetic of the overlap is in the data; the cause is interpretation and has to be read as
> such.** That it is the watcher's wake-ups that desynchronize the two consoles is **not measured**:
> the report publishes neither the run's duration nor how many times it woke up, so there is no way
> to check that the publications explain the ~104 extra frames. The only thing pointing in that
> direction is that B —with the watcher armed but with nobody publishing— stays at 371.

**Where the temporary file lands makes no difference, and not by chance.** It looked like the
obvious explanation for those 100 frames —a temporary file written inside the watched directory
waking the window up on its own—, so a fourth batch identical to C was taken but with the temporary
file **outside**: 473 and 466 frames, against 472 and 471. And the code explains why it had to come
out the same: the watcher **filters by exact path** and only raises the flag when the notice carries
`<dir>/panel.json` (`crates/app/src/watcher.rs`), so any other file in the directory is invisible
to it. The hypothesis fell twice, by measuring and by reading, and it stays written down so that
nobody raises it again.

**Two attribution gaps, both with their number:**

1. **Arming the alarm is the panel's work that today is published as egui's cost.** Working out when
   it will be time to warn that a panel is old is done **twice per frame** and **outside** the
   pane's three clocks, so it falls into «the rest of the frame». Measured with a probe in two runs
   —784 and 860 ns—: **822 ns per call, 1.64 µs per frame**, **0.4 %** of that span and **12 % of
   the panel's own cost**. The bias is small but it goes in the direction that does the most damage,
   because it inflates precisely the figure this decision has to decide about.
2. **Finding out appears nowhere.** The file watcher runs on **its own thread**, and `eframe`'s
   stopwatch is a wall clock over the interface thread's pass. Its cost does not inflate «the rest»
   —that is, it is not charged to egui, which was the risk— but **it is not in the number either**:
   what is published here is what it costs to read and paint the panel, not what it costs to find
   out that it changed.

**What these numbers do NOT prove. As in the two previous annexes, this list is half the value:**

- **They do not compare with 11 August, and batch A itself proves it.** That one published 5.92 ms
  with no panels; this one, in the same configuration, gives **7.01 ms**. Another day, another load,
  another machine in another state. **The only valid comparison is inside this table**, which is
  what the three were taken the same day and in the same window for.
- **B's and C's totals are not subtracted against A's.** Only the subtraction of the panel span is
  honest, because it is the only one whose floor does not change with the grid.
- **The 95 % of the panel span is taken only over frames in which that console received output.**
  The series are only recorded when something arrived, whereas the span is discounted from the total
  always. A frame that the watcher wakes with no console output does not enter that percentile.
- **The frame cost is wall clock, not CPU**, despite the framework's field being called that. With
  the machine loaded, the contention from other processes enters the number.
- **Every run prints «the directory being watched has stopped existing», and it does not affect the
  numbers.** The panel directories are deleted when **the application** ends, not when the program
  of a console dies: the window waits for both and only then closes. So that notice comes out during
  the shutdown, after measuring. It is said here because whoever repeats the batch will see it and
  think they have broken something.
- **The left console costs systematically more than the right one, and nobody knows why.** In all
  eight runs, without exception: in the talkative panel's batch its panel span is ~8.4 µs against
  ~5.2 —**1.6×**— and enqueuing the paint 437 µs against 328 —**1.3×**—. The «both» column hides
  that asymmetry by adding them up. The two consoles have the same grid and the same load, so the
  explanation is in the order in which they are painted or in the focus, and **it has not been
  investigated**.
- **Three things about the method cannot be checked from the report**: the 44 MiB per console, that
  the two started aligned, and the publishing frequency. The report prints neither volume, nor
  duration, nor a count of publications — whoever wants to audit it has to read
  `scripts/measure-frame.sh`, not the output files.
- **Nothing about the worst case**: this is a demanding dump, not the ceiling of full screens with
  continuous repaint. That criterion is still not closed.
- **Nothing about four panes**, and now there is a new reason not to extrapolate: if two consoles
  give 57 columns each, four would give around **28 before discounting the panel** — and the panel
  takes **36**. That is, with four panes the panel **does not fit**, and the tiling would have to do
  something it does not do today. The number cannot be multiplied
  ([#33](https://github.com/mashware/ivy-history-private/issues/33)).

**What it means for this decision.** **D3 does not change, and the change of suspect the previous
annex announced is confirmed and grows.** With both panels on top, what is demonstrably attributable
to the interface framework is **0.50 ms out of 8.33**, **6 % of the gap** —less than August's 0.74
ms, and for a reason that does egui no favors at all: there are fewer cells to tessellate—. The
panels, which were the other half of the load this decision ordered measured, spend **13 µs**, four
orders of magnitude below the budget. **The frame is still spent by interpreting** —4.5 of the 5.8
ms of the still-panel batch— and by a painter of our own that nobody has optimized.

Of the three measurements that were missing, **this one is done**. Two are still missing, and both
depend on the bench with real tiling: the worst case with two panes and the baseline of a single one
inside the application, which stopped being measurable from inside when the window went over to
always opening two.

---

## D4 — Control through the console, not through the API

**Decided.** The agent is controlled by writing into its PTY and reading its output, not by calling
the provider's API.

**Why — it is economic, not technical.** Subscriptions (Claude Max, etc.) are only worth something
inside each provider's official client. Calling the API is billed by tokens separately. A tool that
forces the user to pay for the API for what they already pay for by subscription does not get
adopted.

**Confirmed from outside, twice.** Cline Kanban orchestrates by launching each agent's official
binaries in a process, not by calling models over the API; and Xirp authenticates each agent
*«through its native CLI»* — with thousands of engineers and a company invoice behind it, which is
exactly where paying for the API separately would have cost less to justify. See
[07-landscape.md](07-landscape.md) → E1 and E3.

**Design consequence:** the app must **never** define `ANTHROPIC_API_KEY` (or equivalents) when
launching a panel. That variable takes priority over the subscription's session and would change the
user's billing without them noticing.

**Gray area to watch:** Claude Code's non-interactive mode (`claude -p`) ought to use the
subscription's session, but there are open issues reporting unexpected API billing
([#43333](https://github.com/anthropics/claude-code/issues/43333),
[#37686](https://github.com/anthropics/claude-code/issues/37686) — $1 800 in two days). **Do not
build on that path without verifying it with small, watched spending.**

### Confirmed the hard way — Anthropic banned it, and with a date

This was the assumption the whole decision rested on, and **it stopped being an assumption in
2026**. The chronology, verified in August 2026:

| When | What |
|---|---|
| January 2026 | Anthropic **blocks** subscription OAuth tokens outside its official applications, with no prior notice. Backs down after the protests |
| February 2026 | It **bans it formally**: Free, Pro and Max tokens cannot be used in third-party tools or in the agents SDK |
| **4 April 2026** | **It comes into force.** Pro, Max and Team stop covering use through third-party tools — **opencode**, OpenClaw, NanoClaw and any other that routes requests over OAuth. Compensation: a credit equivalent to one month of subscription |

> **A Claude Code subscription is not a developer credential.** What is bought is the right to use
> **two products** —Claude Code and `claude.ai`—, not a token that plugs into any orchestrator.
> Whoever integrates Claude into their product needs an API key and billing by usage.

**What it means for this project, and it is the line that separates everything:** **we do not use
anybody's token.** We launch the official binary in a pseudoterminal and it authenticates with its
own session, just as if the user had typed it by hand. That is why the app **hosts** agents instead
of calling them, and that is why `ANTHROPIC_API_KEY` is forbidden: it is not hygiene, it is the
difference between being inside or outside the policy. The hard rule stays exactly the same, with an
external reason behind it now.

**And a consequence for Phase 5 that was not written down.** The supervisor —«an agent that watches
the others and warns»— **cannot be a call of ours to a model**: that would be the app consuming API,
that is, the user paying separately for what they already pay for. It has to be **another console
agent**, in its pane, speaking over MCP like any other. The roadmap says that Phase 5 «does not need
any new piece»; that is still true, and **this is the reason it has to go on being true**.

**And about opencode in particular** ([06-references.md](06-references.md)): it supports more than
75 providers —it is the most agnostic of them all— but **with Claude it goes over the API and tokens
since 4 April 2026**. As a panel *producer* it is perfect; as a hosted agent, its Claude user pays
by usage.

Sources: [alternativeto.net](https://alternativeto.net/news/2026/2/anthropic-officially-bans-using-subscription-authentication-for-third-party-claude-use) ·
[winbuzzer.com](https://winbuzzer.com/2026/02/19/anthropic-bans-claude-subscription-oauth-in-third-party-apps-xcxwbn/) ·
[yage.ai — «A Claude Code Subscription Is Not a Developer Credential»](https://yage.ai/share/claude-code-subscription-not-a-developer-credential-en-20260321.html)

---

## D5 — Three channels, not one

**Decided.** See [03-architecture.md](03-architecture.md).

| Direction | Channel | What for |
|---|---|---|
| Agent → app | **MCP** | Create panes, publish state, read output |
| Producer → app | **the panel document** | State that anybody publishes, agent or not. Today it arrives by file; the transport is not the contract (**D17**) |
| App → agent | **PTY** | Buttons, and everything that does not speak MCP |

**Why MCP:** Claude Code, opencode, Gemini CLI and Codex all speak it — the server is written once
and they are all served. And **it does not consume API separately**: the agent is already running
with its subscription and MCP only adds tools to that session. It is the clean channel the API could
not be (D4).

**The PTY stops being the main mechanism** and becomes the universal fallback: clean with whoever
speaks MCP, workable with whoever does not.

---

## D6 — State is published by whoever does the work

**Decided.** The app does not deduce the state by reading the screen or by parsing third-party
files.

**Why:** learned by failing. In the prototype, trying to deduce the state of a train of merge
requests from the raw data produced **three failures in a row** — what counts as «closed», which of
two entries in progress is the real one, what the accumulated phases mean. That semantics is known
only to whoever does the work.

**Consequence:** the contract carries `updated_at` and the app warns if the summary goes stale. A
summary that is only written when a step finishes lies if the process dies halfway through; the
timestamp gives it away.

### External validation — Xirp, August 2026

Spotify opened **Xirp** in public beta on 10 August 2026: a desktop app that hosts sessions of
Claude Code, Codex and Gemini CLI, born out of more than 36 000 internal sessions. It is the
strongest confirmation this decision has had, and at the same time the most precise warning about
where the differentiator really is.

**What it confirms:** they do not deduce the state by reading the screen. It is declared to them, by
*session hooks* of the harness itself — «Session hooks let Xirp detect when an agent is working,
idle, or waiting for input». Whoever has built this at the scale of thousands of engineers arrived
at the same place as D6: the state is asked for, not guessed.

**What separates the two things: it is not enough for the state to be declared, it matters who
declares it.** There are three levels and only the third answers «where has the task got to»:

| Level | Who declares | Vocabulary | What it answers |
|---|---|---|---|
| 1 — inferred | the app, looking at the painting or at the PTY | whatever the app makes up | nothing trustworthy: it is the mistake behind the three failures above |
| 2 — life cycle | the *harness* (Claude Code, Codex), through hooks | fixed: `working` / `idle` / `waiting for input` | whether the program is running or waiting |
| 3 — published | whoever does the work, in `panel.json` | its own, free text | what step it is on, how much is left, what is blocking it |

**Why level 2 cannot grow into level 3.** It is not an implementation gap that more hooks would
plug: the harness **does not know** that information. Claude Code knows that it ran a tool and that
it stopped; it does not know that a work cycle is on the `build` step, that it is two deliveries
into three, nor that it is stopped waiting for a design to be approved. With level 2 the seven steps
of a cycle look identical: `Working`. And a CI, a deployment or a script of one's own —which have no
harness— have nowhere to publish anything.

Source: [Get started with Xirp](https://backstage.spotify.com/docs/xirp/getting-started). The rest
of the comparison, in [07-landscape.md](07-landscape.md) → E3.

### Level 2 reinterpreted fails just like guessing — with someone else's data

**Superset** ([07-landscape.md](07-landscape.md) → E4) does the same as Xirp but with four events
instead of three: `Start`, `Stop`, `PermissionRequest`, `PendingQuestion`. And since its code can be
consulted, the whole failure can be seen instead of reasoned about.

Its bug [#5259](https://github.com/superset-sh/superset/issues/5259), open: the «Task complete»
notification fires **on every turn of the model**. A task with 63 turns produced **63 warnings**.
The cause, literally: Cursor Agent's `stop` hook means *«I finished this turn»*, and the app read it
as *«I finished the task»*. It had already happened to them (#1785) and the fix by *debounce*
(#1786) was closed without merging.

> **A declared event is not enough if the app puts on top of it a semantics the harness did not
> have.** It fails exactly like guessing by looking at the screen, because it is the same thing with
> more steps.

And it is not fixed with a *debounce*: the datum that tells «turn» from «task» **is nowhere in the
system**, because nobody published it. Only whoever does the work knows it.

**What this adds to the table above:** level 2 is not a safe intermediate step. Used for what it is
—«the program started, stopped, asks for permission»— it is correct and useful. Translated into task
progress, it is level 1 in disguise. The boundary is not in whether the datum comes declared, but in
whether **whoever declared it knew the question being asked of them.**

---
## D7 — Identity per pane

**Decided.** When launching a pane's shell, the app injects environment variables:

```
PANEL_ID=a3f2   PANEL_DIR=/run/user/1000/panels/a3f2
```

**Why:** it is what unties the contract from any one concrete tool. Any process running there
inherits the variable, knows where to publish, and the app knows which file corresponds to which
pane. Without this, discovery stays tied to conventions that belong to someone else (today the
prototype looks in `.claude/work/`, which is specific to `flow`).

**It is also MCP's security boundary:** the server gives the agent access to *its* pane and not to
the others. Without identity, four agents step on each other.

### Built — August 11: the exact shape, and where it departs from the example above

Implemented in F1.5. The decision does not change; what follows pins down what the example left
open, and **departs from it in two things**, each one with its reason.

```
PANEL_ID=3f2a9c14   PANEL_DIR=/run/user/1000/ivy-3f2a9c14
```

| | What was chosen | Why |
|---|---|---|
| Where | `$XDG_RUNTIME_DIR`, and the system's temporary directory when that variable is not there or not absolute — on Unix, `/tmp` when the temporary is not absolute either (**D88**) | The user's runtime directory is theirs and theirs alone, it lives in memory and **the system empties it when the session closes**. It is missing on macOS and in a Linux session without a manager, and there the temporary directory is the only alternative that exists everywhere |
| **Without the `panels/` span** | A single new component: `ivy-<id>` | An intermediate span has to be created — and it is the only piece of the path with a **fixed name**, that is, the only one another user can leave prepared in advance in a shared temporary directory, where the mode of the leaf directory no longer protects from a foreign parent. Without it, both things disappear |
| **Eight digits and not four** | 32 bits of `getrandom` | Four digits are 65 536 names. Creation **fails if the name already exists** — so uniqueness is not checked, it is obtained — and at that size a collision would leave a pane without identity. Widening it costs the same |
| Permissions | An explicit `0700` | Measured: without setting it the mode comes out of the session's mask, `0775` on the reference machine. Under the runtime directory it would not be reached, but the fallback may be a shared temporary directory, and a file that tells what the user is up to is going to live there |
| Who deletes | The value itself, when it dies | It covers three paths that a sweep at shutdown does not cover: the half-built error of whoever launches the console, the **panic** — where the interface framework does not run its shutdown hook, checked in its source, but destructors do run — and closing a pane without closing the window, which arrives in Phase 2 |
| How it deletes | The contract's file and then the directory; **never recursively** | The directory belongs to the application, but **its content belongs to the producer**. A recursive delete would carry off whatever had been left there and would turn any badly built path into data loss instead of into an error. If something that is not the contract's remains, the directory survives and it is said where |

**An inherited name does not prove that the directory still belongs to this console.** A closed
console removes its directory, while a surviving tmux or agent session keeps the identity. Under
a shared temporary directory another user could recreate that name and listen for panel documents.
Before connecting, the MCP bridge and session helper therefore inspect the directory without
following a final symlink: it must be a directory owned by the effective uid, with no group or
other permission bits (`mode & 0o077 == 0`). Terminal slash and dot components are removed only
for this metadata lookup, so `alias/` and `alias/.` cannot disguise a final symlink. The direct
OpenCode plugin repeats the same check on every reconnect attempt. Missing paths and metadata
errors fail closed, with the Rust clients naming the directory in their refusal. An inherited
identity under `sudo -E` receives no privileged exception.

This check authenticates the inherited directory before connecting; it does not eliminate the
check/connect pathname race or validate every ancestor. Arbitrary scripts publishing through the
file transport remain responsible for the paths they write. The bridge's initialize handshake
still describes a whole supported identity; validation happens when the first tool opens a channel
(**D88**). Explicit callers managing their own directories retain the low-level connector.

**What the app sets wins over what the child inherits, and without a directory they are removed.**
This is not an implementation detail: the pseudoterminal copies the parent process's environment, so
**launching the application from inside one of its own consoles** — which is going to happen as soon
as it is used daily — would pass the child console the mother's identity, and the two would write in
the same place.

With a directory they are set, and that already overrides what was inherited. **Without a directory
they have to be removed, which is not the same as not setting them**: what is not touched is
inherited, so a console without a panel would keep the identity of whoever launched it, its producer
would publish in someone else's mailbox and the mother's destructor would delete the child's file.
The inherited value is never read back; it is set or it is deleted.

**Who puts the ceiling on what is injected.** The pseudoterminal's box accepts an open list of
pairs, because it has no reason to know what a panel is; the ceiling is that **the only thing passed
to it comes out of the contract**, in the form of two exact pairs. That way, adding a variable to
the user's environment requires changing the contract's box, not a call site. It is what makes the
rule of [D4](#d4--control-through-the-console-not-through-the-api) checkable: the complete list of
what the application defines is four names, and there is a test that fails if a fifth one appears.

**If the directory cannot be created, the window opens anyway and neither of the two variables is
injected.** Same criterion as the shell's fallback, and with more reason: nothing inside the
application consumes them yet, so a console without a directory is still a complete terminal. And
**half an identity would lie** to a producer that reads `PANEL_ID` as «the app is hosting me»: it
would publish in a place nobody looks at.

**What this milestone deliberately leaves undone:** nobody reads the file — that is F1.6, and until
then the pending «discovery through `PANEL_DIR`» of the [contract](04-panel-contract.md) stays open
— and there is no sweep of orphaned directories: a sweep deletes by discovery instead of by «I made
this one myself», which is the dangerous variety of deletion.

**A warning for Phase 2:** if `PANEL_ID` becomes the token with which the MCP server authorizes, 32
bits **are not enough** and it will have to be widened there. Here it is not a token: it is a name.
The pair is an address and routing metadata only. Consumers share the same strict rule: a non-empty
UTF-8 `PANEL_ID` and an absolute `PANEL_DIR`; neither value authenticates a caller. Runtime-directory
selection is shared through `panel_contract::base_dir()` so clients do not invent local path rules.

---

## D8 — The terminal brakes the program: it stops reading, it does not discard

**Decided.** When what a program writes comes in faster than the screen interprets it, the
application **stops reading the pseudoterminal**. That fills the kernel's buffer and the program
blocks on writing, as in any terminal. What is held back has a ceiling in two places, and both of
them are needed:

| Where | How much | What happens when it fills up |
|---|---|---|
| The `Pty::spawn` channel, in `term-core` | 8 reads of 8 KiB (64 KiB) | The reader thread stops reading from the pseudoterminal |
| The bridge's queue, in `app` | 1 MiB | The thread stops collecting from the channel, and so the channel fills up |

**Why:** without this, a program that writes without stopping — `cat /dev/zero`, `cat /dev/urandom`
— exhausts the machine's memory. Measured before the fix: **+343 MiB/s of linear growth**, 512 MiB
in 1.6 s, without the slightest hint of leveling off. With the fix, the same load stays at **2.7 MiB
for 30 s** without losing drain speed.

**Why in two places and not in one.** It is measured: bounding only `term-core`'s channel **brakes
nothing**. A consumer that empties the channel as soon as something arrives makes that ceiling never
be reached, and the bytes pile up all the same, only a little further along. The rule, which holds
for any future queue of this project:

> A ceiling only brakes if whoever consumes that queue stops emptying it.

**Discarded — discarding bytes on reaching the top.** It is the easy way out and the worst one: a
terminal that loses output silently lies about what the program wrote, and this project treats
silent failure as the worst possible outcome. With the brake nothing is lost: what does not fit
stays in the program that wrote it, waiting.

**Discarded — letting whoever paints resolve it.** A per-frame budget bounds the *work* of a frame,
not the memory, and confusing the two was the mistake that led to this failure. The brake lives in
`term-core`, the box that survives a change of interface framework (D3), and is expressed in bytes,
never in frames.

**Consequence for shutdown, in both threads.** A thread waiting for room is not blocked reading, so
it learns nothing of what would normally wake it up. Each one needs its own stop flag, and for
different reasons:

- In `term-core`, `Drop` raises it **before** the `kill`, because the `join` that comes afterwards
  waits for that thread: without it, **closing the window hangs**.
- In the bridge, `Drop` raises it because whoever makes room is whoever collects, and a destroyed
  bridge is never going to collect again: without it the thread **spins forever**. It does not hang
  the shutdown — nobody waits for it — but it leaves a leak for every pane closed at that instant,
  which today is covered by the process dying whole behind it and which will stop being covered as
  soon as there is more than one pane.

Both of them checked, not feared: taking either of the two away puts its test in the red.

**Why it was not decided before.** It was not discarded: it was not looked at. The channel comes
from F1.1, where the choice was *«`mpsc` de la biblioteca estándar frente a `crossbeam-channel`
frente a un callback»* with the criterion of not adding dependencies; bounded versus unbounded never
came up, and unbounded is the default value of `mpsc::channel()`.

---

## D9 — The panel's symbols are chosen by measuring the font, not from memory

**Decision.** The contract's six marks are painted with `●` (`done`), `▶` (`current`), `·`
(`pending`), `…` (`wait`), `×` (`block`) and `›` (`info`). It is not the prototype's set, and the
difference is not aesthetic: **it is the only set of six that the monospaced font has and that
measures what a cell measures**.

**What was measured**, over egui 0.36.1's default fonts and at size 14, where a cell is 8.43
points:

| Symbol | Width | Verdict |
|---|---|---|
| `✓` U+2713, `✗` U+2717 | **0.00** | they are not in the font |
| `⏸` U+23F8 | 14.00 | only in the emoji one |
| `⚠` U+26A0, `✔` U+2714 | 17.77 | only in the emoji one |
| `✱` U+2731 | 13.00 | only in the emoji one |
| `▶` `·` `›` `→` `»` `…` `─` `●` `◆` `■` `▪` `◦` `▸` `∙` `∘` | 8.43 | they are there, and monospaced |

**Why the width and not `has_glyphs`.** Because that predicate **does not answer what its name
promises**: inside it is `resolve_face(c) != replacement_face_key`
(`epaint-0.36.1/src/text/font.rs:719`), and with the monospaced family **the base face is the
replacement one**, so it returns `false` for everything the font does have — checked with the
control letter `A`. It only returns `true` for what falls through to the emoji fonts, that is,
for exactly what does **not** serve. This milestone's first measurement was taken with it and came
out inverted: it gave as absent three symbols that are there, and as good two that throw the column
out of line.

**What was discarded, and why.**

- **Porting the prototype's set just as it is.** Two of its six are not there and two come from the
  emoji font. A panel with boxes in its first column is exactly what the verdict on egui cannot
  afford, because the boxes would be on every line.
- **Switching to the proportional family.** It does not have the fixed width that the column's
  alignment depends on, so it would fix the glyphs by breaking what the glyphs serve to build.
- **Using the emoji ones anyway.** They measure between 1.5 and 2 cells: the column goes out of
  line, and that reads worse than a box — a box looks like a font failure, a crooked column looks
  like a badly made panel.

**The way out if the set falls short** of expressiveness once it is seen in use: painting the marks
as a shape — a rectangle, a circle — instead of as a character. It is deliberately outside the
milestone and is its own piece of work.

**When it would stop holding.** If egui's version changes or if some day a font of our own is
embedded — which is the other known thing that bites, because CJK and a modern `ls`'s icons come out
as boxes. The check is tied down by a test that asks the font and fails if any symbol stops
measuring one cell, so the day it changes whoever runs the suite will find out and not whoever looks
at the screen.

> **Corrected on 28 September 2026: the boxes stopped biting with F2.9, and this set still holds.**
> Since 25 August the machine's fonts are handed to the engine (**D24**), so CJK and the icons of a
> modern `ls` come out with their own glyphs. They go in **behind** the built-in fonts, so the six
> marks above are still drawn by the font they were measured in, and the test that asks the font is
> what would say otherwise.

### What F2.8 added: the three of the claim, measured the same way

**Decision.** A pane's claim — the `attention` field of **D18** — is painted with `?` (`wait`),
`⊘` (`block`) and `∎` (`done`), and **none of the three is one of the body's six**. Measured with
the same criterion and in the same font: the three of them measure **8.43 points**, which is what a
cell measures.

| Symbol | Width | Verdict |
|---|---|---|
| `?` U+003F, `⊘` U+2298, `∎` U+220E | 8.43 | they are there, and monospaced |
| `✓` U+2713, `☑` U+2611, `⏳` U+23F3, `⚠` U+26A0, `⛔` U+26D4, `★` U+2605 | 0.00 or from emoji | out, for the same reason as always |

**And why the body's are not repeated, which also measure right.** Because the same sign in two
places of the same panel invites reading that the application **deduced the one from the other** —
which is exactly what D18 forbids: a `mark: block` on a line of the summary is not a claim of the
pane. With the set repeated, a panel that only tells that something got stuck on Tuesday would look
the same as one that is claiming you right now.

**The color table is its own, and not the marks'.** The marks' one **dims** `done` because what is
finished is where there is nothing left to do; here `done` says the opposite — it finished and
**nobody has looked at it** —, so reusing it would dim exactly what is claiming. *Discarded:*
reusing it «because they are the same names». The names coincide and the meaning does not.

**A claim that nobody holds up is dimmed; it is not switched off.** If the application has something
to confess about that panel — it has been quiet for longer than it should, or its file stopped being
readable —, the claim keeps being painted in its dimmed color: it claims, and there is nobody behind
it. **With one exception, which is in D18**: what is finished is not dimmed for keeping quiet,
because keeping quiet is what whoever finished does. *Discarded:* hiding it, which contradicts this
panel's language — confess and do not hide, which is the reason a stale producer still has a panel
—; and painting it just the same, which would make the tab's card lie about how fresh it is. The
detail — since when, or what failed — is still told by the notice, in its own line and with its own
color.

---

## D10 — The panel's width: in cells, per pane, and foldable with the same gesture

**Decision.** The panel is counted **in cells of the font** —neither a fraction of the pane's width
nor a number of points— and **belongs to each pane**: it is born with 36 and from there it is moved
by whoever is looking at the window, dragging the border that separates the panel from its console.
When not everything fits, the one that gives way is the panel: it narrows until it disappears and
the console keeps its floor of twenty columns.

> **Updated in F2.4 (August 2026).** It used to say that the width was fixed and could not be
> dragged. What still stands is what it really decided —counting it in cells, and that the one that
> gives way is the panel—; what changes is that the number stopped being the width and became **how
> many cells a pane is born with** when nobody has touched it. The three new decisions are below, in
> «What F2.4 added».

### What F2.4 added, with its reason and what was discarded

**1. The width belongs to each pane, not to the application.** Four panes are four producers
publishing things of different sizes, and the point of being able to narrow the panel is to leave
wide the one that is saying something and narrow the one that is not. *Discarded:* a global width —
it would have to give way to the minimum of the narrowest pane or lie in all the others, and the
console's floor is per pane.

**2. Folding is a state apart, not «zero width».** Folded, the pane stops **asking for** width, so
its console keeps the whole rectangle by the **same path** as one whose producer has published
nothing — there are not two ways of having no panel that somebody has to keep in agreement with each
other. *Discarded:* storing a zero in the width. It would lose the working width, and unfolding
would have to invent a value when what the user expects is to come back **to their own**.

**3. The gesture has a minimum, and that minimum is the line between narrowing and folding.** Asking
for less than one cell is what the gesture reads as «close it». And folded **a handle remains**,
stuck to the pane's right edge, facing inwards: without it, the border would go away with the panel
and the gesture that closed it could not be undone with the mouse. *Discarded:* that the handle
should straddle the edge, like that of a border with its panel in sight — that is where the divider
that tiles the consoles begins, and by registering afterwards it would win the tie and leave the
handle dead. *Price accepted and declared:* in a folded console, that band of the right edge grabs
its panel and not the console, so there it cannot be picked up to be carried away.

**And what is not painted, on purpose:** the panel's border **has no line**. Painting it would
subtract its thickness from the tiling and would move the grid of every console, with which the
measurements would stop being comparable with the earlier annexes — the comparison would be lost to
gain an ornament. What makes it findable is the cursor, and that is why its grab band is wider than
a divider's line.

**Why in cells and not in points.** The request was «ancho fijo, sin plegar», and it is met in what
it meant to say —it does not depend on the window's width and it is not dragged— but **measured in
cells**. A width in points narrows in columns when the desktop's zoom goes up, and what the panel
has to keep is **how many characters fit**, which is what decides whether its text can be read. A
panel that on one screen shows whole lines and on another cuts them is not the same panel.

> Careful when testing it: with a cell of 8 points, «36 cells» and «288 points» give the same
> number. A test written with that cell width does not tell the two implementations apart — the cell
> size has to be varied for the difference to show up.

**Why the one that gives way is the panel.** The panel is margin and the console is the work: in a
narrow window what is left over is the state, not the program. With the rule the other way round,
dragging the divider would leave an interactive program with nowhere to write while its panel keeps
its thirty-six columns.

**Why adjusting it does not come in until F2.4.** Folding and narrowing are **the same gesture on
the same border** —folding is dragging all the way to the end—, so doing one and coming back later
for the other works out dearer than doing both together. And what is needed for either of them
belongs to Phase 2: a second draggable border per pane —today only the one that tiles the two
consoles exists— and storing that width per pane so as to restore it on opening, which is F2.5.

**What is already solved for when it arrives.** The painting does not assume that the width is 36:
`set_aside()` already returns narrower panels when they do not fit, and the layout stores **for what
width** the text was wrapped and only redoes it when that width changes — that is, a panel of
variable width does not re-wrap on every frame. What is left to decide is what happens with very
small widths, where the `ref` column has to go on without pushing the text out of the panel.

**What was discarded, and why.**

- **A fraction of the pane's width**, like the tiling between the two consoles. On resizing the
  window the panel would grow and shrink with it, and with it the point where its text is cut: lines
  that were read whole would stop being readable without anyone touching anything.
- **Width in points** — above: fixed in points is not fixed in columns.
- **That the console should give way** — above.
- **Making it draggable already in F1.6.** It was Phase 2 work inside the milestone that had to
  answer whether the panel deserves to exist, and the answer to that did not depend on being able to
  narrow it. It arrived in F2.4.

**When it would stop being valid.** The matter of the four panes is already measured and answered in
the annex of this very decision, and the issue
[#33](https://github.com/mashware/ivy-history-private/issues/33) was closed with those numbers. And the
cost of the gesture itself is measured too: see «Measured — what the panel's gesture costs», below.

### Measured — what the panel's gesture costs (19 August 2026)

**Four runs with the binary that is delivered**: two of batch **K** —two panes, both dumping, and
the border of the left one's panel being dragged without stopping— and two of **C**, identical but
with no gesture. `scripts/measure-frame.sh K` and `... C`, window of 1280×800 points.

> **And they are the ones from after the review, not the first ones.** This PR's review changed the
> binary —four defects in the gesture and in what the report publishes— so the four earlier runs
> described a program that is not the one that is delivered. It is the second time this has happened
> in this project and the lesson is the same: **the batch is run at the end, after the review**.

> **The control is INSIDE each run, and that is what makes the subtraction honest.** The gesture
> only grabs the border of the **left** pane; the right one has its panel published and nobody
> touches it. So the two share frame, machine and moment, and the difference between their panel
> spans is what the gesture costs — without depending on comparing two different runs.

| Run | Gestures · effects | Panel, pane **with** gesture | Panel, pane **without** gesture | Difference |
|---|---|---|---|---|
| K, 1st | 958 · 1 918 | **35.562 µs** (107.3 at the 95th) | 12.238 µs (7.2 at the 95th) | 23.3 µs |
| K, 2nd | 870 · 1 740 | **33.721 µs** (97.3 at the 95th) | 2.821 µs | 30.9 µs |
| C, 1st | — | 4.264 µs | 2.615 µs | 1.6 µs |
| C, 2nd | — | 3.632 µs | 2.190 µs | 1.4 µs |

**What the gesture costs: between 23 and 31 µs per frame**, in the pane that receives it. Against
the 60 Hz slot that is **two tenths of 1%**; against the 120 Hz one, four. And the span covers the
three things the gesture does: serving the border, tiling the width again and **redoing the panel's
layout**.

**The range is wide on purpose, and the reason is in the table.** In the first K the pane
**without** the gesture marks a mean of 12.2 µs with a 95th of 7.2: a mean above its own percentile
is the signature of a single spike, not of a regime. With a floor that moves like that, publishing
«the gesture costs 30.9 µs» to three figures would be feigning precision.

**The neighbor does not pay for the gesture.** 2.2 to 2.8 µs in three of the four runs, with the
gesture and without it; the fourth is the spike above. Narrowing a panel costs the panel next door
nothing, which is what was to be expected of a width that belongs to each pane.

**The effects are double the gestures, and it is worth knowing why before reading the number:** 1
740 for 870 —exactly double— and **1 918 for 958, which is two too many**. **It is not a double
count** — it was probed because so round a ratio looked like one, and there is a test that pins down
the number per gesture. A gesture has two drag frames and **each one pushes the border another eight
cells**, because the point the pointer goes to is computed from where the border is *now*: the width
travels 36 → 28 → 20 and comes back with the next gesture. So there are **two real tilings and two
real layouts per gesture**, and those 30 µs already include them.

> **And the two left over have an explanation, which is what has to be said when a number does not
> add up.** The report counts the **complete** gestures, that is, the ones that got as far as
> releasing the button; the effects are counted per frame. A gesture that produced its two drag
> frames and that the window closed on before releasing leaves its two effects counted and its own
> gesture uncounted. The run of 870 ended between two gestures; the one of 958, halfway through one.

> **What this batch does NOT measure, and it has to be said:** the gesture advances in jumps of
> eight cells, so **there is no frame at all in which the border crosses a single cell**, which is
> what a human mouse moving slowly does. What is measured is the fast drag: fewer layouts, each one
> with a big jump in width. A slow drag would do more layouts each covering less ground, and that
> shape is not in these numbers.

> **WARNING, the same one the annex of the F2.3 gesture carries and for the same reason: the
> per-frame means of K and C are NOT compared with each other.** Injecting input keeps the window
> awake, so K ran 3 844 and 3 490 frames against C's 468 and 376 — eight to ten times more. It shows
> in «the rest of the frame», which in K comes out **cheaper** (144 and 135 µs) than in C (240 and
> 232 µs) simply because it is spread over thousands of frames with no console work inside. What is
> compared is what is in the table: two panes of the **same** frame.

**A blind spot that this batch makes visible.** The grid the report publishes is the one of the
**last** frame, and in a batch with this gesture the width of the left console is oscillating: the
two runs of K ended with 47 and 39 columns for the same pane. It is not an anomaly, it is the
instrument's blind spot that was already declared — and here it forces something practical: **the
run comparator rejects two K batches for having different grids, and it is right to**. Two Ks are
not averaged; they are read separately.

**The full round trip does not move:** 75.1 and 74.9 ms with the gesture, 73.7 and 74.0 without it.
It is dominated by the wait in the queue —72 of the 75— so the gesture does not show up there, nor
would it have any reason to: thirty microseconds over seventy-five milliseconds are four
ten-thousandths.

---

## D11 — Pasting belongs to the terminal, `Ctrl+V` belongs to the program, and what is pasted goes without controls

**Decided.** `Ctrl+Shift+V` pastes: the terminal writes the clipboard's text into the program
inside. `Ctrl+V` **does not paste** — it hands the program the control byte that corresponds to it,
and the program decides what it means. And everything that is pasted **has its control characters
removed**, except the tab and the line breaks.

**Why the work is split between the two shortcuts, and not left to one alone.** They are different
layers attending to each key, and that was checked by looking at what a real terminal does: in Tilix
`Ctrl+V` does not paste, but **inside that same window** Claude does paste with `Ctrl+V`, because it
receives the byte and has decided that for it that means pasting. Vim receives the same byte and
enters visual block; a shell uses it for *quoted-insert*. **Who pastes with `Ctrl+V` is not the
terminal's decision**, and keeping it would break visual block without anybody asking for it.

**The technical workaround it takes to get this**, because it is not free: the desktop emits the
**same event** for both shortcuts —checked in `egui-winit`'s source: the condition is «command + V»
and it does not look at Shift— and **it swallows the key event**. Telling them apart demands
reconstructing the state of the modifier keys by walking the events in order. Reading the field the
framework publishes **is no good**: it is a single one per pass and it describes how they were at
the **end** of the batch, not when the paste arrived.

**Why the controls are filtered, and there are two different reasons.**

1. **The paste mark can be closed from inside.** A clipboard that brings its closing sequence closes
   it on its own, and whatever comes behind enters as typed — **executing with no Enter**. Removing
   only that sequence **is not enough**: a proven attack rebuilds it by taking advantage of the fact
   that search-and-delete does not look again at what it leaves behind.
2. **And the wrapper does not protect from the kernel.** Control characters are processed by the
   line discipline, which sees the bytes **before** the program does: a `0x03` interrupts, a `0x1a`
   suspends the shell, a `0x1c` triggers a dump, a `0x04` closes the input — and with both consoles
   closed, the window. Marking the paste changes none of that. It is the reason `xterm` and `VTE`
   have filtered paste's controls for years.

**A line break comes out differently depending on whether there is a mark or not, and getting it
wrong shows on screen.** Without a mark the key has to be imitated, and Enter sends a **carriage
return**. With a mark the break goes **as it is**: the program knows it is a paste and decides for
itself. Sending it a carriage return inside the mark is putting a control character in the middle of
a text, and it inserts it literally — a box comes out in the command and the shell prints it on
executing. It was found by pasting two lines and looking.

**What it costs, and is accepted.** A text with color sequences loses its escapes and the rest stays
as ordinary text: pasting a colored dump leaves `[32mok[0m` on the line. It is the same thing
`xterm` does by default. From the terminal **there is no way to tell the control the user wanted
from the one that was slipped in**, and one whose paste can execute or suspend is worse than one
that pastes as plain text.

**What was discarded, and why.**

- **Making `Ctrl+V` paste as well.** Simpler and it removes the most delicate mechanism — but it
  takes out vim's visual block and the shell's *quoted-insert*, which are used daily.
- **Reading the modifiers from the framework's field.** It does not tell the two shortcuts apart:
  see above.
- **Removing only the closing sequence.** It escapes, checked.
- **Repeating the deletion until none is left.** It would close *that* hole and none of the
  kernel's.
- **The middle click** (the desktop's primary selection). `egui` does not expose it: its clipboard
  only knows how to read and write the normal one, and the piece underneath that does have it is
  private.
- **Copying.** It drags mouse selection along with it and, in practice, scrollback. It stays in
  F2.6.

**What you need to know so as not to get a fright.**

- **The program inside can ask for what is pasted to reach it marked**, and the terminal asks the
  engine about it. That accessor returns a `bool` and not the engine's type, so that the terminal
  crate carries on without letting its dependencies show through.
- **With a clipboard that holds no text, `Ctrl+V` sent nothing — and this note got both halves
  wrong.** The desktop swallows the key and emits no event when there is nothing to paste, so the
  control byte did not come out either. What was written here was that it is *barely noticeable,
  because an empty clipboard is rare*, and that *it cannot be fixed from here*. **Neither is true.**
  An **image** on the clipboard hits the same path, and in a terminal built to host agents, handing
  an agent a screenshot is an ordinary gesture rather than an edge — the promise this very decision
  makes was unkeepable in exactly the case that matters most. And it was fixable: **D33** carries the
  fix, a copy of `egui-winit` in the tree with that one `return` made conditional. Corrected on
  8 September 2026, with the whole autopsy in D33.
- **On macOS the two shortcuts behave differently** and this rule would break them: there the
  «command» is ⌘, so ⌘+V would give the control byte while `Ctrl+V` arrives as a key. The first
  build on that system is F2.7 and it will have to review this.
- **Pasting can hang the window** for as long as the program inside does not read its input. It is
  not pasting's doing: it is that writing to the pseudoterminal blocks with no ceiling
  ([#50](https://github.com/mashware/ivy-history-private/issues/50)). Pasting only makes it reachable.
  **Corrected on 28 September 2026: it no longer can.** Since **D29** the write goes to a thread of
  its own per console and the window does not wait for the program, and since **D60** a write
  arriving once too much is already owed is refused and said over the console, never waited for.

---

## D12 — The pseudoterminal keeps the kernel's output processing, even though it costs

**Decided.** The pseudoterminal does **not** touch `termios`: `OPOST` stays on and the kernel goes
on turning every line feed into a carriage return plus a line feed. The cost is paid. **This
decision was taken the other way round first, was delivered, and was reverted on discovering why it
cannot work** — what follows is that autopsy, which is worth more than the decision.

**What it costs, measured in this repo** with `cargo run --release --example delivery -p term-core`,
dumping twelve megabytes with `cat` through a real pseudoterminal:

| Line | Processing in the kernel | Without processing | Ratio |
|---|---|---|---|
| log, 70 B | 53.3 MiB/s | 208.2 MiB/s | **4×** |
| short, 2 B | 2.4 MiB/s | 270.0 MiB/s | **112×** |

**The issue's headline —«casi dos órdenes de magnitud»— is true and needs an adverb.** What is paid
for is **every line feed**, not every byte: with log lines it is four times and with lines of two
bytes, a hundred and twelve. A single number looks universal and is not. The example stays in the
repo even though the optimization does not: it measures a real cost that is still there.

**Why it cannot be removed, which is what had to be found out.** Turning `OPOST` off forces the
carriage return to be put back from outside, and **that requires knowing whether the program inside
wants it**. The kernel knew because it translated according to each one's `termios`; from outside it
has to be deduced, and **the information is not there**:

| Terminal state | Does the kernel translate? | Who leaves it that way |
|---|---|---|
| `OPOST` on, `ONLCR` on | yes | nobody has touched it |
| `OPOST` off, `ONLCR` on | **no** | `stty raw`, `cfmakeraw`, `tty.setraw`, crossterm's `enable_raw_mode`, Go's `MakeRaw` |
| `OPOST` off, `ONLCR` on | **no** | **and us too, if we turned it off** |

The last two rows are **the same state, bit for bit** (checked by running it). Standard raw mode
turns **only** `OPOST` off and leaves `ONLCR` set, which is exactly what a switching-off of ours
would leave. So there is no way to tell «nobody has touched anything, translate for me» from «I have
entered raw mode, do not translate for me», and every program that enters raw mode by the normal
route —any TUI of `ratatui`, of Node or of Go— would be injected with a carriage return it asked not
to have.

> **The rule it leaves, and it is a general one:** the moment you lie to the program about the state
> of its terminal, **you lose the information about who set what**. It is not that a better signal
> is missing: it is that the kernel destroys it on accepting the write. That is why no terminal
> emulator turns `OPOST` off.

**What was tried and was not enough.**

- **Making the translation conditional on `ONLCR`.** It is the signal `ncurses` uses —its manual
  says that it only uses the line feed as a movement after `nonl()`— and it **covers `vim`, `htop`
  and `nano`**, which turn it off on starting up. It does not cover anything that is not `ncurses`.
  This was the version that was delivered and reverted.
- **Comparing the whole `termios` against the one we leave.** It fails just the same: `stty raw`
  produces the very same `c_oflag` as ours.
- **Adding `ICANON` to the condition.** It fixes raw mode and breaks an `ncurses` program in
  `cbreak`, which turns `ICANON` off without turning `ONLCR` off. It swaps one hole for another.
- **Turning on the terminal engine's `LNM` mode** (`ESC[20h`), which does this in one line.
  `reset_state()` wipes it —the default mode it goes back to does not include it— so a `reset` in
  the console would leave the staircase **permanent**. And a mode is state that the program can
  trample on: putting back with a preference something that the kernel guaranteed.

**What was learned and is not thrown away.**

- **The window of emulating on reading something that was done on writing.** Even if the signal were
  reliable, the chunk that is already in the kernel's buffer would be translated with the rule of
  *now*. Bound: the ~20 KiB of the buffer; and **with backpressure they are not microseconds but
  seconds**, because the reading thread is stopped waiting for room. Measured: 17 KiB of a log in a
  staircase on opening `vim` behind a big `cat`.
- **How to measure this without fooling yourself**: generating the lines with a `while` in `sh` and
  `printf` gives 12 MiB/s with the switch set both ways, because the bottleneck is the shell's loop.
  It comes out as «1×» and buries the finding under a reassuring, false number. A file has to be
  dumped with `cat`.
- **And this application's bottleneck, while we are at it**: the kernel delivers ~358 MiB/s without
  processing and through `term-core`'s channel ~170 were coming out. If one day the pseudoterminal's
  cost really matters, there is more to be gained there than in `termios`.

**When to reopen this.** If a way appears of knowing what the program wants **without** lying to it
about the state of its terminal. Today there is none, and the issue
[#27](https://github.com/mashware/ivy-history-private/issues/27) is left with this autopsy inside it.

## D13 — Tabs are a list of layouts, and the ones you cannot see work just the same

**What was decided.** The window becomes a **list of tabs** plus the index of the one on show. Each
tab takes what belongs to a layout —its two panes, which one has the focus and where the divider is—
and the window keeps what belongs to the process: the theme, the watcher, the measurement and the
keyboard's modifiers. **What a layout is changed in F2.2**, and the border this decision draws held
up through the change: where here you read «two panes and where the divider is», there is now an
n-ary tree with weights and the focus is a name instead of a position. What stayed up above, in the
window, did not move. See **D14**. The logical half of the frame goes through **all** the tabs; the
interface half paints **only** the one on show.

**Why.** It is the first milestone of Phase 2 because it **decides the shape of the state** on which
the tree of panes (F2.2), the drag (F2.3) and the persistence (F2.5) are written. And because the
problem the project solves —several agents at once— does not fit into two consoles: a context per
task is needed.

**What was discarded, and why:**

- **Hiding the layout behind a border of methods.** It forced into the painting two functions that
  were extracted on purpose so that they could be tested **without opening a window**, and four of
  those methods existed only so as not to index. `Tab` is a **transparent container**: its three
  fields are visible from the window. The benefit of the border would be collected in F2.2, and
  bringing forward work from the next phase is exactly what the project's rules forbid.
- **Lowering the pumping budget of the tabs you cannot see.** It is incompatible by construction
  with what the milestone promises: the margin the kernel gives away before blocking the program is
  **20 KiB**, six times less than the budget of 128 KiB itself, so cutting it does not make the
  background console go slower — **it makes its program stop on writing**. The background ones are
  pumped with the same budget and the frame gets longer: the sharing-out is done by the frame
  itself, with no mechanism, and the price is paid in frames per second of the whole window.
- **Inverting the order with F2.2** (tree of panes before tabs). A tree without tabs is not worth
  anything on its own, and the order was already justified in the roadmap.
- **A series of its own in the report** for what the consoles that are not painted interpret, and a
  counter of closed tabs. Both were cut for having no consumer.
- **Closing the last tab leaving the list empty.** Asking to close does not close on the spot
  —frames go on running and the report is published on exit—, so an empty list would leave the index
  of the active one pointing out of range in every one of them. The last tab is not removed: what is
  closed is the window.

**What it cost, and it is as well that it be written down.** The report's numbers were wrong in five
different places as soon as there were tabs, and none of them was visible from outside: the frame
was closed twice when egui repeated the pass —the guard counted passes and not frames—, the second
recording clobbered the good one and wiped out what had been interpreted, opening a tab launched two
processes that nobody declared, closing it took away the time its console had already measured, and
the report's labels were renumbered on closing a tab, so that a table of numbers could end up titled
with somebody else's name. **The lesson is about the method, not about the code:** in an instrument
that shares out a total by subtraction, every new path that appears —a tab that is not painted, a
chord that creates processes— is a possible leak, and none of them is visible by reading the report,
because the number comes out plausible.

### Measured — the frame with tabs (16 August 2026)

> **These figures are from before the scrollback.** Since each console keeps 3 500 lines,
> interpreting costs double and the full round trip along with it (**D21**). The same batches taken
> again with the scrollback turned on are in the annex «the four batches of the annexes, taken again
> with scrollback» of **D21**; what follows is kept because it is what was measured that day and it
> is what holds up this decision's conclusions.

Three batches, **two runs each**, in release, without touching the machine, with
`scripts/measure-frame.sh C|F|G`. All three with every panel publishing at a rate of once per
second —two in C, four in F and G—, so that they can be compared with each other. A load of 44 MiB
per dumping console.

| | **C** — 1 tab | **F** — 2, the background one idle | **G** — 2, the background one dumping |
|---|---|---|---|
| Each console's grid | 61×21 | 59×21 | 59×21 |
| Consoles dumping | 2 | 2 | **4** |
| Full round trip | 76.4 ms · 76.4 | 75.1 · 76.9 | **90.3 · 95.1** |
| — waiting in the queue | 71.0 · 71.0 | 70.1 · 71.3 | **81.0 · 85.4** |
| — interpreting | 2.21 · 2.17 | 1.97 · 2.25 | 2.03 · 2.24 |
| — enqueueing the paint | 371 µs · 351 | 360 · 346 | 361 · 369 |
| — the status panel | 7.6 µs · 7.0 | 6.5 · 7.4 | 7.1 · 7.6 |
| **the rest of the frame** | **386 µs · 409** | **466 · 399** | **399 · 403** |

**What this proves, which is what the decision ordered to be measured:**

1. **The interface framework's cost does not move on adding a tab.** «The rest of the frame»
—tessellating, drawing and egui's scaffolding— stays between **386 and 466 µs** in the six runs, and
the variation between the two runs of one and the same batch (386 → 409 in C, 466 → 399 in F) is of
the same order as the variation between batches. Put the other way round: **the number with which D3
judges egui does not get worse for having tabs**, not even with four consoles dumping at once. 2.
**The bar comes free.** F takes two grid rows away from each console compared with C —61 becomes 59—
and the frame is indistinguishable from C's in every span. Painting the tabs' cards and sharing out
the height costs nothing measurable. 3. **What does cost is the background work, and it is paid in
the queue.** In G the full round trip goes up from 76 to **93 ms**, and **almost all of the increase
is in waiting** (71 → 83 ms). It is the budget doing its job: four panes swallow 128 KiB each per
frame, so with twice as many consoles dumping every byte waits longer for its turn. Interpreting,
enqueueing and the panel **do not move**, which is what confirms that there is no new cost per pane.

**And what it does NOT prove, which matters just as much:**

- **It does not say what it costs the background console to interpret its 44 MiB.** Its block of the
  report comes out **empty**, and that is by decision: a console that is not painted declares its
  round trip **with no wait** so as not to dirty the four means with samples of a round trip that
  nobody saw. Its time **is** subtracted from the rest —that is what the milestone fixed—, but there
  is no published number for its work. What batch G measures is the **effect** of the background
  work on the consoles in front, not its direct cost.
- **It is not a 120 Hz frame.** With two consoles dumping at full load the whole frame is well above
  the 8.3 ms slot; these numbers describe the **sustained worst case**, not normal use. The valid
  comparison is between batches, not against the budget of a refresh.
- **It says nothing about four tabs.** All of this is with one and with two. With four visible panes
  the panel would eat the whole console and it would have to be measured again
  ([#33](https://github.com/mashware/ivy-history-private/issues/33)).
- **The grid of these batches is 21 columns per console**, not 57: the panel takes 36. They are not
  comparable with the D3 annexes from before the panels.

**A field number worth keeping:** in batch G the header publishes the background tab with a grid of
**59×21**, the same as the visible one. Before this milestone it would have said **24×80** —the
start-up grid— because the size was only computed on painting. It is the check, in the real
application, that an agent in a tab you are not looking at works with the window it really has.

---

## D14 — A tab's layout is an n-ary tree that tiles by weights

**What was decided.** Inside a tab, the consoles live in a **tree**: a leaf is a pane and a branch
is an **orientation** plus a list of children **with a weight**. Four rules define it, and all four
were chosen for what they avoid:

1. **N-ary, not binary.** A branch has as many children as are needed, so three consoles in a row
are **three siblings** and each border moves only its two neighbors. 2. **Splitting in the same
direction as the branch that contains you inserts as a sibling**, instead of creating a new branch.
It is what keeps rule 1 alive after the second cut. 3. **Weights, not normalized fractions.** The
tiling divides each weight by the sum of those of its branch, so «the weights add up to 1» is not an
invariant to be maintained: closing a child is removing it from the list and the rest share it out
by themselves, in proportion to what they already had. 4. **Closing undoes what closing can
create.** Removing a leaf collapses the branch that is left with a single child, and if that child
tiles **in the same direction** as the branch outside, its children **come up as siblings** with the
weight scaled. Without this, rule 1 breaks in three gestures.

The focus is the **name** of a pane, not its place; and each border is identified by its
**position** in the frame's list, as the click already did. The chords are `Ctrl+Shift+D` (split to
the side), `+B` (split below) and `+X` (close the pane).

**Why.** Three reasons, and none of them is a matter of taste:

- **The divider that moves a block.** It is what `tmux` is criticized for most: with three consoles
  in a row, a binary tree nests, and the outer border drags **two consoles at once**. The n-ary one
  makes them siblings. It is measured in this project, not assumed: with the nested shape, a console
  that border should not touch goes from **335.6 to 504.0 points**.
- **The invariant comes free.** That what is painted covers the area **with no gaps and no
  overlaps** is guaranteed by the structure; with free rectangles it would become code to be
  maintained.
- **F2.3 comes out almost done.** Dragging a pane somewhere else is «remove» and «insert», and this
  milestone builds the two primitives anyway. With free rectangles, re-placing would be a packing
  algorithm.

**What was discarded, and why:**

- **A list of columns** (`Vec<Pane>` with weights), repeating the trick of `Vec<Tab>`. It would be
  the shortest one and it **does not meet the milestone**: it only gives one row, and Part I of the
  roadmap asks for «placed however you like». It would have to be redone in F2.3.
- **The plain binary tree**, because of the divider that moves blocks.
- **A grid of rectangles in fractions**, which loses the invariant that the tree gives away.
- **A ceiling on the number of panes.** It cannot be justified with a real scenario: splitting
  demands a human gesture per pane, and the window becomes unusable through geometry —the floor of
  columns refuses to split what it cannot give both children their minimum of— long before
  exhausting memory (~1.06 MiB of queues per pane) or descriptors. The limit comes out of the
  geometry, not out of an invented constant.
- **A stable name for each branch.** It was justified by saying that egui's widget identifier has to
  be stable between frames, and the precedent contradicts it: the click identifies each pane by its
  index and had been working for a whole milestone. **One known case remains**: with the mouse held
  down on a border and the splitting chord pressed with the other hand, the split renumbers the
  borders and the drag in flight switches to moving another one. It takes two hands, the effect is
  visible and it is corrected by releasing. It is reopened in F2.3, which is where re-placing goes
  back to moving positions live.
- **A guard «if the sum of the weights is zero, tile in equal parts».** It leaves the invalid state
  alive and trusts that every path goes through the guard. In its place, the `Weight` type rejects
  the non-finite and the non-positive, so the sum of zero **cannot be constructed**; and dragging
  does not let any neighbor go below 1% of the sum of the two. That floor is relative to the two
  neighbors and not to the area, so it does not bake today's geometry into the state.

  **A correction, made while implementing F2.5, and it needs writing down because the paragraph
  above reads as if the 1% were a floor and it is not.** The one per cent is of **the fraction**, so
  the weight a drag leaves is one per cent of what the two neighbors add up to — and that sum
  **shrinks with every drag**. Dragging every border of a row to the stop, the weights converge on a
  fixed point where each one is a hundredth of the next: **with twenty-two consoles in a row, the
  smallest is worth 10⁻⁴¹**, and from there the tiling —which multiplies two weights— reaches zero
  in a single step. Measured, not reasoned. So **the rescaling carries a floor of its own**
  (`Weight::scaled`): the three places where a weight comes out of another one —dragging, splitting
  and the rescaling of closing— clip the result to the smallest normal weight. The impossible state
  stops existing where it is produced, which is what this project prefers to a guard downstream. And
  the consequence that was discovered along with this: **the on-disk format cannot demand a larger
  minimum**, or it would reject layouts that a gesture produces — see D16.
- **Copying another terminal's chords for splitting.** It cannot be done: **they do not agree with
  each other** — what in one splits to the side, in another splits below. They were chosen for what
  they mean here (*right*, *below*). For closing there is a precedent that does not contradict
  itself —the `x` of `tmux` and of Zellij kill the pane—, so closing the pane is `X`. **`W` stays
  with the tab**, which is what it does in any browser, and none of the three touches `C` or `V`,
  which belong to the clipboard. All three carry a repeat guard because each one launches or kills
  programs: without it, a second with the chord held down closes the consoles, the tab and the
  window.

**What it cost, and it is as well that it be written down.** The milestone was delivered in seven
PRs and **what it broke was not what was being built**: the PR that taught it to **split** closed
fourteen blocking findings and **thirteen were in code that PR did not write**, and the one that
taught it to **close** closed three, all of them outside its own. What breaks is **what the new
operation makes reachable** — three panes where the report indexed an array of two sides (a panic on
exit, that is, losing everything measured), two sibling branches where the border search cut on
«changed» instead of on «found», a tab with a single console where the text «left» stopped
describing anything. And in the instrument, the same lesson in different clothes: **what a console
carries undeclared when it closes lives in three places** —what was recorded in its measurement,
what the pumping left interpreted and what it cost to re-read its panel—, and looking only at the
first declared **zero** in precisely the frame of the chord. Every new path is a possible leak, and
none of them is visible by reading the report: the number comes out plausible.

### Measured — the frame with four panes (17 August 2026)

> **These figures are from before the scrollback.** Since each console keeps 3 500 lines,
> interpreting costs double and the full round trip along with it (**D21**). The same batches taken
> again with the scrollback turned on are in the annex «the four batches of the annexes, taken again
> with scrollback» of **D21**; what follows is kept because it is what was measured that day and it
> is what holds up this decision's conclusions.

Three batches, **two runs each**, in release, without touching the machine, with
`scripts/measure-frame.sh C|H|I`. All three with **every** panel publishing at a rate of once per
second and a load of 44 MiB per console, so that they can be compared with each other. The spans are
those of **pane 0** of each run.

**The window is asked for, and that is new.** All the runs are of **1280×800 points**, asked for
with `MEASURE_SIZE` and checked in the report. Before this milestone the size was decided by the
desktop, and that made the measurement irreproducible without anything saying so: on the same day,
three runs of the same batch gave grids of **36×47, 40×66 and 61×57**. That is why **these numbers
are not compared with the earlier annexes** —theirs were taken with windows whose size nobody wrote
down— and are compared with each other.

| | **C** — 2 panes | **H** — 4 in a row | **I** — 4 in a square |
|---|---|---|---|
| Each console's grid | 49×39 | **49×20** | **24×39** |
| Cells in total | 3 822 | 3 920 | 3 744 |
| Consoles dumping | 2 | **4** | **4** |
| Full round trip | 52.0 ms · 51.6 | **100.2 · 101.4** | **97.2 · 99.4** |
| — waiting in the queue | 46.5 · 46.2 | **89.9 · 91.1** | **87.2 · 89.3** |
| — interpreting | 2.12 · 2.13 | 2.18 · 2.07 | 2.20 · 2.20 |
| — enqueueing the paint | 458 µs · 464 | 316 · 320 | 259 · 256 |
| — the status panel | 6.1 µs · 6.5 | 15.4 · 18.4 | 25.8 · 8.9 |
| **the rest of the frame** | **444 µs · 402** | **530 · 513** | **503 · 463** |

**What this proves, which is what was left to measure:**

1. **The interface framework's cost does not get worse on doubling the panes.** «The rest of the
frame» —tessellating, drawing and egui's scaffolding— stays between **402 and 530 µs** in the six
runs, and the variation between the two runs of one and the same batch (444 → 402 in C, 503 → 463 in
I) is of the same order as the variation between batches. It is the same conclusion the D13 annex
drew for the tabs, now for the panes, and it **closes the question the issue
[#33](https://github.com/mashware/ivy-history-private/issues/33) left open**. The three batches have
almost the same number of cells in total (3 744 to 3 920), which is exactly what makes them
comparable: what changes is how many consoles they are shared out among. 2. **What the background
work costs is paid in the queue.** The full round trip goes from ~52 ms with two consoles dumping to
98 and 101 ms with four, and **almost all of the increase is in waiting** (46 ms in C, 88 in I and
91 in H). Interpreting **does not move** (2.07 to 2.20 ms in the six runs), which is what confirms
that there is no new cost per pane: it is the budget of 128 KiB per pane doing its job, with twice
as many consoles swallowing from the same frame. 3. **Enqueueing the paint goes down, and it goes
down with the cells.** 458 µs with 1 911 cells per console, 316 with 980, 259 with 936. It is not an
improvement: it is that each console paints less. 4. **The panel gives way and the console does not,
checked in the worst case.** What is **measured** is this: with the four **in a row** each console
publishes **20 columns** of grid —exactly `panel::MINIMUM_CONSOLE`—, and in a **square** it
publishes the **39** of the batch of two. And from there the tiling is **deduced**, which the header
does not publish: in batch C each pane is 39 columns of console plus the panel's 36, that is 75; in
a row it gets half of that, some 37, so the panel does not fit and **gives way down to 17** to leave
the console its floor of 20. In a square what is halved is the **height** —24 rows instead of 49—
and the width is not touched, so the layout somebody would really use does not take a single column
away from the console.

**And what it does NOT prove, which matters just as much:**

- **It says nothing about a large window.** On this machine the desktop lets you get as far as **5
  120×1 357 points** —checked by asking for more and seeing what it grants—, and the batch measures
  1280×800. With four in a row across the width of that window the panel would fit whole and
  conclusion 4 would be another one: the interesting one is the one that was measured, because it is
  the one that squeezes.
- **The panel's span is microseconds and its ratio between runs means nothing**: 25.8 and 8.9 µs in
  the two runs of the same batch I. It is published for completeness, not so that trends can be read
  into it.
- **It does not measure splitting or closing live.** The batches open the layout and dump; the ~100
  ms freeze on closing ([#62](https://github.com/mashware/ivy-history-private/issues/62)) is still
  unmeasured with panes, and this milestone goes down that path far more often than before.
- **It is not a 120 Hz frame.** With four consoles dumping at full load the full round trip is well
  above the 8.3 ms slot: it describes the **sustained worst case**, not normal use. The valid
  comparison is between batches.

**One run was marked invalid while the script was being tuned, and it is as well that it be written
down why.** A batch I published a window of **1 279.8125×799.8125** points having asked for 1
280×800: the compositor delivers the size with a fraction and **not always the same one**. The
script's guard threw it out with an exact comparison, and now it carries **half a point of margin**
—the same one the application uses so as not to fight with the desktop—, because half a point does
not move a single cell of the grid and without it the guard discards runs that are identical.

## D15 — Moving a pane transforms the tree by value, and only replaces it if the transformation reaches the end

**What was decided.** A pane is **grabbed with the mouse** and **dropped to one side of another**;
on dropping, the layout is rearranged to make room for it. Five decisions define it:

1. **The operation is done by value and is atomic by construction.** `Nodo::mover` receives the
   whole tree, transforms it and the original is only replaced **if the transformation reaches the
   end**. There is no path at all in which a console ends up alive and outside the tree.
2. **The destination is one of the four halves of another pane**, and which one is chosen by the
   distance to the edge **in proportion to the side it belongs to** —cross-multiplying, not
   dividing—. In points, a wide, short console would only let itself be dropped on from above and
   from below.
3. **Before dropping, the half the console is going to occupy is visible**, and not a thin strip at
   the edge: it is literally the place it will occupy. A strip would read as the window managers'
   gesture for snapping to the screen, which does not exist here.
4. **The veil does not promise what is going to be refused.** The geometric floor —the same one
   splitting already uses— is consulted **before** pointing at a destination, so where dropping is
   going to be denied nothing is marked.
5. **Panes are identified by their name, and so is the identifier with which egui holds the drag.**
   Dividers stay by position.

**Why.**

- **Composing «close» with «split» does not give atomicity, and not through an implementation
  oversight.** `colapsar` discards the surviving child's weight and `absorber` scales the
  grandchildren's, so **once a leaf has been removed the previous shape can no longer be
  reconstructed**: if splitting then failed, there is nowhere to go back to and the console is left
  outside. By value, failing means having changed nothing.
- **And an operation that mutates in place with prior validation would also work — as long as
  everyone remembers to validate.** This project has counted with numbers what happens to invariants
  that depend on somebody remembering: 15 of the 17 blocking findings of the two previous milestones
  were in code those PRs did not write. Transforming by value makes it **impossible to break**.
- **The floor is checked against the tree as it is now**, and it is conservative: removing a leaf
  almost never takes area away from the ones that remain —`absorber` costs them one divider, and
  nothing more—. The intuitive formulation —«validate against the tree that comes after removing»—
  **is incoherent**: you cannot validate against a tree that does not exist yet.
- **Identity was not refactored, and with this the case D14 left open for this milestone is
  closed.** The leaves already had a stable name; what was missing was for the control's identifier
  to carry it **too**. With the slot inside it, anything that renumbers while the button is still
  held down makes the in-flight drag point at something else — and two hands are not needed:
  grabbing a console and pressing `Ctrl+Shift+→` without letting go **rearranged two consoles of the
  destination tab**. Dividers stay by position because their drag **survives nothing**: it is
  resolved within the same frame.

**What was discarded, and why:**

- **Composing `cerrar` + `partir`** from the window: they are not atomic, for what is above.
- **Mutating in place with prior validation**: it works, but atomicity comes to depend on every
  future path.
- **Closing the console and reopening it at the destination**: it kills the work inside, which is
  the whole reason for the milestone. And it is measured: closing a live console freezes the window
  for a median of **50.72 ms**, **404 ms** if the program ignores `SIGHUP`, and it can hang it
  indefinitely.
- **`Result<Nodo, Nodo>` for the result of moving.** That was the design; it was changed to a tuple
  because the error variant carried the whole tree and because `Err` did not always mean the same
  thing. A tuple does not invite reading «it failed» as «nothing changed».
- **Undoing a move.** Not needed: a move that cannot be completed never gets to start.
- **Moving a pane to another tab.** Out of F2.3 by Part I of the roadmap, not by difficulty.
- **A stable name for the dividers.** See above: their drag does not cross frames.

### Measured — the frame while dragging (18 August 2026)

Two batches, **two runs each**, in release, without touching the machine, with
`scripts/measure-frame.sh C|J`. Both with one tab of **two panes**, both panels publishing at a
rate of once per second, a load of 44 MiB per console and a window of **1280×800 points**, with both
consoles at 49×39 cells. The **J** adds a single thing: **the window drags itself** from beginning
to end —it grabs a console, carries it over the other one and drops it, alternating the side—
through the same door the real mouse comes in by. The spans are those of **pane 0**.

| | **C** — no gesture | **J** — dragging non-stop |
|---|---|---|
| Injected gestures | — | **1 724 · 1 722** |
| Of those, the ones that rearranged | — | **862 · 861** |
| Frames with output | 358 · 363 | 357 · 357 |
| **Frames in total** | **467 · 532** | **6 898 · 6 888** |
| Full round trip | 37.2 ms · 37.1 | **37.3 · 38.5** |
| — waiting in the queue | 35.0 · 35.0 | **35.1 · 35.6** |
| — interpreting | 849 µs · 763 | 813 · 1 058 |
| — enqueueing the paint | 205 µs · 189 | 163 · 234 |
| — the status panel | 3.6 µs · 3.6 | 2.6 · 4.6 |
| the rest of the frame | 222 µs · 220 | 112 · 105 |

**What this proves:**

1. **Dragging non-stop makes nothing the console perceives any worse.** The full round trip —how
   long an available byte takes to reach a paid-for frame— is **37.1 to 38.5 ms across the four
   runs**, and the variation between the two runs of the same batch (37.3 → 38.5 in J) is of the
   same order as the variation between batches. Waiting in the queue does not move either: **35.0 to
   35.6 ms**. And the frames **with output** are the same (357 to 363), which means the pumping
   budget does its job just the same.
2. **The gesture declares its cost.** Attending to the mouse, painting the veil, redoing the tiling
   on dropping and injecting the events are **all** timed and subtracted, so none of them is
   published as the framework's cost —the figure D3 judges egui by—. The review found a fifth piece
   that had been left out: copying the last frame's rectangles. It was not timed: it was
   **removed**, by moving the vector instead of copying it, which is better than declaring work that
   does not need doing.
3. **The rest of the frame stays well below the budget** even while dragging: 112 and 105 µs on
   average, 258 and 302 µs at the 95th percentile.

**And what it does NOT prove, which matters just as much:**

- **The «rest of the frame» means of C and J are NOT compared with each other, and the table puts
  them together only so that it can be seen why.** Injecting pointer events keeps the window awake
  —just as a human mouse moving keeps it awake—, so J runs **some thirteen times more frames** than
  C. The mean cost per frame goes down because it is shared out among thousands of frames that carry
  no console work inside them, not because the gesture makes anything cheaper. Reading «112 µs
  against 222» as an improvement would be exactly the plausible number this project has spent three
  milestones avoiding. The same goes for interpreting, enqueueing and the panel. Conclusion 1 rests
  on the full round trip, which is per frame **with output**.
- **Half the gestures are discarded on dropping, and the report says so: 1 724 injected and 862 that
  rearranged.** It is not a failure of the gesture: the injector grabs the console **by its slot**,
  so after the first swap it grabs the same one again —which is now in the other slot— and drops it
  on top of itself, a case the gesture rejects on purpose. The batch therefore measures a
  **mixture** of the two paths, and both of them are real when dragging. Probed with instrumentation
  in the dropping path, not deduced.
- **Publishing only the injected gestures would have let an empty batch through**, and this the
  review found: in a narrow window the geometric floor rejects **every** drop, the layout does not
  change even once, and a run like that declared «1 724 done» and was taken as good. The annex would
  have published that the gesture costs nothing. Now the script invalidates the run if none of them
  rearranged.
- **The «(left)» or «(right)» label of each span table describes the slot the console ended up in**,
  and in batch J one console changes slot halfway through the run: half the samples were taken with
  it on the other side. The numbers do not come out wrong because the two consoles share the width
  equally and have the same grid, but the label asserts a provenance the run does not support, and
  it would stop being harmless in a batch of different widths.
- **There is no batch with drags of four panes**, and it is not an oversight: the gesture's cycle
  only returns the layout to the starting one **with two**. With more, inserting splits the
  destination's weight and removing shares it out among the ones that remain, so the weight
  concentrates — measured with four in a row over 1 200 points, the first console goes from **298.5
  to 119.4** points in a single round-trip cycle.
- **The frame count of batch J is not stable, and with it neither is the gesture count.** Three
  consecutive runs gave **6 898, 6 888 and 6 854** frames —very close— but a fourth, taken while the
  machine was compiling, stopped at **764** with only 191 gestures. The frames **with output** were
  the same in all four (357 to 359), which is what makes conclusion 1 hold up: it rests on the full
  round trip, which is per frame with output, and not on anything divided by the total. It is
  another reason not to compare the means of the rest between runs, not only between batches.
- **It does not measure a person's gesture.** It measures a synthetic gesture that grabs the center
  of a console, crosses in two frames and drops. A human drag lasts longer and moves more slowly.
- **It does not measure splitting or closing live**, just like the previous annex: the ~100 ms
  freeze on closing ([#62](https://github.com/mashware/ivy-history-private/issues/62)) still goes
  unmeasured with panes.

**Two things the first version of the batch got wrong, and they stay written down because the
numbers came out plausible.** The first: the gesture asked for a repaint on every frame so as not to
fall asleep halfway through a drag, and with that the batch stopped being «the comparison one plus
the gesture» and became another regime —**2 219 and 7 008 frames** with the rest at **163 and 103
µs**, that is, the gesture *making the frame cheaper* by half—. The second: counting the injected
gestures as if they were rearrangements, which is the one that would have let the empty batch above
through. Both were caught by this PR's review, not by the measurement.

---

## D16 — The layout is saved in a file of its own, and what is not understood opens factory-fresh

**What was decided.** On closing the window and opening it again, back come **the tabs, the tree of
panes with their weights, and for each console the width of its panel and whether it was folded**.
It is saved in a JSON file of its own, `$XDG_CONFIG_HOME/ivy/layout.json` —falling
back to `$HOME/.config/…`—, and **nothing else**: not each console's working directory, not the size
of the window, not which tab was active. Nine decisions define it:

1. **A file of its own, and a single one for the whole machine.** Not one layout per launch
   directory: that would bring in «the project» as a concept and no phase has defined it.
2. **The path has to be absolute, or nothing is persisted.** Only an environment variable that is
   **non-empty and absolute** is accepted; if none resolves, **nothing is read and nothing is
   written** and the window opens factory-fresh. **There is no fallback to a temporary directory**,
   unlike what the panel contract does.
3. **It is written with a 500 ms delay, counted from the first unwritten change**, not from the
   last. And **two** references are needed, with different roles: the deadline is **armed** when the
   layout differs from the **last one seen** —that is, when something changes—, and on expiring it
   is **written** if it differs from the **last one written successfully**. With only one, neither
   of the two ends works: comparing always against what was last written, a non-writable directory
   leaves the difference standing and the window never falls asleep; and saving the failed attempt
   as if it had gone well, **a failure is never retried** if the layout does not change again, and
   everything since the last good write is lost instead of half a second.
4. **The write is atomic**: a temporary is written in the **same directory** as the destination
   —`layout.json.<pid>.<attempt>`, so that two instances do not tread on each other's
   temporary, and **created exclusively**: if that name already exists it **is not adopted**,
   because the temporary is what gets renamed over the destination and a link planted there would be
   written through— and it is renamed over it. The attempt number was added while implementing, and
   because of a scenario this decision had not seen: **the process number is recycled**. A temporary
   that survived a violent death —between creating it and renaming it— is cleaned up by nobody, so
   with the name depending only on the process, **every future session the system gave that number
   to would be left unable to save anything**, forever and with a single warning that whoever opens
   from an icon does not see. Counting the attempts, an orphaned temporary gets in the way of **one
   attempt** and not of a session; and none is adopted, which was the point. The destination never
   holds a half-written document. Permissions `0600` on the file and `0700` on its directory. If the
   rename fails, the temporary is deleted and a warning is given **once per session and per
   reason**; what retries is the next change. **Amended after #185: the next change retries only
   what it could fix.** A refusal nothing in the session can lift —a newer version on disk (item 7),
   a session forbidden to overwrite, no absolute path (item 2)— ends the attempts after the first one,
   which is the one that warns. Retrying those cost the whole copy, comparison and validation on every
   change, twice a second while dragging, for a write that could only succeed after a restart. The
   disk's own failures and a layout that does not fit keep being retried, because a disk empties and
   closing consoles makes a layout fit. Discarded: deciding from the veto before the first attempt,
   which silences the warning and leaves a measurement batch —always forbidden to overwrite— with no
   attempt for the script's guard to see.
5. **A restored console starts a fresh identity**: new `PANEL_ID` and `PANEL_DIR`, an empty panel
   until somebody publishes.
6. **Tolerance per document, not per field.** What is not understood is discarded **whole** and the
   window opens factory-fresh —one tab, one console— without preventing startup.
7. **The format version is mandatory.** One **higher** than the known one **is not overwritten for
   the whole session**. One that is **known and older** is read with that version's reader and
   rewritten in the new one on the first save — and **retiring support for an old version is a
   separate decision, and it gets written down**. Without that second rule, a file of the previous
   version would fall into «invalid shape» and be clobbered, which is the exact damage the version
   exists to prevent.
8. **Two ceilings and no more**: **64 KiB** of file —because it has an arbitrary size, and reading
   it with no cap is reserving with no cap at startup— and **128 leaves** of document, checked **by
   walking the tree before the first `fork`**. And **if a `fork` fails halfway, nothing is
   installed**: the window opens factory-fresh, the same atomicity as D15 — half a restored window
   is a state nobody has designed. In the document, **leaf and branch carry an explicit label**
   (`hoja` / `rama`), they are not deduced from which key is present: that way the failure says what
   and where.
9. **The file saves geometry and never instructions.**

**Why.** Today the window always opens the same, so all the work of F2.1 to F2.4 —tabs per context,
as many panes as you like placed as you like, each panel with its own width— **is lost on every
close**, and arranging the window is a gesture that has to be repeated every morning.

And the decisions above are not tastes. Each one has something already measured in this project
behind it:

- **The delay is 500 ms, and the number is not at random**: it is the same as the breather with
  which the window rereads the panels when the watcher has gone blind, which is already in the code.
  With it, a continuous drag writes **twice a second** instead of sixty, and what is lost on dying
  —half a second of gesture— is not noticed.
- **The delay is counted from the first change and not from the last because, counted from the last,
  it never writes while the user drags.** The layout changes **over and over while the button is
  still held down**: moving a divider shares out weights as soon as the pointer moves, and the
  panel's border as soon as it crosses a cell —its width is counted in whole cells, so it does not
  change on every frame, but it does many times per gesture—. Rearming the deadline on every change
  pushes it for as long as the gesture lasts: a ten-second one is lost entirely. Counted from the
  first, that same gesture writes once per delay.
- **The write is atomic because there is no guaranteed clean shutdown.** The application is closed
  by the window manager or by a signal, and **closing one console freezes the window for a median of
  50.7 ms** —a tab of two consoles, around 101 ms; and **404 ms that same console** if the program
  inside ignores the hang-up signal— and it can hang it
  ([#62](https://github.com/mashware/ivy-history-private/issues/62)). Saving only on the way out would
  have lost the whole session in the normal case. **The «can hang it» half stopped being true with
  D32**, which bounds the wait; the freeze this annex measures is the other half, and **D55** took it
  from ~50 ms per console to 1.20–1.37 ms.
- **The identity is new because keeping it would be deducing the state.** A panel restored with
  yesterday's `panel.json` would paint **as current** the last state of an agent that no longer
  exists: «compiling» from a build that died last night. The app does not deduce the state (**D6**),
  and this would be deducing it by omission.
- **Tolerance is per document because this file is always written by the application.** It is the
  opposite of what the panel contract does, and on purpose: that one is written by a third party by
  hand and losing the whole panel over a field nobody looks at would be the worst possible thing;
  here **guessing half a layout is worse than not restoring it**.
- **The version goes in on day one because the file freezes the shape of the tree on disk.** Without
  it, the first change of shape puts the old file in the same drawer as the corrupt one and
  **deletes it in silence**. It only goes up with what an old reader would misinterpret; adding an
  optional key does not raise it. And a higher version is not overwritten because having two
  binaries on the machine is not unusual, and clobbering the file would mean the old one taking the
  new one's layout with it.
- **The leaf ceiling exists even though D14 discarded a ceiling on the number of panes, and it does
  not contradict it.** Splitting demands a human gesture and **the geometry refuses before creating
  anything**; restoring forks N shells **before any geometry exists** —on opening, the size of the
  window is not known—. The floor that protects splitting is **downstream of the `fork`**. It is a
  ceiling **on the document**, not on panes.
- **And the leaf ceiling is not derived from any particular geometry**, because there is none: it
  comes from the byte ceiling letting **on the order of a thousand leaves** through —between six
  hundred and a thousand depending on how compact the document comes—, and **the number is chosen
  against the resource the ceiling guards, not against what fits on screen**. Every restored leaf is
  a process, a pseudoterminal, its descriptors and ~1.06 MiB of queues: a thousand leaves would be
  more than a gigabyte of queues **before any geometry exists**, that is, the ceiling turned into
  the disaster it came to prevent. **128 leaves** are some 136 MiB, a bounded and declared number,
  and they are still above the **order of a hundred consoles** that fit in the window used for
  measuring —seven columns of twenty cells by fifteen rows of three, with a body of 14—, which is
  the only window this project has a written size for. **No exact figure of leaves per byte is
  published, and that is on purpose**: it depends on the shape of the document, not on its size.
- **The file does not save instructions because that is where the sensitive boundary lies.**
  Launching a console stops being harmless «as soon as the program or its arguments come from
  **configuration**», and this file is configuration. It saves geometry; the type it is read into
  **does not have a single free-text field**, and restoring calls the constructor that launches the
  user's shell, never the one that takes a program by parameter. The day any of that changes,
  launching has to be reviewed as a sensitive surface and treated like a `panel.json` of foreign
  origin.

**What was discarded, and why:**

- **The persistence store the interface framework brings.** It gives away **the path of the file and
  the cadence of the saving** —the two things this milestone decides— and puts the product's state
  inside the piece **D3** declares replaceable. It would also require **turning on a feature that is
  not asked for today** —it does not come as standard— and that drags into the tree a serialization
  format and a directory resolver nothing else needs.
- **Saving the log of gestures that built the tree** instead of the tree. Tempting because the
  operations already reject what does not fit, so validating would come out free. **It does not
  cover half the milestone**: the panel's width and folding are not born of splitting or of moving,
  they live in the console (**D10**), so the per-console mirror is needed all the same.
- **Tolerance per field**, for what is said above.
- **A ceiling on the tree's depth.** The JSON interpreter already cuts off at 128 levels, and **the
  option that turns that cut-off off is not asked for in any manifest of the repo**, so a ceiling of
  our own **would pass its test by construction** — the disguise this project already has on file.
  And with the invariant «a branch has two children or more», the ceiling of 128 leaves already
  bounds depth, nodes, tabs and children per branch: **one number does all four jobs**.
- **A cap on the panel width read in.** The tiling already clips it and there is a test that pins it
  down.
- **A guard against the not-a-number value in the weights.** JSON has no literal for it, and the
  weight's type already rejects the non-finite and the non-positive, so **a zero sum cannot be
  constructed**.
- **A minimum of our own for the weights read in.** There was one —`1e-6`— and **it was a defect,
  not a guard**: it rejected layouts **that a gesture produces**. Dragging clips the *fraction* at
  one per cent, not the weight, so what it leaves is a part of what the two neighbors add up to, and
  that sum shrinks with every drag; with a long row you go below `1e-6` without effort, and **the
  column floor means nothing shows on screen**. That is: the window looked fine, the file was
  written, and on reopening it was discarded whole with a warning that whoever opens from an icon
  does not see. The reason that minimum existed —that the tiling multiplies two weights and the
  product can underflow— **is true, and its place is the rescaling**, not the reader: see D14's
  correction. **The cap from above stays**, because a hand-written file can indeed bring a weight no
  gesture produces.
- **Forcing the write to disk before renaming.** The guarantee that is needed is «half a file is
  never seen», and the rename gives that on its own; surviving a power cut was asked for by nobody,
  and that push inside the frame is milliseconds on a busy disk.
- **Deleting the previous session's panel directory.** On the normal branch it lives under the
  user's runtime directory, which **the system empties on logging out**, so there is nothing to
  clean up and building the cleanup would be a mechanism with no scenario. **Where there is no such
  directory** —macOS, or Linux without a session manager— **it falls back to the system's temporary,
  which is NOT emptied**: the type's own destructor says so, and it warns precisely because of that.
  And that destructor **does not run when the application is killed or closed by the window
  manager**, which is the shutdown mode this decision leans on further up. So on that branch **empty
  directories pile up**, and that goes written below as a limit instead of being papered over with a
  cleanup the normal branch does not need.
- **An environment variable of our own so that the measurement batches do not touch the real file.**
  It is enough for the script to point the configuration directory variable at the run's own:
  nothing else in the code reads it. One variable fewer and one contract fewer.
- **Saving the size and the position of the window.** Restoring them would hand the size back to
  something that changes between sessions, and out of that came **three different grids on the same
  day**, which is what makes incomparable the measurements D3 lives off. It is worth not confusing
  it with what already happens: **today there is no default size** —if nobody asks for one, the
  desktop chooses it—, and the 1280×800 belongs to the measurement batch, not to startup.
- **Saving each console's working directory.** Knowing where a child process is, is read from
  Linux's virtual file system and does not exist the same way on macOS or on Windows: it would bring
  forward F2.7's work or open up one path per platform. And it would be **a route out of the file
  that ends up at the launch**, that is, the line above.
- **Saving which tab was active and which console had the focus.** It opens on the first one.

**What the milestone adds to the measurement report, and why it is decided here and not while
implementing it.** The frame's cost is shared out **by subtraction**, so all work that is not
declared is published as the framework's cost — the exact figure D3 judges egui by. This milestone
opens two paths inside the frame and both of them are declared:

- **Comparing the window's mirror with what was last written**, which runs **on every interface pass
  and over the whole window**.
- **Writing the file** when the deadline expires. Its stopwatch covers **from copying the layout
  until the rename returns** —or until the temporary is deleted, if it failed—: the copy, the
  validation and the serialization go inside, not just the call that writes. **It was made precise
  while implementing**, because the earlier text said «from resolving the path» and with a hundred
  consoles the copy and the validation walk can dominate the sample: a badly delimited span ends up
  attributed to the disk.
- And the apportionment between the two: **what never reaches the disk is not a write**. A deadline
  that expires with nothing new to save, and a layout the format does not admit, are counted in the
  mirror's span, which is what they are —comparing and validating—. That way the write's span
  describes **only** what writing costs, and the report can publish «so many writes out of so many
  attempts» without mixing. **A session that can never write makes one attempt** (item 4, amended
  after #185), so in a measurement batch —always forbidden to overwrite— the write series holds a
  single sample and no deadline is armed after it. **So the drag batches stop paying the copy and the
  validation once per deadline**: that work was timed separately and subtracted, so the framework's
  figure is untouched, but a batch's total frame after #185 is not comparable with one before it —
  and neither ever measured what a session that **can** write pays, since serializing and renaming
  were always vetoed.

**And each one needs TWO things, not one, because subtracting and publishing are different
mechanisms.** The rest of the frame is worked out by subtracting from what it cost as a whole **what
the spans declared**, and what feeds that subtraction is each pane's spans plus the window's
accumulator. **A series only keeps samples in order to print them: it subtracts nothing.** So a span
with a series and no accumulator ends up published and **undiscounted** —its cost still shows up as
the framework's cost—, and a span in the accumulator and with no series ends up discounted and
**with no row to audit**. The two new paths go **into the accumulator and, on top of that, with a
series of their own**.

**And the precedent of the panel's span is not inherited, and it is worth saying so:** that one
travels **inside a pane's spans**, which is a field per pane; the mirror is of **the whole window**
and does not have that vehicle. Whoever reads «like the panel» will do what the panel did and will
be left without the subtraction.

**The mirror's, moreover, accumulates per pass and records a single sample when the frame closes**:
recording it per pass puts two samples into the frame egui repeats, and then the mean describes «per
pass» and not «per frame».

And **reading the file at startup does not go into any frame**, on purpose: it is interpreted before
the window exists. There is no span to declare because there is no cost to attribute.

**And the batch has to be able to give itself away, with the sharing out this project already uses:
the report declares, the script invalidates.** The **report** publishes how many times the file was
written —that number lives inside the process and the script cannot know it—, and the **script**
invalidates the run if none was written, just as it already invalidates the one that published no
panel or the one that completed no drag: without that number, an empty series reads as «this costs
nothing». The script also points the configuration directory **at the run's own and deletes it at
the start of each one**, or the next run starts with the layout the previous one left. And **what
the batch asks for explicitly rules over what was restored**: if the run asks for two tabs and four
panes, that is what it opens, even if the file says otherwise.

**Two consequences of that, and both have to be written down because otherwise they are discovered
late:**

- **The saving deadline is a new reason for the window to wake up**, and that changes **how many
  frames a run has**. D15's annex already warns that a batch with more frames **does not compare its
  per-frame means**; this milestone reintroduces that same confounder, so it is declared in the
  annex instead of being discovered by reading it.
- **With the two rules above put together, no run restores anything** —the directory is deleted, and
  what the batch asks for rules—, so **the most expensive path the milestone adds would be excluded
  from the measurement by construction**. A **batch of its own** is needed, one that writes a known
  layout file **before** opening and launches **without asking for tabs or panes**, so that the file
  wins, and whose report declares **how many consoles were restored** and what restoring them cost.
  Without it, «the cost of forking N consoles is already paid» is a claim **no run supports**.

**Six accepted limits, written down because none of them is mitigated and all of them are discovered
by using it:**

- **A write failure is only retried with the next change, not with time.** It is the good side of
  the two references —what retries is the change, so the window does not stay awake retrying and
  there is no need to invent a wait—, and its price: if the disk fails at ten and the user **does
  not move anything again**, that day's arrangement never reaches the file. A line on the error
  output says so, which whoever opens from an icon does not see.

- **Two open instances clobber each other's layout**: the last one to save takes the file. It is a
  direct consequence of the layout being one and global, and putting a lock between processes on it
  would be a mechanism bigger than the milestone.
- **Restoring does not revalidate the geometry.** A tree saved on a big screen and opened on a small
  one leaves consoles squeezed below their minimums. It is accepted because the alternative
  —revalidating and falling back to factory— **would throw away the user's layout for opening the
  laptop without the monitor**, which is a worse product; and there is a mitigating factor already
  built: **the one that gives way is the panel** (D10), so the console keeps its floor for as long
  as the panel has something to give.
- **Whoever opens the application from a desktop icon does not see the error output**, so for them
  every read failure is silent. It is an accepted shortcoming: F2.5 does not premiere any interface,
  and it is the reason not to widen the set of failures beyond what fits on one line.
- **Every console comes back in the directory the application was launched from, not where it was.**
  It is the first thing anyone notices: with four consoles in four repositories the exact tree and
  the exact widths come back, and four shells in the wrong place. And the reason for leaving it out
  is **not** that it would bring forward another milestone: it is that **knowing where a child
  process is, is read from an interface that only exists on Linux**, so doing it would create work
  per platform — which is precisely what F2.7 exists to bound. And it would also be **a route that
  leaves the file and ends up at the launch**, that is, the line above.
- **On a machine with no session runtime directory** —macOS, or Linux without a manager— **the panel
  directories pile up**: there the fallback is the system's temporary, which is not emptied, and the
  destructor that deletes them does not run when the application is killed or closed by the window
  manager. With a fresh identity per restored console, there are as many empty directories as
  consoles per startup. It is not mitigated here because on the normal branch there is nothing to
  mitigate, and a cleanup of our own would be a mechanism with no scenario in 99 % of cases.

### Measured — 24 August 2026: what it costs to open with the consoles you left

**Batch `L`**, which is the one this milestone adds and the only one that walks the restored
startup: it seeds a known layout file —one tab with **four consoles in a row**— and launches
**without asking for tabs or panes**, so that the file rules. **Its control is `H`**, which opens
those same four by asking for them, and it was taken **on the same day and with the same binary**:
the `H` figures of D14's annex are from August and from another binary, so comparing against them
would mix the cost of restoring with everything that has changed since then. The four runs publish
the **same grid** —49×20 per console— which is what makes them comparable.

| | **L** — restored | **L** (2nd) | **H** — asked for | **H** (2nd) |
|---|---|---|---|---|
| Consoles restored from the file | **4** | **4** | — | — |
| — reading the file, interpreting it and validating it | **35.1 µs** | **35.2 µs** | — | — |
| — opening the window with them | **5.261 ms** | **5.506 ms** | — | — |
| Full round trip (pane 0) | 49.4 ms | 47.2 | 57.6 | 45.7 |
| **the rest of the frame** | **401 µs · 393 fr.** | **379 · 410** | **461 · 403** | **370 · 410** |
| checking whether the layout changed | 652 ns | 688 | 920 | 659 |

**The three things this proves:**

1. **Restoring costs 5.42 ms once and only once, and it happens entirely before the first frame.**
   Of that, **35 µs** are reading the file —**0.65 %**— and the rest is opening the window with
   those consoles: **1.35 ms per console**. That second span **is not just the `fork` and the
   `exec`**: it includes arming each console's directory watcher and describing the whole tree for
   the first mirror, which are the other two jobs that grow with the number of consoles.
2. **It leaves no trace at all in the frame, and that was the question.** The «rest of the frame»
   —the figure D3 judges egui by— comes out at **401 and 379 µs** restoring and **461 and 370 µs**
   asking for: the same numbers, with the two ranges overlapping. The mirror's span does not move
   either (652-920 ns in all four). It is what the design claimed without measuring it, and now a
   run supports it.
3. **The report declares where the consoles came from**, and the script **invalidates the run** if
   it does not say so. Without that line, a batch in which the file never got to rule would publish
   the factory-fresh window under the label of a restored one: the same number of consoles, the same
   grid, and not one guard going off. It is the same disguise the witness prevents with the panels
   and the count of effects with the gestures. **And the comparator looks at it too**, because `L`
   and `H` are identical in geometry by construction: without that datum it gave the green light to
   averaging one with the other.

**And three warnings for whoever comes back to these figures:**

- **The first version of this annex published 4.45 ms, and it was badly labeled.** The stopwatch
  stopped when the processes existed, not when the window was built, so it left out each console's
  watcher and the walk of the tree — **two jobs that grow with N**, inside a number that claims to
  measure exactly that. A review caught it, and the difference is **15 %**. It stays written down
  because it is the project's hard rule seen live: what is not declared is published as somebody
  else's cost.
- **The full round trip scatters a lot between runs of the same batch** —49.4 against 47.2 in `L`,
  57.6 against 45.7 in `H`— and **the scatter is the machine's, not the milestone's**: it shows up
  the same in both batches. What can be read is the **comparison between the batches of the same
  pair**, which is what they are there for.
- **One `H` run was discarded as contaminated**, and it is worth knowing how it was caught: it
  published **5.7 s** of mean in the queue with a 95th percentile of 76 ms —a single sample sets the
  mean off— and its report carried a **keyboard latency** section the others do not have. Somebody
  pressed a key while it was running. It is the «do not touch the machine» warning turned into
  something that can be checked afterwards: **if the report talks about the keyboard, the run is no
  good**.

---

## D17 — The contract is the document, not the file: the source of truth is the app's state

**What was decided.** `panel.json` stops being **the** definition of the contract and becomes **one
transport** for a document that can also arrive by others. Three parts:

1. **The contract is the panel document** — `phase`, `lines`, marks, styles, expiry. That is what
   gets versioned and what nobody may break. Where it lives and how it travels is separate.
2. **The source of truth is the state the app holds per pane**, not the file. The file feeds that
   state; it is not it.
3. **Every delivery brings a source and an order.** Who publishes and how two of them are told
   apart. Two producers on the same pane stop treading on each other by chance.

> **Amended by [D43](#d43--a-connection-holds-a-pane-the-file-is-not-read-while-it-does-and-the-hold-dies-with-the-connection), which is what answered this point rather than
> restating it.** The third part read «an increasing number per source» until the channel existed,
> and when it did the number turned out to protect a reordering the transport cannot produce: one
> endpoint per pane, and the file unread while a connection holds it, means there is exactly one
> writer. The source stayed — as the **holder's name**, claimed on the first publication and released
> when the connection closes — and the sequence number was cut, with the reason in D43.

**Why, and the reason is not theoretical.** The plan already had **two writers** of the same place:
the file producer and the agent through MCP's `panel_set` in Phase 3 (**D5**). With no source and no
order, whoever rules is whoever wrote last, which is the worst possible way of deciding it — it
depends on the operating system's scheduling. The symptom does not show up today because there is
only one real producer, and that is why it is worth deciding it now and not when it bites.

**And a reason of origin, which is worth admitting.** The first producer was an agent that writes
files, and for that the file is unbeatable. The three reasons that justified choosing it —failure
isolation, it is read with `cat`, it crosses SSH with no protocol of its own— are still good, but
**none of them says why it has to be the only one**. The decision about the medium was taken with a
single kind of producer in view.

**The candidates, measured by the only thing that decides: what the producer has to do in order to
publish.**

| | File | MCP | Socket | OSC over the PTY | HTTP API |
|---|---|---|---|---|---|
| Publishing from a script | `cat >` | no | needs a client | `printf` | `curl` + a port |
| Publishing from an agent | already does it | yes, if it speaks MCP | no | yes | yes |
| Crosses container / SSH | only with a mounted volume | no | no | **free, configuring nothing** | with a network and a port |
| Survives the producer | **free** | if the app saves it | if the app saves it | if the app saves it | if the app saves it |
| Knows who writes | **no, nobody** | yes, by session | yes, the process's credentials | **yes, by construction: it is its PTY** | only if you authenticate it |
| Warns the producer that its document is broken | **no** | yes | yes | no | yes |
| Attack surface | whoever writes in the directory | bounded by `PANEL_ID` | whoever reaches the socket | **a `cat` of a file with sequences publishes whatever it likes** | anyone on the network |

**What that table decides:**

- **The file stays as standard**, and not out of inertia: it is the only one where the dumbest
  possible producer publishes without writing code, and the only one where the state survives the
  producer dying without anybody programming anything. For a contract whose value depends on
  somebody publishing, the entry bar is the most important property it has.
- **MCP comes in in Phase 3 delivering the same document to the app's state, without writing the
  file.** Writing it would mean the app detecting itself through the watcher, a detour with a frame
  of delay and with no advantage at all.
- **OSC over the PTY stays as a candidate to be evaluated after F2.7**, and it is the one that plugs
  the hole nobody plugs today: a CI inside an `ssh` inside the pane, with no mounted volume and no
  port. It is also **the only transport where the per-pane bounding cannot be faked**, because it
  arrives over that pane's PTY and not through an identifier the sender chooses. Its limit is shown
  by cmux: notices fit through there, not big bodies ([07-landscape.md](07-landscape.md) → E6).

**What was discarded, and why:**

- **The HTTP API.** Everything it brings is covered by another transport, and it adds a port,
  authentication, discovery and a network surface that [03-architecture.md](03-architecture.md)'s
  security table does not have today. It is the only one of the five that turns «whoever writes in
  your directory» into «anyone who reaches your machine».
- **The local socket as the main transport.** It is what Herdr does, and its real advantage —knowing
  which process writes— does not make up for losing the producer that cannot speak it: a container,
  a CI on another machine, an `scp`. That is where the difference between «out of band» and «out of
  process» lies ([07-landscape.md](07-landscape.md) → E8).
- **Replacing the file instead of accompanying it.** Any transport that demands a client raises the
  entry bar, and the entry bar is what makes it possible for the format to have producers — which is
  the only thing that can turn into a moat ([01-vision.md](01-vision.md)).
- **Leaving the source of truth in the file and having MCP write there.** It keeps a single source
  and it is still read with `cat`, but it gives the agent write permission over its pane's file by a
  path that does not go through `PANEL_ID`'s bounding, and it makes the app's state depend on the
  file system giving notice in time. Inspectability is preserved another way: the file is still the
  standard transport and is still there.

**What this does NOT change, and it is worth saying so that nobody reopens it by mistake:**

- **D6 stays intact.** This is about **which way** the state arrives, not about who decides it. It
  is still declared by whoever does the work, in their own vocabulary.
- **The document does not change shape.** A `panel.json` written today is just as good tomorrow.
- **The watcher was not the problem.** It was checked before writing this: the app uses **a single
  watcher** for all the directories, not one per pane, and the system's limits on the development
  machine are 1 024 instances and 524 288 paths. Watching files does not scale badly here, and that
  argument must not be used against the file.

**Consequences, and one of them is pending work:** [04-panel-contract.md](04-panel-contract.md)
splits in two —the document and the transports—, **D5**'s channel table stops naming `panel.json` as
if it were the channel, and **it remains to be decided how the source and the order are expressed in
the document itself**: Herdr solves it with `--source` and a strictly increasing `--seq`, and here
there is neither one nor the other yet. It is not decided in this entry because with a single live
transport it would be inventing the shape before having the second case in front of us.

---

## D18 — The producer declares whether its pane claims, and the app does not deduce it from the body

**What was decided.** The panel document gains an optional field, `attention`, with three values:
`wait`, `block`, `done`. Absent means that pane claims nothing. The producer writes it.

| Value | What it says | Example |
|---|---|---|
| `wait` | **it is waiting for you**: a question, a permission, a decision | «do I approve the design?» |
| `block` | stuck on something else, not you | «waiting for another service's contract to exist» |
| `done` | it finished and nobody looked | the cycle ended and the diff is still unreviewed |

**And the app accumulates it upwards:** pane → tab, with `wait` beating `block` and `block` beating
`done`. `wait` goes first because **it is the only thing that gets unstuck with you in front of it
right now**; a pane blocked by a third party can go on waiting five minutes more.

**Why a field is needed and the body that already exists will not do.** The contract has been able
to express «waiting» and «blocked» from the beginning — `mark: wait`, `mark: block` — but those are
**lines of the body**, and the body is free by design. To paint a tab's indicator or fire a
notification from that, the app would have to **read the body and decide what it means**: that a
line with `block` in the middle of a summary means the whole pane is claiming. That is the app
deciding what somebody else's work means, that is, **D6 broken from the inside**, which is the worst
way to break it: without any document saying that it was broken.

**Why this does not contradict the free-body rule.** The field does not name a metric or a domain
vocabulary: it names **the only question the app needs to answer in order to navigate** — do I have
to go there?—. There are still no fields for cost, for context or for progress, and `phase` is still
free text painted literally. The app understands navigation; the semantics still belong to the
producer.

**And without the field the app invents nothing.** A producer that does not publish `attention` has
the same panel it has today and its tab is not marked. There is no default value deduced from the
body, or from the `phase`, or from the console's activity. **Saying nothing is saying nothing**, not
«this is not claiming because it does not look like it».

**Why now and not in Phase 4.** Because it unblocks three things that are written down as pending in
three different documents and all of them were waiting for this decision: the notification on going
to «waiting» ([04-panel-contract.md](04-panel-contract.md) → Pending), the indicator of a tab you
are not looking at —today what runs in a background tab is not announced in any way—, and Phase 4's
summary view, which orders by this very thing. And because the claim that **nobody else orders by
what claims you stopped being true in August 2026**: Herdr accumulates agent → panel → tab →
workspace and its sidebar answers that question better than ours does
([07-landscape.md](07-landscape.md) → E8).

**What was discarded, and why:**

- **A work-state enum along the lines of `idle` / `working` / `blocked`** — what Herdr, Xirp,
  Superset and Warp have. It describes **what the program is doing**, and that is either deduced by
  the app (D6) or declared by a harness that does not know the question (**D6**, «level 2
  reinterpreted fails just like guessing»). `attention` does not ask what it is doing: it asks
  whether it claims you, which is what the app needs in order to navigate and the only thing the
  producer can answer without getting it wrong.
- **Deducing it from the presence of a `mark: wait` in the body.** It is the cheap thing and it is
  what breaks D6 from the inside. It would also be fragile in the silliest way: a historical line
  —«I waited for approval on Tuesday», with `mark: wait`— would mark the tab forever.
- **A numeric urgency field.** Three values answer the question; a number forces every producer to
  calibrate against the others, and nobody can do that because nobody sees the other panels.
- **Having the app fire the system notification on its own when a `wait` appears.** It is left for
  when the field has producers: notifying is interrupting, and a false interruption costs a great
  deal more than a false indicator. The indicator comes in now; the notification, when it is known
  that the field is published properly.

### What F2.8 added: who holds a claim, and the exception for what has finished

**A claim is seen dimmed when the application has something to confess about that panel** — it has
been quiet for longer than it should, or its file stopped being readable. It says both things at
once: *it claims*, and *there is nobody behind it*. The reason for dimming it instead of hiding it,
along with what was discarded, is in **D9** → «what F2.8 added».

**An exception, and it is a product one: `done` is not dimmed by silence.** Whoever finished **stops
publishing by definition**, so their silence contradicts nothing: it is their normal state. Without
the exception, half an hour after finishing —the standard deadline— *everything* finished would be
painted dimmed, and **finding in the morning what finished at night is exactly what the field is
for**. What does dim it is **not being able to read its file**: there what is shown is the last
thing that arrived, and nothing says that it is still true.

*Discarded:* treating the three classes alike, which is simpler to explain and dims the product's
central case. *Also discarded:* that `done` should **never** be dimmed — that would be hiding a
broken panel behind a plausible summary, which is what this panel exists in order not to do.

**The review found it, not the design.** The design wrote «the stale is dimmed» for all three
without looking at what silence means in each one.

### Measured — 24 August 2026: what the bar costs now that its card accumulates

**Each tab's card says the most its panes claim, so painting the bar now walks.** That work **is
declared** —it has a line of its own in the report's header— and therefore is **not** published as
the framework's cost, which is the figure **D3** judges egui by.

Four runs, two of batch `F` (the background tab still) and two of `G` (the background one dumping
like the one in front), with a 1280×800 window and **all four consoles publishing a claim** —half
`wait` and half `done`—, with **both cards marked** in all four runs.

| Run | Frames with a bar | The bar, per frame | The rest of the frame |
|---|---|---|---|
| `F` #1 | 512 | **2.346 µs** | 256.2 µs |
| `F` #2 | 479 | **2.689 µs** | 268.1 µs |
| `G` #1 | 553 | **2.689 µs** | 290.0 µs |
| `G` #2 | 552 | **2.398 µs** | 293.9 µs |

**Between 2.35 and 2.69 µs**, that is **one and a half hundredths of 1 %** of the 16.7 ms budget at
60 Hz — with two tabs and four consoles claiming. The two runs of each batch **can be averaged**
according to the script's comparator.

**What goes into that number and what does not.** In goes the walk that accumulates each tab's
claims and the painting of its cards: the stopwatch lives **inside** the function that paints them,
not in whoever calls it, because from outside the span covers the whole call whatever is done inside
it — and then taking work out of the measured region breaks no test. **Attending to the bar's click
does not go in**, which runs at another point of the pass and **has gone undeclared since before
this milestone**: it is a known leak, small and prior, and fixing it here would have risked counting
twice what changing tab already declares on its own.

**And the number describes the healthy path**: live producers, publishing within the deadline. With
stale or broken panels, the question the card asks also composes the text of each pane's warning,
and that is not measured — it stays inside the declared span, so it is not given away to anybody,
but it is not this number.

**Why it is measured with two tabs and not with one.** With a single tab **there is no bar**, so a
one-tab batch would claim «the cost is declared» **with the code taken out**. The script prevents it
on its own from this milestone on: in a batch of two tabs or more, the run is marked `.INVALIDA` if
the report does not carry the bar's line **or if the bar never got to mark a single card** — because
the walk happens whether there is a claim or not, and without that second guard a run whose
producers did not publish the field would measure half the work and would read just like a good one.
It is the same guard that already prevents publishing as good the batches with no panel, with no
gestures or with no restoring.

**What this number does NOT say.** It is not compared with the `F`/`G` runs of **D13**'s annex: they
are from another day and from another binary, and there the rest of the frame came out between 386
and 466 µs. What holds is what is inside this table — that the span exists, that it is separated
out, and that it is **two orders of magnitude smaller** than the residue it would have been
subtracted from.

---

## D19 — We position ourselves against terminal emulators, not against agent orchestrators

**What was decided.** The user to be won over is **the one who today has WezTerm, Ghostty, Kitty,
iTerm2 or Windows Terminal open with agents inside** — not the one who uses Orca, Superset or
Conductor. Evolving towards development-environment features comes **afterwards**, and conditional
on this having users.

**Why, and there are three independent reasons:**

- **Against terminals the panel has no competition.** No emulator has state per pane: the comparison
  is «what you already have, **plus** one thing none of them has». Against orchestrators the
  comparison is a list of fifteen features where you are always behind, and **a product with 70 % of
  Orca and nothing of its own is worse than Orca**.
- **The surface race cannot be won.** Orca has been going since March 2026 with a team and daily
  pushes; its Chromium browser per worktree, its mobile app and its Linear integration are whole
  projects each ([07-landscape.md](07-landscape.md) → E7).
- **The clock runs the other way.** Every month spent on parity is a month in which Herdr may grow
  the field it is missing in order to take away the only leg that is left (E8). And there is a
  precedent from the current year: **Herdr came out with far less surface than tmux and reached 31
  888 stars in five months**, for doing one thing tmux did not do. Adoption did not come from
  parity.

**The bar that does have to be cleared**, and it comes out of this league and not the other:

| What a terminal user takes for granted | Status |
|---|---|
| Scrollback, selection, copying, searching | **F2.6** |
| Pasting | done — **D11** |
| It runs on their machine: macOS and Windows | **F2.7** |
| **That their fonts show** — CJK and the icons of a modern `ls` | **had no milestone until today → F2.9** |
| **Being able to configure it** — font, size, colors, shortcuts | **had no milestone until today → F2.10** |
| That the mouse reaches the program inside (`vim`, `htop`) | inside **F2.6**, now explicit |
| Clickable links (OSC 8) | pending, not disqualifying |
| Ligatures, Kitty's graphics protocol | **out** at this decision; optional programming ligatures were later requested in #335 (D70). Kitty's graphics protocol remains out |
| Latency and cost per frame | measured and published — **D3**, D13, D14, D15 |

**The three that had no place in any phase, and the reason they appear on changing league:**

1. **The system fonts.** In a development environment it is a nuisance; **in a terminal it
   disqualifies**. The user opens it, runs `ls`, sees empty boxes and closes it. It was already
   written in D3 in these words —*«it is what separates this from being usable daily»*— and it had
   gone without a milestone since August.
2. **Configuration.** Zero mentions in the roadmap before today. It is **the first thing anybody
   does** on installing a terminal, and without it there is no adoption worth the name.
3. **The mouse inside the TUI.** The SGR/X10 reports are not implemented, so today you cannot click
   inside a text program. It was on file as F2.6 material, but as a note inside another milestone
   and not as something anybody was going to check.

**What stays outside the bar, and who it stays outside with** — it is here to be known, not to be
done:

| | Who has it |
|---|---|
| **Fan-out**: the same assignment to N agents in N worktrees, compare and keep the winner | Orca |
| A built-in browser; pointing at an interface element and sending it to the agent | Orca (*Design Mode*), Superset |
| A file editor inside; an annotated diff; attribution of what the agent wrote | Orca, Superset |
| Commit, push and opening the MR/PR from the app; a review queue | Orca, Conductor, Vibe Kanban |
| A companion mobile app | Orca |
| Remote worktrees over SSH or on a VPS | Orca, Superset, Herdr |
| Restoring agent sessions after a restart; hibernating the idle ones | Orca, Herdr |
| Linear and Jira integration; a registry of installable skills | Orca, Superset |
| A task board and dependency chains | Cline Kanban, Vibe Kanban — see «Ideas with no phase» in [05-roadmap.md](05-roadmap.md) |

**The criterion for letting something from that list in early: that it serves the contract, not that
the neighbor has it.** In favor, the remote worktrees: a producer publishing from another machine is
exactly the case the file wins and Herdr's socket loses (**D17**). Against, the *Design Mode*: it is
magnificent and it is of no use at all to what differentiates things here.

**What this decision does NOT change, and it is best not to reopen it:** **we still do not compete
with tmux or with Zellij** ([01-vision.md](01-vision.md)). Positioning against emulators does not
drag in the most expensive part of the league next door —the session surviving the window being
closed, attaching and detaching, a background server—, and that is precisely what makes this bar a
short one.

**When this decision is reviewed:** when there are users who are not you, or when one of the list
above publishes an open contract with a body. The first changes who has to be convinced; the second
changes whether there is anything to defend.

---

## D20 — The left button inside a console selects; moving the pane goes to `Ctrl+Shift`

**Decision.** Dragging with the primary button **inside a console's body selects text**. To **move
the pane somewhere else** —the **D15** gesture— you have to drag with **`Ctrl+Shift`** held down.
And when the program inside asks for the mouse, **`Shift` alone forces the terminal's selection**,
which is the convention of `xterm` and of all those that came after.

**Why a gesture already delivered and measured is being touched.** Up to here the primary claimed
each console's whole rectangle and **moved the pane**. That turned «selecting text» —what is done
every day in a terminal— into something that had to be learned, and left the free gesture for what
is done every now and then. **D19**'s bar is a WezTerm user, and there dragging selects: a terminal
where dragging moves panes does not clear that bar, however much moving panes is what differentiates
us most.

**Why `Ctrl+Shift` and not another key.**

- It is **the prefix the application already uses for everything of its own**: splitting, closing,
  changing tab, pasting. Moving a pane is of that family; giving it another prefix would be
  inventing a second language.
- It leaves **`Shift` alone free**, and that is needed: it is what forces the selection when the
  program claims the mouse. Spending it on moving would make selecting inside `vim` impossible.
- It leaves **`Ctrl` and `Alt` free**, which is where the other terminals put **block selection**.
- **`Super` is discarded** separately: on GNOME and on KDE it drags the whole window.

**What was discarded, and why.**

- **Leaving the primary moving and qualifying the selection.** It is what there was, and it is the
  opposite of what the bar asks for: the daily gesture behind a key and the rare one in the free
  gesture.
- **A header per pane, in the style of Tilix**, which would be the handle for dragging and would
  carry the close button. **A good idea, and that is why it is not discarded entirely: it becomes a
  milestone of its own** (see the roadmap). **That milestone landed, and it is D45**: the price
  stated below is unchanged, and what answers it is that a measured run does not paint the strip —
  so the published batches go on comparing with each other. Here it is discarded **as part of F2.6** because of its
  price: a header **takes height away from every console, always**, and this project already took
  that decision the other way round when it stopped painting the tab bar with a single tab —
  precisely so that the grid would be identical to the earlier one and the measurements could be
  compared. With a header, **the published control batches stop comparing with anything**. And it
  does not solve the whole problem: `Shift` would still be needed to force the selection inside a
  program that asks for the mouse.
- **An invisible handle on the top edge**, with no row of its own. It costs no height, but it has to
  be discovered and it leaves no room for a close button. It stays as an option for the header
  milestone.

**And the two paths coexist.** Choosing the modifier now does not close the door on the header
later: Tilix has both things. What this milestone does not do is put a layout decision inside a
milestone about scrollback and selection.

**What it drags along, and it is in the milestone's design:**

- **The drag's meaning is fixed when it starts**, read from the press event itself. A frame's
  modifier state describes **the end of the batch of events**, not the instant of the click.
- **Batch `J` of the measurement script stops measuring what it says.** It injects bare primary
  drags, so with the primary selecting it would measure selection and would declare zero
  rearrangements — and the script would invalidate it on its own. The chord is added to the
  injector.

---

## D21 — The scrollback is 3 500 lines per console: the bar chooses it, and the measurements discard the rest

**Decision.** Every console keeps **3 500 lines** of scrollback. The number is set by this project
and the engine's —10 000— **is not inherited**. The **3 500 comes out of the bar**: it is WezTerm's
default, and WezTerm is who **D19** says we measure ourselves against. What the measurements do is
**discard the others** and put a price on the chosen one.

**What was measured**, with `cargo run --release --example scrollback -p term-core`, a 49×180 grid,
an 8 MiB dump, **five samples per point** and publishing the median:

| Ceiling | Memory ceiling per console | Reflow: short lines | Reflow: **lines that wrap** | Reading 8 MiB |
|---|---|---|---|---|
| 0 (what there is today) | 0 | 7.7 µs | 6.4 µs | **44.1 ms** · 182 MiB/s |
| 1 000 | up to 4.1 MiB | 72 µs | 148 µs | 46.4 ms · 173 MiB/s **(+5 %)** |
| **3 500** | up to **14.4 MiB** | 280 µs | **984 µs** | **81.6 ms** · 98 MiB/s **(+85 %)** |
| 10 000 (the engine's) | up to 41.2 MiB | 1.70 ms | **4.07 ms** | 89.9 ms · 89 MiB/s (+104 %) |

A cell takes up **24 B** — asked for, not derived by summing fields.

**Two warnings about that table, because a review had to bring them out:**

1. **The memory column is a calculation, not a measurement, and it is a CEILING.** The engine **does
   not reserve the scrollback in advance**: its storage grows as it fills. Those figures are what a
   console can come to occupy with the scrollback full, not what it occupies when it opens.
2. **The two reflow columns are two real regimes, and the one that decides is the one on the
   right.** A line **shorter than the grid is not reflowed**: on narrowing, the engine pushes it
   whole and it leaves by the cheap door. The first version of this bench measured **only** short
   lines and published a number **four times smaller** than the real one. A terminal session has
   both kinds, so the ceiling is chosen against the expensive one.

**Why the engine's is discarded.** With 10 000 and content that wraps, reflowing costs **4.07 ms per
console**, and one divider of the tree moves **two**: that is **8.1 ms per frame** while dragging,
that is, **half the budget at 60 Hz**, in a gesture that lasts as long as the button is held down.
And up to 165 MiB with four consoles, with nothing bounding how many there are (**D14**).

**The price of 3 500, said in full and accepted.**

- **Dragging a divider costs ~2 ms per frame** with two consoles of content that wraps — **12 %** of
  the budget. It is only paid **while dragging**, and only in the frames that cross a cell boundary:
  the tiling comes out early when the grid does not change.
- **Reading a dump gets 85 % more expensive** in the bench: from 182 to 98 MiB/s. **Inside the
  window that translates into 39 % more interpreting and 1.2 % of the full round trip** — the bench
  reads across the width of a 180-column grid and the window swallows 128 KiB per frame, so they are
  not the same regime. See below the annex with the four runs.
- **Up to 14.4 MiB per console** of full scrollback, 58 with four.

It is the price of a terminal remembering what came out, and it is declared instead of hidden.

**And it forces the published measurements to be taken again.** All the annexes' tables —D3, D13,
D14, D15, D16, D18— were taken with the scrollback **off**. As soon as it is turned on, their
«interpreting» span describes another regime. **Taking them again is part of the milestone**: an
annex that measures one regime under the label of another is exactly the badly labeled number this
project does not publish.

**What was discarded, and why.**

- **Inheriting the engine's 10 000.** It is what would have happened by not touching a line, and it
  costs half the frame in a normal gesture. A dependency's default is not a decision of this
  project.
- **1 000 lines.** Almost free —+5 % on reading, 1.8 % of the frame on dragging— but they are twenty
  screens: they serve for looking back, not for finding something from a while ago. It does not
  clear the bar.
- **Letting whoever uses the application choose it.** It is the right thing **in the medium term**
  and it is in the roadmap (**F2.10**, configuration), but today there is nowhere to put it:
  bringing forward half a configuration system inside this milestone is putting another milestone
  inside.

**When it would stop being valid.** If the engine's version changes —the numbers are its own—, if
F2.10's configuration arrives, or if somebody measures that reflowing has stopped growing with the
scrollback. The bench is in the repository so they can be taken again in a minute.

### Measured — 25 August 2026: what the scrollback does to the real window

**The bench measured the engine; this measures the application.** Four runs of batch `C` —two
consoles dumping, with a panel—, **all four with the same binary and consecutive**: two compiling
with the ceiling at zero and two with the 3 500 lines in place. The two of each pair **can be
averaged** according to the script's comparator, and the table publishes the mean with the two runs
behind it.

| | with no scrollback | with 3 500 lines | |
|---|---|---|---|
| **available byte → paid-for frame** | **74.6 ms** (74.19 · 74.94) | **75.5 ms** (75.62 · 75.40) | **+1.2 %** |
| of that, waiting in the queue | 71.7 ms (71.65 · 71.80) | 72.0 ms (71.99 · 71.95) | +0.4 % |
| **interpreting** | **1.05 ms** (0.95 · 1.16) | **1.46 ms** (1.51 · 1.41) | **+39 %** |
| enqueueing the paint | 249 µs (229 · 269) | 234 µs (237 · 230) | within the noise |
| **the rest of the frame** (the framework) | 279 µs (257 · 300) | 262 µs (262 · 263) | within the noise |

**What has to be read here: the scrollback makes interpreting 39 % more expensive, and the full
round trip 1.2 %.** Every frame swallows a fixed budget of 128 KiB per pane; interpreting it now
costs half as much again, and that is paid **inside** the frame, not in the queue. The framework
does not move, which is what **D3** watches, and the `recolocar` span goes declared separately from
this milestone on so that it does not slip in there.

**An earlier version of this annex published «the round trip doubles, 37.1 → 75.4 ms» and it was
false. It is withdrawn, and it is worth writing down why**, because the failure is exactly the one
the project's hard rule describes:

1. **The «with no scrollback» column was not from the same binary or from the same day.** It came
   out of a run of **18 August**, even though the text claimed «the same binary of the day and half
   an hour apart». Nobody could see it by reading the annex.
2. **And the conclusion was not supported by the data.** There are runs of batch `C` from **19
   August**, with the scrollback **off**, that already gave **73.7 and 74.0 ms** of round trip and
   71.5 ms of queue. That is, the jump from 37 to 74 ms had already happened **before** turning the
   scrollback on: attributing it to it was hanging somebody else's cost on it.

**And that leaves an open question that does not belong to this milestone:** between 18 and 19
August the full round trip of the same batch, with the same window and the same grid, went from **37
to 74 ms** with the scrollback off — **and it is not CPU cost**: interpreting moved from 763 to 851
µs and the framework's residue from 220 to 240. The frames are similar in number and in cost, so
what doubled is **what each byte waits**, not what it costs to attend to it. It points at when a
repaint is asked for, not at how much painting costs. It stays as an issue of its own.

**When the 39 % is noticed, and when it is not.** Only with a sustained dump —a `cat` of a big file,
which is what this batch does on purpose—, and there it translates into less than a 2 % delay. A
normal command does not fill the queue and does not wait for anything. **The scrollback's expensive
price is not here**: it is in reflowing the text on a change of width, which is what the next annex
measures.

### Measured — 24 August 2026: the worst case of reflowing, with the divider moving non-stop

**Why a new batch is needed and the ones there were would not do.** Reflowing the text is paid **per
console**, and neither of the two batches with gestures changes the width of more than one: `K`
narrows the left console's panel, and `J` rearranges the tree on dropping without crossing cell
boundaries while in flight. The divider that shares two consoles out is the only one that changes it
**for both at once**, so it is the only one that exercises the ceiling of 3 500 lines on its
expensive side. It is batch **`M`**.

| | batch `C` (control, still) | batch `M`, run 1 | batch `M`, run 2 |
|---|---|---|---|
| frames in the run | 429 · 470 | 799 | **3 488** |
| frames with output | 358 | 357 | 357 |
| injected gestures · effects | — | 159 · 480 | 697 · 2 093 |
| **reflows** | 3 · 3 (the ones from startup) | **320** | **1 397** |
| what each one cost | 671 · 524 µs | **654 µs** | 197 µs |
| **the rest of the frame** (the framework) | 262 · 263 µs | **257 µs** | 163 µs |

**The first thing, which is what the batch came to check: the gesture does not slip into the
framework.** With the divider moving non-stop and reflowing two consoles three hundred and
twenty-odd times, the residue stays at **257 µs**, inside the 262–263 of the still control. The
`recolocar` span is declared and is subtracted from its frame, so the 209 ms reflowing cost in that
run **are not published as egui's cost** — which is exactly what the project's hard rule is after.

**The second: reflowing costs what the bench said, and the two runs do not say the same thing.** The
654 µs a time of the first are of the same order as the bench's 984 µs and the still control's
524–671. The second gives **197 µs**, three times less, and **why they differ is not isolated**.
Both are published and what is known is said:

- **They are not averaged.** Both have the same 357 frames with output but one lived 3 488 frames
  and the other 799 —injecting gestures keeps the window awake, just like a human mouse—. It is the
  same warning **D15**'s annex carries for batch `J`, and here it bites harder because the
  difference is fourfold.
- **The explanation the data RULE OUT** is that the cheap run measured the scrollback half full:
  reflowing grows with the lines kept, and in the long run the dump finished early, so most of its
  reflows happened with **more** scrollback behind them, not with less. It predicts the opposite of
  what came out.
- **The one left standing** is what each reflow competes with: the short one spent 45 % of its
  frames dumping and the long one 10 %. It is consistent with the numbers **and it is not measured**
  — isolating it demands that the report publish how much scrollback there was behind each reflow,
  which is instrumentation this milestone does not add. It is said as a hypothesis, not as a result.

**What this batch does NOT measure, and it is worth knowing before trusting it: resizing the
window.** Changing the size of the window changes the width of **every** console of **every** tab at
once —the background ones are remeasured on every frame against the new area—, so a single frame of
that drag reflows eight consoles where the divider's gesture reflows two. With the scrollback full
that is on the order of **8 ms per frame**, more than the whole frame. **No batch covers it**, and
not through oversight: they all ask for a fixed size and the script invalidates the run that did not
have it, which is precisely what makes the project's measurements comparable. The work **is**
declared —the span sums all the frame's consoles and is subtracted from it—, so it is not a badly
labeled number: it is a **path with no number**. Measuring it demands a batch that resizes, and that
batch has to decide first what to do about the size guard.

**The third, and it is the number somebody who drags a divider with a console dumping takes away:**
two reflows per gesture, a good half millisecond each, inside a frame that already pays 76 ms of
waiting in the queue. The gesture looks fluid because what it costs is not in the painting; what is
delayed is **what the console had pending to show**.

**What the batch uncovered along the way, and it was not about performance.** The injected gesture
pressed on the divider's line **in the same frame** in which it carried the pointer to it, and so it
lost the tie against the console next to it: egui admits all three as candidates —the line measures
two points and both consoles touch it— and the tie is won by the last one registered. The result was
that the batch **dragged a console** and published the **D15** gesture under this label. It is fixed
by bringing the pointer close in one frame and pressing in the next, which is what a hand does:
verified that a human drag always wins. **It does not happen to whoever uses the application**, and
that is measured and not reasoned.

### Measured — 24 August 2026: the annexes' four batches, taken again with scrollback

**Why they are taken again.** Keeping scrollback makes **interpreting** more expensive, and the
tables published in the annexes of **D13** (tabs) and **D14** (four panes) were measured with the
scrollback off: leaving them as they are would turn them into figures that no longer describe this
application. Eight runs, two per batch, the same day and with the same binary, and **the script's
comparator says that the two of each pair can be averaged**.

| | **F** — 2 tabs, the background one still | **G** — 2 tabs, all four dumping | **H** — 4 in a row | **I** — 4 in a square |
|---|---|---|---|---|
| Each console's grid | 47×39 | 47×39 | **49×20** | **24×39** |
| Consoles dumping | 2 of 4 | **4** | **4** | **4** |
| Full round trip | 75.3 ms · 74.8 | 77.7 · 78.8 | 77.1 · 80.0 | 79.2 · 80.0 |
| — waiting in the queue | 71.9 · 71.4 | 72.0 · 72.5 | 71.7 · 73.5 | 73.1 · 73.5 |
| — interpreting | 1.41 ms · 1.42 | 1.31 · 1.44 | 1.21 · 1.45 | 1.51 · 1.48 |
| — enqueueing the paint | 203 µs · 215 | 203 · 246 | 146 · 171 | 128 · 135 |
| — the status panel | 5.4 µs · 5.1 | 5.1 · 6.6 | 10.9 · 12.4 | 5.3 · 5.5 |
| — the tab bar | 2.78 µs · 2.08 | 2.41 · 2.90 | (there is none) | (there is none) |
| **the rest of the frame** | **236 µs · 243** | **263 · 298** | **319 · 355** | **274 · 291** |

**These figures are NOT compared with those of D13's and D14's annexes, and saying so matters more
than the table.** Those are from 16 and 17 August, with another binary and another day of machine:
the same batch `C` gave 52 ms of full round trip there, on 18 August it gave 37 and on the 19th
—still **with no scrollback**— it already gave 74. What these eight runs are good for is (a) leaving
published what the application does **today**, and (b) answering among themselves, which is the
valid comparison because they are from the same day and the same binary.

**What they answer among themselves:**

1. **The framework's residue still does not grow with anything.** Between **236 and 355 µs** across
   the eight runs, with the variation between the two of one and the same batch (319 → 355 in `H`)
   of the same order as the variation between batches. It is the same conclusion as the annexes of
   **D13** and **D14**, now with the scrollback on and with the `recolocar` span declared separately
   so that it does not slip in here.
2. **How many consoles dump barely moves the full round trip.** From two dumping (`F`, a mean of
   75.0 ms) to four (`H`, 78.6) there is **less than 5 %**, and the wait in the queue stays pinned
   between 71.4 and 73.5 ms across the **eight** runs. In D14's annex, in August, that same
   difference was **double** (52 → 100 ms) — but that is from another binary and is not crossed with
   this: what is claimed here is today's, and today it does not move.
3. **Interpreting moves little and enqueueing the paint goes down with the cells**, just as before:
   146 µs with `H`'s 980 cells, 128 with `I`'s 936, 203 with `F`'s 1 833… the other way round from
   what the cell count would lead you to expect between the first two and the third, and that is why
   it is published with no conclusion: they are microseconds inside a round trip of 75 ms.
4. **Reflowing barely appears in these four**, and that is correct: none of them drags anything, so
   the between two and five reflows of each run are the ones from startup, when the compositor hands
   over the size. What each one costs —from 43 µs in a square to 976 µs in a row— depends on how
   much text there is to reflow, and the expensive case has its own batch: the **`M`** above.
## D22 — What «the text» that gets copied is, and the four rules the selection inherits from the engine

**What was decided.** Copying returns what the terminal engine calls «the text of the selection»,
and **those rules are written down here** even though the code belongs to the dependency. Inheriting
the code is correct; inheriting the decision without saying it, not — in this project no decision
lives only in the code, and least of all if it lives in somebody else's.

**The four rules of what «the text» is:**

1. **A line that wraps is copied WITHOUT the break.** A line longer than the grid is split by the
   terminal, not by the program. Copying it with a break inside returns something **nobody wrote**, and
   pasting it into a shell runs two commands instead of one. Two real lines **do** keep theirs: what tells
   them apart is who put the ending in.
2. **The padding at the end of each line is not copied.** A grid is full of empty cells to the right of
   what was written; dragging them turns any paste into a line with a tail of spaces.
3. **A wide character's companion is not duplicated.** A CJK character or an emoji takes up two cells and
   are **one**; copying both would return it repeated.
4. **A tab's gap is skipped.**

**The three ways to lose a selection, and they are its as well:**

- **Changing the number of columns clears it.** The text is laid out again: what was marked stops being
  where it was, and keeping the range would leave a piece nobody chose highlighted. **With the same
  columns it does not**: resizing only the height keeps it, because nothing is laid out again.
- **Switching to the alternate screen clears it.** A full-screen program —`vim`, `less`, `htop`— switches
  to the alternate one when it starts, and what was marked is not on the new screen.
- **Clearing a region that crosses it clears it.** The program erased what was marked; surviving would
  mean highlighting empty cells, or whatever it writes there afterwards.

> **When `htop` clears your selection, the reason is here.** That was the point of writing them down:
> without this section, the explanation lives in a dependency's source and whoever looks for it does not
> even know it exists.

**A fifth rule that was missing, and that the dump to text broke: what goes attached to a character
is text too.** An emoji selector —`U+FE0F`— or a combining mark **take up no column**, but the
program wrote them. Copying with the mouse has always kept them, because the engine does that;
**this crate's dump to text threw them away**, so a program that wrote `☁️` read as `☁`. It is not a
clipped character: it is **another** character. Fixed in August 2026, on adding to the view what
goes attached to each cell (**F2.9**), which is when the defect became visible for the first time.

**And one rule this project adds, because the engine cannot know it.** A wide character's companion
**is highlighted along with its head**. The engine only does the opposite path —it marks the head
when the companion falls inside, so that its text does not come out cut off— and that is enough for
it because **its** painter draws the background of a wide character two cells wide. Ours paints the
background of **each cell separately** (**D3**), so without adding it a wide character at the edge
of the selection comes out with half a letter highlighted and half not, right where there is CJK or
emoji.

**How what is selected looks: by swapping its two colors.** There is no highlight color in the
theme, and it is not to save anything: a cell's two colors **were already legible one on the
other**, so swapping them cannot leave text illegible. A color of our own can, and it would do it
right where the program has chosen odd colors — which is where nobody would test it. The swap goes
**after** reverse video, so selecting over `vim`'s cursor shows it like the rest of the text instead
of leaving it indistinguishable.

**Copying does not filter control characters, unlike pasting.** What comes out is already text from
cells and the engine does not keep escapes. Pasting filters because it protects **the inner
program** from what comes in (**D11**); here the direction is the opposite and there is nobody to
protect.

**What was discarded, and why.**

- **Writing our own rules of what «the text» is.** They are ten fine rules —wrappings, companions, tabs,
  padding— that the engine already holds up and that would have to be kept in step with it. Rewriting them
  is guaranteeing that one day they disagree with the highlight, which does come out of its coordinates.
- **Pulling the selection range out through `term-core`'s API.** It is a type of the engine's and it would
  be the leak that that crate's founding criterion prevents. What whoever paints needs is **a `bool` per
  cell**, and with that the highlight's two fine rules are still decided by the engine.
- **Filtering the controls when copying**, for symmetry with pasting. None come out: what there is are
  cells. It would be copying the paste guard without its reason.
- **A highlight color in the theme.** See above: it can leave text illegible, and the swap cannot.

**When it would stop holding.** If the engine changes its rules for converting to text —the numbers
and the behavior are its—, or if one day the painter drew the background of a wide character in one
go, in which case the rule this project adds would be superfluous.

### Measured — 25 August 2026: what having text marked costs

**With its control inside the same run.** Batch **`N`** opens two consoles dumping the same thing
and marks text **only in the left one**: the right one is not touched. It is the same shape batch
`K` uses for the panel gesture.

| queuing the painting | marked console | untouched console | difference |
|---|---|---|---|
| batch `N`, run 1 | 277.4 µs | 191.9 µs | **+85.5** |
| batch `N`, run 2 | 297.5 µs | 187.3 µs | **+110.2** |
| batch `C` (still), run 1 | 236.7 µs | 204.8 µs | **+31.9** |
| batch `C` (still), run 2 | 230.4 µs | 189.6 µs | **+40.8** |

**And here is what the first version of this annex published wrong.** It said «marking costs ~100
µs», taking the difference between the two consoles of the run. But **that difference exists with no
selection at all**: the still batch already gives 32 and 41 µs between left and right. The console
next door **is not a clean control** — there is an asymmetry of position that does not belong to
this gesture.

**What is left, with the two sums that can be done and their disagreement stated:**

- **Difference of differences** (98 − 36): **~62 µs per frame**.
- **The same position across runs** (287 on average against 234, the left one with and without marking):
  **~53 µs**.

**The two do not quite agree, and with this data the gap does not close.** The runs of batch `N`
lived **1 066 and 2 640 frames** against the **429 and 470** of the still one: injecting gestures
keeps the window awake, so the two do not share a regime — the same warning the annex of **D15**
carries for batch `J`. A sign of it: `N`'s control comes out **cheaper** than the same position in
the still batch, which is the opposite of what any explanation of «marking costs» predicts. What is
published is the range, **50–60 µs per frame**, and that it **is not isolated**. Isolating it calls
for a batch that marks and unmarks alternately inside the same run, against the same console.

**What is clear, and it matters more than the figure: the cost does NOT grow with how much is
marked.** While a selection is alive, composing the picture asks **for every cell of the grid**
whether it goes highlighted — the 1 911 of a 49×39 console, two marked or two thousand. Which means:

- marking a word costs the same as marking the whole screen;
- the cost **grows with the grid and with the number of consoles**, not with the gesture;
- and it is paid on **every** frame while the selection stays alive, not only while it is being dragged.

The first version of this annex explained it as «twenty extra rectangles when painting», and the
arithmetic contradicted itself: sixty microseconds across twenty rectangles would be three each,
when queuing the 1 911 whole cells costs 190. The extra rectangles do exist —a highlighted cell has
a background different from the window's, and the painter skips the ones carrying that background—
but they are not the bulk.

**How far the measurement reaches, said before anyone trusts it too much.** The batch marks **twenty
cells of one row**, which is one of the gestures people do make —a path, a command—. What **has
not** been run is the other extreme: marking the whole screen, which is the «copy all the output»
gesture. There, on top of the per-cell sweep comes **one background rectangle per marked cell** —the
painter skips the ones carrying the window's background, and a highlighted one does not carry it—,
and that is 1 911 rectangles per frame against the twenty of the batch. That that addend is not the
bulk is **argued, not run**.

**And it lasts longer than it seems: a selection stays alive —and charging— until somebody throws it
away.** It is not thrown away by typing, nor by losing the focus, nor by switching tabs, nor by
moving the pane: only a plain click or the three engine rules above. The everyday path that pays for
it is marking a line, copying it and carrying on working: the next `cargo build` sweeps the grid on
every frame **with the mark already off screen**. And **each console holds its own**, so with four
panes it is paid four times.

**One question stays open, and it does not belong to this milestone:** asking per cell with the
selection alive can be narrowed by looking first at whether the row falls inside the range. It is
not done now because it would be optimizing against a figure this annex has just declared **not
isolated**, and this project does not optimize against numbers it does not hold up.

**And the gesture is declared with THREE numbers in the header, not two.** To the injected gestures
and the ones that took effect is added **in how many frames text was left marked** —frames and not
gestures: each gesture extends the selection once per drag frame, so it comes out larger—. That
third one is read by the batches in both directions: the one that measures marking demands **more
than one per gesture**, and **all the rest demand zero**. The reason is **D20**: ever since the
primary button on its own marks text, any gesture that loses its handle turns into a selection
**with nothing else giving it away**, because it is injected whole and declares effects just the
same. Checked by taking the chord off the gesture that moves consoles: batch `J` invalidates itself
saying it left text marked in 183 frames. And in the other direction, the one that marks demands
**more than one per gesture** and not simply more than zero: each gesture extends the selection once
per drag frame, so asking for «at least one in the whole run» would let through a batch in which
only the first one marked.

---

## D23 — The mouse to the inner program: who keeps each gesture, and what it is told in

**Decision.** A console whose program **has asked** for the mouse receives the gestures that happen
over it, in a report **this project writes** because the engine's library does not bring one. The
handout has a fixed order, is decided **on press** and is not read again, and `Shift` is the way out
that returns the gesture to the terminal come what may.

### The order, and why it is that one

| Who keeps it | When | Why it goes there |
|---|---|---|
| **Moving the pane** | `Ctrl+Shift` held | It belongs to the window. If a program could take it away, opening `vim` in a console would leave it nailed where it is |
| **The selection** | `Shift` held | Without this way out, inside `htop` or `vim` there would be **no** way to get a line off the screen. It is what every terminal does with that same key |
| **The program** | It has asked for it | The normal thing as soon as there is a full-screen interface inside |
| **The selection** | Nobody else claimed it | The everyday case: a shell does not ask for the mouse |

**For the wheel the order is the same with one more branch**, and that is the one that makes `less`
and `man` scroll without asking for the mouse: `Shift` → always the scrollback; the program asks for
it → its own two numbers; **alternate screen** → arrows; if not → the scrollback.

**And the meaning is fixed on press**, exactly as in **D20**: the gestures all come in through the
same button, and re-reading the chord on every frame would make releasing a key halfway through a
drag change what the gesture means **with the button still held down**.

### The encoding is ours, and where it stops

The engine's crate knows how to say **whether** the program wants the mouse —they are its flags— but
assembling the report was done by Alacritty's binary, not by its library. So the table belongs to
this project:

```
extended:  \x1b[<{button};{column};{row}M   on press
           \x1b[<{button};{column};{row}m   on release
legacy:    \x1b[M{32+button}{32+column}{32+row}
wheel:     button 64 (up) and 65 (down);  dragging adds 32;  Ctrl 16 and Alt 8
```

Columns and rows **start at one** and are those of the visible grid.

**What is left out: encoding the coordinates in UTF-8.** The legacy one puts each number into a byte
with 32 added, so it **saturates at column 223** — and what `xterm` does without that mode is
exactly that: stop reporting. Past that point **nothing is sent**, never an approximate number:
coordinates that are wrong are understood by the program as a click somewhere else, and that is
worse than receiving nothing. It is the same cut-off this project already declared for the function
keys, and for the same reason: writing a table without being able to see it working is guessing.

**And `Shift` does not travel in the report**, even though the encoding has room for it: in this
window that key forces the selection, so a gesture with it held down never gets as far as being
encoded. Putting it in would be a dead branch that on top of that suggests the opposite of what the
window does.

### What was read in the engine and not assumed

**Alternate scrolling comes ON from the factory.** It is in the engine's default mode —read in its
source, the same as `xterm` does—, so in practice what decides that the wheel comes out as arrows is
**the alternate screen**. The flag is still asked about because a program **can turn it off**: `vim`
does as soon as it wants the mouse for itself, and giving it arrows then would move its insertion
point without anyone touching a key. The first test for this was written assuming the opposite and
**failed**: it is one more case of somebody else's API being read.

### The measurement: its own series, and a label that says exactly what it covers

What the mouse costs is recorded in **its own series**, not in the key's. It is the same reason that
one excludes pasting: they are two different populations under a label that only describes one. A
drag puts in **dozens of samples per gesture** —one per cell crossed— against the single one a key
puts in, so averaging them together would leave the figure **D3** publishes describing mostly the
mouse.

**The label is «encoding the mouse gesture → byte in the program», and not the key's.** The first
version of this annex published it as «from the frame that sees the gesture to the byte», which is
what the key's series measures — and **it is false**: this clock starts much later, with walking the
tree, handing out the pointer and resolving the cell **outside**, inside the clock that already
times attending to the mouse as a whole. Under that label it published a span strictly shorter than
the one it named, which is the error this project punishes above all the others. Fixed in the code,
in the report and here. **The two numbers do not compare.**

**Annex — measured.** Batch `O`, two runs of the same binary, averageable according to the
comparator. It makes **the same walk** as the batch that marks text and differs only in that the
program of the left console has asked for the mouse; the right one is the control inside the same
run.

| Run | Gestures | Reports | Encoding and writing, average | The 95 % |
|---|---|---|---|---|
| 1 | 1 001 | 3 004 | 5.126 µs | 10.480 µs |
| 2 | 1 387 | 4 164 | 5.286 µs | 10.200 µs |

**What encoding a gesture and writing it to the program costs: 5.1–5.3 µs on average and about 10 µs
for the 95 %.**

**And the reports come out at exactly three per gesture** —press, one movement, release—, which is
what confirms that no path reports extra: the first version sent **four**, with a movement published
**after** the release.

**And this figure is isolated, unlike D22's.** It does not come from subtracting one console from
the other: it is its own series, timed between two points of the same path, and its samples are
counted **per report** and not per frame. What **cannot** be done with this batch is subtracting the
control console from it to get «what the mouse costs per frame»: the asymmetry between left and
right exists with no gesture at all, and that subtraction has already published two false numbers in
this very document.

**One more warning, of the same kind as D15's:** the two runs **do not share a regime** —1 001 and 1
387 injected gestures—, so their **per-frame** averages are not averaged. This series' ones are,
because its unit is the report.

**And the effects are counted per GESTURE, not per report.** A mouse gesture is the press, its
movements and the release; counting each one, the batch published **three times as many effects as
gestures** and its column stopped being comparable with those of the other batches, where the
effects never exceed the gestures. It counts the press, which opens the gesture, and the wheel turn,
which **is** a whole gesture by itself. With the final code the batch declares **1 001 gestures and
1 002 effects**.

### The new guards, and in both directions

The report header now declares **four** numbers of the gesture, not three: to the injected ones, the
ones that took effect and the frames with marked text is added **how many mouse reports went out**.
That fourth one is read the same way as the third:

- **And the arrows do not count.** What comes out when the wheel is translated are the same keys the
  keyboard sends; publishing them under «N mouse reports» put samples that did not belong into that series
  **and made the guard fire** that demands zero reports in every batch that is not the mouse's — a correct
  run with a `less` scrolling was marked invalid.
- **The mouse batch demands more than one per gesture.** Without that number, a run in which the program
  never got as far as asking for the mouse —the sequence that does not come out, an old binary— would
  publish the spans of a **selection** under the mouse's label, and the two batches would be
  **indistinguishable from reading the report**: they make the same walk.
- **And it demands as well that it marked nothing**, because the two paths running at once would mix their
  spans.
- **All the rest demand zero.** A report that got away would put a trip to the pseudoterminal inside
  another batch's average.

**Tested by failing, which is the only way to know they are worth anything.** Taking off the batch
the line that makes its program ask for the mouse, the run invalidates itself saying it **left text
marked in 1 990 frames** — the whole gesture resolved as a selection, exactly the scenario the guard
exists against. Without it, that report would have been published as good and its spans would be
those of the batch that marks text with the mouse's label on top.

---

## D24 — The system fonts are looked for when they are needed, not at startup

**Decision.** The terminal **does not enumerate the machine's fonts when it opens the window**. It
looks for them the first time a character appears that none of the registered faces covers, and
registers then the one that covers it. **Chosen with the numbers in front**, the same as the
scrollback ceiling in **D21**.

### The numbers, measured here

`cargo run --release --example fonts -p app`, five samples per point and **three runs of the binary
with the machine at rest**. Development machine with **259 families** installed.

**Cold and warm go separately, and that is half the annex.** The bench caches the files it opens, so
the first sample pays for opening them and the rest do not. **The lazy path always pays the cold
one** — once per new writing system, which is exactly what happens in production.

**And that is why the cold column is published as a range of runs and not as a value.** The cold
**happens once per run**: inside a run there are not five cold samples to average, there is **one**.
So the only way to know how much it moves is **to repeat the run**, and it moved — by up to **40 %**
in the number that holds up this decision. Publishing one run's value as if it were *the* value is
exactly what the project's hard rule forbids, and the first version of this annex did it.

| What | Cold (3 runs) | Warm |
|---|---|---|
| **Enumerating** the system fonts | **17.3 – 23.2 ms** | 15.1 ms |
| **Registering 4 faces** in the text engine | **6.2 – 7.8 ms** · 24.5 MiB of font in memory | 1.9 – 2.2 ms |
| **Registering 1 face with the window running** | **3.4 – 4.8 ms** — from **21 % to 29 %** of a frame at 60 Hz | 1.2 – 1.5 ms |
| Finding who covers 日 **by script** | **570 – 852 µs** | 140 – 210 ns |
| Finding the color emoji **by generic family** | **7.2 – 10.8 µs** | 0.67 – 0.98 µs |
| Finding who covers it **by opening all 259 in order** | from 94 µs to **18.2 ms** | 94 – 152 µs |

> **The warm column is stable, and the contrast is the finding.** Enumerating warm gave 15.07, 15.10,
> 15.17 and 15.12 ms in four consecutive runs — four figures within 1 % — while its cold went from 17.3
> to 23.2. What varies is **opening files**, not walking lists. And it serves as a warning: **the first
> version of this annex published 13.7 ms warm, a number that has not been seen again in any of the four
> runs.**

### Why lazy, in one subtraction

**At startup they would cost between 23.5 and 31.0 ms** —enumerating plus registering, adding the
two extremes of each range—, and they are ms that **today are not paid**.

> **What it compares against, said precisely, because the first version of this annex said it wrong.** It
> said that it «multiplies by 5.3 what it takes today to reach the first frame», taking the **5.42 ms** of
> **D16** as if they were the whole startup. **They are not**: that number is what **restoring the
> layout** costs, and it is **the only span of the startup this project has measured**. Opening the
> window, setting up the framework and loading the factory fonts **are not measured**, so there is no
> total to divide by. What can be stated: they are **between four and six times the only measured span**,
> and they would be paid **before the first frame**.

**Looking when it is needed costs between 4.0 and 5.7 ms** —finding the face plus registering it,
adding the extremes again— and it happens **once per new writing system**, not per character or per
frame. They are **from 24 % to 34 % of a frame**: a measurable, one-off jolt, and in exchange for
paying nothing on opening.

**The decision holds across the whole range, and that is the reason for publishing it.** The worst
case of the lazy path (5.7 ms) against the **best** of the startup (23.5 ms) is still a distance of
**more than four times** — and the startup is paid **always**, while the lazy one is paid **only if
the character appears**. No combination of the measured extremes turns it around.

> **That percentage is four times what the first version of this annex said, and it is worth saying why.**
> It published «1.3 ms, an 8 %» because of three errors that the review found **by running the bench**:
> the median of five samples published the **warm** case when the lazy path pays the **cold** one; the two
> halves of the sum were measured **with different fonts** —the search with one of 19.5 MiB and the
> registration with another of 3.8—; and the search timings measured **a copy of the font file**, not the
> search.

> **And those 24.5 MiB are not a detail.** It is the **whole font file per face**, not the glyphs that get
> used. With the lazy path only the memory of what really appears on screen is paid for.

### What the measurement found and was not looking for

**A font that «covers» an emoji can paint it in black and white, and win by alphabetical order.**
Walking the families in order, the one that takes `😀` on this machine is **DejaVu Sans**, and the
bench publishes it in those words: `DejaVu Sans [monochrome]` — with `Noto Color Emoji` installed. It
is exactly the defect Alacritty has had documented for years.

**Asking for the generic emoji family fixes it, and on top of that it is the cheapest thing in the
whole annex: 7.2 – 10.8 µs.** So the rule is not «the first one that covers it», it is **«the first
one that covers it with the right presentation»** — and the number above is already measured **with
that rule**, not with the coverage-only one.

**And `Noto Color Emoji` has no color outlines: it is a 136×128 bitmap.** It confirms from the hard
side what reading egui's source already said — its rasterizer only draws outlines, so registering
that font **would not even give the silhouette**: it would give nothing.

**The spread of walking everything is not «depending on where it falls in the order», it is
depending on how much the winner weighs.** `Adwaita Sans` comes early and weighs 0.88 MB: 94 – 134
µs. `Noto Sans CJK HK` comes later and weighs 19.5 MB: 12.2 – 18.2 ms. What is paid for is opening
the file, not walking the list — and the first version of this annex attributed the range to the
position, which is a false cause.

### What storing the already-drawn emojis costs

This terminal's cell is **16.3 points high**, asked of the text engine and not invented. A glyph
rescaled to that cell takes up:

| Desktop scaling | Per glyph | A thousand glyphs |
|---|---|---|
| 100 % | 1 024 B | **1.0 MiB** |
| 200 % | 4 096 B | **3.9 MiB** |

**The ceiling is sized in pixels and the desktop decides how many there are per point**, so the
figure depends on the scaling — the same as **D10** established for the panel's width. The first
version of this annex published a single figure with an invented cell height treated as pixels.

**Watch out for the other number:** the **68 KiB** of the bitmap are its size **once decoded to
RGBA**. The font stores it compressed — the whole `Noto Color Emoji` file, with all its glyphs, is
10.5 MiB.

### What this bench does NOT say, and has to be measured separately

- **What painting the already-rasterized glyph costs.** That lives inside the frame and is measured with a
  batch of the script, not with a bench. It goes in the delivery that brings the color.
- **What the whole startup costs.** Only one span is measured —restoring the layout—, so today it cannot
  be said what fraction of the startup anything added to it would be.

### What the lazy path forces you to remember, and it is not obvious

**The characters that NOBODY covers have to be remembered too.** A covered character is looked for
once and its face stays registered, so the next one does not look again. But one that no font on the
machine has **leaves nothing registered**, so without remembering the failure it would walk the
candidates again every time it appeared — and that is exactly the character a broken program spews
in a loop. It is a requirement of the delivery that brings the discovery, and it comes out of this
decision.

### And two compilation consequences that were discovered here

**Discovering fonts on Linux leans on fontconfig, and linking it would require `pkg-config` and its
headers to compile** — checked: the machine where this was written does not have them and the
compilation fails in the build script, not in the code. It is loaded **at run time** instead, which
is the same technique —and the same reason— by which `libwayland` arrives today by rebound. The
price is declared: the absence of fontconfig is discovered **at startup** instead of at compile
time, and it is the case this milestone's acceptance criterion forces it to withstand without
falling over.

**And a «development-only» dependency is not harmless for the binary that gets published.**
Development dependencies take part in version resolution: adding `fontique` moved `windows-sys` from
0.59 to 0.60 for `arboard` —the clipboard, which **does** go in the binary— and brought in the
`windows 0.62` stack. On Linux nothing changes; **on Windows, which is F2.7's platform, it already
changes what gets compiled**. The review found it by reading the lock file.

---

## D25 — The color emoji is painted as an image, and every glyph lives in an atlas of its own

**Decision.** A character whose presentation asks for color **is not asked of the text engine**: it
is rasterized separately and painted as an **image**. And every rasterized glyph is pasted into **a
single texture** of 512×512 pixels, not into a texture each.

The two halves come out of different reasons, and both are measured.

### Why as an image and not through the text engine

**epaint 0.36.1 does not know how to rasterize a color glyph.** Its rasterizer takes **only** the
outline (`outline_glyphs`) and paints it with `set_paint(WHITE)`; in its whole text module there is
not one mention of a font's color tables. Checked by reading its source at `text/font.rs:241`.

So registering `Noto Color Emoji` in the fallback chain —which is what **D24** does for CJK and the
icons— **would not give color emojis**. It would give the silhouette. And on this machine it would
not even give that: `Noto Color Emoji` is a **136×128 bitmap with no outlines**, so the engine would
have nothing to draw.

**What there is in the tree is the piece that is needed**: `skrifa` —which epaint already uses
underneath— knows how to pull a glyph's bitmap out, and this project's painter **paints cell by
cell**, so the emoji as an image is one more branch in an `if` that already existed, not a change of
architecture.

### Why an atlas, and the number that decided it

**The first version gave one texture per glyph, and the measurement knocked it down.** epaint's
tessellator cannot join into the same mesh two shapes with different texture identifiers —it says so
itself: «Mixing different `texture_id` in the same…»— so **every emoji on the screen split the draw
batch**.

**Almost three times what drawing costs, and for work that is not the framework's.** With an atlas,
all the emojis on the screen are drawn together. It is what the terminals that are this project's
bar do, and for this reason.

**But the atlas alone was not enough, and the second number says so.** The engine joins
**consecutive** shapes that share a texture, and the painter walks the grid cell by cell: a letter,
another letter, an emoji, another letter… so the draw batch split just the same, now between the
font atlas and the emoji one. The solution is to paint **in two passes** —all the texts and then all
the emojis—, which is the same discipline this painter already separated the backgrounds from the
glyphs with, and for the same reason.

### The whole subtraction, with the control run with the same binary

| The rest of the frame — the figure **D3** judges egui with | Two consoles dumping emojis |
|---|---|
| One texture per glyph | **743.4 µs** |
| Atlas, painting intermixed | 565.9 µs |
| Atlas and two passes | 370.8 µs |
| **Atlas, two passes and the ASCII shortcut** | **356.3 µs** |
| *Control: the same two consoles in ASCII* | *268.8 µs* |

**What has to be looked at in the control is not the difference with the batch, but that it has not
moved.** 268.8 µs with this binary against the 274.3 the previous milestone published: **a screen
with no emojis pays nothing** for the terminal knowing how to paint them, not even the extra pass.
That is what makes the rest of the table acceptable.

With emojis, drawing costs 33 % more and queuing 26 % (289.8 → 366.1 µs). It is the price of **117
emojis per console** on screen, which is a load deliberately worse than any real use.

### The ASCII shortcut, and the defect that almost came in with it

The color pass walks the whole grid to paint a handful of emojis, so it discards ASCII **without
consulting the Unicode tables**. Duplicating a rule that lives somewhere else is only acceptable if
something warns when the two stop saying the same thing, and that something is a test that walks the
128 characters with both selectors behind them.

**What the shortcut saves does not stand out from the noise, and it is said that way.** 366.1 µs
against 377.5 in the painting span: the difference between two runs of the same batch is of the same
order. The 92 µs the extra pass costs are paid by **walking the grid again**, and that the shortcut
does not avoid. An earlier version of this annex attributed them to it — a number with the label
changed, which is exactly what this project says is worse than having no number.

**It was written and knocked down on the first go.** The shortcut discarded **all** ASCII, and
Unicode gives `Emoji=Yes` to the **hash, the asterisk and the ten digits**: `#` followed by the
selector is `#️⃣`. Twelve keys that would have come out monochrome, in the milestone whose point is
that they come out in color. The shortcut now discards only the ASCII **with nothing attached**.

**Going to colour was half of it, and the other half came out in #283.** The drawing was asked of the
emoji font for the digit alone, and `Noto Color Emoji` keeps a digit's own entry as a thin `1` in a
32×78 ink box — 1 216 inked pixels against 14 582 for the keycap on the 136×128 strike. The
keycap is a **ligature of the whole sequence**, reached only by shaping it, so a character carrying
the keycap's mark — one of the twelve keycap bases — is now shaped with `harfrust` — the shaper epaint runs underneath — on the glyph the
atlas does not remember yet, and remembered apart from its bare digit. It stays one cell wide, as
every emoji carrying the selector does. Discarded: drawing the digit over a keycap frame of our own
(a drawing the font does not have is invented), and shaping every emoji sequence (flags, skin tones
and joined families are split into cells by the engine before they get here, so there would be
nothing to shape).

**And a keycap drawn whole was still a speck, because an emoji carrying the selector takes one
cell.** The engine gives `☁️` and `1️⃣` the width of their base character — and so does every program
counting with `wcwidth`, which is why the terminal cannot simply disagree. So such an emoji is
**drawn across the next cell when that cell is blank**, in the console and in the status panel, up to
the two cells a `😀` gets; the grid, the cursor's column and every program's count stay as they were.
Blank means a space painted like the emoji's own cell — same background, same inversion, same
selection — because agent programs hide the terminal's cursor and draw their own as an inverted
space, and a status bar paints its blanks in a colour of its own; inside an inverted bar or a
selection, where the neighbour looks like the emoji's own cell, it still spills. Not into the cell the cursor sits on — it is
painted after the emoji and would cover half of it — reading the cursor's **position only**, never
its blink or whether a program hid it, or the emoji would change size every half-second or on every
redraw — so a program that parks its hidden cursor right after an emoji keeps that emoji in one
cell; and a cursor sitting on a spilled emoji covers both cells, as over a wide character. The one
mismatch accepted: with no colour font, or the atlas full, the emoji falls back to a one-cell
silhouette while a cursor on it still covers two. In the panel the emoji that spill are those whose
base is text by default — colour only because of what follows it, the ones the engine gives one cell
in the console — while `🔥` and its kind keep the room they were laid out in. Written together
with no space (`1️⃣2️⃣`) they stay one cell each: that is the declared price. **What was discarded,
with the user**: widening the grid always, as kitty and WezTerm do, which desynchronises every
program counting with `wcwidth` — line editing drifts and a backspace leaves half a drawing — and
widening only for a program that asks with DECSET 2027, as Ghostty does, which is correct and today
changes nothing a person sees, since almost no program asks. Honouring 2027 later is not excluded by
this.

**Measured** with batch `Q` (its consoles dump emoji, and one in three of its selector emoji is
followed by a space), two runs on `main` and two on this change, the same session, release builds:
the rest of the frame 363.5 · 376.7 µs against 328.8 · 375.1 µs, and enqueueing the paint 374.1 ·
461.3 µs (left) against 352.5 · 446.3 µs — **no measurable change**, the differences inside the
spread between runs. The machine was busier than in the annexes above, so these absolutes compare
with each other and not with those. What does change is the count: 14 colour glyphs rasterized
against 11, the three selector emoji now also drawn at two cells. **No batch walks the panel's
spill**: batch `Z` carries no selector on purpose, so that half is unmeasured and declared so.

### The ceiling stopped being a counter and it is better that way

The design asked for «the memory of the rasterized glyphs has a declared ceiling». With an atlas,
the ceiling is **a fixed-size texture**: the memory the graphics card devotes to the emojis is **1
MiB and does not change**, whether one appears or a thousand. At this machine's cell height about
eight hundred glyphs fit —17 pixels high, 18 wide—, many more than a person sees in a session. When
it fills up **it is said on standard error** and the emojis not already inside come out monochrome.

A counter would have given the same limit with memory that grows up to it. The fixed texture gives
it from the first emoji.

### What is left out, said with its reason

**The color outlines —`COLR`— were left out here, and #176 brought them in** (next section). The
reason given was that painting them required a rasterizer of our own, which this milestone discarded
because it invalidates the whole measurement history.

**The skin-tone modifiers do not either.** They are **wide** characters, so the terminal engine
gives them a cell of their own instead of attaching them to the base emoji's, and the presentation
rule cannot look at the next cell: it does not have it in its signature. The base comes out in color
and the modifier comes out on its own.

### Colour outlines, painted by a library (#176)

**`Segoe UI Emoji`, the font Windows asks for emoji, is `COLR`**: layers of outlines with a palette,
no bitmap. Discovery already reached it — fontique names it as the emoji family on Windows — and the
bitmap-only rasterizer answered nothing, so on Windows every emoji came out monochrome or blank. Two
of the colour tests failed there and were listed as known failures (#188); the others skipped
themselves for want of a colour font, and now run.

**Decision.** A face that has no colour bitmap for the glyph but has a `COLR` record for it is drawn
with `vello_cpu` and its `text` feature, whose glyph painter (`glifo`) understands `COLR` versions 0
and 1 — gradients, composites, clip boxes. The image goes into the same atlas, under the same key,
timed by the same span, and the report says how many of its glyphs came from outlines, so a batch
cannot publish one path's cost under the other's name. **The bitmap is asked first**: a face that
has one for the glyph never reaches the outlines, and the emoji fonts Linux and macOS ship —
`Noto Color Emoji`, `Apple Color Emoji` — are bitmap ones. A machine whose emoji font is drawn in
outlines does change: its emoji go from monochrome to colour.

**What turning `text` on reaches beyond that branch**: Cargo builds one `vello_cpu` for both crates,
so epaint's letters are drawn by a `vello_cpu` that carries the glyph painter's cache and its hooks
too, idle for them. That is why the control `C` is taken again with this build, not borrowed from an
earlier one.

**It is not the rasterizer this decision refused.** What was refused was code of our own drawing
outlines, which would have taken over the letters and so every published frame. This draws only
colour glyphs, on an atlas miss, with a library — the same `vello_cpu` epaint already rasterizes its
letters with, unhinted, which a colour glyph never is. Not the same *path*: epaint builds `vello_cpu` without `text`, so turning it on adds
`glifo` to the build. Saying «epaint already does this» would be false.

**The rules it draws by.** A glyph with no `COLR` record is not drawn here even though the library
would draw its outline — that would be a white silhouette winning over the next face, the defect
**D24** fixed. A graph the library cannot walk is not drawn either: it drops the error and hands back
a blank, which remembered as a success would stand where the emoji is for the whole session. The
glyph is placed on its advance, never on its ink, so every emoji of a face shares one scale and one
baseline. It is drawn directly at the size it will occupy instead of drawn large and shrunk.
Vertically, the box runs between the face's line (ascent to descent) and its `OS/2` Windows extent,
whichever reaches further, because a face promises to keep its ink inside that extent. **The line
alone cut `Segoe UI Emoji`.** Its line is flatter than its emoji (its advance is 1.43 times its
line), so Windows CI drew 😀 touching the top or the bottom of its box. In `Noto Color Emoji`'s
`COLR` form the line and the extent are both 950 up and 250 down, so its box is the same either
way. The figures below were measured before this change and still hold for that reason. A layer drawn «in the text colour» is drawn white: the
atlas is not keyed by colour, and the Windows suite checks four of `Segoe UI Emoji`'s emoji — a
face, a hand, a cloud and a keycap — for a layer drawn that way.

**Discarded.** Walking the paint graph with `skrifa` onto `tiny-skia`: our own painter, and
`tiny-skia` has no sweep gradient, which `COLR` version 1 needs. DirectWrite's
`TranslateColorGlyphRun`: Windows-only code for a problem a cross-platform library solves (**D1**),
and a `COLR` font on Linux or macOS would still be blank. Shipping a bitmap emoji font for Windows:
about ten megabytes, another emoji set than the system's, and every `COLR` face still blank.

**Measured** with batch `colr`, which points the machine's emoji family at a pinned
`Noto Color Emoji` in its `COLR` version 1 form; its control is `C`. The figures are that font's —
heavy in gradients — not `Segoe UI Emoji`'s, which no script here can run. Both measured on
21 November 2026 on the same machine, one tab of two consoles, 43×38 each.

- **Drawing the outlines is paid once, and it is a millisecond.** In both `colr` runs one frame drew
  **14 glyphs, all 14 from colour outlines**, in **1.273 ms and 1.282 ms**: about 91 µs a glyph at the
  cell's size. Bitmap emoji are not comparable from these runs, because batch `Q` was not run with them.
- **A screen with no emoji pays nothing measurable.** Batch `C` was run five times on this branch and
  five times on its base, alternating. The rest of the frame came out at **230–404 µs** here and
  **186–429 µs** on the base. Enqueueing the left console's paint came out at **212–336 µs** here and
  **189–338 µs** on the base. The ranges overlap, so the honest reading is no measurable change.
  **The medians are not equal, though**: 275 µs here against 215 µs on the base for the rest of
  the frame. No code that runs without an emoji was changed. The only candidate is the build itself,
  because the `text` feature is now compiled into the `vello_cpu` that epaint uses for its letters.
  That candidate was not isolated. A later batch that finds the same gap should start there.

### The status panel, by another route to the same glyphs (#178)

**Its text is laid out by the engine a row at a time, not cell by cell**, so there is no cell to decide
in. A row carrying a character whose presentation asks for color is laid out as a job in which that
character and its selector are **transparent** — a transparent section keeps its advance, so nothing
after it moves — and the console's own glyph is drawn where it landed, from the same atlas and by the
same presentation rule, queued after all of the panel's text for the batching reason above. A row
with no such character is painted exactly as before. Three things were discarded: replacing the
emoji with a space (a space is narrower, and the rest of the row shifts), routing the panel's rows
through the console's cell painter (the panel has no column model, and a grid bug would then break
ordinary panel text), and drawing the image over the visible outline (the outline shows around it).

**The glyph is asked at the room it was laid out in, and the atlas remembers it by that width.**
The atlas is shared with the console and used to remember a glyph by character and height alone, so
whichever surface painted a character first fixed its drawing for the other — `☁️` two cells wide in
a console that gives it one, or squeezed into one cell in a panel that laid out two. Remembering it by
width too gives each surface its own drawing, and the price is atlas room, not memory: the texture
stays one fixed mebibyte, but a character can now hold two or three of its roughly eight hundred
places — the console's one or two cells and the panel's room, whose width is the sum of the glyphs'
advances so it does not change with the column the emoji falls in. A failure is remembered once for
every width, since no width makes a face paint in color. Asking the panel
at the console's width instead was tried and taken back, because it needs a second table of
character widths to agree with the engine's and it lets the drawing spill over the letters beside it
wherever the panel's face lays the emoji out narrower. The height is one cell for a harder reason: a
change of height empties the atlas, so a panel asking another would empty it on every frame that
painted both.

**Where it landed is read by index and checked by character.** epaint pads a shaped cluster that has
fewer glyphs than characters, but not one that has more, so past such a cluster the index drifts
while staying in range; a glyph whose character is not the expected one gives no slot and the
character is shown in ink. A shaped cluster stamps its first character on every glyph it produced,
so a selector comes back carrying its base's character.

**Its rasterizing leaves the panel's span by a read of its own.** Reading the color cost empties it,
so the console's read, taken before the panel is painted, never sees the panel's glyphs — moving that
one read later would subtract them from the console, which never did the work, and leave them inside
the panel's span as well. No test can see which span a few microseconds landed in; the placement of
the two reads is the guard. Batch `Z` puts emoji in the panels, with the consoles dumping ASCII, and
its control is `C`. The emoji were placed where their two extra characters wrap nothing — both
documents lay out into the same thirteen rows at the panel width a batch window gives — and none
carries a selector, because the panel notes `U+FE0F` for font discovery, no face egui ships covers
it, and a batch that searches the machine is invalidated as not being `P`.

**What it does not reach**: link labels stay monochrome; the panel's sanitizing, by the contract's
list of what is shown as itself, turns the joiner into a space, so a family emoji is painted as its
separate members; and a single word longer than the panel is cut by characters, which can leave a
character on one row and its selector on the next.

**Measured** (26 September 2026, release, 1280×800, two consoles dumping 44 MiB of ASCII, two runs of
each batch, `compare` says both pairs can be averaged):

| | `C` — panels without emoji | `Z` — four emoji per panel |
|---|---|---|
| The status panel, left pane (mean) | 5.14 · 4.67 µs | 68.23 · 10.16 µs |
| The status panel, right pane (mean) | 2.87 · 2.99 µs | 8.61 · 6.91 µs |
| The rest of the frame (mean) | 208.0 · 214.1 µs | 258.5 · 226.2 µs |
| Colour glyphs rasterized | 0 | 4, in one frame (1.32 · 1.03 ms) |

**A panel with emoji costs 3.9 to 5.7 microseconds more per frame and per pane**, pairing each run
with the plain one taken beside it, which is laying out its colour rows as jobs and queueing four
images. The right pane rasterizes nothing: its panel asks the same glyphs at the same widths and
finds them in the atlas the left one filled. **The 68.23 µs is one frame of the first run and not a
steady state**, so the two runs are not averaged for that pane — `compare` says the windows match,
not that the spans do. It is not rasterizing, whose 1.32 ms is taken off, and it is not the
machine's fonts being enumerated the first time a colour glyph is asked either: the second run
enumerates them too, rasterizes its four glyphs, and shows no such frame. Cold font files on the
first launch after a build are the likely reason and were not measured. **The rest of the frame
moves by 12 to 50 µs** between runs paired in order, a spread as wide as the difference, so it is
published as what it is: the images are one more draw batch, and these two runs cannot say more
precisely what that costs.

### What was discarded

- **Registering the emoji font in the fallback chain**, as is done with CJK. It does not give color: it
  gives the silhouette, or nothing. It is the first thing that was tried and the one that reordered the
  whole milestone.
- **One texture per glyph.** Measured above: it almost triples **D3**'s figure.
- **Rescaling on the graphics card** instead of on the CPU, uploading the bitmap at its original size. It
  would be 68 KiB per emoji in video memory instead of 1.2 KiB, sixty times more, and in exchange for a
  filtering that at this scale cannot be told apart.
- **A rasterizer of our own for everything**, which is option C of this milestone's spread. It stays
  written as a **destination** for when this project changes framework (**D3**), not as a discard.

---

## D26 — The configuration is written by a person, and that is why it is tolerated line by line and in plain text

**Decision.** The file that whoever uses the terminal writes to configure it is **plain text
`key = value`**, with whole-line comments, and **it is tolerated line by line**: the one that is
not
understood is ignored, it is said **with its number**, and the rest of the file is applied.

It lives in `$XDG_CONFIG_HOME/ivy/config`, **next to** the layout and not inside it: that
one is written by the application, this one is written by a person.

### This does NOT reopen D16: it is the side D16 already resolved

**D16 discriminates by author, in these words**: *«Tolerance is per document because this file **is
always written by the application**. It is the opposite of what the panel contract does, and on
purpose: **that one is written by a third party by hand** and losing the whole panel over a field
nobody looks at would be the worst possible thing»*.

The configuration file falls on that second side. A half-finished document in the layout can only
come from an error of our own, and guessing half a layout is worse than not restoring it; here there
is an **exact spare for every line** —the factory value— and whoever writes gets a key wrong.

> **An earlier version of this design proposed «narrowing D16», and it was wrong.** The design's
> challenger uncovered it and it was checked by reading the decision. It is written down because the error
> is instructive: a decision that already covered the case was going to be changed, for not having reread
> it whole.

### Why plain text and not JSON, which is already in the tree

**Line-by-line tolerance is the requirement, and a global syntax error breaks it.** In JSON a
forgotten comma prevents the whole document from being built — and the forgotten comma is the most
common error there is when typing.

And **per-field tolerance does not save you from that**: the panel contract is JSON and tolerates
per field, but it can afford to because **a program generates it**. Here, no.

In plain text the worst case is **one line**. And it takes comments, without which a configuration
file does not explain itself. It is what **Ghostty and Kitty** do, which use this same format — a
confirmation, not the reason.

**TOML discarded**: it would solve the comments but not the global error, and it adds a dependency
to the tree that the root manifest's policy forces you to justify.

### The comment is whole-line, and it is not a simplification

**The hash is at the same time the comment character and a color's prefix.** A comment at the end of
the value splits `color.1 = #ff0000` right before the value, so the first version of the parser
**ate all the colors**. The suite uncovered it on the first attempt, with seven tests in red.

### The version is optional, unlike in the layout

**And the reason is the same one D16 gives for requiring it there.** That file **freezes the shape
of a tree**, so an old reader would misinterpret a new file. This one is flat: an unknown key is
ignored and said, and one that is missing takes its factory value. **There is no shape to freeze**,
so requiring a ritual line would not protect against anything.

What is kept is the half that does protect: **a version greater than the known one is not applied**.

### Under the measurement bench, the file is not read

**And this holds up more than it seems.** The measurement script already isolates the configuration
directory, but **whoever launches the binary by hand with the measurement variables —which is how
the documentation says to open a particular layout— would read theirs**. Their font size would
change the grid, and on the grid depend **all** the figures published by this project.

There is a second consequence that makes it more necessary still: the script **invalidates a run**
if a batch that does not dump in Japanese looks for fonts. With a configured font read under
measurement, **every batch would enumerate and every run would come out invalid**.

The veto is **of the whole file** from day one, even though the colors do not change the grid: «only
these settings» would be a list that forgets to grow.

### What is left out, with its reason

- **Bringing a theme in from another file** (`incluir`). It was cut in the design challenge: it added a
  path, a recursion limit, a cycle rule and a directory convention **for zero acceptance criteria**.
- **A catalog of named themes**, which is what people really share —Ghostty ships more than 460—. It is
  **the first thing that comes in after** this milestone, not a discard.
- **Reloading on save.** No criterion asks for it, and the watcher that exists follows paths fixed when it
  is set up, one per console: it is not usable as it is.
- **Choosing the program each console launches.** As soon as the program or its arguments come from
  configuration, launching it becomes **sensitive surface** and has to be audited as such. It deserves its
  own review, not a corner of this milestone.

### Copying is not a rebindable chord: it is a switch, and until this milestone it did not copy

**The desktop keeps `Ctrl+Shift+C` before it is a key.** It delivers it already turned into a copy
request, and **without saying whether there was Shift**: its check is «command plus the letter C».
So a terminal's copy chord **never arrives as a key**, and the shortcut table cannot reach it.

From there come this annex's two consequences:

1. **The action comes out of the table.** Eleven rebindable actions, not twelve. Accepting
   `atajo.copiar = ctrl+shift+e` would have been publishing a chord that never fires. The key still exists
   —it is the one this terminal already published— and **only takes `ninguno`**, which is what gives the
   chord back to `vim` or to `tmux`. Any other value is rejected saying why.
2. **Copying had been broken since F2.8, and nobody saw it because its test seeded the key.** The request
   fell through to the keyboard table, which translates `Ctrl+C` to `\x03`: **asking to copy interrupted
   the program**. The tests of the milestone that delivered it queued an `Event::Key` with the letter `C`
   —a path that in the real window is not walked—, so they went a whole milestone over dead code. Now they
   queue the event the desktop sends.

**The fix is the same one the project already used for pasting**, and it was written as pending in
the code itself since F2.6: *«whoever brings the clipboard will have to decide it earlier, in the
loop, where the modifier state is available»*. With Shift it copies; without it, `Ctrl+C` still
interrupts. Switched off, the event falls through to the table and comes out as `\x03`, which is
exactly what whoever has just given the chord back to their program expects.

**What is lost, and it is said:** the guard that prevented holding the chord down from copying again
on every frame. The desktop's event **does not carry the repeat flag** —`egui-winit` discards it on
purpose—, so there is nowhere to look. It cost one identical copy per frame while the key was held
down; until today that guard lived on the key path, which is not walked, so in the real window it
**was never active**. Nothing changes by losing it.

### A chord without Ctrl arrives twice, and consuming only one leaves it acting and typing

`egui` emits the key's text **alongside the key** whenever it is printable and there is no command.
The eleven factory chords got away with it by chance: the ones without Ctrl fall on keys that type
nothing. But the file accepts `shift+e` and the document publishes it as valid — and so,
`atajo.partir-al-lado = shift+e` split the console **and typed an `E` in the shell as well**.

Both are consumed: on consuming the key as a shortcut the text that comes behind it is discarded.
The alternative —rejecting the chords without Ctrl on printable keys— was cheaper and took
`shift+F5` and company away from whoever configures, for the same price.

---

## D27 — The whole repository goes in English: code, documents, commits and branches

**This project's own code was in Spanish from the first commit** —7 August 2026—: types, functions,
fields, modules, the names of ten files, the comments, the rustdoc, the 853 test names and their
assertion messages. Also what whoever uses the terminal sees: the configuration file's keys and the
on-screen messages.

**And that decision was not written down anywhere.** This is the first time it appears in this
document, eighteen days after being taken. The project has as a hard rule that *there are no
decisions that live only in the code*, and this one broke it from day one — which is exactly why it
grew without anyone looking at it: there was never a paragraph to go back to.

**What there was in practice was an unstated hybrid.** In English, the crate names and everything
that touches the ecosystem (`Screen`, `Size`, `Point`, `spawn`, what comes from
`alacritty_terminal`, `egui` or `serde`); in Spanish, the domain of our own (`Ventana`, `Pestaña`,
`Celda`, `Desplazamiento`, `RatonReclamado`). The border was real and it worked, but not being
stated it produced lines like `pub struct Screen` with a `pub struct Celda` next to it.

### The decision

**Everything that lives in the repository goes to English**: the code of our own, the file names,
the comments, the rustdoc, the tests, the documents, **the commit messages, the branch names and the
PRs**. And also what whoever uses the terminal sees: the file's keys —`background`, `size`,
`key.split-right`— and the on-screen messages.

**With no compatibility for the Spanish keys, and it is not an oversight**: nobody has this terminal
installed yet, so there is no configuration file out there to respect anything of. An alias table
would be debt from minute zero, maintained for a user who does not exist. It is the only window in
which this change comes for free, and that is why it is done now.

**Spanish stays only in the working conversation**, which does not live in the repository.

### Why

- **The bar this project set itself is that of a WezTerm, Ghostty or Kitty user** (**D19**), and none of
  the three reads in Spanish. A terminal that cannot be read cannot be contributed to, nor audited, nor
  forked.
- **The later, the more expensive.** They are 43 860 lines of our own today, and every milestone adds to
  them.
- **It asks for a clean tree**: during the migration any open branch conflicts on everything. That is why
  it goes right after F2.10 merges and **ahead of its fourth delivery**.

### What was discarded

- **Leaving it in Spanish and writing the D that was missing.** It closes the breach in minutes and costs
  nothing. It is discarded because it does not solve the underlying problem: the code still cannot be read
  by anyone from outside, and the price of fixing it goes up with every milestone.
- **English only from here on.** It is the worst of the three: it leaves two languages mixed **with no
  border to explain them**, which is worse than either of the two pure ones — and it is exactly the state
  this milestone is coming out of.
- **Machine-translating the comments.** This repository is worth what its comments explain, not its lines
  of code: they carry the why of every decision with a lot of nuance —«it lies», «the disguise on file»,
  «the defect and the fix give the same result»— and a mechanical translation turns them into noise. They
  are translated by hand, and that is where the milestone's time goes.
- **Accepting the keys in both languages.** It is what **D26** would do if there were somebody whose
  already-written file had to be respected — but there is not: the terminal is not installed anywhere. An
  alias table is born as debt and is maintained forever for a user who never existed.

### The commits, the branches and the PRs, from now on

**They do not wait for the milestone.** They are written in English from this very decision — they
are new material every time, so there is nothing to migrate and putting it off would only produce
more material to translate. The ones already written stay as they are: rewriting the history costs
more than it clarifies.

**The documents are the exception, and only while the milestone lasts.** A decision in English
inside an `02-decisiones.md` in Spanish reads worse than either of the two pure things. They are
translated **whole**, in one go, in their delivery — they are not mixed file by file.

### What this milestone has to decide before writing code

It is left open on purpose, because deciding it here would be doing the milestone's work ahead of
time:

1. **Where the border lies** with the ecosystem, stated and not merely practiced.
2. **How it is delivered.** A single change of 43 860 lines is reviewed by nobody.
3. **What mechanism for the on-screen messages**, if any is needed beyond writing them in English.
4. **What happens to the already-stored knowledge**, written in Spanish.

### The risk that is not about translation

**The measurement history leans on names that appear in reports and in scripts.** The measurement
bench invalidates runs by what the report **says** —`settings:     vetoed`, the drag counts, the
fonts witness—, and those texts are looked for by guards in `scripts/measure-frame.sh` with
`grep`. Renaming without looking does not break a test: **it breaks a guard**, and a broken guard
publishes a lying run as good. It is the same failure mode this project already paid for once, when
the script's guard was dead and its verification was false.

### The glossary, and the two borders that order it

This is what the milestone had to decide before writing code, and what the nine following deliveries
apply without discussing it again. **It is not a translation table: it is a table of decisions**,
and every row that is not obvious carries its reason alongside.

#### The first border: whose the name is

After this milestone there are no longer two languages, so the border **stops separating languages
and moves to separating owners**: when the word the ecosystem already uses is adopted and when one
of our own is coined. The question survives the translation, and that is why it has to be answered.

> **A new name is decided with three questions, and the first one to give a «yes» rules.**
>
> 1. **Is the type *the* one of a dependency** —re-exported, or a wrapper that has to follow its shape?
>    **Its name is adopted.** `Pty`, `Screen`, `Size`, `Dims`, `TermColor`, `Underline`, `Watcher`.
> 2. **Is it a word whoever uses the terminal already has?** The bar is its user (**D19**), so the
>    criterion is checkable and not a matter of taste: **the word appears in the configuration, the
>    documentation or the action names of two or more of {WezTerm, Kitty, Ghostty, tmux, Zellij}**. If so,
>    **it is adopted even though the type is ours**: `Pane`, `Tab`, `Cell`, `Divider`, `MouseReporting`,
>    `Tiling`.
> 3. **If it is neither of the two, the name is ours** and describes **our** mechanism: `Pump`, `Weight`,
>    `PtyOutput`.
>
> **And one prohibition that governs all three: a word the ecosystem already uses for something else is
> not adopted.** That disqualifies `Window` for the pane —it is what Kitty calls a pane and what tmux
> calls a tab— and `Border` for the divider, which in Kitty and in tmux is the frame *of* a panel.
>
> **And the asymmetry that leaves has to be said, because it is glaring:** `Window` is indeed the name of
> the real window. It is not an exception to the prohibition, it is the prohibition read properly — what
> it forbids is adopting a word **that means something else**, and for the system window `Window` means
> exactly that, the same as in `winit`. The one who uses it for something else is `egui`, where a
> «window» is a floating panel inside the application; that is the name to watch out for when importing,
> not the concept.

**This rule is not new: `term-core` has practiced it since F1.1 and has it written three times**,
all three in the same file, in the comments of the cell, of the mouse reporting and of the scroll.
The one that states it whole is the scroll's: **«Vocabulary of our own, not the engine's, for the
same reason as the rest of this crate: whoever uses it does not have to know its dependencies' types
nor migrate with them.»**

> And here a correction this annex makes to itself, because the decision log cannot carry a false
> citation: the first version said **four** times and included the combining-characters type, which
> **does not carry that comment** —its rustdoc talks about why it does not go inside the cell—; and the
> sentence it quoted was **a graft of two of the three comments**. There are three, and the literal
> citation is the one above.

What was missing was not the rule, it was **stating it and saying where it does not apply**: its
argument has a subject —«whoever uses this crate»— and **in `app` there is nobody**, because it is
not published. There question 2 rules, the user's one, and not the one about the migration nobody is
going to do.

#### When the right name clashes with a `use`, the foreign one gets the alias

Two names of this glossary **do not compile as they are**, and both for the same reason: the module
where the type of our own lives already imports an identical name from a dependency.

| Name of our own | What blocks it |
|---|---|
| `Scroll` (scrolling through the scrollback) | its file imports `Scroll` from `alacritty_terminal::grid` |
| `Watcher` (the panel directory's watcher) | its file imports the `Watcher` *trait* from `notify`, and **it has to be in scope** in order to call `watch` |

> **The solution is not to change the name of our own: it is to alias the foreign one.** A
> `use … as GridScroll` for the type, and a `use notify::Watcher as _;` —anonymous trait import, which is
> exactly for this— for the one that is only needed in scope. The project's vocabulary does not give up
> its place to a dependency's, which on top of that should not be showing: that is what this crate has
> been defending since F1.1.

**And a classification of the first version that was wrong, not just in the way:** the watcher was
listed as a name **adopted** by question 1, «the type *is* the one of a dependency». It is not.
`notify::Watcher` is a **trait** with `watch` and `unwatch`, and the type of our own does not follow
its shape: it keeps a watcher inside and raises a flag. It is a name **coined** by question 3, and
it happens to coincide with the ecosystem's.

**Warnings that do not break today and are worth having written down**, because they are one `use`
away: `Cell` —the sibling file already imports the cell from `alacritty_terminal`—, `Flag`
—`notify::event::Flag` exists, in the same import list that module already uses—, `Row`
—`epaint::text::Row` is «a row of laid-out text», exactly what the type of our own is— and `Theme`
—`egui::Theme` exists, same word and same domain—. None of them collides today; all four would
collide with one more `use`.

#### The second border: what is alive and what is on disk

It was written down too, and in the right place: the enum that says which way a branch tiles exists
**separately** from its living twin because «**the names of this enum are the format**: they appear
literally in the file of everyone who uses the application; reusing the living type would tie the
file to a name the code may want to change tomorrow». The same with the node, the child and the tab,
which exist twice for that reason.

**If the two twins end up called the same in English, exactly that distinction is lost.** So:

> **A type whose names are the on-disk format carries the `Stored` prefix.** The test is mechanical and
> **without exceptions**: does the type derive `Serialize` or `Deserialize` in that module? Prefix.

| Alive | On disk |
|---|---|
| — (`Documento`) | **`StoredDocument`** — the root: its two fields *are* `{"v":…, "pestañas":[…]}` |
| `Tab` | `StoredTab` |
| `Node` | `StoredNode` |
| `Child` | `StoredChild` |
| — (`Hoja`) | `StoredLeaf` |
| `Orientation` | `StoredOrientation` |
| — (`Sonda`) | **`StoredProbe`** — it only reads the version, but it reads it *from the file* |

The first version of this rule **broke itself twice**: it left out the format's root and the type
that reads the version, both with `derive` on them. A mechanical test with two hand-written
exceptions is not a mechanical test, and that is why it does not have them now. The types of that
module that do **not** derive serde —the configuration file's problem, the saving policy, the write
failure, the already-validated document— carry no prefix.

**And the prefix warns about the type, but the format is made by the fields *and the variants*.**
Two enums carry `rename_all`, so **the names of their variants are the file**: renaming the rows
variant leaves the file writing `"rows"` while the measurement script keeps seeding `"filas"`. That
**is not caught by the compiler, nor by the suite, nor by the prefix**: only by the guard of the
batch that restores, and only if somebody runs it. It is this milestone's real hole, and it is
written here so that nobody discovers it twice.

**`panel-contract` carries no prefix**, and it is consistent: **the whole crate is the format**, so its
name is already the label.

#### The pane is a `Pane`, and the gesture is `split`

Checked with source and not from memory, which is what this project demands of any foreign API:

| Terminal | Noun | Where |
|---|---|---|
| WezTerm | **`pane`** | `trait Pane` in `mux/src/pane.rs`; `ActivatePaneDirection`, `PaneSelect` |
| tmux | **`pane`** | `select-pane`, `pane-border-style` |
| Zellij | **`pane`** | `Action::NewPane`, the `pane {}` block of its configuration |
| Ghostty | `split` | `new_split`, `goto_split`, `split-divider-color` |
| Kitty | `window` | a pane is a «kitty window» inside a tab |
| Alacritty | — | it does not have the concept; it delegates it |

**`Pane` for the thing** —three out of five, and `Division` **is used by nobody**: it is the Spanish
form—, and **`split` for the gesture**, which was already committed without anyone noticing: this
very decision wrote `key.split-right` as an example of the new keys, which is exactly what WezTerm
and Ghostty do. The eleven shortcut keys move to `key.*`.

**A gap that is declared instead of being filled:** none of the five has a word for a panel's
**weight** in an n-ary tree. The closest are Kitty's `bias` —**an integer from 10 to 90**, and of a
pair— and Ghostty's `ratio` —a 0–1, also of a pair, and **which on top of that is not user
vocabulary**: it lives in its internal tree of splits and not in its configuration, so **it does not
pass this glossary's own question 2**—. Here the weights are n and add up to whatever. That is why
`Weight` is coined and not adopted: adopting `bias` or `ratio` **would lie about the tree's
arithmetic**.

#### The calque trap, which claimed five names

**A calque that coincides with an ecosystem name and does not mean the same thing is worse than a
free translation.** The rule was written at the start of the glossary and even so it caught five
names already proposed:

| Type | First name, and why it falls | Ends up |
|---|---|---|
| `Resultado` | `Result` belongs to `std` and means something else | **`Outcome`** |
| `Recogida` | `Drain`: `vec::Drain` and `HashMap::Drain` are **iterators**, and this type is a result with two fields | **`Pickup`** |
| `Adjuntos` | `Combining` names the **referent**, not the type: the type is a packed span of 27+5 bits pointing to a separate vector | **`CombiningSpan`** |
| `Reparto` | `Partition` belongs to disks and `Iterator::partition` splits by a predicate; and `Allotment`, to a native speaker, is a rented vegetable garden | **`Tiling`** |
| `Gesto` (the mouse's) | `MouseEvent` names an **input**, and the type is an **output**: the report already says «reports to the program» | **`MouseReport`** |
| `Puente` | `Reader` without implementing `io::Read` lies by convention, and what the type does —bring the output **and the death signal**— is exactly what a `Reader` does not do | **`PtyOutput`** |

#### The tiling of `Layout`, which is the real clash

Three types and one file wanted the same word:

| Today | What it is | Ends up | Why |
|---|---|---|---|
| `Reparto` | the rectangles of the parts of an area **and the lines that separate them** | `Tiling` | «tiling» is the term of tiling window managers, which is this exact domain |
| `Hueco` | the tiling **inside** a pane, between console and panel | `PanelTiling` | **paired on purpose**: its comment says it has a name «for the same reason `Reparto` has one… two tilings that read differently would be **two languages for a single idea**» |
| `Maqueta` | an already-computed text layout | `PanelLayout` | here `Layout` is indeed what it is: laid-out text |
| `Disposicion` | how many consoles each tab opens with | `OpeningLayout` | it is what is **asked for** on opening, not what results |
| `disposicion.json` | the file | `layout.json` | the name whoever opens their configuration folder expects |

#### What is never translated

- **`panel.json`'s keys and values.** And the good reason is **not** that they are already in English:
  they are **a contract with outside producers** (**D17**), so the «no compatibility» licence this
  decision grants **does not reach them**. That licence covers the shortcut keys and the layout file,
  which only this machine reads. Watch out as well for three that **are not named like their field**: the
  key is `v`, `stale_after_minutes` and `ref`.
- **System and protocol identifiers:** `OPOST`, `SIGWINCH`, `TIOCSWINSZ`, `SGR`, `CSI`,
  `XDG_CONFIG_HOME`, `SHELL`, `PANEL_ID`, `PANEL_DIR`.
- **Crate names:** `alacritty_terminal`, `egui`, `eframe`, `epaint`, `serde`, `serde_json`, `notify`,
  `skrifa`, `fontique`, `portable-pty`, `winit`, and our own `term-core`, `panel-contract`, `host-load`.
  (**Not `fontdb`**, which the first version of this list cited: it is not in the tree — this project's
  font crate is `fontique`.)
- **Proper names and labels:** `WezTerm`, `Ghostty`, `Kitty`, `tmux`, `vim`, `htop`, `Nerd Font`, `COLR`,
  `D1`–`D27`, `F1.0`–`F2.12`, and **the batch letters `A`–`R`**, which have no language.
- **And the literals whose *length* is the test load.** The example that paints the check screen places a
  wide character **right on the column-80 boundary**, and its comment says that «without this row, the
  painter branch that handles that gap is looked at by nobody». **That file does not have a single
  test.** Translating that text moves the character and **silently disables the branch**. A text like
  that is measured in columns, not in words.

#### The homonyms, which are not duplication

It was believed that the coexistence of an English `Problem` with two Spanish `Problema` was the
price of not having written the border. **It is false**: one is «what happens to the whole
configuration file» and the other «why a layout document is no good». They are different types in
different modules, and the language was only making them look like relatives. **Neither is merged**
—merging changes behavior under a green suite, and that is another milestone— and the criterion
gives three answers:

- **Same concept, different referent → shared noun and qualifier of its own:** `ConfigProblem`,
  `LayoutProblem`, and `panel-contract`'s `Problem` stays as it is because its crate already qualifies it.
  Likewise `Linea` (the configuration file's) → **`ConfigLine`**, against the panel's `Line`, which stays.
- **Identical concept duplicated on purpose → identical name:** the two test `Directorio` both become
  **`TestDir`**, because their comment says «the same mould the sibling module uses» and qualifying them
  would hide that they are a pattern. Not `TempDir`: it belongs to `tempfile`.
- **Different concepts that shared a name by accident → names with nothing in common:** the two `Gesto`
  are not relatives. The window's one is what **the measurement batch injects into itself** →
  `InjectedGesture`; the mouse's one is what the mouse did in order to encode it for the program →
  `MouseReport`. Translating both to `Gesture` would turn the accident into something intended.

#### The rest of the glossary

**Core of the window:** `Ventana`→`Window` · `Pestaña`→`Tab` · `Division`→`Pane` ·
`Consola`→`Console` · `Nodo`→`Node` · `Hijo`→`Child` · `Peso`→`Weight` · `Frontera`→`Divider` ·
`Orientacion`→`Orientation` with variants `Columns`/`Rows` —**the variants name the result and not
the axis**, which is what its comment defends— · `Lado`→`Side` · `Cierre`→`Closing` ·
`TamañoPedido`→`RequestedSize` · `ConQueAbrir`→`OpenWith` · `Puerta`→`Entry` · `Bombeo`→`Pump` ·
`Orden`→`WindowRequest` (**not `Command`**: it is already taken by what gets launched).

**The family of «what X leaves for the rest of the frame» is four, not seven**, and they share a
suffix because their value is recognizing each other: `DelRaton`→`MouseOutcome` ·
`DeLaRuedaAlPrograma`→`WheelOutcome` · `DeLaBarra`→`TabBarOutcome` · `DeLaDivision`→`PaneOutcome`.

And the other three **are not of the family and do not carry the suffix**: `DeLaRueda`→`WheelInput`,
because it is what the wheel **asked for** and not what it leaves; `DelBoton`→`ButtonPress`, which
is an event; and `DeLaSesion`→`SessionHeader`, which is the report's header.

**Mouse and keyboard:** `RatonReclamado`→`MouseReporting` · `QueHaceElArrastre`→`DragIntent` ·
`QuienSeQuedaLaRueda`→`WheelOwner` · `Donde`→`CellPos` · `Accion`→`Action` · `Acorde`→`Chord` ·
`Atajos`→`Keymap` · `Desplazamiento`→`Scroll` · `Mitad`→`Half`.

**Configuration, session and panel:** `Ajustes`→`Settings` · `Colores`→`Colors` · `Aviso`→`Warning`
· `Avisos`→`Warnings` · `Falta`→`Missing` · `Valido`→`Validated` · `Fallo`→`SaveFailure` ·
`Persistencia`→`Persistence` · `Estado`→`PanelState` · `Reclamo`→`AttentionState` (**not `Claim`**:
in English it means «assertion» or «right») · `Fila`→`Row` · `Vigilante`→`Watcher` ·
`Vigilada`→`Watched` · `Bandera`→`Flag` · `Cola`→`Queue` · `Tema`→`Theme` · `Metrica`→`CellMetrics`.

**The measurement bench** — its rule was already written and is kept: **«it is named for the act and
not for the thing»**, because a type called `Configuracion` inside the measurement module would read
as «the configuration» instead of as «what reading it cost». `Serie`→`Series` · `Tramos`→`Spans` ·
`Medida`→`Timings` · `RestoDelFotograma`→`RestOfFrame` · `Guardado`→`Saving` ·
`Descubrimiento`→`Discovery` · `Rasterizado`→`Rasterizing` · `EnPuntos`→`SizeInPoints` ·
`Reflujo`→`Reflow` · `Barra`→`TabBar` · `Restauracion`→`Restoring` ·
`LecturaDeAjustes`→`ReadingSettings` (**not `SettingsRead`**, which reads as a participle and breaks
the `-ing` family) · `Arrastres`→`Drags` · `ArrastresInyectados`→`InjectedDrags` · `Vuelta`→`Round`
· `CosteDelColor`→`ColorCost` · `Despertares`→`Wakeups` · `Resumen`→`Summary` · `Barrido`→`Sweep` ·
`Prueba`→`TestCase` · `Veredicto`→`Verdict` · `Montaje`→`Harness`.

**The ones the first version left out**, found by crossing the glossary with the types that exist
today: `Opciones`→`Options` (the load meter's command-line arguments) · `Entrega`→`Delivery` (what
dumping the file cost) · `Recogida`→`Pickup` (the result of a pickup from the queue, which was only
deducible from the calque's row) · and the four of the panel contract, internal or from examples:
`Banco`→`Bench` · `Contador`→`Counter` · `Forma`→`Filler` · `Uso`→`Usage`.

**Fonts and glyphs:** `Cara`→`Face` · `CaraDelSistema`→`SystemFace` · `Descubridor`→`Discoverer` ·
`Presentacion`→`Presentation` · `EnElAtlas`→`InAtlas` · `Glifos`→`Glyphs` · `Celda`→`Cell` ·
`Adjuntos`→`CombiningSpan`.

#### Three figures in the body that the inventory leaves behind

D27 was written with the numbers of its day and **the milestone's inventory raised them**. They are
not corrected above —this document adds with a date, it does not edit backwards— but they are here
so that nobody cites them stale: **46 458 lines** of our own and not 43 860, **859 test names** and
not 853, and **eighteen files** with a Spanish name and not ten. The difference is not a counting
error: **F2.10 came in between**, and it is the literal demonstration of the «the later, the more
expensive» that the decision itself uses as an argument.

#### What «whole, in one go» means for the documents

D27 says that the documents «are translated **whole**, in one go, in their delivery — they are not
mixed file by file», and the sentence admits two readings. **The one its own reason holds up
rules**: the example it gives is «a decision in English inside an `02-decisiones.md` in Spanish»,
that is, **the mixture inside one file**. So:

> **«Whole» is each document, and «in one go» is its delivery.** No document is left half-done —that is
> what is forbidden—, and the delivery may carry one change per document. With 3 186 lines in this file
> alone, a single change with the eight documents inside is reviewed by nobody, and what D27 protects is
> not the size of the change: it is that half a document is not read in each language.

It is written down because the opposite reading is defensible reading the sentence alone, and a
decision that admits two readings gets discussed again within three weeks.

#### The files are eighteen, not ten

The decision counted the ten of `app/src` and left out eight: `ajustes.rs`→`settings.rs` ·
`teclado.rs`→`keyboard.rs` · `sesion.rs`→`session.rs` · `fuentes.rs`→`fonts.rs` ·
`medida.rs`→`measure.rs` · `pintor.rs`→`painter.rs` · `tema.rs`→`theme.rs` ·
`vigilante.rs`→`watcher.rs` · `puente.rs`→`pty_output.rs` · `raton.rs`→`mouse.rs` ·
`barrido.rs`→`sweep.rs` · `fuga.rs`→`leak.rs` · `pantalla.rs`→`screen_demo.rs` ·
`lectura.rs`→`read.rs` · `entrega.rs`→`delivery.rs` · `historial.rs`→`scrollback.rs` ·
`api_sin_terceros.rs`→`api_without_deps.rs`, plus `app`'s fonts example.

**And one more that was on no list: `scripts/medir-fotograma.sh` itself**, which becomes
`measure-frame.sh`. Its name is cited in **fifteen files** —three modules, this document and ten of
the stored knowledge—, so it is renamed in the bench delivery and **its prose citations are fixed in
the prose deliveries**: a citation of the old name inside a comment that is still in Spanish breaks
nothing executable, and the alternative is a change that crosses the whole repository.

#### A discrepancy that is declared instead of being resolved in silence

For what the inner program **asks** about the mouse, the standard says *mouse tracking* (DECSET
1000–1003) and the terminals that are the bar say *reporting* (`mouse-reporting` in Ghostty,
`bypass_mouse_reporting_modifiers` in WezTerm). Two reviews gave different names and **both
citations are good**. **D19** rules: its user's vocabulary. It stays `MouseReporting`, and it stays
written that the other option had an argument.

#### What was learned renaming `app`, and was not foreseen

The delivery that renamed the window's family left four things written down, all of them found while
doing the work and none deducible from the plan.

**A rename that wanders off into another file always compiles.** It is the property that makes it
dangerous: the compiler cannot object, because the name is changed in the declaration *and* in every
use. In this delivery the map wandered into three files it had no business in, and each one taught
something different:

- To the mouse's `Gesto` it gave `InjectedGesture`, which is the name this very annex reserves for the
  **other** `Gesto`. What was written up here so that two homonyms would not be confused ended up
  producing the confusion the other way round: **the right name, on the wrong type**.
- To the keyboard's `Accion` it translated **one** variant out of twenty-one. It is the same shape as the
  previous delivery's `Lado` leak, and it survived the same sweep for the same reason: **a regular
  expression over declarations never sees an `enum` variant**. The oracle that does see it is diffing the
  **sets** of names declared before and after, not looking for declaration lines.
- To `Hueco` it gave `PanelTiling` **with the wrong reason**, saying that a rustdoc link forced it. The
  link names a type in the file that was indeed being renamed, so updating the citation was enough. And
  what makes this case the most instructive of the three is that **the conclusion was correct**: the
  tiling of `Layout`, further up, dictates `Hueco`→`PanelTiling` and says that the pairing with `Reparto`
  is on purpose. It was reverted believing it a leak and came back on checking the diff against the
  annex's 84 pairs. Two lessons, not one: **a plausible reason is not a true reason**, and **a correct
  conclusion with a false reason does not defend itself** — the first time somebody audits it, it falls.

**The exemption list is compared with the case on.** The glossary guard carries a list of names that
stay in Spanish on purpose because they are declared by a file of a later delivery. It was compared
in lowercase, and so each entry covered **two different things**: the foreign type and the local of
our own with the same name — `Gesto` and `gesto`, `Nodo` and `nodo`, `Hueco` and `hueco`. The second
is precisely what the guard exists to catch. Rust solves it for free, because the types go in
`CamelCase` and the locals in `snake_case`.

**An exemption that names something of the file itself protects nobody: it hides work.** Two entries
of that list —`ArrastresInyectados` and `QueHaceElArrastre`— are declared in `lib.rs`, that is, in
the file the delivery was renaming. They were there by oversight and their effect was exactly the
opposite of the one the list is after. Rule: **every entry of the list is verified against the file
that declares the name**, not against the intention of whoever wrote it.

**«Finished» is an assertion, and it is split in two when it cannot be held up whole.** The guard
had a single list, `FINISHED`, and it was given the ability to declare **a file** finished so that
it could be pointed at `lib.rs`. Pointing it there showed that it cannot be done: besides the names
that file only *reaches* —declared in files of later deliveries—, there are **499 Spanish
identifiers the glossary never had to decide**, a range's `desde` or an index's `cual`, and that is
without counting the 356 test function names, which are prose. Calling that «finished» would be a
mislabelled number, which is what **D13** forbids.

So there are two lists, and each one says what it really asserts:

| List | What it asserts | What reads it |
|---|---|---|
| `FINISHED` | a crate whose identifiers are **all** in English | the three guards |
| `CITATIONS_CHECKED` | a file whose **citations** are checked, even if its identifiers are not | the citations guard |

**And the second list carries two halves, not one**: the file, and **the crate whose names it may
cite**. A comment in `lib.rs` has the right to name something a sibling module declares; without
that second half, the guard was denouncing seventy perfectly live citations. What was discarded:
giving the guard the whole tree as its universe, which would have made it blind to a citation of a
name from another crate.

What this buys, measured on its first use: **one hundred and fifty-eight comments** that named
identifiers that no longer exist, **two of them doubly dead** —pointing at a name that an *earlier*
rename had already retired and nobody had noticed—. A comment like that is not caught by the
compiler, is not caught by the suite, and whoever looks for the name it gives them finds nothing.

#### What no guard of this milestone can see, said before it bites

Three classes, all three found renaming the window and none catchable by the guard.

**A type name that is also an ordinary word, replaced in prose.** `Hueco` is both things, and the
replacement reached two comments where it was the word: «known measurement gap» —the formula with
which this project declares a span it does **not** time— stopped saying that. The citations guard
reads what goes **between backticks**, and here the name went bare in the text. It was caught by
sweeping the new `CamelCase` names looking for them **outside** backticks, and **that has to be done
by hand in every delivery that renames a type whose name is also a word of the language**.

**A swap of names between two things of the same type.** All the set-difference oracles —declared
names, literals, numbers— are **invariant under a swap**: if `ancho` and `alto` swap names with each
other, the set comes out identical and the compiler says nothing. What does see it is a
**positional** oracle: pairing the changed lines one by one and checking, token by token in the same
position, that each identifier changed to what the chain of renames says. Measured in this delivery:
**5 693 lines paired, 0 unexplained changes**. It comes cheap and it has to be done.

**A test name that cites a method.** `absorber_conserva_la_geometria…` while the method is already
called `absorb`. It is not seen by the subtraction of declared names —it did not change— nor by the
citations guard —it is not a comment—. And it has no mechanical fix: **328 of the 366 test names**
of the big file carry some word this delivery renamed, and almost all of them are the word of the
language inside a sentence, not a citation. They go with the prose, which is a delivery and not a
sweep.

#### And a figure nobody had counted: the prose had been lying for many deliveries

On checking which citations from outside the code this rename killed, the number alongside came out,
which is the one that matters: **599 prose citations already named things the code does not have,
and they are from before this milestone.** This delivery killed 121 more (11 in the decision log, 1
in the configuration one, 109 in the domain knowledge); the decision log itself carries **152 dead
from before** and 11 new ones.

That is why **they are not fixed here**: fixing 11 out of 163 in one file would be papering over 7 %
of the problem and leaving the document looking reviewed. What changes is the brief of the prose and
document deliveries: **they do not translate, they repair** — and now they know how much.


#### The fixing PR: the guard saw **no** uppercase name at all, and asserted what it had not looked at

It goes on its own, between the fifth delivery and the sixth, and for two reasons that do not allow
hiding it inside a rename: **a guard that asserts falsely is worse than none**, and the blindness
fixed here is not the fifth delivery's debt — it has been on since the third, the one that declared
the three outside crates finished.

**Splitting a name before every uppercase letter leaves a constant turned into loose letters.**
`ANCHO_DE_FABRICA` came out as fourteen one-character pieces, and the two-character floor the
checker applies afterwards threw them all away: **no rule was ever asked about any of them**. So a
name in `MAYUSCULAS_CON_GUION` could carry any Spanish word and the guard passed it as clean. The
new rule is one line: **a piece with no lowercase letter is a single word**.

What it taught on being fixed, which is the measurement that justifies the PR: **three constants**
in a module already declared finished (`ANCHO_DE_FABRICA`→`DEFAULT_WIDTH` ·
`CONSOLA_MINIMA`→`MINIMUM_CONSOLE` · `SUELO_AL_ARRASTRAR`→`DRAG_FLOOR`) and **none** in the three
outside crates — those really were clean, which is something only known now.

**A finer rule was written first and thrown away.** Cutting where the *case* changes —`PTYOutput` as
`pty` + `output` instead of `p`, `t`, `y`, `output`— is prettier and **does not change a single
verdict**: the letters it saves are thrown away by the same floor, and the word behind is found just
the same. No name in the tree tells the two rules apart, and this file does not keep mechanisms with
no scenario.

**And the list of finished files asserted «all identifiers in English» of five files of which it was
true of none.** Their families are indeed renamed and they have no name left that the glossary
retired —which is what the list really checks—, but they still carry the **lowercase local the
glossary never had to decide**: a chunk's `resto`, a clock's `ahora`, a header's `fase`. That is
exactly the blind spot the guard declares in its header, and the list was covering it up.

**Counted: 195 distinct identifiers** — mouse 7 · keyboard 30 · painter 27 · fonts 57 · panel 74.
The method, because the number without it is worth nothing: the identifiers of each file are
extracted with the same sanitizing the guard uses (strings, characters and comments out), **the test
function names are removed** —which are prose and go with the prose—, one instance of each name is
kept and it is classified by hand. The list, name by name, goes below. The six exempted on purpose
(`Tema`, `tema`, `color_del_panel`, `color_del_reclamo`, `fondo_del_panel`, `con_atributos`) do not
count. A name that appears in two files counts in both: the inventory is per file, which is the
list's unit.

**And the first count said 190, which is the class of failure this document exists in order not to
repeat.** Four were missing, and all four for the same reason: three were **locals inside a test**
—`solo_clics`, `en_otra_distribucion`, `sin_alto`— that sit **next to a test whose name carries
those same words**, so reading the listing I took them for test names and discarded them; the fourth
was `Texto`, a type from another module that the painter names, and which counts by the same rule by
which `fuentes` counts in three files. It was uncovered by an adversarial review with a single
brief: **recount**. The lesson is not «count better»: it is that **a number produced by hand needs
somebody else to reproduce it**, and that is why the per-file breakdown is published and not only
the total. (The fifth candidate, `A_EMOJI`, does count: its `A_` is the Spanish preposition, the
same as in the `A_TEXTO` next to it.)

**Four names that compile and read badly, found by the fifth delivery's reviews.** All four are of
the same class: **two different things with the same name in sight**, which is exactly what the
glossary exists to prevent and what a mechanical rename cannot see.

| It was | Becomes | Why |
|---|---|---|
| `Action::PageUp` · `PageDown` | `ScrollPageUp` · `ScrollPageDown` | In `(historial(Key::PageUp), Action::PageUp)` the same name is **two types** in the same expression: egui's key and what the application does with it |
| `Action::GoToStart` · `GoToEnd` | `ScrollToTop` · `ScrollToBottom` | They go to `Scroll::ToTop` / `Scroll::ToBottom`, so now **they are named as they end up**; and «start/end» in a terminal reads as the beginning and the end of a line |
| `Action::SplitBeside` | `SplitRight` | D27 itself wrote `key.split-right` as an example of the new keys: the code's name said something different from the key that configures it |
| `Action::SplitBelow` | `SplitDown` | Its pair, for the same reason |
| `Glyphs::paste` | `Glyphs::blit` | Two `paste` with no relation in the same crate: this one **pastes an image into the atlas**, the other is the clipboard |
| `paint_character(width: bool)` | `is_wide` | Two lines from `metrics.width: f32`. A `bool` named like a measurement |

The configuration file's strings —`"partir-al-lado"`, `"subir-una-pagina"`— **are not touched
here**: they are the keys whoever uses the terminal writes and they have their own delivery, which
is the configuration keys one. There they become `split-right` and `scroll-page-up`, under the
`key.` prefix.

**And the citations guard was blind to the module names, for a reason that is not visible reading
it.** That guard asks whether the cited name exists **anywhere in the crate**, and **all** the
retired file names still exist — as a **local variable**: «pantalla» is a `Screen` in two examples,
«fuentes» is a list of faces in three modules, «sesion» is a header in the measurement module,
«vigilante» is a watcher in the module now called `watcher.rs`. So a comment that says «the rule
lives in fuentes» **passes**: the guard finds the local and takes the citation as good. And widening
it would never have found it, because the name it is looking for **really is there**.

What makes it possible to close is that the **old** names are known: the annex writes every file
rename as an arrow, so the question stops being «does this name exist?» and becomes «is it one we
retired?», which has an exact answer. The new guard **derives the seventeen stems from this very
document** —the same mechanism as the glossary: derive, never duplicate— and looks for their
citations `` `así` ``, `` `así.rs` ``, `` `carpeta/así.rs` `` and `` `así::funcion` ``.

It found **eleven** —measured by running this guard against main's tree, which is the only way for
the number to be reproducible—, and **nine were real**: two in `app`'s manifest, three in the
painter, one in the big file, two in the guard itself and one in the measurement script
—`vigilante::repartir`—. The other two were not file citations: one was a word between backticks and
is rewritten as a word, and the other —«the other tests reach as far as `pantalla`»— meant a type,
so it moves to naming it (`Screen`). Rewriting one and renaming the other is not the same operation,
and the guard's script says both.

**It reads the code's side and not the prose's**, and that border is a decision already taken: the
599 dead citations from before the milestone have their delivery, so `docs/`, the two `README`s and
the stored knowledge belong to them. What is left here is the code, its manifests and the
measurement script — where a dead citation **has no delivery** and would stay.

**And there is a second half of the same gap that is NOT closed with a guard, and it is worth saying
why.** The same mechanism —the universe of names is the whole crate— lets through a dead citation of
a **field or parameter** as soon as a sibling module has a local called the same: `caras` survives
in `fonts.rs` because the fonts example has a local `caras`, `problema` survives in `panel.rs`
because `session.rs` has one, `pegados` survives because the painter has one, `marcado` because the
big file has one.

Narrowing the universe for a lowercase citation —«it has to exist **in this file**»— was tried and
measured, over the seventeen files of the citations list and with the same criterion on both sides:
**61 warnings on main, 43 on this branch**. The difference is the citations this PR repairs. And
**of the 43 that are left not one is real**: they are crate names (`eframe`, `app`, `keyboard`),
methods of a sibling module that a comment has the right to name (`arrancar`, `resolve`), contract
fields that arrive from another crate (`done`, `mark`, `style`, `info`, `current`), single letters
(`a`, `c`, `i`, `y`), things from `std` (`next`, `unwrap`, `text`), the value of an environment
variable between backticks (`consola`) and configuration file keys (`tamano`).

**With 43 out of 43 false it cannot break the compilation**, and the exception list that would be
needed would be the mechanism widened until it stops failing. What does change: **the 19 true
warnings are fixed now** —eleven names in ten lines of the big file, three in fonts, one in the
panel, one in the keyboard, two in the examples and one in the guard itself—, and the criterion is
noted down with the condition that makes it usable: **when the big file, the measurement, the
settings and the theme are in English**, «it exists in this file» stops being noise, because then a
lowercase name that is not in the file is no longer a sibling's Spanish local. That is the delivery
that can add the guard.

**And the list of files with checked citations said «all the files of the crate» with two exceptions
nobody had written down.** The binary was missing and it comes in. The guard itself does **not**,
and that is the answer and not an oversight: putting it in gives **65 citations** just as this
delivery leaves it —and the number **grows with the file**, because what it reports is the
vocabulary and the shapes that file talks about: retired names (`banderas`, `RatonReclamado`),
example names (`CamelCase`, `like_this`, `Type`) and ordinary words between backticks. A guard whose
subject is the names that are no longer there cannot be judged by «it cites a name the code has»:
**of the 65, one was real**.

**The one that was real came out of reading the 56, and it is worth writing down:** the comment that
explains the citations guard named three live identifiers, and one —`prioridad`— **had disappeared
from the whole tree**. Which means that the sentence explaining the guard was one of the citations
the guard describes. One in 65 is not a guard, but it is a good sweep. What **can** be aged on it is
a file name, and the new guard reads it for that: it found two.

The list is renamed for what it checks —**«files with no name that the glossary retired»**— and the
195 go to the big file's delivery, which is the one that already had that brief for its neighbors.
**And the lesson is written whole**, because it is this PR's: *a list that exaggerates what it
looked at is read once, is believed, and the next delivery uses it to decide what is done.* That is
exactly how five files ended up declared finished.

#### What the adversarial review found in the new guard, and it is half the PR

A mutator with a single instruction —«prove that something in this diff is not covered»— returned
**six gaps, each one with its surviving mutation executed**. None was visible reading the code, and
one of them is the disguise this project already has on file.

**1. The file-citation reader was exercised by no assertion.**
`fn cites_the_file(..) -> bool { false }` left the eight tests **green**, and so did breaking each
one of its four shapes separately.
The reason has to be written down because it is general: **the only test that calls it looks for
citations, and the tree is clean — so breaking the finder leaves the suite greener, not redder.** A
finder needs an oracle of its own; the absence of what it finds is not one. In comes
`the_reader_of_file_citations_knows_the_four_shapes`, with the four shapes and three negatives.

**2. The floor of seventeen arrows counted, it did not identify.** Returning the seventeen stems
with a suffix stuck on —seventeen names that do not exist— left the floor happy and the test passed
without looking at anything. It is exactly «asserting about the count instead of about the effect».
Now **four stems are asked for by name**.

**3. The two-character floor was held up by nothing.** Raising it to 3, to 5, or **removing it
entirely**: the eight green. With five, the guardian stops seeing `celda`, `hueco`, `serie`, `nodo`,
`fila` and `mitad` inside any compound — and that number is the one the partition's new comment uses
to explain the defect it fixes. Two new cases, one per value: a retired name of three letters and
another of five. Lowering it has no case and it is said: removing a floor only hardens.

**4. «No lowercase» is not «all uppercase», and the difference is a digit.** Written
`chars().all(char::is_uppercase)` the test passed just the same, and the defect stayed **reopened
for every numbered constant**: the digit breaks the condition, the piece is split into loose letters
again and the floor throws them away. New case with a number inside.

**5. The root manifest covered up the crates'.** A single question «is there any `.toml`?» was
satisfied by it alone, so removing the `.toml` files from the walk stayed green — and **two of the
dead citations this guard found lived in a crate's manifest**, not in the root one. Now both are
asked for separately.

**6. The two lists could be emptied without anything failing.** Emptying the clean-files one: green,
because the finished crates kept giving targets and the «this has no files» guard is never asked of
a list **with no elements**. And removing from the citations list any entry —including the one this
PR adds— too: a file that comes off the list simply stops being looked at, and nothing misses what
it does not report. The citations list becomes **derived from the tree** («all the files of the
crate» is checked, not believed, with this file as the only written exception) and the two name
lists carry a cardinality floor.

**And deriving it from the tree claimed two more dead citations**: when `app`'s four examples came
into the list, `` `resolver` `` came out in two of them, when the method has been called `resolve`
since the previous delivery. Plus two foreign names that had to be declared (`yes`, the program;
`Zsym`, a Unicode script code).

**The ten surviving mutations were run again one by one: all ten fall now.**

##### The 195, name by name

**They are gone**: they were renamed by the eight-modules delivery, which is the next section. The
list is kept because it is the measurement with which it was decided to split that delivery in two,
and because the per-file breakdown is what let somebody else reproduce the number.

**`mouse.rs` — 7:** `abajo` · `apretar` · `arriba` · `dentro` · `fuera` · `solo_clics` · `soltar`

**`keyboard.rs` — 30:** `antes` · `codigo` · `cual` · `cuerpo` · `en_otra_distribucion` · `escrita` · `escrito` · `esperada` · `esperado` · `historial` · `hostil` · `letras` · `mando` · `nuevo` · `otro` · `parte` · `pelado` · `pisada` · `publicado` · `queda` · `repetida` · `salida` · `salto` · `sin_publicar` · `soltar` · `suyo` · `tabla` · `veneno` · `venenos` · `viejo`

**`painter.rs` — 27:** `Texto` · `debajo` · `dibujo` · `esquina` · `fondo` · `fondo_apagado` · `fondo_realzado` · `fuentes` · `hubo_color` · `letra` · `letra_realzada` · `malla` · `muchas` · `pasada` · `pegados` · `primera_imagen` · `proporcion` · `que` · `rejilla` · `rejillas` · `salida` · `sin_selector` · `suyo` · `tamano` · `textos_sin` · `ultimo_texto` · `una`

**`fonts.rs` — 57:** `A_EMOJI` · `A_TEXTO` · `Texto` · `a_buscar` · `alfa` · `alto_final` · `ancho_final` · `ancho_maximo` · `antes` · `buscados` · `canal` · `coleccion` · `comun` · `coste` · `crudo` · `cuadrado` · `cuadro` · `cubre` · `datos` · `definiciones` · `delante` · `despues` · `empezo` · `fallidos` · `familia` · `fotograma` · `fuente` · `fuentes` · `gastado` · `genericas` · `glifo` · `grande` · `icono` · `letras` · `mapa` · `mascara` · `nombre` · `origen` · `otro` · `pendientes` · `pequeno` · `pixeles` · `primera` · `registradas` · `resueltos` · `se_rindio` · `segunda` · `selector` · `siguiente` · `sin_fuentes` · `sin_presupuesto` · `sitio` · `sitios` · `suya` · `suyo` · `todas` · `uno`

**`panel.rs` — 74:** `a_pintar` · `ahora` · `anchos` · `bloque` · `cabe` · `cabeza` · `callado` · `caso` · `casos` · `clave` · `contenido` · `corte` · `cual` · `cuando` · `cuerpo` · `dicho` · `disponible` · `entera` · `envejecer` · `estrecha` · `fase` · `fecha` · `fichero` · `fondos` · `frase` · `fresco` · `fuente` · `fuentes` · `identidad` · `izquierda` · `larga` · `largo` · `marcas` · `minutos` · `muchas_enes` · `otro` · `palabra` · `paso` · `pegada` · `pendiente` · `pintado` · `pintando` · `primera` · `publicada` · `publicado_hace` · `publicar` · `recien` · `recortada` · `referencia` · `relleno` · `repetida` · `resto` · `ruta` · `salida` · `saneado` · `sangrada` · `segunda` · `segundos` · `siguiente` · `sin_alto` · `sitio` · `sobran` · `suyo` · `tal_cual` · `textos` · `todos` · `tope` · `tres` · `ultimo_bueno` · `valor` · `viejo` · `vigente` · `visibles` · `visto`

#### The eight-modules delivery: 340 names, and why the delivery was split in two

**The plan said 419 names and they were 638.** The plan's figure was written before anyone had
counted the five files the fifth delivery declared finished; with their 195 inside, the delivery
that was going to carry «the big file and the three modules with no delivery» turned into **nine
files and 33 000 lines of source touched**, twice the fourth delivery, which already left three
scope leaks with seven oracles in front of it. It is split in two, and the cut is not by size but
**by product**:

- **The eight modules of this delivery**: mouse, keyboard, painter, fonts, panel, theme, watcher and
  pty_output.
- **The big file on its own**, which is one file and 23 142 lines, and the one with the most tests in the
  repository.

**340 names**, one by one: panel 74 · pty_output 60 · fonts 53 · watcher 47 · theme 43 · keyboard 30
· painter 26 · mouse 7. Four of them have **two destinations** because they named two things —the
`SourceCache` and egui's fonts view were both «fuentes», the bitmap and the charmap were both
«mapa», the reduced image and a test size were both «pequeño», and the chunk that comes out of the
channel and the byte count the frame takes were both «trozo»—, so the map **is not fit for a `sed`**
and that has to be said before writing it, not after.

**And the count by hand failed again, twice, in the same direction.** The three modules the fifth
delivery renamed the file of but not the contents were estimated at 99 names, then at 123 with a
filter, and they are **150** (theme 43, watcher 47, pty_output 60). Both times the error was the
same: **filtering the identifier list before reading it**. The filter discarded what was already in
the repository's English corpus, and with it went `con`, `marca`, `estilo`, `base` and a dozen more
short names that turn out to be words of both languages. The lesson, written down because it already
takes three attempts: **the complete list is read whole or it is not read**; a filter that saves
reading work saves exactly the work that finds the names.

**The method that does converge, and it is the one to repeat in the big file's delivery:** apply the
map, **scan the file again**, and what is left in Spanish is either an oversight or a sibling's
name. Theme uncovered it: on rescanning, `brillante` came out, a test local that the context listing
had shown me only in three comment lines.

**What is left, and whose it is.** Of the eight modules, seven keep **not one** identifier in
Spanish. The eighth, `theme.rs`, keeps six: `Colores` and the five fields it reads from it
—`paleta`, `texto`, `fondo`, `divisor`, `foco`—, which are declared by `settings.rs` and **belong to
the configuration keys delivery**. `Colores` goes into the exemption list for that reason; the other
five are not needed, because the glossary never decided them and the guard does not see them.

**And the framing this delivery was written with was false, said because it is the fault this
milestone has been punishing.** «All of `app` except its big file» reads as if the surface were
closed, and **`app/src` has twelve modules, not nine**: besides `lib.rs`, still entirely in Spanish
are `measure.rs` (96 Spanish roots), `session.rs` (93), `settings.rs` (57) and `main.rs`, which is
four lines whose only job is `lib.rs`'s. Each one has its delivery written —the measurement and the
on-disk format, the configuration keys, and `main.rs` with the big file—, so **nothing is missing
from the plan**; what was missing was the sentence. It was caught by an adversarial review that was
asked to refute the assertion, not to check it, and that is the difference between the two things.

**Eight files on the guard's list, and not one new assertion.** The eight go into the «files with no
name that the glossary retired» list, which is what the guard **checks**. The stronger assertion
—«all their identifiers in English»— is true today of seven of them, and **it is not turned into a
list**: the guard cannot verify it, and this milestone has already closed a whole PR over a list
that asserted what nobody had looked at. It is written here, with its method and its residue, which
is where a measurement lives.

**The exemption list goes from 27 entries down to 20.** Out go the eight that existed because
`theme.rs` and `watcher.rs` were not translated (`Tema`, `tema`, `con_atributos`, `color_del_panel`,
`color_del_reclamo`, `fondo_del_panel`, `Bandera`, `Vigilante`) and `pty_output.rs`'s (`Puente`); in
comes `Colores`. Every line that goes out is a line somebody had to delete, which is what the list
is written that way for.

**Three findings no mechanical rename could give, and all three were given by an adversarial review
before any code was written:**

1. **A test helper with no `#[test]`** in `watcher.rs` —`un_vecino_del_directorio_no_levanta_nada`— which
   the guard **does** see, because it carries a Spanish functional word, and which would have broken the
   compilation the instant the file entered the list. It looks like prose and it is not.
2. **Two things with the same name in sight, both already in English.** In `fonts.rs`, a local `attached`
   counting squares pasted into the atlas, two hundred lines from a parameter `attached` carrying a cell's
   zero-width characters. **No guard can see it**: both are English. It becomes `pasted`.
3. **The citations guard's documentation aged again.** It named three live identifiers to explain why its
   universe is a crate and not the tree; one had gone before the previous PR, and the second —`parar`—
   goes with the module that declared it. **Twice is a pattern**, so it is written down in its place: *a
   guard's comment ages just like the code it watches, and it has no guard*.

**And a warning about the universe of names, which is the other side of the same thing.** Three dead
citations appeared **without this PR writing them**: `` `tamano` `` in the settings, `` `nuevo` ``
in the big file and `` `parar` `` in the guard itself stopped being covered the moment the name that
was covering them left a sibling module. They are not regressions of this delivery: they are the
ones that **were already lying** and survived because the universe is the whole crate. Every module
that gets translated uncovers its own, and that means **the remaining deliveries are going to find
more, not fewer**.

**What the delivery's four reviews changed, which is half its value.** None found a behavior change
—the positional oracle paired **1 667 lines token by token with zero mismatches**, zero moved string
literals that are not a format capture and zero numeric literals—, and even so the PR came out
different from how it went in:

- **Three dead citations the guard did not see, and a new gap in the citation reader.** A citation written
  `` `Tema::con(...)` `` came back **empty**: the reader only removed a `()` with no arguments, so the
  all-or-nothing rule discarded it whole and `Tema` was never asked about — a type that no longer exists
  in the tree. Accepting the `nombre(argumentos)` shape gives **four warnings, three of them real**, and
  all three name things a delivery renamed two PRs ago (`Lineas(0)`, `Ir(1)`, `desplazar(AlFinal)`).
  **The wide version of the same idea was measured and thrown away**: splitting a citation on the spaces
  and asking about each word gives **309 warnings and not one real**, because `cargo doc`, `panel.json`
  and `Mayúsculas` are words between backticks that cite nothing. Four against 309: the line goes in, the
  other does not.
- **Two citations that are not read even with that**, and both are still fixed by hand:
  `visibles + sobran` and `Tema::con(...)` carry spaces or arguments, and that gap **is not the declared one** in the
  guard's header —the «it survives in a sibling module» one—: here the name survives in none, it simply is
  not read.
- **Four new names that lied, and they came out of reading what they name.** `to_paint` was the worst: its
  own comment says that **the painting does not use it** and that leaving it in production would be an
  open door, so the name asserted exactly the role the comment forbids; it becomes `last_understood`.
  `dispatch` dispatches to nobody: it raises flags and returns a boolean, so it becomes `raise_matching`.
  `answered` reads as «already attended to» and the value is a claim **nobody** has attended to, so it
  becomes `answer`. And `cap`, unqualified in a long function, becomes `head_cap`.
- **Four names for two concepts in fifteen lines.** The theme received `faint: bool` while its caller
  passes it `cell.dim`, and inside coexisted that `faint`, the `Dim`/`DimForeground` variants and a
  `dim()` method whose comment says «brings a color closer to the background». The parameter is renamed
  after the field it receives (`dim`) and the method to say what it does (`toward_background`).
- **A coverage gap predating the milestone, closed because this delivery touches that function.** Of the
  six colors the configuration file can change, **the cursor's was checked by nothing**: swapping its
  assignment for the focus one left the 718 tests green. Now it is asserted **through
  `TermColor::Cursor`**, which is the door through which the painter asks for that color — asserting about
  the field would check that the value is noted down, not that it is used.

**And two homonyms that are accepted knowingly, with their reason written.** This project's `Flag`
type coexists with `notify::event::Flag`, and the `Watcher` type with the `notify::Watcher` *trait*,
which is why it comes in without a name (`as _`). Both names are **decided by the glossary**
(`Bandera`→`Flag`, `Vigilante`→`Watcher`) and both clashes are resolved with ordinary Rust idiom
—full path and `as _`—, so a decision is not reopened without a new reason. What **is** stated is
the price: a future `use notify::Watcher` in that file does not compile, and whoever writes it has
to know why.

**A warning this very section earned while being written, and it has to be told here because it is
the price of deriving.** The guard **pulls the vocabulary out of this document**, and it is enough
for it to find a name between backticks, an arrow and another name between backticks to put the
first one into the retired vocabulary — even if the sentence is ordinary prose and the arrow,
punctuation. It happened: an explanation of why a function was changing its name put Rust's boolean
type on the Spanish side, and the guard reported **101 identifiers** in the three outside crates,
all of them that type. It is fixed **by rewriting the sentence and not by weakening the extractor**:
that sensitivity *is* the mechanism, and loosening it so that the prose fits is exactly how a
derived glossary stops deriving. The rule of use is stated: **in this document, an arrow between two
names is vocabulary, not punctuation.**

**And it happened twice.** The second was while writing, two sections further down, the explanation
of why the guard does not protect the 317 renames: the sentence used two example names with an arrow
in between to *show the shape*, and the extractor read it as vocabulary just like the first time.
Which means that **not even knowing the trap avoids falling into it**, because the shape that has to
be cited is exactly the one that fires. The practical conclusion, and it is the only one that works:
**in this annex the shape of an arrow is never cited with names; it is described in words.**

#### The big file's delivery: 317 names in 23 143 lines, and what was learned measuring

`crates/app/src/lib.rs` is a file of **23 143 lines** with **1 599 distinct identifiers** and **356
test functions**, and the one with the most tests in the repository. It goes alone in its delivery,
and the cut was not by size but because *«all of `app` except the big file»* and *«the big file»*
are two different products.

> **Checked on 28 September 2026: the file had 23 142 lines**, the figure the eight-modules section
> gives, both before this delivery and after it (`git cat-file` over the revisions on either side).
> The 23 143 above is kept as written; no revision that reached `main` had it.

**It was split into five areas by first appearance of the name** —constants and geometry, the
console, the tree and the tabs, the window, and the 16 000 lines of tests— and each area had its own
map proposal. **The five came out mutually consistent: 317 names and zero conflicts**, not one with
two incompatible destinations. That was not guaranteed with five reviews blind to each other, and
the reason is the one this annex keeps repeating: they reused the glossary and the precedents of the
eight modules already done instead of coining anew.

**Twenty-three names mean two or three different things depending on where they are**, and that is
this delivery's real work. `alto` is a `y` coordinate in the three gesture helpers and a height in
the rest. `cuantas` are weights in the tiling, consoles on opening and tabs in the bar. `suyo` has
**six** destinations: an index, an area, a cost, a size, a result and a group's weight. A single
destination would make the name **lie** everywhere but one, which is exactly what the previous
delivery had to fix four times.

**So the tool changed and the method with it.** The renamer now accepts **a line range per entry**,
and the ranges **are not written**: they are derived from the `fn` headers, which are the real unit
of scope of a local. Writing them by hand from a report already went wrong —the numbers age as soon
as somebody edits the file— and in comes a checker whose only question is the one that matters: **is
there any occurrence left with no destination?** A name that stays in Spanish because its range did
not cover it **compiles just as well**, and neither the compiler nor the guard sees it.

**And the tool that measures this milestone had a defect, found by an adversarial review and not by
me.** The sanitizing that separates code from prose **was eating the line breaks** of the comments
and the strings, so **every subsequent line came out shifted**: 120 positions in this file. A range
computed on that renames the wrong thing **silently**. Fixed and checked in the 31 files of the
tree: the breaks that go in are the ones that come out. And with it, a second blind spot of the same
scanner: its identifier pattern was **ASCII**, so it split in two the four names with an ñ and did
not count them. Checked afterwards: the only non-ASCII identifiers in the tree are in this file, the
measurement and the session, so **it hid nothing in the eight modules already merged**.

**Out of the compiler came thirteen errors from 2 561 substitutions, and all thirteen of the same
class:** a field or a method declared by the measurement, the session or the settings, renamed at
its call site here. Returned to the name their module still gives them. **The guard caught another
22 dead citations** that the tool does not move, because they live in line comments and not in
documentation links.

**Two new names lied and were changed by hand**, and both are the class no guard sees: `absorb` had
**two different `dentro` in twelve lines** —the branch's direction and the list of children—, which
a single destination would have turned into two things with one name in sight; and `subidas` was a
list of scrolls in one function and the granddaughters that get promoted in another, where
`scrolled` was false.

##### What putting the big file on the list does NOT buy, measured

**And here goes this delivery's most important correction, because a review found it and not me.**
Putting `app/src/lib.rs` on the «files with no name that the glossary retired» list **protects none
of the 317 renames**. The reason is in the mechanism: the Spanish vocabulary is **derived** by the
guard from **this annex's arrows**, and **none of the 317 old names is an arrow** — they are locals,
fields and parameters, and the glossary decides **types**. Proven by mutating: returning `mid_y` to
`alto` in the four places of one function leaves the guard at **9 of 9, green**.

So the honest assertion is exactly this one and not a word more: **the file keeps no name that the
glossary retired**, and the 28 exemptions were needed because those 28 **are** in the annex's
tables. A return of any of the 317 is **invisible** to the guard, and saying otherwise would be the
list that looks watched and is not — the disguise this document has had on file since the fourth
delivery.

**What this delivery did protect, and with numbers:** the positional oracle. 1 855 hunks with not
one unbalanced, 2 617 lines paired one by one, **3 208 identifier substitutions across 368 distinct
pairs, 99.5 % matching the maps exactly**, zero operator or punctuation changes, zero numeric
literals and **76 strings that change, all and only in format captures** —with each `{nombre}`
replaced by a gap, the 76 pairs come out byte for byte identical—. That is what has to be repeated
in the remaining deliveries, and not a list.

**And the citation reader does not learn two more shapes, also measured.** It missed ``
`division.redimensionar()` `` and `` `rectangulos[0]` ``, and accepting the dotted access and the
bracketed index gives **45 warnings, of which 26 are `panel.json`** and the rest `lib.rs`, `.ttc`
and `fs.inotify.max_user_instances`. Two or three real against forty of noise: **it does not go
in**. It is the third time an extension of that reader has been measured and the second it has been
rejected, and the three figures stay written —4 against 309, and now 45— because the criterion is
the proportion and not the occurrence.

##### The exemption list, and the disguise that repeated itself for the third time

Putting the big file on that list cost **28 exemptions**. They are fields and methods declared by
the measurement, the session and the settings that this file **only uses**: the guard judges a file
by the identifiers that **appear** in it, not by the ones it declares. And the number that matters
is another one, measured by a review with an analyzer of its own: **the file names 55 foreign
Spanish identifiers**, and only 28 need an exemption — the other 27 are caught by no rule, because
the glossary decides types. They are verified one by one against the file that declares them,
grouped by owner, and **every line is deleted by its module's delivery** — the list is the progress
marker its own comment says it is. And **the 28 are load-bearing, measured one at a time**: removing
any of them leaves the suite red, always through the same test and always pointing at this file.
Zero decorative exemptions, which are the four this milestone has already deleted.

##### A new class: the hand rename that eats the prose

**And this delivery's real defect was mine and not the map's.** Renaming a parameter called `a` to
`to` over a range of lines reached **the Spanish preposition «a»** inside a string literal and in
eleven comments. The literal is the one the report prints when the visible tab has no focus, so **it
came out in every measurement batch**: «el teclado no llegaba a ninguna consola» had turned into «no
llegaba **to** ninguna consola».

**Nothing saw it.** The bench's script has no pattern for that line, and the only test that touches
it asserts about the prefix «NO EXISTE», so the corrupt tail went through. It was found by a review
reading the diff.

The class is new and worth naming, because this milestone's tool exists precisely to avoid it: **the
tool tells code from prose; a `re.sub` over a range of lines does not.** All of this delivery's hand
fixes —and there are about thirty— were done that way, and the only one with a one-letter name is
the only one that ate a word. The rule that stays: **a name of one or two letters is not renamed by
line range**; either it goes through the tool that knows the regions, or it is looked at line by
line.

**And my own check of that was worth nothing**, which is the most useful half of the finding: I
looked at the diff **after** committing, so it came out empty. Twice in a row. A sweep done over the
wrong state of the tree is worse than not doing it, because it publishes a zero. The sweep goes
against the whole branch —`main...HEAD`— and **through the proxy of the tool that wraps `git`**,
because that wrapper **summarizes** the diff output and that is what returned zero changed comments
when there were sixty-four.

**And one thing the delivery did NOT achieve, said because the draft asserted the opposite.** The
file was left with **one identifier of its own in Spanish**: `en`, the preposition, in four places
it declares itself —a test helper with fifteen calls and three `Pos2` parameters—, plus a parameter
`a` in two others. The guard does **not** see them **by design**: the two-character floor throws
them away before they reach any rule, and that is written in its header since the first delivery.
They were already like that in `main`, so this rename did not introduce them; they were found by a
review that was asked to **refute** the assertion, and they are fixed here. The lesson is the usual
one and that makes three: **an assertion is given to somebody to break it, not to confirm it.**

**And adding them broke two cases of the rules test, which is this delivery's finding.** The
`pestañas` exemption —which is needed, because the session declares that field— exempted the
`pestanas` case along with it, which is **the only possible subject** of the accented-plural rule.
And the `gesto` exemption switched off the case that demonstrates that the list cannot exempt a
file's own local. Both times: **green, checking nothing**.

That makes three —`color_del_reclamo` in one delivery, `pestanas` and `gesto` in this one—, so it
stops being a fix and becomes mechanism:

1. **The exemption compares the whole name and as it is: neither the case nor the accents are folded.**
   And the distinction is not a patch: **a name with an accent and the same one without it are two
   identifiers**, and this repository writes some with and some without on purpose.
2. **The type/local pairs are derived from the list** instead of being written, filtered by what the
   glossary really retired, so the set adjusts itself as the list grows.
3. **Every positive case of that test is checked against the list, and first**, so that the message says
   the cause —«somebody exempted this case»— and not the symptom, which was thirty lines further down.

Three mutations tie it down: exempting a positive case, folding the accents again and folding the
case again. All three fall, and each one names its own cause.

#### The keys delivery: what whoever writes the file sees, and a guard that deletes itself

It is the delivery this decision used as an example from day one —`background`, `size`,
`key.split-right`— and the only one that changes **what a person types**. The keys end up like this:
`background`, `text`, `cursor`, `divider`, `focus`, `panel`, `color.0`–`color.15`, `size`,
`version`, and `key.<action>` with the eleven names from the code (`split-right`, `split-down`,
`close-pane`, `new-tab`, `close-tab`, `previous-tab`, `next-tab`, `scroll-page-up`,
`scroll-page-down`, `scroll-to-top`, `scroll-to-bottom`) plus `key.copy`, which can only be switched
off. The value for switching off is `none`. And they are the six the eight-modules annex left noted
down in `theme.rs`: they are translated here, as it said.

**Two accommodations are withdrawn without replacing them, and what is lost has to be said.** The
first were the two spellings of the size (`tamaño` and `tamano`); the second, the key and modifier
names in Spanish —`mayúsculas`, `supr`, `repág`, `avpág`, `inicio`, `fin`, `espacio`, `izquierda`…—.
Both existed because of the same problem: **an ñ that is not on the keyboard in front of you**. With
`size` and `shift` that problem does not exist, so the accommodation is left with no reason. The
four arrows `←↑→↓` survive, having no language, and `up`/`down`/`left`/`right` are added, which is
what WezTerm and Ghostty call them.

**«No alias table» was tied down by nothing, and now it is.** It is the half of this decision that
most easily comes back through the back door: leaving a branch that keeps accepting the old key is
noticed by no test and seen by no compiler. There are three assertions —the key, the modifier and
the action names—, and all three fall when the old half is put back.

##### The exemption list deletes itself, and of its three mechanisms one survives

The five entries it had left —`Ajustes`, `Colores`, `atajos`, `colores`, `guardados`— belong to this
module, so this delivery kills them all. **Emptying it was not an option**: the rules test has a
floor written precisely to refuse to check an empty list, and with zero entries it falls. And there
is no delivery left to add anything to it: from the prose one onwards **not one identifier gets
renamed**. It is the rule this very file applies twice: a mechanism with no scenario is deleted.

**But of the three points the previous annex left written, one was not the list's, and that cost two
attempts.** The first two —comparing the whole name with the case on, and deriving the type/local
pairs— describe how the exemption had to behave, so they go with it. **The third does not**: it said
that *every positive case of that test is checked against the list, and first*, and what it defends
is not the list but that **each case keeps exercising its rule instead of getting out through a
shortcut**. The list was a shortcut; **it was not the only one**. The other is still alive, and it
is worse because it is not written by hand: the guard gives up on a piece as soon as the **English**
side of the glossary claims it, and that side **is derived from the annex**, which the prose
deliveries are still editing. The day an arrow adopts `celda`, `reparto` or `maqueta` as English,
the case that carries it stops checking anything and the suite stays green. So the loop comes back,
pointed at that set, with its floor and with one more assertion: that each subject still really is a
remainder. Three mutations tie it down and all three fall.

**What really does disappear.** The other half —that folding the case when comparing should not
blind the rule— **belonged to the list** and goes with it. Replacing it with a direct assertion
about the lowercase name was tried and **a review refuted it**: `segments` already lowercases, so
that assertion walked the same path, byte for byte, as the one thirty lines above. A duplicate
presented as a replacement is worse than the gap, because the gap is visible. It was deleted.

**And it dragged two more things along, both verified by mutation.** `unaccent` existed separately
from `fold` **only** because the list compared with the case on; with the list gone, its only client
lowercases it on the next line, so its six uppercase arms were reached by nobody. They are merged
into `fold`, and taking the ñ or the lowercasing out of it leaves the guard red. The other was **a
real blocker**: the list of positive cases was left with no reader, and `clippy` with `-D warnings`
turns an unused constant into an error.

**And an exemption a reviewer took for dead that was not.** `restauradas` was proposed for deletion
reasoning that nobody cited it any more; it was removed, the guard went red naming the two live
citations, and it was put back. `frontera` and `supr` did come out: both were left with no citation
in this very delivery. The rule that stays written: **an entry is withdrawn by removing it and
seeing what happens**, never by reading it.

**How it got past me, which is the part that is useful:** I declared the gate green reading a run
that had not recompiled the test crate. A gate that answers from the cache has not been run. That
makes three sweeps of this milestone publishing a zero for looking at the tree in the wrong state.

##### Four names the annex had already decided, and I did not read

**This delivery's real defect was mine and the review caught it.** The annex of this very decision
had already assigned the module's names, with its reason, and I deduced them from scratch instead of
reading them: I put `Problem` where it says **`ConfigProblem`** —there is another `Problem` in
`panel-contract`, and the published criterion is «same concept, different referent → shared noun and
qualifier of its own»—, `Line` where it says **`ConfigLine`**, `Fault` where the arrow says
**`Missing`**, and `Directory` where it says **`TestDir`**, which is on top of that the name the
sibling module had already carried since the previous delivery.

**Nothing caught it, and that is the half that matters.** The glossary guard derives the **Spanish**
vocabulary from the arrows and does not look at the English side, so an English name different from
the published one compiles, passes the suite and gets merged. In a mechanical delivery the compiler
is the oracle of the rename, but **not of which name was the right one**: only the annex says that,
and it has to be read.

**And a fifth one, which is the one that is visible from outside:** the key `text` failed the
criterion this very decision wrote for choosing words —that it be used by the configuration, the
documentation or the action names of **two or more** of WezTerm, Kitty, Ghostty, tmux and Zellij—.
The first three publish that key as `foreground`. It becomes `foreground`, and with it the field,
which is what the painter already called it.

##### The document and the code, set against each other for the second and third time

The eleven shortcuts already had their two guards. The **eight keys of the table** had none, and the
**example blocks** neither: they are what somebody copies and pastes, and nothing interpreted them.
Measured before writing them: changing the document's `key.` prefix or the `none` value left **the
873 tests green**. Now the table is passed to the parser key by key, and every `key = value` line of
every block is really parsed, with a floor in case the walk stops latching on and asserting as well
that the line **applies something** —`version` apart, which by definition stores no value—.

And the half that **cannot** be derived, said so that nobody reads it as covered: a key the code
accepts and the document is silent about is caught by nothing. The `match` cannot be walked, and
copying it into the test would be having the list twice again.

##### Four citations that were already lying, and whose they were

Translating this module uncovered **four comments in `session.rs`** that named `interpretar`,
`ruta_de` and `Problema` — **their own old names**, renamed by the previous delivery. They passed
because the guard asks about the whole crate and `settings.rs` still declared those words. It is
exactly what the previous annex announced —«the remaining deliveries are going to find more, not
fewer»— and the first confirmation with numbers: **the milestone's last mechanical delivery fixes
citations from the second to last**.

### Amended on 28 September 2026: the documents' names, which the milestone closed without ([#150](https://github.com/mashware/ivy-history-private/issues/150))

F2.12 closed on 5 September with every document English **inside** and six of them Spanish **by
name**, plus the `prototipos/` directory: `02-decisiones`, `03-arquitectura`, `04-contrato-panel`,
`06-referencias`, `08-configuracion` and `09-glosario-de-prosa`. The last was created by one of the
milestone's own deliveries, PR #148, under the convention it was retiring — the convention still
producing instances, not a leftover. `07-panorama` went with them although «panorama» is English
too, so that the file carries the name its own heading uses, «Landscape». #150 renamed all seven
with every link, `include_str!` path and tracked stored-knowledge citation that named them, and the
two sentences above that describe the file while it was still `02-decisiones.md` keep that name,
because they are history.

**Decided with it: a name is held by a guard, not by this note**, beside the language guard in
`crates/app/tests/prose.rs`, which states its reach where it can be checked. Two limits are
accepted, not missed:

- **A name is judged by two word lists, not by the prose oracle**, because a name is one to three
  words and the oracle needs three. A Spanish word on neither list passes by its name; the body is
  still judged, because every document has to have a row.
- **The root's files are not judged, and its directories only against the names this repository
  retired.** A personal `CLAUDE.md`, `FLOW.md` or directory sits at the root of the main checkout
  untracked, and a guard that fails on somebody's own setup is one that gets switched off.

**Discarded: judging every path `git ls-files` lists.** It would judge exactly the tracked tree and
leave personal files alone by construction, but no test of this workspace shells out to `git`, and
the suite would stop running from a source tree without its history.

## D28 — A module has a ceiling, and a guard holds it

**One file of this repository has 3 119 lines of production code and the other eleven have between 97
and 542.** It is not a pattern, it is a single file nobody ever split: `crates/app/src/lib.rs`, the
window's core. Inside it one block takes a third of the total — `impl Window`, 1 110 lines across 47
methods.

**And the rule against it was not written anywhere**, which is the same shape of problem **D27**
exists for. The Spanish grew to 43 860 lines because «here we program in Spanish» lived only in the
code; a file grows to 3 119 lines for exactly the same reason. This is the paragraph there was never
one of.

### What the measurements say, because they change what the rule should be about

| | |
|---|---|
| Production functions across the whole workspace | 443 |
| Median function | 10 lines |
| 90th percentile | 38 lines |
| Functions over 60 lines | 13 of 443 |

**The functions are already fine**, so a rule about function length would be a rule about nothing —
it would flag thirteen lines of existing, argued code and prevent no future defect. What is out of
hand is **files and `impl` blocks**, and that is what the ceiling is about.

### The decision

- **A module holds at most 600 lines of production code.** Comments and the `#[cfg(test)] mod tests`
  block do not count: Rust keeps unit tests in the file by convention, and this project writes
  comments that argue. Counting them would make the rule punish the two habits it most wants to keep.
- **An `impl` block holds at most 300 lines of production code.** A type whose behaviour does not fit
  in 300 lines is more than one type, or its seams are in the wrong place.
- **The ceilings are held by a test**, not by review. A rule with no guard is what this whole entry is
  about; `crates/app/tests/shape.rs` is the guard, built the same way as the two that hold the
  language: floors so it cannot pass by looking at nothing, and every exception declared by name with
  its reason.
- **`crates/app/src/lib.rs` is the one declared exception, and it is a ratchet.** The guard records
  the number it has today and fails if it **grows**. Every delivery that takes a piece out lowers the
  recorded number; nothing can put one back.

### What was discarded

- **Splitting the file in one delivery.** It is 24 000 lines with 15 302 of tests that reach into
  private items; a single change moving all of it would be unreviewable, and it would collide head-on
  with the delivery that is translating that very file.
- **A ceiling on total lines rather than production lines.** It would have counted 7 319 comment lines
  against a file whose comments are the reason its decisions are traceable, and the cheapest way to
  pass would have been to delete argument.
- **A ceiling on function length.** Measured first: it would flag 13 of 443 and prevent nothing.
- **Leaving `lib.rs` out of the guard entirely.** An exception with no number is an exemption, and the
  next reader cannot tell «too big, being worked on» from «fine». The ratchet is what makes the
  difference visible.

### The order the pieces come out in, and why each one is a piece

Written down so the next delivery does not have to rediscover it. Sizes are production lines.

| What | Lines | Where it goes | Why it is separable |
|---|---|---|---|
| ~~The injected drags of the measurement bench~~ | 300 | **done** — `injected.rs`, 311 lines | Instrumentation living in the core. **Not `measure.rs`**: that module was at 540 lines of its 600 when this row was written and had filled its ceiling by the time the row was read again, so it could absorb 60 then and could not later — the guard's own arithmetic ruled out the obvious home. (It has since been split into `measure.rs` and `measure/header.rs`; no number is restated here, because a count in prose is a test that does not run.) |
| ~~Reading the settings file, and the measurement veto~~ | 50 | **done** — `settings.rs`, which went from 291 to 349 | The module that already owns that vocabulary. **It was not on this table**: the delivery that added the font by name needed room in `lib.rs` and this was the piece it was already touching — the ceiling decided where the code went, which is what the table below says a ceiling is for |
| ~~The layout tree — `Node`, `Tab`, tiling~~ | 455 estimated, **692 real** | **done** — **two** modules, `tiling.rs` (208 lines) and `tree.rs` (504) | A closed data structure; its serialisation already lives outside, in `session.rs` |
| ~~Routing the mouse~~ | ~350 estimated, **430 real** | **done** — `pointer.rs`, 448 lines in two blocks. **Not `mouse.rs`**, and that was a close call: see below | The whole path from the queue to the effect, which is what D23 divides |
| ~~Publishing the report~~ — **and *not* measuring what is out of sight** | ~150 estimated, **96 real** | **done** — `report.rs`, 105 lines. **Neither with the injected drags nor in `measure.rs`**: see below | The window's accounting of itself. The row bundled a second method with it and that was wrong — the audit that unbundled them is below |
| ~~The tab bar~~ | ~76 estimated, **100 real** | **done** — `tab_bar.rs`, 118 lines, with `TabBarOutcome`, the `#[must_use]` it never had and the two guards that hold it | Painting and hit-testing one strip, coupled to nothing else. Its rectangle arithmetic came with it |
| ~~Saving~~ — **the watcher did not move** | ~50 estimated, **50 real** | **done** — `session.rs`, from 418 lines to 471 | Same vocabulary. The watcher stayed: `watcher.rs` is deliberately window-free and thirty lines earn no module |

**Every estimate in this table was low, and by the end the pattern is the point.** 300 → 311, 455 → 692,
~350 → 430, ~150 → 96, ~76 → 100 and ~50 → 50: **six of the seven** finished rows came in over, one
exactly on and none under, and the two largest were out by 52 % and 23 %. **Three of the seven also
went somewhere other than where this table
sent them** — the injected gestures, the router and the report — and each time the reason was the same:
the ceiling or the seam was consulted *before* the move and the named destination did not survive it.
Read this table as the order the pieces come out in, which it gets right, and not as their sizes or
their homes, which it does not.

**And the rows were struck late.** Until the tab bar's delivery, three finished pieces still read as
pending here and two of them named modules the code never reached, while the working record claimed the
table had been closed with each piece's real cost. A table that is only right after somebody checks it
is a table nobody can read.

**That correction is worth keeping rather than editing out.** The first version of this table sent two
pieces to `measure.rs` because that is the module whose name matches them. Doing the arithmetic the
guard does says it fits neither: `measure.rs` is at 540 of its 600. A ceiling is only useful if it is
consulted **before** deciding where something goes, and this table is the first time it was.

### What the first piece coming out taught, recorded because the next one will hit it too

`lib.rs` went from **3 119 production lines to 2 819**, and two guards fired on the way — neither of
them the one that measures size:

- **The on-screen-sentence walk dropped from 78 sentences to 72 and said so.** Six of the literals a
  user can read moved out of a file that walk names, into a file it did not. **A module split
  silently narrows every list that names files**, and the only reason it was caught is that this
  project floors its walks on a count instead of only judging what they find.
- **The identifier guard refused the new test file** for not being on its citations list, which is
  the same class: a new file is an unguarded file until somebody puts it on the lists.

So the checklist for taking a piece out is four lists, not one: `shape.rs`, `prose.rs`,
`glossary.rs` (twice — the clean-files list and the citations list) and `settings.rs`'s walk. It is
written here rather than left to be rediscovered by whoever takes the layout tree out next.

**It is six, and the two that were missing are the lesson of the third piece.** See below.

That leaves `lib.rs` at about 1 400 production lines and `impl Window` at about 480 — over the
ceiling still, and the ratchet is what keeps it moving. **Re-derived by reading the extents rather
than estimating them, the whole order moves about 1 226 lines and lands the file nearer 1 577.**

**The second piece came out sideways, and that is the entry's own lesson working.** The delivery that
added the font by name had to add ten lines to `lib.rs`, the ratchet refused them, and the way
through was to take out a piece the delivery was already touching: reading the settings file left for
`settings.rs`. `lib.rs` is at **2 804** and `impl Window` at **1 108**. A ceiling that only fires when
somebody sets out to refactor fires almost never; this one fired on an ordinary delivery and cost it
one function's move.

**And it fired the same way a second time, which is what makes it a pattern rather than an anecdote.**
The delivery that tells the focused console's cursor apart (**D36**) needed ten lines in `lib.rs`, and
the piece it was already editing was painting a pane: 62 production lines, no `self`, gone to
`painter.rs` — a module at 398 of its 600 — with **no visibility widened**, because privacy in Rust is
per module and `painter` is a child of the crate root. Ten in and 62 out leaves `lib.rs` at **1 382**;
`impl Window` is **427**, 62 out and nothing back in, because the ten lines are in the painting loop
and that lives in `impl eframe::App for Window`. Both times the ceiling fired on somebody who was not
refactoring, both times the answer was the same, and both times it cost one function's move.

### The third piece, and the two things this table had wrong about it

**`lib.rs` went from 2 803 production lines to 2 111** — 692 out, the first number the order actually
moved. Two corrections come with it, both worth more than the row they fix.

**One piece was two.** The table said «the layout tree, 455 lines, a module of its own», and reading
the real extents said 692 into two: the arithmetic that shares an area out knows no console, and the
tree that uses it knows no rectangle. A single module would have landed at 712 and been over the
ceiling this entry exists to hold: 712 in one file against a ceiling of 600. The estimate was not
just low, it was pointing at a file that could not have been written. **Extents are read, not summed from memory**: three separate figures in
this milestone's own planning were produced by adding up line ranges, and each of the three was wrong.

**The checklist was four lists and it is six.** The two that were missing are the two that could not
complain:

- **`crates/app/tests/scripts.rs` names every module of both crates** so a shell script that moves
  with a piece cannot leave the guard reporting on nothing — the exact accident it was written for,
  in the delivery before. It was not paid, the two new files went unwatched, and the suite stayed
  green, because it was **the one list with no floor over its own length**. It has one now, and that
  is the general rule this entry can offer: *a list that names files defends nothing until a number
  defends the list*.
- **`crates/app/tests/prose.rs`'s corpus floor** is a number over the total comment lines of the
  files it names. The two new files carried 713 lines of argument out of it and the floor stayed
  where it was, so the guard read 12 908 against 12 500 and passed while blind to all of them. A
  floor stated over a corpus has to move when the corpus does.

#### Measured: what the split costs the frame, which the design had written off without checking

**The design declared this delivery a non-measurement**, on the reason that it «touches no measured
span». That reason is false, and reading the call sites is all it took: `Node::place` calls
`tile` and runs **on every frame** — once for the tab being painted and once for each background tab,
which is a rule of its own that a background tab keeps its real grid. The tiling was never off the
frame's path.

It matters because a move genuinely can shift a number here. `--release` in this workspace has no
`[profile.release]`, so it takes the defaults: **sixteen codegen units and no LTO**. Units are
partitioned from modules, and a call that crosses one only inlines if it is generic or marked. Before
this delivery `place` and `tile` were the same module; now they are two, with the caller in a third.

**Batch `H` — four panes in a row, the shape that walks the tree most — five runs on the branch and
five on `origin/main`, same machine, same afternoon, same 1280×800 window and the same 49×20 grid on
all ten. `compare` says every pair is averageable.**

| | The framework's share, per frame | Walking the tree to see if the layout changed |
|---|---|---|
| `origin/main` | **511.9 µs** mean, 481.0–547.6 | **0.972 µs** mean, 0.935–1.019 |
| This delivery | **549.5 µs** mean, 478.7–635.3 | **1.018 µs** mean, 0.901–1.129 |

**There is nothing here.** The branch's cheapest run is cheaper than main's cheapest, and its own
spread (157 µs across five runs) is more than four times the 37.6 µs that separates the two means.
Drop each side's first run — the one that pays for a cold page cache, and on the branch it is the
635.3 — and the means are 528.0 against 519.7. The span that isolates the moved code says the same
thing more sharply: the branch's range **contains main's entirely**.

The honest reading is the one D3's annexes already use for this shape: the variation between two runs
of the same tree is of the same order as the variation between the two trees. **Sixteen codegen units
did not cost this delivery a frame.**

**Recorded because the next four deliveries inherit the question, not the answer.** This one moved
code that runs every frame and the boundary cost nothing measurable; that is evidence about *this*
move, not a licence for the rest. What it does retire is the design's shortcut — «it touches no
measured span» has to be checked against the call sites, and here it was wrong.

#### The mouse went to a module of its own, and `mouse.rs` was the alternative

**The table above sent this piece to `mouse.rs` and it did not go there.** That is a changed decision,
so it is written down here with what it cost rather than left in a `//!` comment — a reviewer had to
point out that it had been, which is the whole failure mode this file exists to prevent.

**The arithmetic did not decide it.** `mouse.rs` is 99 production lines, so 430 more would have left
it at 529 of its 600 — under the ceiling, and the planned home was viable. What decided it is what
`mouse.rs` says it is: it holds **the encoding a program asks for** — which bytes an `SGR` report is,
which modes exist, what `Shift` overrides. That is a translator. The mouse piece is a **router**: it
reads a frame's queue, works out whose the gesture is, and carries that out. Putting them together
would give one module two subjects and one name, and the name already belongs to the translator.

**And the first cut of the router was wrong**, which is worth more than the destination. It moved only
the deciding and left the six methods that act on the verdict behind, under a header claiming «it
decides; it does not do» — a line the code did not draw, because the router itself reports to the
program. A blind idiom audit found it, and **the visibility surface settled it without anybody's
opinion**: with the doers outside, **eighteen** items and fields had to be crate-visible for them;
with the doers inside, **two**. *A seam that forces a module to open itself is the wrong seam*, and
that is a test anybody can run.

The cost is that the two halves are **two `impl` blocks**, because together they are over the 300-line
block ceiling. That is a real price of the ceiling, and the honest place for it is here.

#### The report went to a module of its own too, and its row bundled a method that is not the report

**The table sent this piece to `injected.rs`, «with the injected drags», on the reasoning that
everything the bench needs from the window belongs together.** It did not go there, and the reason is
the same shape as the mouse's: `injected.rs` is *the half that pretends to be a hand* — instrumentation
that reaches into the window through one method. The report is the window **accounting for itself**,
which runs whether anybody is measuring or not. Two subjects, and the name already belongs to the
hand. `measure.rs` was the other candidate and it holds **no `impl Window` at all**, so the seam was
already drawn; that it is also at 583 production lines of its 600 only agrees.

**And the row bundled `measure_the_ones_out_of_sight` into «the report», which it is not.** That
method gives the consoles of an unseen tab the width they really have, so a program running in a tab
nobody is looking at formats for its own columns instead of the eighty it started with — the
milestone's promise that a background tab keeps running *properly*. Only the stopwatch inside it is
accounting. A blind idiom audit caught it after it had already been moved, and it went back.

**The lesson is about the name, and it is the second time in this order.** «Measure the ones out of
sight» measures a **grid**, not a cost; «routing the mouse» turned out to be the whole path from the
queue to the effect. **A row of this table is a name, and a name is exactly what a piece can be
misfiled by.** What settled both was the same thing: reading what the code does rather than what the
row calls it.

#### The order is finished, and what it actually cost

**`lib.rs` came in at 3 119 production lines and ends at 1 434** — **54 % gone**, in five deliveries
over one day. **`impl Window`'s block in this file went from 1 105 to 487 — and the type did not shrink
at all.** Its methods left for satellite `impl Window` blocks in `pointer.rs`, `report.rs`, `tab_bar.rs`
and `session.rs`, so summed across the crate the type went **1 124 → 1 138 production lines, over seven
blocks in five files**: it grew by fourteen. The order published «1 110 → 489» and that figure is a
**block**, not a type; the guard measures blocks and cannot see a type at all, which is
[#187](https://github.com/mashware/ivy-history-private/issues/187). What is true without qualification is the
module: this file went from 3 119 to 1 434. The ticket asked for «about 1 400» and never showed
its arithmetic; the plan re-derived it by reading and said ~1 577; the real answer is **1 434**, and it
is lower than the re-derivation because two of the five pieces came out bigger than they were priced.

| Piece | Priced | Real |
|---|---|---|
| tiling and tree | 455 | **692** |
| the mouse | ~350 | 430 |
| the report | ~150 | 96 |
| the tab bar | ~76 | 100 |
| saving | ~50 | 50 |

**Not one behavioural test was edited across the five**, which was the invariant the design set and
the only one that could have been quietly broken. Every piece left as it was, became visible to the
crate, and `lib.rs` re-imported it by name — so the 16 953 lines of tests reaching into its private
items reached them where they always had.

**Three practices came out of this order and are worth more than the split:**

1. **Ask the compiler what should be visible.** A mechanical move opens everything, which is how the
   first delivery put `pub(crate)` on the field of `Weight` and spent the one thing a module boundary
   buys — a field the rest of the crate cannot reach. Close it all, build, open only what the compiler
   names: across the five deliveries that answer was consistently **fewer than half** the items it
   started with, and in one delivery it was **nothing at all**.
2. **Re-import by name, never with a glob.** A glob is shadowed in silence by a later item of `lib.rs`
   sharing a name; named, that same day is a duplicate definition and the build stops.
3. **A number over a list is what defends the list.** The sixth guard list went unpaid in silence
   because it was the only one without a floor over its own length, and the prose corpus went blind
   for the same reason. Both now carry one.

**And one that cost fifty lines of invisibility to learn.** The saving delivery first appended its
`impl Window` block to the *end* of `session.rs` — **after the file's test modules**. Everything
compiled and every guard passed, because the walk that measures a module's size stops at `mod tests`:
fifty lines of production code had landed where no ceiling could see them. **Where a moved block is
placed inside its destination is not cosmetic**, and the rule is one line: production code goes above
the tests, or it is not measured.

**And a mechanical move opens what it need not — but «it widened the invariant» was the wrong way to
say it, and the correction is worth more than the finding.** Making every moved item and every struct
field crate-visible is what makes a split compile in one pass, and it put `pub(crate)` on the field of
`Weight`, a newtype whose only reason to exist is that its constructor refuses zero, negatives and
what is not a number.

**What that did NOT do is widen anything.** A private field of a type defined in the crate root is
already visible to every module of the crate — the crate root's descendants are all of them — so
`session.rs` could construct a bad `Weight` before this delivery and after it alike. Measured with a
four-line program rather than argued. The first telling of this entry said the move «gave it away»,
and a later reviewer took that apart; the true statement is smaller and better: **the split is the
first moment the compiler could hold that invariant, and the mechanical pass declined to.** A module
boundary is the only thing in Rust that can make a field private *from the rest of the crate*, and a
sweep that opens everything spends that the moment it is bought.

The fix is not judgement but arithmetic: close everything, build, and open only what the compiler
names. Thirteen fields were opened and twelve were needed; of the items, less than half. **The
compiler is the cheapest reviewer of visibility there is, and it was never asked.**

### The ceiling designs the code around it, and a delivery says when it did

**This entry records the ceiling as a limit, and it is also a force.** A fix written under it will
sometimes put state where the types would not have put it, because the place the types want is a
file that may not grow. That is expected, and it is not a sign that whoever is under the ratchet is
doing it wrong. What would be wrong is to present the shape afterwards as if it had been chosen
freely.

**The clearest case so far is #170** (PR #197), the notice that stopped naming keys on consoles those
keys do not reach. Every placement in it was defensible on its own terms, and none of them was chosen
by the types:

- The mark «does the keyboard reach this console?» went **on the held paste** rather than becoming an
  argument of the function that paints a console, because that call site is in `lib.rs` and the
  ratchet allowed it no extra line. The delivery measured the file at 1 434 before and after.
- **Which module each piece landed in was decided by which module had room**: the mouse pass, the
  painter and the paste's own type, in the delivery's own words.
- An `impl` block went over 300 lines, and two functions moved between `pointer.rs`'s two blocks to
  bring it back under.
- Two test-only helpers had to move too, because the guard counts by file and stops only at a module
  literally called `tests`.

**And the first of those placements is also the one that went wrong later.** A mark stored on the
paste was a second answer to the question the cursor already answered from the frame, and the two
drifted apart: #211 (PR #281) was a notice promising `Ctrl+Enter` in a window that no longer had the
desktop's focus, while the cursor beside it said nobody was typing there. The fix removed the mark
and painted both from one value built in the frame loop. The ceiling did not cause that defect, but
it chose the shape the defect lived in. PR #197 said the ceiling had shaped the delivery; D31's
first amendment (#170), the record a later reader opens, never mentions the mark or the ceiling, so
a reader of the decision could not tell that this shape had been forced rather than chosen.

**The other half of the same pressure is [#187](https://github.com/mashware/ivy-history-private/issues/187).**
The `impl` ceiling measures blocks, not types, so a type can pay it by spreading into more blocks
without shrinking at all. That is how `impl Window` ended up across five files by the time #187 was
filed — twelve when it was closed — while `lib.rs`'s number went down, as the section above records.
The amendment below closed it.

**So the rule is one sentence.** When the ceiling decides where a piece goes, the delivery names the
placement and says that the ceiling chose it — in its PR, **and** in the decision it amends, because
the decision is what the next reader opens. Then the next person knows which shapes are free to be
undone once there is room.

### Amended: the `impl` ceiling holds a type, not only a block

**The 300 is now held twice**: per block, as before, and per **type** — a type's `impl` blocks
summed across every module of the crate, up to each module's first `mod tests`, the cut every count
of D28 makes. The rationale always said «a type whose behaviour does not
fit in 300 lines»; the guard counted blocks, and the gap was walked through by the practice this
entry itself recommended. Summed on 28 September 2026, `Window` was **1 782** production lines over
**fifteen blocks in twelve files** — against the 1 124 in two blocks the rule landed on — while every
single block was under 300 and the suite was green.

**What counts toward a type**: its inherent blocks, keyed by the last segment of the type's path, so
`impl crate::Window` and `impl Window` are one type — **and the impls of traits this crate declares**,
because moving a type's methods into `impl OurTrait for Window` would be the same walk around the
ceiling by another door. **A foreign trait's impl does not** — `Display`, `Drop`, `eframe::App` are
contracts somebody else wrote, and the per-block ceiling still holds them. The review of the first
version found that door; the first version excluded every trait impl.

**Two distinct types sharing a name are summed together**, because a text walk has no compiler to
tell them apart. Unratcheted, that only makes the ceiling fire sooner; ratcheted, one could grow while
the other shrank by as much, so a type entry whose name is defined twice is refused. And the walk
reads a one-line header at column zero with no generics of its own: a wrapped header, an `impl<`, an
`impl dyn` or an `impl` indented inside a module fails a guard of its own rather than being charged
to the wrong type or to none. None exists today.

**The five types over it are ratcheted at the number they had that day**: `Window` 1 782, `Pane` 591,
`PanelState` 378, `InjectedDrags` 334, `Node` 321. They may only go down.

**What this changes for whoever pays the ceiling**: moving a type's methods into a child module no
longer pays it. A child module still pays the **module** ceiling — that part of the practice stands —
but a type that needs room has to lose lines, or become two types with a seam between them.

**And a ratchet entry names one thing.** A block entry is keyed `module::header`, and nine names of
this crate matched more than one block of their module (`src/lib.rs::impl Window`, `src/tree.rs::impl
Node`, …): the lookup handed the entry's number to every block of that name, so a small sibling could
grow to it with nothing firing. The staleness check now refuses an entry whose name matches two
blocks, and it holds each kind of entry to its own ceiling: it used to call a module «still too big»
as long as it was over 300.

**Discarded**: keeping the per-block rule and only stopping publishing a block's number as a type's.
Honest and cheaper, and it left the first hole open on purpose — the one the practice above walks
through every time.


---

## D29 — Writing to the program does not wait for it, and what it does not take is said out loud

**Writing to a pseudoterminal blocks when the program inside is not reading its input.** That is not a
defect of anybody's: it is the mechanism, and **D8** uses it deliberately as the terminal's only real
brake on the output side — stop draining, the kernel's ~20 KiB fill, the program stops.

**The same fact, on the input side, was a hang.** `Pty::write` was `write_all` on the thread that
paints. With the program inside busy —a `cargo build` in the foreground, an editor mid-save, a
stopped process— the window stopped repainting for as long as it took, and `Ctrl+C`, which travels
that same thread, could not interrupt it. Typing never got near the 20 KiB; **pasting did**, and
pasting shipped in F2.6.

**The project held the correct knowledge, verified and measured, and had applied it in one direction
only.** That is the shape of this entry: not a missing measurement, not an unread API — a fact used
as a tool going out and never read as a risk coming in.

### The decision

- **The write is handed to a thread of its own**, one per console, and `Pty::write` returns without
  waiting. It is the shape Alacritty uses for the same problem — **and since #159, on Linux, with its
  non-blocking descriptor too** (the amendment below).
- **In order and dropping nothing** while the console lives.
- **The queue carries no ceiling.** The reason is volume, not principle: what arrives is what
  somebody's hands produced. There is no producer here to outrun a consumer the way a program
  dumping hundreds of MiB/s does on the other side, which is why D8's ceiling is not copied over.
- **What the program is not taking is published**, and the frame says it once. `Pty::backlog` answers
  how many bytes are owed and since when.

> **Corrected on 28 September 2026: the queue has a ceiling now.** The deferred row below — the
> ceiling with backpressure to the caller — was decided in **D60**: once 256 KiB are owed, a further
> write is refused and said over the console, and nothing already owed is dropped. The «carries no
> ceiling» bullet and that row are kept as what this decision said on its day.

### What was discarded, and why

| | |
|---|---|
| **A non-blocking descriptor** (Alacritty's model, and the first candidate) — **reversed on 26 September 2026 for Linux**, see the amendment | Read in `portable-pty` 0.9 rather than assumed: the writer and the reader are both `dup`s of the master, so they share one open file description. `O_NONBLOCK` on one lands on the other — and the reader's blocking `read` **is** the output chain, D8's brake included. It would mean rewriting the side that works |
| **A ceiling with backpressure to the caller** (what D8 really did) | It is the honest ceiling and it is not discarded for ever: it is deferred. It needs an answer to «what does the window do when it cannot accept a keystroke», and that is a product question, not an implementation detail |
| **Discarding when full** | The failure that takes longest to understand from the other side of the screen. D8 refused it on the output side and this refuses it too |
| **Giving `Ctrl+C` a path of its own** (a signal to the process group) | Measured over a real pty in three termios states: in raw mode `0x03` **is not a signal** — the program asked to read that byte — so signalling would interrupt the very TUI agents this terminal exists to host. WezTerm, Alacritty, Kitty and xterm all write the byte. And in canonical mode the interrupt was never broken: the queue cannot jam there, because the line discipline discards instead of blocking |
| **Moving the output pump to its own thread** | It is the literal reading of the first draft of the root cause, and a challenger showed it cures the deadlock and leaves the freeze |

### What it cost, and what it bought

| | |
|---|---|
| The write that stopped for **5.71 s** | returns in **3.62 µs** |
| The mouse path, batch `O`, four runs alternating with `main` built minutes earlier | **5.5–6.0 µs** against **8.2–9.1** |

**And a published span changed meaning, which is declared rather than left to be noticed.** «From a
key in the frame to the byte in the program» — D3's 36 µs and 3.42 ms, D23's 5.1–5.3 µs per gesture —
now stops **at the queue**: the byte reaches the program on a thread nobody times. The names in the
report say so. The second figure existed *because* the write waited.

### What it left open, with its measurement

- **A console whose program stops reading for good goes deaf and its writer thread leaks.** Measured:
  fifteen seconds after dropping such a `Pty`, the thread was still there — killing the child does
  **not** unblock a write to the master. One thread and its payload per console closed in that state,
  for the life of the process. Undoing it needs an interruptible write, which is the discarded row
  above coming back. **Closed on Linux by the amendment below; still open on macOS and Windows.**
- **Why a blocked write does not end when the child dies is not explained.** Measured at ≥19 s in one
  run and 54 s in another, with a child that never wrote a byte. The slave end *is* dropped at spawn,
  so the obvious answer is already ruled out. It is written here so nobody closes it with a guess:
  this investigation was caught doing exactly that once. **Answered as observed behaviour by #159**,
  below — the mechanism still is not traced.

### Amended on 26 September 2026: on Linux the master is non-blocking ([#159](https://github.com/mashware/ivy-history-private/issues/159))

**The new signal is a measurement.** On Linux 7.0, a blocking write to a master whose buffer toward the
program is full **does not end when the program dies and every copy of the slave is closed**: the
master answers `POLLHUP`, a read answers `EIO`, a non-blocking write answers `EAGAIN` rather than `EIO`,
and the write parked in the kernel keeps waiting. Nothing a closing process can do wakes it — measured:
`tcflush` in every direction, `TIOCSWINSZ`, `TIOCPKT`, `tcflow`, `TIOCSTI`, closing another copy of the
master; `vhangup` is refused without privileges. A signal aimed at the thread ends the call with a
partial count, and the rest waits again at once. The block is tied to the buffer being **full**: a
write that fits returns in 47 µs on a dead slave. The most likely mechanism, read in upstream
`pty_common_install` and not traced here, is that the master counts itself on the slave, so the
write's «the other side is gone» check can never pass while a master descriptor is open — and the
blocked write holds one. So the premise of the discarded row — that the blocking write is harmless to
keep — was false, and in **two** places: the writer loop's `write_all`, and `portable-pty`'s own writer,
whose destructor writes a newline and the end-of-file character on a full buffer.

**What changed:**

- The master's open file description is **non-blocking**, so the reader's duplicate gets the flag too,
  exactly as the discarded row warned.
- **The reader waits for what its blocking `read` waited for**, in a `poll` for input, and reads again.
  It never reads while the channel is full, so D8's brake is untouched, and a survivor holding the
  program's side still parks it, which D32 bounds as before.
- **The writer waits for room in a `poll` that also watches a pipe the console holds**, and closing
  drops the pipe's writing end. A master that answers a hang-up with no room means the program's side
  is gone, and the writer gives up. The destructor's write fails at once on a full buffer.
- **Windows and macOS keep the blocking write**, and the leak with it. ConPTY is a pipe nobody has
  measured here yet (F2.7). And macOS's `poll` is known not to wait on a pseudoterminal — it answers
  at once — which is why the `filedescriptor` crate beneath `portable-pty` switches to `select` there:
  this shape would spin an idle console and read a full buffer as a program gone. The review caught it
  before it shipped; F2.7 decides what macOS does, with a measurement.
- **Every descriptor is made before the program is spawned**, so a process out of descriptors fails
  the open instead of dropping a child nobody waits for.
- **A `poll` that wakes with neither the event it waited for nor a hang-up is an error**, not a retry:
  retrying «would block» after it would spin.
- **An idle console is measured to cost no processor time** (`tests/idle_console_cpu.rs`, its own
  process): a reader that retried «would block» without waiting delivered every byte and left every
  other test green.

**What it costs, measured and not hidden.** A non-blocking reader pays a failed read and a `poll`
each time it catches up with the program, where the blocking one slept inside a single `read`.
`cargo run --release --example delivery -p term-core`, alternating builds on the same machine and
day: log lines through the kernel's output processing came out at **48.2 MiB/s** on average over four
runs (43.2–50.7), against **54.3** over five on `main` (51.4–57.9) — about **11 % less**. The other three
rows of that bench showed no consistent difference. It is paid on the reader thread, not in the
frame, and it sits beside a grid that absorbs **55.7 MiB/s** in the same bench family (`throughput`), so
on a dump the two now run close. No frame batch was run to see whether that ever shows in a frame.

| Discarded | Why |
|---|---|
| **A signal to interrupt the writer** | The handler is process-wide, installed by a library for its own thread, and the rest of the write waits again the moment it is retried |
| **Emptying the program's input from the slave side at closing** (open the slave, flush it until the writer gets through) | It keeps D29 but chases the writer with a loop of its own, and only frees one bufferful per pass — a long paste needs many — and the destructor's write can still find the buffer full after the last one |
| **`poll` for room before a write that still blocks** | «There is room» means *some* room: a larger write still parks in the kernel, and if the program dies in the meantime it is the same leak, narrower |
| **Closing another copy of the master from `Drop`** | Measured: it does not wake the writer. The blocked write holds its own |

### Amended on 20 November 2026: on Windows the close ends the writer ([#163](https://github.com/mashware/ivy/issues/163))

**Windows keeps the blocking write, and loses the leak.** The writer still parks in `write_all` when
the program reads nothing. But closing the console now closes its pseudoconsole, and the
pseudoconsole's host then closes the pipe the writer waits on, so the write fails with a broken pipe
and the thread ends (D111). Overlapped writes with `CancelIoEx` were not an option: `portable-pty`'s
pipes are anonymous pipes, which do not support overlapped I/O. **macOS keeps the leak** until F2.7.

---

## D30 — A test that needs a terminal mode proves it has one, and no control character can prove it

**The tests that hold D23 up were passing over a terminal they had never managed to configure.** Their
script asked for a mode, printed a marker and let the test start; chained with `;`, the marker was
printed **even when the mode command failed**. Measured with `stty` missing: **744 passed, 1 failed** —
fourteen of the fifteen tests hanging off those two scripts stayed green.

**They stayed green because the evidence they read can be written by somebody other than the program.**
With `ICANON` on, the line discipline delivers nothing to the program without a newline, and a mouse
report has none — so the `cat -v` inside received nothing. With `ECHO` on, the line discipline printed
those same bytes itself, and `ECHOCTL` renders an escape `^[`, character for character what `cat -v`
would have printed. The kernel forged exactly what the assertion looked for.

This is the project's first recorded disguise —asserting on the place a value is recorded instead of on
the effect it has— in its sharpest form: **the effect was real and the author of it was wrong**.

### The decision

- **A script that sets a mode chains strictly to its marker.** `&&` and nothing else, so the marker
  cannot be reached when the mode command failed.
- **And the test proves the mode took, because an exit status is not a mode.** It writes a byte and
  requires the answer only the program can give.
- **The probe byte is above `0x7f`.** `cat -v` writes it in meta notation, `M-^@`; the line discipline
  echoes the raw byte. Those four characters have no second author.
- **A guard holds the first half over the source, in both crates.** `crates/app/tests/scripts.rs`
  judges a literal only when it both sets a mode and later prints a text some test waits for, so loop
  syntax and scripts whose failure already shows red are let through by construction.

### What was discarded, and why

| | |
|---|---|
| **The marker alone, chained with `&&`** | Necessary and not sufficient, and the gap is not theoretical: replace `stty` with `true` —exits zero, changes nothing— and the chain sees nothing wrong. Measured: **15 of 15 green** without the probe, **15 of 15 red** with it |
| **`0x7f`, the erase character** (what shipped first, and a review caught it) | It looked like the one control character canonical mode eats. **The erase key is configuration, not a property**: measured over three pseudoterminals, with it moved to `^H` a canonical terminal returns `^?` exactly like a raw one. The old probe passed **all fifteen** over the very terminal it exists to reject |
| **Any other control character** | `ECHOCTL` and `cat -v` both use caret notation, so they are indistinguishable by construction: `0x1b` comes back `^[` from either |
| **A tab** (`0x09`), proposed as the remedy for the row above | Measured before it was applied: `cat -v` leaves a tab alone, so the wait would never be satisfied and all fifteen tests would have gone permanently red |
| **Banning `;` in a shell literal** | It fires on four scripts measured safe — loop syntax, and scripts whose failure already shows red — and a guard that fires on correct code gets deleted |
| **A guard over `app` only** | The identical defect was fixed in `term-core` by the work on the blocking write, comment and measured number included, and nobody carried it across. A guard watching one crate would repeat the mistake it exists to stop |

### What it left open

- **`stty` and `cat -v` are still the assumption.** D1's other two platforms have neither in that
  shape. What changes is that the assumption now **fails loudly** instead of publishing a green: that
  is what makes the port findable rather than silent.
- **The guard reads a shape it cannot parse in two cases** — a literal continued across lines, and a
  raw string carrying a quote of its own. Both leave the same footprint, and a test fails on any line
  that names a mode command and yields no literal at all. Declared rather than papered over.

### Extended on 28 September 2026: a fourth disguise — the premise is proved by whoever holds it ([#191](https://github.com/mashware/ivy-history-private/issues/191))

This decision is where the project keeps the ways a test covers nothing while staying green. The
record numbers three of them: asserting on the place a value is recorded instead of on the effect it
has, cited as the project's first recorded disguise; and, cited as the second and third «of D30»,
picking a number where the defect and the fix give the same result, and a test over a terminal that
never set the mode it needs. The third is this decision's own case, which
the text above files as the first disguise in its sharpest form. **#62 produced a fourth, and it is
not a variant of the third.**

**A backgrounded list exits 0 whatever happens inside it.**

```
sh -c '{ nosuchcmd & } && echo REACHED'   # prints REACHED
```

The regression test for closing a console with a survivor needed a process holding the slave end of
the pseudoterminal. It launched one with `{ setsid … & }` and waited for a marker the **shell**
printed. On a host without `setsid(1)` — every macOS, and **F2.7** goes there — the chain would reach
its `exec` with nobody holding anything, and that version of the test would pass over a live bug.
It was never run that way, on macOS or anywhere. What was run is the same missing survivor against
the test as it stands now — the survivor's command made nonexistent, a row of D32's mutation round —
and it goes red because nobody prints `HELD`. D32 tells the case and carries its numbers; they are
not repeated here.

**The rule, in its general form: the process that holds the premise is the one that announces it.**
A marker printed by whoever launched it says nothing about what it launched. The third disguise is
the special case where the premise is a mode and the holder is the program; this one is about any
premise a scenario depends on — a survivor, a child, a held descriptor, a peer that is listening.

- **The proof is the holder's own marker.** `HELD` in `crates/term-core/src/lib.rs` is printed by
  the survivor, so seeing it come out of the master proves the survivor exists *and* that it can
  write through the descriptor it inherited. It comes after `stty -echo &&`, so it also proves the
  `stty` exited 0.
- **An effect only the premise produces is a weaker fallback, and must be called that.** The test
  `closing_a_console_whose_program_left_a_survivor_ends_its_writer_before_the_close_returns` has no
  marker of its survivor's own; it asserts the close took at least `READER_GIVE_UP`, which a close
  with nobody holding the slave does not spend. That makes a missing survivor go red in the usual
  case, not in every case: **a loaded machine can meet a lower bound on time with no survivor at
  all**. Where the holder can print, it prints.

**Two neighbouring lessons from the same case**, both already held by tests:

- **A «did it finish?» test needs a floor.** A test that only bounds a wait from above passes on
  every accidental rescue, in hundredths of a second. The floor is written as the constant —
  `took >= READER_GIVE_UP` — so a delivery that makes the mechanism cheaper moves the bound instead
  of going red.
- **The other half of the claim needs its own test.** The hang test said nothing about the healthy
  close, so the whole timed wait could have been made unconditional with the suite green:
  `closing_an_ordinary_console_does_not_spend_the_abandon_deadline` is that half.

**No guard enforces it**, for the reason the «Banning `;`» row above already gives: a scan for
`{ … & } &&` followed by a shell-printed marker would fire on
`closing_a_console_whose_program_left_a_survivor_ends_its_writer_before_the_close_returns`, whose
floor is an acceptable fallback declared as one, and a guard that fires on acceptable code gets
deleted.

---

## D31 — A pasted line that does not fit is held and asked about, never sent to be cut

**An over-long pasted line is not lost by the far side: a different command is executed, and the
screen shows the one that was not.** The line discipline **closes** a line at its buffer limit
instead of refusing it, so a program reading lines receives something syntactically complete and runs
it. Measured with the shell's stdout going to a file, where the echo cannot forge it: a 4 200-character
`echo …_TAIL_OK` produced 4 090 characters and no tail, while the terminal echoed all 4 200 followed
by a normal prompt. **No error, no mark, and the visible line is the one that did not run.**

**The ticket called it data loss and it is worse than that.** The everyday shape needs nobody's
malice: a long command whose trailing flag falls off. The cut is also **aimable** — padding puts the
boundary on any byte — which makes it the clipboard-hijack class through a vector no filter can see,
because there is nothing in the text to filter. That is recorded for completeness; **the argument is
the everyday case**.

**And it reaches more than it looks.** `bash` and `zsh` are in raw mode at their prompt and
**canonical while any foreground command runs**, so it reaches anybody who pastes while something is
running. The first version of this investigation concluded the opposite from a measurement taken only
at the prompt; a challenger overturned it.

### The number is not `MAX_CANON`, and that matters more than it sounds

`getconf MAX_CANON /` is **255**. What bounds a canonical line on Linux is `N_TTY_BUF_SIZE`, a
**compile-time constant of the kernel's `n_tty` driver** — not POSIX, not reachable through `termios`,
and different on other kernels. The project called it `MAX_CANON` in the ticket, in the code and in
its knowledge store. **A mislabelled constant is the same defect as a mislabelled measurement**, and
the consequence is concrete: `4 095` must never be written into code or a test as if it were a
contract, because **F2.7 goes to a platform with another number**.

### The decision

- **A paste whose longest line is over the threshold is not written.** It is held, the question is
  painted over the console, and the person decides. **Holding and not warning**: nothing reaches the
  program until the line closes, and once it closes the shell has already run the truncated command —
  a warning would arrive after the incident and label it.
- **A wrapped paste is never held.** A program turns the paste markers on only if it handles pastes
  itself, and every program that does is a line editor in raw mode, where the canonical buffer does
  not exist. `readline` turns the markers off before running a foreground command, which is the exact
  moment the shell goes canonical: the correlation is the same switch. **This matters most of all
  here** — in a terminal for hosting agents, pasting a long prompt into an agent's console *is* the
  paste, and holding those would put a false warning on the one path that matters.
- **The threshold is declared per platform and is never a contract.** It is what decides whether to
  ask a question, not an assertion about what will arrive. Linux gets its measured number; everything
  else gets **no gate**.
- **Saying yes costs `Ctrl+Enter`; a bare `Enter` stays the program's.** The accident that decides
  this runs the other way from the obvious one: with the question standing, somebody types a command
  and presses `Enter` to run it, and a bare-`Enter` yes would append the held paste onto the line they
  just typed — over the boundary by construction, truncated by the kernel, the incident arriving
  through its own cure.
- **The paste is composed when it is SENT, not when it was held.** The mode moves underneath a
  standing question, and bytes composed unwrapped delivered to a prompt that has since turned the
  wrapping on make the line editor execute each line as it arrives.

### What was discarded, and why

| | |
|---|---|
| **Keeping silent** (what it did) | Silence **is** the incident: the far side runs a different command and the screen shows the one that did not run. **D8** refused silent discarding on the output side and **D29** on the input side; this was the third path and it had been discarding all along |
| **Splitting the line at the boundary** | Measured: in a shell the tail then **runs as a command of its own**. It is the right fix for `cat` and a catastrophe for a shell, and **no mode query separates them** — both are canonical |
| **Warning after sending** | Arrives after the shell has executed the wrong command. It labels the incident instead of preventing it |
| **Reading the terminal's mode as the gate** | It can be read — `portable-pty` exposes it, 254 ns, proven end to end — and it is **not a gate**: the mode flips at every command boundary and **D29**'s queue puts an unbounded gap between the reading and the write. Worth only as «raw, therefore no limit», which is what the wrapped-paste rule uses indirectly |
| **Copying Darwin's number out of a header** | Nobody has run it on a Mac. A number taken from a header and published as measured is exactly what the paragraph above forbids |
| **A bare `Enter` for the affirmative** | See the accident above. It is also the key people press to dismiss anything, which would hand the risky action to the habitual reflex |
| **Doing what the other emulators do** | **They do nothing.** None of WezTerm, Kitty or Ghostty chunks a paste, none warns by size by default, and **none has an issue of this shape in its tracker**. The rule says look it up rather than re-invent; looking it up said there is nothing to copy, which is why this is a position and is argued rather than cited |

### What it leaves open, declared rather than discovered later

- **The gate is stateless.** It measures the paste alone and cannot see what is already unterminated
  on the other side — text typed at the prompt before it. A short paste on top of a long line still
  truncates. **It narrows the window; it does not close it**, and it must not be described otherwise.
- **Windows has no such question.** Every termios accessor of `portable-pty` is `#[cfg(unix)]` and
  ConPTY has no line discipline. F2.7 needs a branch that cannot ask and can only assume.
- **A short paste goes through while a question stands**, so held bytes land *after* it if the answer
  is yes — the reverse of the order they were pasted in. The price of not freezing the console.
- **Every pane paints its own notice and only the focused one can answer it.** The keyboard half of
  the same gap the mouse already has: the notice has no clickable answer either. **Closed by the
  amendment below.**

### Amended: the notice names only what works where it is painted, and a click answers it

**The bullet above shipped as a declared gap and it was worse than it reads.** Every pane of the
active tab painted «`Escape` discards it · `Ctrl+Enter` sends it anyway», and those keys are read for
**one** pane — so on every other console the notice named keys that went to a different program. The
instruction was not merely absent: it was wrong, and it was on screen.

- **The notice says how to answer it only where it can be answered**, and where the keyboard does not
  reach it says what does. **It does not go quiet there**: a paste held in silence is the failure this
  whole decision exists to stop, so «paint nothing on an unfocused pane» was refused.
- **A click discards and never sends** — *a click on the question's box; sending by a box of its own
  came in with the next amendment*. Sending a line the program will cut in half is the
  irreversible half of the question and keeps costing a deliberate chord; dismissing a question
  nobody asked for is the safe half, and it is the one a hand on the mouse wants. It is also what the
  report asked for in those words.
- **Only a completely bare press answers**, which is stricter than the conditions a drag is decided
  by, deliberately: a press carrying any modifier at all belongs to another gesture, and the chord
  that moves a console would otherwise move a pane **and lose the paste without a word**.
- **The press is read from the frame's button queue**, not from the pointer's position, and the whole
  queue is still routed with only that one press taken out.
- **Where the notice goes is one pure function**, called by the painter to draw it and by the mouse
  pass to know what was pressed. Nothing is stored between them.

#### What was discarded, and why

| | |
|---|---|
| **Routing the keys to an unfocused pane** — «make the instruction true instead of changing it» | Two standing questions would fight over the same `Escape`, and `Escape` has other owners in this window. The notice stops claiming what it cannot deliver instead |
| **Painting no notice on an unfocused pane** | Cheaper and cleaner on screen, and it restores the exact failure this decision exists to prevent: a paste held where nobody can see it |
| **Sending by click as well as discarding** | It needs a second target inside the notice and the compose-at-send-time rule applied to it — the rule this feature already broke once. Left open here; **taken by the amendment below**, with a target of its own under the notice rather than inside it |
| **Keeping the notice's rectangle on the held paste**, written by the painter and hit-tested a frame later (what shipped first) | Four separate defects, all found in review: a stale rectangle, a painter that mutates its subject, a box whose size moved with the focus, and state that could disagree with itself. Replaced by the pure function |
| **A guard requiring the press to resolve to a cell of the console** | Written for the frame a panel is first published and **read the same unrefreshed state**, so it would have said yes in exactly that case. Removed rather than kept: a guard whose comment promises more than it does is worse than none |
| **Judging the press at the pointer's latest position** (what shipped first) | It is not where the button went down. A batch carrying the press and then a move — a flicked click, or any frame that batches — let the real press through to the program and could answer a question nobody aimed at |
| **Skipping the frame's whole button queue** while a question was answered (what shipped first) | It also carries the **release** of a press already live for a program, and dropping that leaves the program believing the button is still down for ever, then stealing the next release |

#### What it leaves open

- **On a narrow console the notice fills it**, so any bare click there discards the question. Measured
  with a panel published beside it: the console 168 pt wide and the box its whole height. The
  direction is safe — discarded, never sent — and what fills the screen *is* the notice, but nobody
  decided it. Either the box gets a ceiling on how much of a console it may take, or the answer gets
  a target of its own inside it.
- **A click on the notice of an unfocused console also focuses it and clears its selection**, because
  the pass below does that for any click on a pane. Kept: you pressed on that console.
- **Sending by click**, per the table above. **Closed by the amendment below.**

### Amended again: a box of its own under the notice sends the paste

**The question can now be answered «yes» with the mouse, and it still never shares a target with
«no».** A second box, «Send it anyway», sits directly under the question; a bare press on it sends
the held paste, and a bare press on the question still discards it. #199.

- **Under the question, never inside it.** Inside, «a click discards it» would stop being literally
  true of the box it is written in and both sentences would have to change; outside, the question,
  its wording and what a click on it does are exactly what the amendment above shipped.
- **Composed at the click**, through the same `HeldPaste::send_anyway` the chord uses, against the mode
  the program is in at that moment. The paste gate keeps its three callers.
- **It is the person's own paste**, so it takes the console back from an agent that held it (**D44**),
  exactly as `Ctrl+Enter` does — unlike a panel's button, which is the application writing on the
  person's behalf. It is not timed as a keystroke, for the reason a paste never is.
- **Only a completely bare press sends**, by the same filter that decides a press answers at all.
- **A question at its ceiling makes room for the button.** With several consoles the question
  usually reaches the half-console ceiling (**D39**), and a button placed under it would never fit;
  so the question is laid out leaving the button's height free, which clips the tail of the
  explanation and never the way out — the keys are first in the galley. **The button is the one that
  gives way**: where even that shortened question would not show its whole instruction, or the
  button would run past the console's right edge, there is no button and the question is exactly
  what it was, so no console loses the notice to make room.
- **Where the button goes depends neither on the reading's ending nor on whether «stopped receiving»
  (**D41**) was dismissed** — two things that can change between a paint and a press: that sentence goes
  **under** the button's place, so dismissing it moves nothing above it. A first review round found
  the version that hid the button while the sentence stood — dismissing it made the button appear
  exactly under the dismissing click, so a double click, or egui running the same press through a
  second pass, sent the paste.

  **Amended on 26 September 2026** ([#290](https://github.com/mashware/ivy-history-private/issues/290)):
  **egui 0.36 does not run the same press through a second pass.** Before every pass it moves the
  frame's events out of the raw input (egui 0.36.1, `data/input/raw_input.rs:137`, called at
  `context.rs:854`), so a pass repeated after a discard sees no press **event** — what it carries
  over is state: the pointer's position and held buttons, the keys held down, the modifiers. The
  double click stands as the reason; the second pass does not. The once-per-frame guard on this
  press is **kept, as defence** against an egui that starts replaying: it costs one frame compare,
  and a replaying upgrade would turn one click into two actions with nothing to warn but the
  tripwire test `egui_hands_a_frame_s_events_to_its_first_pass_only`. Discarded: removing the guards,
  which saves a few lines and leaves that upgrade silent; and keeping only those in front of a
  destructive act, which splits one rule into two for the same price.
- **It is not offered on a console whose writer gave up** (**D29**), where the write would go
  nowhere. Its place stays empty there, and a press on it answers whatever is drawn there — the
  sentence about that console when it stands, the grid otherwise.
- **The price is paid by «stopped receiving», and it is named here rather than left to be found.**
  With the button standing, that sentence goes under it, so at the heights where it fitted under the
  question alone and does not fit under the button it is no longer drawn — and **D39**'s
  whole-or-nothing rule then leaves nothing. It is the only order that keeps the button still under a
  dismissal; both notices at once is rare, and where several consoles share a window the question
  already reaches its ceiling and the sentence never had room. On a console whose writer gave up
  there is no button, and the sentence goes back under the question.
- **A write that fails keeps the paste held** and its notice standing, so a click never makes a
  paste disappear without having sent it.

#### What was discarded, and why

| | |
|---|---|
| **A button inside the question's box** | Both sentences would have to be rewritten — «a click discards it» is false of the box that holds the button — and the hit target of the dismissal would have a hole in it |
| **Two explicit buttons, «Discard» and «Send», with the box itself inert** | It undoes what the amendment above delivered: the dismissal a hand on the mouse wants would shrink from the whole box to a button |
| **No notice at all unless the button fits too** — the strict reading of **D39** | Consoles short enough to show the question and not the button would lose the question, which is the failure this whole decision exists against; `Ctrl+Enter` still sends there |
| **Omitting the button whenever the question is at its ceiling** | That is the ordinary case with four consoles, so the button would almost never appear where there are several panes |
| **Placing the button and making the silence notice yield to it, or the other way round** | Needed only because both competed for the same place; with the silence under the button's place they never do |
| **No button while the silence stands** (what shipped first, inside this delivery) | The button then appeared under the very click that dismissed the silence, and the geometry depended on a reading's ending that can change between a paint and a press — an aimed press on «Send» could land on a grown question and discard |

#### What it leaves open

- **The sentence on an unfocused console still says «focus this console to send it anyway»** with a
  button right under it. True, and roundabout; kept so that neither sentence changes. (A third
  sentence arrived later for a different case, and the amendment below records it.)
- **The pasted text is not shown**: the button sends what the person pasted moments ago, which
  `Ctrl+Enter` already sends the same way.

### Amendment — a third sentence, for a window that lost the desktop's focus (September 2026)

**The notice and the cursor answer the same question — is anybody typing into this console? — and
they answered it apart.** **D36** taught the cursor the window's focus: alt-tabbed away, every
console outlines its cursor. The notice asked only whether its console had the keyboard inside the
window, so the focused console of an absent window said «Escape or a click discards it · Ctrl+Enter
sends it anyway» under a cursor saying nobody was there. #211.

- **That console now says «A click discards it · come back and Ctrl+Enter sends it»**, and goes
  back to the usual sentence the moment the window returns. Every other console keeps «focus this
  console to send it anyway», which stays true with the window away: a click on it focuses both.
- **The notice is handed the value the cursor is painted from** — one value that says where the
  keys go: here, here once the window is back, or to another console. The mark the paste used to
  carry, written by the mouse pass, is gone: a second reading is what let the two drift apart.
- **The third sentence may never lay out taller than the other two**, and a test holds it with the
  built-in font at every width from 4 to 120 cells. The box is still measured against all three at
  run time — two layouts more per console holding a paste — because a font of somebody's choosing
  (**D26**) is exactly what that test cannot walk. The box is measured against every sentence so
  that it keeps its size when the focus moves (the amendment above), which means a longer third
  sentence raises the floor of **every** console —
  in a focused window too, where it is never shown — and consoles that used to hold the question
  would hold the paste in silence. The sentence first agreed on did exactly that, and so did the
  first shorter one, at twenty cells.
- **Escape is not named**: it does not reach an absent window either.
- **The price, named**: the natural way back to a window is a click, and a click on the notice
  discards the paste. The sentence opens with exactly that; a click on the grid brings the window
  back and keeps the question standing, and one on «Send it anyway» sends it.

| Discarded | Why |
|---|---|
| **The click sentence on the focused console too** («focus this console…») | It asks for what that console already has, exactly where it is read |
| **Leaving it as it was** — the instruction is true once the window comes back | Two marks of the same window contradicting each other about the same console |
| **«A click discards it · come back to this window to send it with Ctrl+Enter»** (the first agreed) | Longer than the other two: measured against all three, the box stopped fitting narrow consoles that showed the question before, in a focused window too |
| **Measuring the box only against the sentences that can show right now** | The box would change size the frame the window's focus moves, and the mouse pass would hit-test a rectangle other than the one painted |

### The sentence D6 needs, and why it is here

**D6** forbids deducing state *«by reading the screen or by parsing third-party files»*, and its
reason is three prototype failures about what «closed» meant: **D6 is about meaning**. This
investigation asked the kernel for a file descriptor's `termios` — one bit the program itself set,
read back from the kernel's own record, with one correct answer and no interpretation — and the
literal reading of D6's own table (level 1 is *«the app, looking at the painting or at the PTY»*)
covers it, while its subject does not.

**So D6 is amended here rather than argued about again: what D6 forbids is deducing the STATE OF THE
WORK. The program's own I/O configuration is not that.** Reading it supplies no vocabulary to the
panel and answers no question about how a task is going. What is still forbidden is what D6 always
meant: inferring from someone's output what their work means.

## D32 — Closing gives the reader thread a deadline and then abandons it, and does not signal a process group

**Closing a console could freeze the window for ever, and the destructor runs inside the interface
pass.** The reader thread sits blocked inside `read()` on a descriptor cloned from the pseudoterminal
master, and it was joined with no deadline. When the program inside leaves behind something that
outlives it — a `sleep &`, a development server, anything that made a session of its own — that
survivor inherited the **slave** end, so there is no end of file, the `read` never returns and the
join never returned either: no repaint and no keyboard for as long as the survivor lived. Measured: a
`Drop` that had not returned after eight seconds. **#62.**

**The shape is as old as the crate; F2.1 is what made it reachable**, by letting a tab be closed hot.
And two accidents were hiding it — the terminal's echo returning the bytes `portable-pty`'s master
writer emits on drop, and **#157** having moved that writer out of `Pty`'s fields into the write
thread, where it drops *before* the join instead of after. So it only showed with the echo off, which
is the first thing `vim`, `htop` or a TUI agent does. The whole autopsy is in the work's artifacts;
what belongs here is the decision.

**The thread cannot be woken.** That is the fact everything else follows from: it is blocked on a
descriptor a survivor holds **by design**, and the stop flag it already has is only consulted after a
`read` returns — which is exactly what never happens. There is nothing to signal, nothing to close and
nothing to interrupt that gets the descriptor back. So the only thing that can bound the wait is the
wait itself.

### The decision

- **The reader thread is joined with a deadline of 100 ms and abandoned if it does not come.** The
  thread owns the sending half of a channel it never writes to; the channel goes quiet exactly when
  the thread ends, whichever way it ends, so the ordinary close reads `Disconnected` at once and joins
  as before. `READER_GIVE_UP` in `crates/term-core/src/lib.rs`.
- **The number separates «microseconds» from «never» and is not a measurement of anything.** It is
  three orders of magnitude above what the ordinary case needs and it is deliberately **not** a
  threshold for how long a frozen window is tolerable: nobody measured that here and no number in the
  code claims it.
- **It is spent per console, not per gesture.** Closing a tab drops every pane it had, in series,
  inside the same interface pass, so four consoles each over a survivor is four times this. That is
  the number to quote.
- **Abandoning costs a thread and a descriptor while the survivor stays silent.** The stop flag is
  already raised, so the survivor's **next byte** ends the thread: the read returns, the delivery sees
  the flag and it leaves. Only a survivor that never writes again holds them for the life of the
  process. It is the same trade **D29** recorded for the writer thread (**#159**) — which since that
  issue the writer makes only off Linux.
- **No signal goes to a process group.** Not because it would do nothing — see the table — but because
  it would not be enough and because sending one is a decision about what closing *means*, which this
  fix does not take. **`Ctrl+C` stays a byte** either way: in raw mode `0x03` is data a TUI agent asked
  to read, and signalling on **close** is a different act from signalling on a **key** (**D12**,
  **D23**). The fix does not blur them.

### What was discarded, and why

| | |
|---|---|
| **Signalling the process group instead of the pid**, which is what the issue proposed first | It is **insufficient, not useless**, and the weaker claim shipped in a comment before a review caught it. A plain `sleep &` does hold the slave end and *does* stay in the child's group, so a group signal would close it; a survivor that called `setsid` is in a group of its own and is exactly what it does not reach. So a deadline is needed anyway — and adding the signal would additionally kill the background jobs of whoever closed the pane, which nobody has decided. Left open, deliberately |
| **Waking the reader instead of bounding the wait** — the store's own rule asks for a stop flag *and its own trigger* | There is no trigger to give it. `close()` on our clone does not unblock a `read` already in flight, the master stays open for resizing, and no signal takes back a descriptor another process holds. Measured indirectly: the only thing that ever woke it was the echo, by accident |
| **Killing the survivor** — `killpg`, or walking `/proc` for holders of the slave | It is somebody's `nohup`ed build, dev server or daemon. A terminal that kills what you deliberately detached, because you closed the pane it was started from, is worse than one that takes 150 ms |
| **Closing the master to force `EIO`** | The reader thread owns its **own** descriptor from `try_clone_reader()`, so dropping ours changes nothing for it — and the master is kept on purpose, for `resize` |
| **Doing the close off the interface pass** (a thread, a queue of dying consoles) | It removes the freeze without bounding anything, so the leak becomes unbounded and invisible instead of bounded and declared. It is also a bigger change than the defect, and the ~101 ms that F1 is about would still be paid somewhere |
| **`JoinHandle::is_finished()` plus a bounded poll**, which a reviewer argued for and which would remove a field, an unused sender and an unreachable match arm | A fair reading, and the module already polls elsewhere for exactly this class of reason. Kept the channel because `recv_timeout` blocks and wakes **on the event**, where the poll wakes a hundred times to look at a boolean inside the interface pass — and the version that would need a comment explaining why it busy-waits there is the poll. The reviewer's better idea, a small `Ended` guard type whose «did it end within this?» *is* the question, is left open |
| **A number chosen from what a person tolerates** (the ~600 ms where a delay stops reading as a delay) | It shipped in a comment for one afternoon and was removed: nobody measured it in this project, and a mislabelled number is worse than no number |

### What it left open

- ~~**The ~101 ms that was already being paid** is untouched: it is `portable-pty`'s fixed 50 ms poll per
  console, and it needs a different change — SIGHUP then an immediate SIGKILL, or moving the close off
  the interface pass. Confirmed with ten samples in the work's diagnosis.~~ **Closed by D55**, and by
  neither of the two changes named here: the cost was the poll's *grid*, not its grace, so this crate
  hangs up the program itself and looks every millisecond. An ordinary close went from 50.88 ms to
  1.20–1.37 ms.
- ~~**Nobody is told.**~~ **Answered, and the answer is «nothing».** The question was what the window
  tells somebody whose console left a thread behind, and it was carried by **#192**, whose own body
  offered four candidate answers. The first was chosen: nothing, and the trade stays a comment in the
  source. What decided it was not taste but reachability — the abandonment happens inside `Drop for
  Pty`, while the pane is being destroyed, so there is **no console left to speak over**, no area to
  paint on, and one frame later no tab. And there is no honest place to print from either: this crate
  prints nothing anywhere, and the thing on the other side that does print only speaks while the
  console lives. The three alternatives are discarded below. **D38** is where the vocabulary that
  came out of that ticket lives, and it covers the two silences that *can* be shown; this one cannot.
- ~~**Whether closing should signal the group at all.**~~ **Answered: no** (**#193**). The table above
  already established it would be **insufficient** — a survivor that called `setsid` is in a group of
  its own — so the deadline is needed whatever is decided about the signal. What was left was the
  product half, and it is settled the same way: a group signal would additionally kill the background
  jobs of whoever closed that pane, and a terminal that kills what you deliberately detached because
  you closed the pane it started in is worse than one that takes 150 ms. **Closing a pane does not
  kill what its console left running**, and that is now a decision rather than an omission.

### What was discarded in answering those two

| | |
|---|---|
| **A line in the frame's report**, the shape **D29** uses for the input side | It is where a developer looks and not where a person does — and the review that built D38 found that D29's own line goes to standard error, so the precedent is thinner than it reads. It would have made the trade auditable without making it visible, and the question asked was what the *window* says |
| **Something in the pane's own status panel** | The panel belongs to **whoever does the work** (**D17**), and the application painting its own state there is precisely what **D6** forbids. It would also outlive nothing: the pane is going away |
| **A counter handed in at `spawn`**, which a reviewer suggested | It makes the leak countable without deciding what anybody sees, which is the useful half of the idea and the reason it is recorded — but a counter nothing reads is a mechanism with no scenario, and the audit it would feed is **#159**'s, not this one's |
| **Signalling the group anyway, and documenting the cost** | The cost is somebody else's `nohup`ed build being killed. A cost that lands on a person who did nothing wrong is not documented, it is refused |
- **`F2.7` inherits one concrete thing rather than a worry.** Every mechanism here is Linux-specific —
  the kernel hangup, `EIO`, the `VEOF` echo, `setsid(1)` itself — and the regression test is written so
  that a host without `setsid(1)`, which is every macOS, **fails loudly instead of passing quietly**.
  That is the right failure, and it gives the first run on another platform a named thing to fix.

### Measured

`cargo run --release --example closing -p term-core`, seven rows, same machine and same day. Each row
**declares** what it measures — that comes from the scenario, which is known — and its elapsed time is
checked against the band that claim implies; a row outside its band says so and the run exits
non-zero. Reading a cause off the clock was the first version's mistake and it would have published
this crate's own deadline as the library's poll the first time the machine was busy.

| Row | Before | After |
|---|---|---|
| a live shell | 50.76 ms | 50.38 ms |
| already dead | 0.07 ms | 0.03 ms |
| ignores SIGHUP | 201.80 ms | 201.90 ms |
| grandchild, echo on | 50.72 ms | 50.75 ms |
| **grandchild, `-echo`** | **no return** | **150.65 ms** |
| **grandchild, `raw -echo`** | **no return** | **150.86 ms** |
| stopped child | — | 202.52 ms |

**The two that never returned now return, and the rest did not get more expensive**: the deadline is
only spent when somebody outlived the program, because with nobody holding the slave end the thread
has already gone by the time it is consulted. That is asserted and not read off a table —
`closing_an_ordinary_console_does_not_spend_the_abandon_deadline` fails if the healthy close ever
starts paying it.

**And the sibling wait is bounded too**, which was the adjacent risk: a *stopped* child never dies of
a signal it can hold, so `child.wait()` beside the join has the same unbounded shape. It does not
happen — after its grace period the library's `kill` falls through to a real `SIGKILL`, which nobody
holds — and the `stopped child` row is that reading turned into a number. What the row shows is the
grace period; what it proves is that the wait is reached and returns.

**The regression test is mutated, twice.** Five deliberate breaks of it, each run against a tree with
the deadline reverted to the unbounded join: **all five red** — 2.06 s with no mutation, 0.10 s with
the echo left on (it comes back in 50.7 ms, under the floor), 2.01 s without the drain, 2.06 s with
the receiver released early, and 5.01 s with the survivor's command made nonexistent. A first round,
before the test had its floor and before the survivor announced itself, had **two of those passing in
hundredths of a second** — green over a live bug. That is what the two rounds are for.

## D33 — The key that never arrived: `egui-winit` is served from a copy in the tree, with one `return` made conditional

**Decided.** The dependency that turns the desktop's events into the framework's — `egui-winit`, which
`eframe` pulls in — is no longer taken from crates.io. It is served from `vendor/egui-winit`, a copy
of the published **0.36.1** that differs from it in **one thing**: the `return` that follows the paste
shortcut only fires when something was actually produced. Everything else is byte for byte the
published crate. It is the **first patched dependency in this repository's history**, and
`docs/05-roadmap.md` carries the amendment to the policy that said versions come from published
releases.

**Checkout representation, November 2026** ([#190](https://github.com/mashware/ivy/issues/190)).
The source drift lock compares CRLF checkout pairs as LF before checking its recorded lengths and
hashes, including the source left after the authorized patch regions are removed. Git's ordinary
Windows checkout otherwise changes the manifest from 10,316 to 10,729 bytes without a source edit.
The recorded snapshot remains unchanged. Every other byte, a lone carriage return and the presence
of the final newline remain significant; trimming whitespace, regenerating hashes for Windows or
omitting the guard there would hide changes the lock is meant to catch.

**What was broken, said in product terms.** Pressing `Ctrl+V` with a **screenshot** on the clipboard
did nothing at all. Not the paste — correctly, that is `Ctrl+Shift+V`'s job — but not the control
byte either, and not even the keypress. In a terminal built to host agents that is the gesture for
handing an agent an image: the agent reads `\x16` as «go and look at the clipboard yourself», which is
how a picture reaches it. The same silence hit an **empty** clipboard, so `vim`'s visual block did not
open in a freshly started session and a shell's *quoted-insert* did nothing.

**Why the terminal could not fix it in its own code.** The dependency recognises the chord, reads the
clipboard **as text**, finds none, pushes no event, and returns **unconditionally**
(`egui-winit-0.36.1/src/lib.rs:1027-1035`) — dropping the key event along with the paste that was
never built. Nothing downstream can recover it, because **the information is never written**: the
modifier state records only that Ctrl was held and never which key, and the text branch sits below
the return. So `eframe`'s `raw_input_hook`, which this application already implements, is not late —
it is reading a place where the keypress was never put.

**Almost nothing on our side needed changing, and the exception is the interesting part.** With the
key delivered, `\x16` comes out unchanged: `key_bytes` maps `Key::V => 22` from the event's own
modifiers, `Chord::from_written` structurally refuses to bind `Ctrl+V` so no shortcut can claim it
first, and the paste door matches only the paste event.

> **But the fall-through also delivers `Ctrl+Shift+V`, and that one is ours.** `is_paste_command`
> does not look at Shift, and `key_bytes` lets Shift through on purpose — it does not change a
> control code, which is what `xterm` does. So with a screenshot on the clipboard, pressing the
> terminal's **own paste shortcut** would have sent `\x16` into the program: press it expecting a
> paste and `vim` opens visual block. Before this fix that chord did nothing, and *a chord of the
> application reaching the program inside* is the one thing D11 and D13 both refuse. **Found in
> review, not in use.** So `keyboard::bytes` grows one arm that answers silence for
> `Ctrl+Shift+V` — and deliberately **not** for `Ctrl+V`, because that byte belongs to the program
> and is the entire point. Silence for one and the byte for the other is the distinction, and it has
> a test that fails if either half moves.

The rest of the fix is upstream of us — which is
also why it is **not** a regression: the line that derives the byte from the paste event was born in
F1.3, when pasting did not exist and that event was merely a convenient way of noticing the chord.
F2.6 gave it a contract it had never had. **The failure shape worth remembering is that one**: a
guarantee resting on an event a dependency emits for its own reasons. `Event::Paste` is named after
the gesture and is emitted for the *payload* — no payload, no event, and the gesture disappears with
it.

**Measured, and it is the point of the whole thing.** The chain was read in the dependency's source
and then **observed on a running window**, because a challenger pointed out — correctly — that
nobody had seen it happen. With an image on the clipboard, `Ctrl+V` produced:

```
Key { key: ControlLeft, pressed: true,  modifiers: NONE }
ModifiersChanged(Modifiers { ctrl: true, command: true })
Key { key: ControlLeft, pressed: false, modifiers: { ctrl: true, command: true } }
ModifiersChanged(Modifiers::NONE)
Key { key: V, physical_key: Some(V), pressed: false, repeat: false, modifiers: NONE }
```

**No paste and no press of the V.** With text on the clipboard the same chord produced a paste
normally — which is what rules out the other explanation available, a clipboard backend that could
not see the session's selection at all.

**And the observation showed something the source reading had not: the RELEASE survives.** The
swallow is inside the pressed branch, so the key-up falls through in both cases. That is a fix with
no patched dependency at all, and it was **discarded**: it fires after a successful paste too (a
second byte on every ordinary paste), it arrives with no usable modifiers when the user lets go of
Ctrl first — which is what both traces did — a terminal must deliver its byte on key **down**, and the
exact shape of a release event is precisely the kind of thing that differs per platform, with F2.7
about to build on two new ones. It fits the symptom; the patch fixes the cause.

**What was discarded, and why.**

- **Waiting for upstream.** [emilk/egui#4065](https://github.com/emilk/egui/issues/4065) is the same
  complaint, **open since 2024**; [#5615](https://github.com/emilk/egui/pull/5615), which would settle
  it, is unmerged with no maintainer approval; #5618 was closed without landing. `master` has since
  gained an image branch (`Event::PasteImage`) but is unreleased **and still returns
  unconditionally**. Sending the change upstream stays worth doing; it is not a plan.
- **Upgrading.** 0.36.1 (7 August 2026) is the newest published `egui`, `eframe` and `egui-winit`.
  There is nothing to upgrade to.
- **Turning the dependency's `clipboard` feature off.** It gates only which backend compiles — there
  is not one `cfg(feature = "clipboard")` in its whole source — and the interception is unconditional.
  Without a backend the clipboard read returns nothing **always**, so **every** `Ctrl+V` would be
  swallowed instead of only some. It makes it worse.
- **A hook of our own inside `eframe`.** There is none upstream of the translation: `raw_input_hook`
  runs after the input is assembled, `EpiIntegration::on_window_event` only forwards, and
  `NativeOptions::event_loop_builder` hands over the builder **before the loop exists**.
- **Driving our own `winit` loop.** Structurally clean — intercept the window event before the
  translation sees it — and it means dropping `eframe::run_native` and reimplementing viewport
  management, surface creation, resizing and redraw scheduling. A rewrite of the application's shell
  for one key.
- **A fork on a server instead of a copy in the tree.** **F2.7 is the first build on macOS and
  Windows**, and a path resolves with no fetch and no credentials on all three. A fork would add one
  more unverified thing to that milestone.
- **Reading the image ourselves** and pasting a path to a temporary file. `arboard` is already in the
  tree with image support, so it would cost no new dependency — and it is the terminal deciding what
  somebody else's image means (**D6**), plus a file nobody deletes. Delivering the key makes it
  unnecessary: the program inside reads the clipboard itself.

**What you need to know so as not to get a fright.**

- **This fix carries no regression test, and that is not an oversight.** It cannot: `winit`'s
  `KeyEvent` has a private field, so **no crate but `winit` can construct one** — being inside the
  vendored copy does not help, because the privacy is `winit`'s and not `egui-winit`'s. And
  `on_window_event` demands a real `Window`, which winit creates only from a live event loop bound to
  a compositor. And `on_keyboard_input`, where the branch lives, is **private** and reachable only
  through that method. There is no test in this repository that needs a display, and there is no cheap
  way to add the first one.
- **What replaces it is a guard over text** — `crates/app/tests/vendor_patch.rs` — which fails if the
  manifest stops pointing at the copy, if the `return` stops being conditional, or if anything else in
  the copy drifts. Its own doc comment says what it does not prove, which is anything at all about
  what pressing the key does. Mutated four ways, red on each.
- **The verification of the behaviour was done by hand on the real application**, and the observation
  is worth recording here because it is not the obvious one: in `cat -v`, the first `Ctrl+V` printed
  `^` and the second completed it to `^V`. That is not half a result — it is the line discipline's
  *literal-next* running, which cannot happen unless the real `0x16` arrived, and it is exactly what
  D11 says the chord should mean to a shell.
- **The whole suite runs against the copy.** On the day of this delivery, 972 tests across 17 suites,
  three of them the new guard — because the patch replaces the crate for everything that depends on
  it. What the suite cannot do is *see* the changed line: every keyboard test builds the framework's
  events by hand, above the layer that was failing.
- **The day #4065 lands, `vendor/` and the patch section go away together.** That is the exit
  condition, and it is the reason the copy carries its diff in a comment at the changed line rather
  than in a patch file: whoever removes it has to see what it was for.

### Amended on 25 September 2026: a second change, for the close chord (#274)

**What was broken, said in product terms.** `Ctrl+Shift+X`, the factory shortcut for closing the
focused console, never closed it from a real keyboard: the program inside received `^X` (0x18) — the
byte `nano` exits on — and the console stayed. Seen by the user twice while reproducing #271;
`Ctrl+Shift+D` worked in the same window.

**Why, and why it is the same shape as the first change.** `is_cut_command` is `command && X` and
ignores Shift, and on a match the copy pushes `Event::Cut` and **returns before the key**. So the chord
was a cut before it was a key: the shortcut table, which matches only keys, never saw it, and the
terminal wrote the cut as its control byte. It is the copy chord's story again (D26's annex): a chord
the desktop turns into a clipboard request, with a whole family of tests seeding a key — here some
twenty of them, `Key::X` with Ctrl+Shift — that the desktop never delivered. So the suite was green
over a shortcut nobody could use, and **by all the evidence it never worked**: `is_cut_command` is
upstream's and older than the chord.

**The change**: that arm becomes `command && !shift && X`. A bare `Ctrl+X` is a cut exactly as before
and still reaches the program as 0x18; `Ctrl+Shift+X` arrives as the key, **repeat flag included**, so
holding it closes one console and not one after another; `key.close-pane = none` hands it back to the
program as the same byte it got before. The copy and paste chords are untouched, and so — **on
purpose** — is the same function's Windows arm, `shift && Delete`, which has the same shape: it turns
Shift+Delete and Ctrl+Shift+Delete into a cut before they are keys, so on Windows they would reach the
program as `0x18` instead of D52's `CSI 3;2~`/`CSI 3;6~`. Nothing here can observe it; F2.7 decides it
on the platform, and the guard pins that arm as upstream's so the change is a visible decision.
**Decided in D115**: the arm is gone, as is `is_copy_command`'s Windows Ctrl+Insert arm, and
`is_paste_command`'s Shift+Insert arm requires Shift alone.

**What was discarded.** Handling `Event::Cut` with Shift in `keyboard.rs` as a lookup of the chord,
with no change to the copy. It keeps the copy at one change, and it pays for it with state of its own:
a cut carries no repeat flag and autorepeat pushes one per repeat, so it needs a latch — set on the cut,
cleared on the X's release or on losing focus — to keep holding the chord from closing every console
in the tab. The patch fixes the cause in one condition; the latch rebuilds, downstream, the
information the copy throws away. Chosen by the user at the fix's gate.

**What guards it.** `crates/app/tests/vendor_patch.rs` now cuts **two** places out of `src/lib.rs`
before hashing the rest — the paste branch and `is_cut_command` — and pins the new arm and upstream's
other two. The residual's constant was computed from the tree before the change, by a script that
first reproduced the old constant exactly. The behaviour itself is out of reach of any test, for the
reason above, so it is checked by hand (D53). Since D115 it cuts **four** places — the paste branch
and the three clipboard predicates — and pins each predicate whole.

## D34 — The step between cells is a whole number of pixels

**Decided.** The cell's width and height are taken **down** to a whole device pixel, in
`painter::CellMetrics`. The font's measurement is not trusted to land there on its own, because it has
no reason to.

**What was broken, said in product terms.** The logo Claude Code paints out of block characters came
out crossed by thin dark lines, a progress bar came out speckled, and a run of `─` opened a
one-pixel gap every few characters. Reported from a screenshot beside the same screen in Tilix, which
does not have it.

**Measured, and it is what settles the shape of the fix.** With `Ubuntu Sans Mono` at 14.7 points the
cell came out at **8.232 × 17.625 pixels**; with the built-in font at 14, **8.4287 × 16.3125**. So
this predates any setting: the fractions are what the text engine answers, not what a configuration
asked for. The drift is `0.232` of a pixel per column and `0.625` per row, so a whole one accumulates
every **4.3** columns and every **1.6** rows.

**And the mechanism is not the one this record first gave, which is the correction worth reading.**
It said antialiasing paints every boundary as a half-covered pixel, and named two cells of different
background meeting there. **Backgrounds never had it**: epaint aligns every filled rectangle to the
pixel grid on its own —`round_rects_to_pixels`, on by default, and its own documentation says it is
what «makes filled rectangles tile perfectly (without feathering)»— so two contiguous cells share a
rounded boundary whatever fraction they were asked for at. **The shared-edge assumption in this
statement was corrected in November 2026; see the amendment below.**

**What the defect actually is: the engine rounds each glyph's origin, separately.**
`round_text_to_pixels` is on by default too, and this painter queues **one glyph per cell**, so over a
step of `8.232` the origins `0, 8.232, 16.464, 24.696…` are each rounded on their own and come out at
`0, 8, 16, 25, 33, 41` — steps of `8, 8, **9**, 8, 8`. That ninth pixel is a column of nothing between
two glyphs about eight wide, and it returns every time the accumulated fraction crosses a half. Which
is exactly «a gap every few characters», and exactly the hairlines through a logo drawn out of block
characters, because those were glyphs too.

> **Verified by tessellating and reading the vertices**, not by reading the source and reasoning from
> it — a run of six was painted on a fractional cell and on a whole one, and the quads' left edges
> came out at those numbers. The guard does the same: a test that reads `output.shapes` looks at the
> coordinates the painter asked for, which is the one place this defect never was.

> **Only the horizontal period was observed, and the vertical one is arithmetic.** What the report
> carried is a run of `─` opening a gap every few characters, which matches the 4.3. Nobody counted
> rows, and this record does not claim they were counted: the 1.6 comes from dividing the row's
> fraction, and it is written that way rather than as «exactly the period the seams came out at» —
> which is what the first version of this record said about a number nobody had measured.

**Snapping the corner the grid starts from was in this and came out, and that is the second
correction.** It reads as the necessary other half — a pane's corner is a fraction by construction,
since the tree splits the window by weight — and it buys nothing. A fraction shared by every cell of
the pane survives the engine's rounding as the same offset for all of them: measured, a whole step
from a corner at `101.37` gives rounded origins `100, 108, 116, 124, 132, 140`, which is one step. What
has to be whole is the **step**, not where the run starts. It had a guard demanding it, which is how a
mechanism with no scenario stays: the guard was written from the same wrong model as the code.

**Amended in November 2026 — selected backgrounds can still expose a one-pixel crack.** The
whole-device-pixel step stays. At a fractional scale its point representation is inexact, and
`(origin + column * width) + width` need not equal `origin + (column + 1) * width` in `f32`.
If those two edges straddle a half-device-pixel tie, the tessellator rounds them to different
pixels. Actual egui tessellation reproduces this with two identical blank fills, with antialiasing
on and off: at scale 1.5, width `7 / 1.5`, origin 1, columns 5/6 end at pixels 43/44; at scale
1.75, width `9 / 1.75`, origin 2, columns 11/12 end at 111/112. Neither glyph shaping nor different
selection colors is needed. The glyph-placement repair in #58's first PR (#64) left this path
unchanged; a later alpha.8 screenshot exposed the remaining background cracks.

**Both sides now name the same grid boundary.** A cell's far corner uses the next column and row
coordinates, through exactly the same expressions as its neighbors' near corners. Its near corner
and the floored step do not move, so glyph placement and selection-color inversion keep their
existing rules. Snapping the whole origin is still unnecessary. Inflating every rectangle would
hide a crack by overlapping different colors; merging same-color runs would not solve the boundary
between different colors or rows. Disabling antialiasing does not solve the reproduced gap.
Regression coverage must inspect the production painter's tessellated backgrounds at fractional
scales and nonzero origins; a whole-pixel width assertion alone cannot see this defect. The exact
scale and origin of the reporter's screenshot remain unmeasured, so the deterministic cases do not
claim to predict its fifteen crack positions. No new frame-cost measurement is claimed.

**What was discarded, and why.**

- **Rounding up.** It makes the cell **wider than the glyph's own advance**, so a run of `─` stops
  touching and the gap this exists to close comes back. Going down leaves the cell a fraction
  narrower than the advance instead, and consecutive glyphs overlap by that fraction.
- **Rounding to the nearest, which is what this shipped first and is the mistake worth recording.**
  It reads as the neutral choice and it is not: nearest goes **up** whenever the fraction is at least
  a half, on each axis independently, which is about half the bodies. Measured with the family in
  use: at 17 points the advance is `9.52` pixels and nearest gives a cell of **10** — `0.48` of a gap
  at every column boundary, which is the reported defect reappearing at a body the settings file can
  ask for. At 14 points it gives 8 against an advance of `7.84`, and at 14.7 it takes a row height of
  `17.625` to **18** — the same mistake on the other axis, at the body this was reported from.
  It survived the first round of guards because **«the cell is a whole number of pixels» is satisfied
  by `floor`, `round` and `ceil` alike** — the second disguise of **D30**, a number where the defect
  and the fix agree. What pins it down is a separate guard: the cell is never **larger** than what
  the font measured, with the one-pixel floor as its only declared exception.
- **Choosing a font body whose cell is already whole.** Swept from 8 to 24 points in steps of 0.05
  with the family in use: **exactly one** lands on whole pixels, 12.5 → 7 × 15. One body in the whole
  usable range, and not one anybody asked for. A fix that depends on a magic number is not a fix.
- **Turning antialiasing off.** It is the framework's, applies to the letters too, and the letters
  are the thing this terminal exists to draw well.
- **Merging runs of same-coloured cells into one rectangle.** It would close the seams between
  backgrounds — and only those: the glyphs and the row boundaries stay where they were. It is an
  optimization wearing a fix's clothes.

**What is not measured, and it is said rather than guessed.** The cost per frame of this change has
not been taken. What it adds is two roundings per cell inside the painting stretch the bench already
measures, so there is no undeclared path for **D13**'s rule to leak into the framework's figure — but
the grid itself changed size, so the control batches published before this are **not** comparable
number for number with the ones after.

## D35 — The block elements are drawn by the painter, and since #103 the box drawing block too

**Decided.** The thirty-two characters of Block Elements — `U+2580`–`U+259F`: the full block, the
halves, the eighths, the quadrants and the three shades — are drawn as rectangles measured against
the cell, in `painter::blocks`. **In the console's grid** they are never asked of the font — the
status panel is not the grid and still lays its rows out as text, which is written down further
below. **`U+2500`–`U+257F`, the box drawing lines and corners, deliberately stayed with the font
until the amendment at the end of this decision.**

**Why a block is not a letter.** Its whole job is to fill its cell exactly and to meet its neighbour
with no seam. What a font ships is a glyph drawn for one nominal size, rasterized with antialiasing
and stamped inside a cell of another — so a run of full blocks comes out with hairlines through it
even after **D34**, and the shades come out as moiré instead of an even wash.

**The case that brought it in, read and not inferred.** The status line's progress bar is ten `░`
(`U+2591`) in dim green — verified by dumping the raw bytes it emits, `E2 96 91` behind
`ESC[32m ESC[2m` — and beside the same bar in Tilix ours was visibly speckled.

**Everybody outside solves it the same way.** Kitty, WezTerm, Ghostty and VTE all draw these
characters themselves. This is the same answer, kept to the one range where the defect **survived
D34** — the gap in a run of `─` was in the box drawing range and closed with the step, and what a
whole step does not close is a glyph rasterized for one nominal size and stamped into a cell of
another, which is what a block is made of. *(The gap did close; what replaced it was an overlap, and
the amendment at the end of this decision draws the box drawing range too.)*

**Two rectangles are enough for all thirty-two**, which is not obvious: the three-quadrant characters
look as though they need three, and `▙` is the left half plus the bottom half. Every one of them
folds the same way, and the guard checks the **area covered** rather than the shape of the fold —
written as «upper-left plus lower-right», a plausible-looking slip, `▙` covers half the cell instead
of three quarters and every assertion about it being two parts still holds.

**A block element is still noted for font discovery, like every other character, and that is a
correction.** The first version of this skipped the note — no font is asked for these glyphs, so why
send the machine's families to be walked for one — and it reads as free. **It buys nothing.**
`resolve`'s oracle discards, before touching the system, anything the loaded faces already show, and
the factory faces carry this whole range behind whatever family the settings name. So the search never
fires for a block in the first place; the skip would only add a check on every cell of the grid, on
the hottest path in the project.

> **It was found by mutation and not by reading.** Deleting the skip left the whole suite green, and
> the test written to catch that was green too — because what it measured, «did this walk the
> machine's fonts», is *already* no for a block with the skip in place. The test stayed, turned
> around: it now pins **the premise** —a row of blocks walks nothing, a row of Japanese does— so the
> day a build ships without a factory face covering the range, this reasoning is reopened instead of
> quietly becoming false.

> **And the status panel would not have been covered either way.** It hands each row to the engine in
> one go instead of cell by cell, and it notes the whole row's text, so a producer that publishes
> `░░░░░░░░░░` in its summary still gets the font's dither, moiré and all. It is the same shape as
> [#178](https://github.com/mashware/ivy-history-private/issues/178) — the panel is the one surface that
> lays out text as text — and it is out of this delivery for the same reason: making the panel paint
> cell by cell is a different piece of work.

**What was discarded, and why.**

- **A real dither for the shades.** A 25 % dot pattern has to land on the pixel lattice to read as
  even, and at a cell of eight by seventeen there are not enough pixels for it to read as anything but
  noise — which is the very defect this replaces. A flat wash of a quarter, a half and three quarters
  of the ink is what the comparison screenshot shows.
- **Extending it to the box drawing block.** Its lines close with the neighbouring cell because
  **D34** never lets the cell be larger than what the font measured, so consecutive glyphs overlap by
  that fraction instead of parting — on both axes, which is precisely the half of **D34** that had to
  be corrected in review. The visible defect is gone without synthesising the range, and synthesising
  it means also deciding about heavy, double, rounded and diagonal, which is a different and much
  larger piece of work. *(Reversed by the amendment below, in a module of its own, `box_drawing`.)*
- **Rounding a sliver away to zero.** An eighth of a cell of seventeen is a little over two, so the
  parts are snapped like everything else in **D34** — but a part that would round to nothing is given
  one pixel, because a vanished mark is not a thinner mark: for `▏` used as a rule, the rule is
  simply gone.

**What is not measured.** These characters change from a text shape to one or two rectangles per
cell, and **no batch of `scripts/measure-frame.sh` puts a block element on screen**, so the bench
cannot see the difference in either direction. That is a gap in the bench, not a number.

### Amended in November 2026 (#103): the box drawing block is drawn by the painter too

**Decided.** `U+2500`–`U+257F` are drawn against the cell in `painter::box_drawing`, all of them
except the three diagonals `╱ ╲ ╳`: light and heavy lines, corners, tees and crosses, the double
lines, the dashed ones, the half lines and the four rounded corners. The discarded alternative
above, «extending it to the box drawing block», is reversed.

**The new signal, measured and not inferred.** A screenshot showed Claude Code's input-box border
dotted and blurred. A probe painted a run of `─` through the production painter at scales 1, 1.25,
1.5, 5/3 and 2, with the built-in font (Hack) and with Ubuntu Sans Mono, and rasterized the tessellation.
Two defects showed at every scale and with both fonts:

- **A bead at every seam.** The overlap the paragraph above counted on is real, and it is the
  defect: the glyph's raster is 1 px (Ubuntu Sans Mono) or 2 px (Hack) wider than its cell, and the
  overlapping pixels are antialiased ends composited twice — `0.24 → 0.34` at a seam in an edge row.
  **D34**'s floor is what puts the next glyph on them; with no overlap they would be a dim end
  pixel instead. Either way the line does not read as continuous.
- **A stroke that straddles rows.** The stroke's height and thickness are the font's, so it lands on
  two or three partly covered rows (`0.64 / 1.00 / 0.74` at 1.25). The bitmap sits on whole pixels,
  so the blur is inside the raster, and no rounding of the origin can remove it.

**How it is drawn.** The cell is taken to device pixels by rounding its two corners, which the next
cell shares. Every thickness is a whole number of pixels, and every band is centred by rounding
down from the cell's own pixel size, so a band leaves one cell on exactly the rows the next one
enters on.
The shape is rasterized into a mask and the mask is cut into disjoint rectangles. Two rectangles over
the same pixel of a translucent ink would paint the bead again. A light line is
`round(points × scale / 12)` pixels, never fewer than one: one at the default size at 100 %, two at
166 % and 200 %. Heavy is twice that. A double is two light lines a light line apart, and each of
its lines stops against the line it would otherwise cross; a single line that crosses a double
(`╪` `╫`) runs through both of its lines unbroken. Arcs are a quarter circle tangent to both bands,
in whole pixels. A dashed line whose cell has no room for every dash and its gap is drawn as
one-pixel dashes one pixel apart, as many as fit: dropping gaps gave dashes of uneven length, and
drawing it solid made it read as the plain line. In such a cell `┄` and `┈` look alike. What a shape comes out as at a cell size and a thickness is rasterized once and
kept in a per-thread table; a frame looks each cell up and places its rectangles.

**What was discarded.**

- **Only `─` and `│`, the two the report showed.** A box drawn from two sources does not close: a
  corner from the font would meet the painter's lines on other rows.
- **One rectangle per arm, painted directly.** Arms meet at the joint, and with an ink that is not
  opaque the joint would be painted twice — the same bead, drawn by us.
- **Antialiased arcs.** Tried and seen on the desktop: at a radius of a few pixels the curve's
  pixels came out at half ink beside lines at full ink, and the corner read as a gap. So did the
  first hard-edged version, which inked only the pixels the centre line crosses: two pixels touching
  at a corner. The arc now inks whole any pixel whose coverage — estimated from its distance to the
  curve — is at least a third, which makes every step share an edge with the next, and it ends a
  pixel inside the cell.
- **The diagonals.** A diagonal on a pixel grid is a staircase whichever way it is drawn, and
  nothing in a box has to meet it on a shared row.

**What is not measured.** Like the blocks, no batch of `scripts/measure-frame.sh` puts a box drawing
character on screen. A box character now costs a lookup and a few rectangles instead of a text
shape, and that is a gap in the bench, not a number.

## D36 — Only the console being typed into fills its cursor, and none of them does it when the window is not

**Decided.** A console that does not have the keyboard paints its cursor as **the outline of a
block**, whatever shape its program asked for; the one that does paints that shape, filled, exactly
as before. And when **the window itself** has lost the desktop's focus, every console outlines,
including the one the keyboard would go to. The question is asked in `painter::paint_cursor`, before
the shape is looked at and after the guards that leave — no cursor in the view, a shape of `Hidden`,
no cell where it stands — so a hidden cursor stays hidden and one
scrolled out of the view is still not painted.

**Why it exists.** Until this, the cursor was painted identically in every console, so with two open
they both looked like the one being typed into. The only thing that said otherwise was a border of
one pixel around the pane, and with four consoles that border is easy to lose. It is the same problem
the border already solves, answered where the eye is already looking.

**The unfocused mark ignores the program's shape, and that was settled outside rather than decided
here.** Kitty ships `cursor_shape_unfocused` with `hollow` as its **default**, and WezTerm applies
only the block outline to its inactive panes. Two independent implementations agree, and the reason
is visible once written down: the outline of a beam is a one-pixel rectangle nobody can see, so «its
own shape, outlined» reads as no cursor at all in two of the four shapes.

**The window's own focus is the more frequent half.** Leaving for another application happens more
often than moving between consoles, and it is the case those settings exist for. It is read from
egui's own flag (`InputState::focused`, documented as *«false when the user alt-tabs away»*) instead
of tracked here, so nothing new is kept between frames.

**Why the cursor did not blink, and it does now: see D37.** This entry deferred blinking rather than
refusing it, and named the price: a settings key of its own, a declared repaint path in the report,
and a decision about what the batches do with it. All three were paid by
[#210](https://github.com/mashware/ivy-history-private/issues/210), so what is left here is the shape of
the argument — a cursor that blinks is a timer, and a timer is **work at rest**, which is exactly
what every published control batch of `scripts/measure-frame.sh` measures. **One line of this
paragraph was wrong and it is worth keeping the correction visible**: it said the key would be «off
by default». It is not a default in either direction — it is a ceiling, absent by default, over a
terminal that blinks. D37 says why.

**What was discarded, and why.**

- **Outlining the shape the program asked for.** Above: two of the four shapes become invisible, and
  both reference implementations do the opposite.
- **Dimming the unfocused cursor instead of hollowing it.** It needs a derived colour, which means
  deciding it for a palette the settings file can replace entirely — a dimmed cursor over a
  dark-on-dark theme is a cursor nobody sees, and the outline works whatever the twenty-two colours
  are.
- **A fifth arm inside the shape `match`.** Every arm would ask the same question and `HollowBlock`
  would ask it twice. One branch before the `match` is the same behaviour with one place to read.
- **A `focused` field on `Pane`**, written once per frame the way the held paste's notice already
  learns who has the keyboard. It would have been **correct** — both writers of the focus run before
  that marking, which is itself the last thing before the painting — and it is **not cheaper**: the
  field and its initializer are two production lines in `lib.rs`, under a ratchet with zero
  tolerance, so it hits the same wall as the parameter for less. The parameter is computed in the
  loop that paints, so no ordering can make it wrong.
- **Reading the focus as a position** (`where_the_focus_is`) rather than as a name. The focus was
  made a name precisely so that reordering the tree cannot move it; the loop has `pane.name` in
  hand, so the position is a detour through an invariant it does not need.

**What it cost in `lib.rs`, because D28's ratchet is part of this delivery's story.** The change
needs ten production lines in the window's core and the ratchet has **zero tolerance**, so the piece
this delivery was already editing came out first: painting a pane left for `painter.rs`, 62 lines,
no `self`, and **no visibility widened** — privacy in Rust is per module and `painter` is a child of
the crate root, which `paint_a_console` already relied on. `lib.rs` went 1 434 → 1 372 → **1 382**,
and `impl Window` 489 → **427**. That also retired the constraint recorded in `paint_a_console`'s own
docblock, which said a seventh argument on `paint` had been refused because the call site in
`lib.rs` had to keep its shape.

**What is not measured, and why no batch changes.** **Several batches do move the focus** — a press
moves it, so `J`, `K`, `M`, `N` and `O`, which all inject one, move it too; the first version of this
paragraph said no batch touched it, which came from grepping the script for the word instead of
asking what a click does. What none of them changes is **how many consoles have it**: exactly one
does, in every published run, before and after this change alike. And what the unfocused branch does
is **strictly less work**: one stroked rectangle over one cell, in place of a filled rectangle plus
the glyph underneath repainted on top. It is not a new painted path, so there is nothing new for the
report to declare (**D13**), and no control batch stops comparing with the ones already published.

> **The trap this delivery found, and it is not about the cursor.** `TermColor::Cursor` and
> `TermColor::Foreground` are **the same value** in the factory theme. So the usual fix for a
> painting test — write the concrete colour instead of calling what is being mutated — pins nothing
> here: swap the two entries of the palette, or paint the cursor with the foreground, and a test
> built on the default theme stays green. Every assertion about the cursor's colour runs on a theme
> whose cursor differs from its text, and that mutation is on the list. Before pinning any colour by
> its literal, check whether another entry of the palette holds the same one.

## D37 — The cursor blinks by default, the program can switch it off, and `cursor.blink` is a ceiling over both

**Decided.** The console being typed into, in a window that has the desktop's focus, **blinks its
cursor** — 600 ms lit, 600 ms dark — and it does so with **nobody having asked**. The program inside
can switch it off (a steady `DECSCUSR` shape, or `CSI ?12l`) and hand it back (`CSI 0 SP q`).
`cursor.blink` in the settings file is a **ceiling in both directions and not a default**: `on`
blinks however steady a shape the program asks for, `off` never blinks however loudly it asks, and
the line absent leaves the program deciding on top of a default of «yes». It **stops after 15 s of
keyboard silence, ending painted**, and every keystroke puts the phase back to the start, so a cursor
is lit the instant you type.

**Why by default, which is the half that was decided twice.** The first decision — asked of the user
at the issue's start, and recorded — was *who wins between the settings file and the program*, and
that answer stands: the program decides, the key is the ceiling. What that question did not settle is
what happens when the program says **nothing**, and nothing is what a shell prompt says. The engine's
own default is steady (`CursorStyle::default()` is `{ Block, false }`), so «the program decides»
silently meant «no blink, ever, in a shell» — which is where a cursor is looked at most, and is not
what any other terminal on the machine does. It was caught at the delivery's own scope gate, by the
user, in one sentence: *the way terminals do it, not only when the program asks for it*. The
reference implementations agree — kitty, GNOME Terminal and WezTerm all blink out of the box.

**Where the default lives, and why it is one line.** `crates/term-core/src/screen.rs` builds the
engine's `Config` with `default_cursor_style: CursorStyle { shape: Block, blinking: true }`. **No rule
of this application's own is added:** `Term::cursor_style()` already falls back to that field, and
DECSCUSR's `0` is `set_cursor_style(None)`, which restores exactly it. So every sequence keeps the
meaning its standard gives it and what changes is only what the terminal falls back to. A terminal
reset (`ESC c`, `tput reset`) clears the style and lands on the same default, which is the behaviour
one would want and not a coincidence: it was read in the source, not assumed. **One consequence
reached by another door** is that OSC 50 `CursorShape=` sets the shape alone, so a program that picks
its shape that way inherits the blink; it follows from the rule and carries a test of its own so
nobody has to guess. **And a second, found by completing the coverage**: the style is one field on
the terminal and the alternate screen does not scope it, so what a full-screen program asks for
**outlives it** — leave an editor that wanted a steady cursor and the shell comes back steady, until
a reset. xterm and kitty behave the same way, which is why it is pinned as it is rather than worked
around, and the settings manual says so where somebody would meet it.

**The alarm is armed from what the painting drew, never from what the program asked.** This is the
defect the design would have shipped. The engine's blink bit is **independent of whether the cursor
is shown**: a full-screen program sends `CSI ?25l`, keeps reporting that it wants a blink, and with
`cursor.blink = on` the ceiling forces it — so a window would wake at 1.7 Hz to paint nothing at all.
The painting is the only place the three guards that leave `paint_cursor` have already run, so the
painting sets the flag and the deadline reads it. **And the flag has to be asked about twice**: it is
set by the interface half, which does not run while the window is minimized or covered, so the
`logic` half — which does run there, alone — was rearming every 600 ms off a stale `true`. The
deadline asks egui for the desktop's focus as well, from `raw.focused` rather than the processed
field, because `Context::run_logic` refreshes only the first and says so. Both were found by
challengers against the code, and both had a failing test before they had a fix. **The size of that
second one was overstated in the first telling of this entry, and the correction is left visible**:
it said «for ever», and it is not — the keystroke anchor is refreshed by the same half that is not
running, so the fifteen-second stop ends it. What the guard removes is on the order of twenty-five
needless frames per minimize. The defect was real; the number attached to it was not, and in a
record whose whole worth is that its numbers can be checked, that distinction is the point.

**Why it stops after 15 s, and why that is not a nicety.** This is a terminal for **hosting agents**:
its panes are left open for hours with an agent's editor sitting in insert mode, which is exactly the
state that asks for a blink. Without the stop, one such pane keeps the window repainting at 1.7 Hz
for as long as the machine is on, and **D13**'s «the window is at rest» — the premise every published
measurement rests on — stops being true permanently. kitty ships
`cursor_stop_blinking_after 15.0` on by default with the same reasoning. The stop is tested at its
boundary in both directions: at exactly 15 s the blink is over and the cursor is **painted**, and the
last deadline before it is clamped to the stop so the frame that repaints it lit is asked for.

**What the measurement does with it, which the design got wrong once.** The report declares
`blink: N frame(s) woken by a blinking cursor` with the reason next to the number, and
`scripts/measure-frame.sh` marks a run `.INVALID` when N is not zero. The rule is **one-directional by
construction**: unlike the guards for emoji, fonts and mouse reports, there is no batch in which it
should fire. The first plan for keeping it at zero was that the run would write `cursor.blink = off`
into the configuration directory it already isolates — **and that does not work**, because the same
measurement gate that isolates the directory also vetoes reading the file at all. The ceiling comes
from the gate itself instead, which is the regime that already vetoes the settings and forbids
saving. Without this, every batch would have spent its first fifteen seconds waking the window 1.7
times a second, and the script's own rule would have marked every measurement this project has ever
published as invalid.

**What was discarded, and why.**

- **Leaving the engine's default alone** («steady»), which is what this delivery's own design assumed
  until its scope gate. A shell would never blink, and a shell is the common case.
- **Writing the default into `crates/app` as a rule of its own** rather than into the `Config` the
  engine already consults. It would need a second place to read whenever a sequence changed meaning,
  and `CSI 0 SP q` would have had to be reimplemented to restore it.
- **Ghostty's softer rule** — an explicit setting still lets `DECSCUSR` through and only DEC mode 12
  is ignored — and an **asymmetric** third that was named by a challenger: `on` as a floor and `off`
  as a ceiling, so a program may switch a blink off but never on. A ceiling somebody can set and a
  program can climb over is not a ceiling; WezTerm's `cursor_blink_rate = 0` and kitty's
  `cursor_blink_interval 0` are both absolute. And the asymmetric key is rejected on its manual:
  «`on` means at least, `off` means at most» is a rule nobody can predict from reading one line of a
  settings file.
- **A `cursor.blink.rate` key, and reading the desktop's own interval.** The value is checkable
  rather than invented: `gsettings get org.gnome.desktop.interface cursor-blink-time` answers `1200`
  on this machine, described as the cycle, so 600 ms per half puts this terminal in step with every
  other window on it. Reading it at runtime is what is refused, for reasons this repository can
  check: the application links neither GTK nor gsettings, so the alternative is spawning a process on
  the start-up path — the one cost **D26** lets only the font-by-name setting pay — and it is a
  Linux-only answer to a question **F2.7** is about to ask on macOS and Windows. A rate key is
  something nobody has asked for.
- **Blinking for ever while the program asks.** See the fifteen-second stop above.
- **A deadline of its own beside the painting**, instead of inside `Window::set_the_alarm`. That
  function is already called from both halves of the pass, which is the exact defect the knowledge
  card `the-heartbeat-was-armed-a-frame-late` records; a new call site would have to earn that fix
  again, and that card says the second half was only ever found by mutation.
- **A `blinking` field on `Pane`**, or a second `bool` argument through the painting. Seven call
  sites of a second `bool` read as two independent questions when it is one, and a field on `Pane`
  hits that block's own ratchet — the same shape **D36** already discarded for the focus. The focus
  and the blink travel as one value instead, which is why painting a pane still takes seven
  arguments.
- **Having the measurement report declare frames painted with the desktop's focus, and invalidate on
  zero**, proposed by a challenger so that a zero could not mean «the window never had the focus»
  rather than «the blink was off». It is unnecessary here because the ceiling is applied in code by
  the measurement gate: the zero is guaranteed whatever the focus did, and the line says so.

**What it cost in `lib.rs`, because D28's ratchet is part of every delivery's story here.** The blink
needs fifteen production lines in the window's core, not the nine the previous delivery had pre-paid
by moving the deadlines out. So a second piece came out in this one: **applying what the settings file
asks for left for `settings.rs`** — 19 lines, the piece this delivery was already editing, which is
exactly the way through D28 records. `lib.rs` went 1 367 → **1 362** and `impl Window` 411 → **401**,
both re-measured. The ratchet's verdict on this delivery is therefore that it **did not** pay for its
own lines out of what was already free, and the record says so rather than rounding it off.

---

## D38 — How a console's reading ended is a value its own thread reports, and «nobody was listening» is not «the program finished»

**Every ending used to look identical from outside.** What tells whoever hosts this crate that a
console is over is the output channel closing — and a program that exited, a read that failed and a
reader thread that died all close it the same way. So the host could know that a console was over and
never why, which is the whole of what **#192** asks and the output-side twin of what **D29** already
publishes for input.

### The decision

- **The reason is a value, and the thread that ended is what reports it.** `term_core::Ending`,
  stamped into a cell shared with `Pty` and read through `Pty::ending()`. Nothing outside deduces a
  reason from a closed channel, which is **D6** applied to this: only whoever did the work knows what
  its ending meant.
- **It is stamped before the thread lets go of the channel that announces its end**, so anybody who
  has noticed a console is over can already ask why.
- **`EndedWell` merges a program that exited with a console somebody closed**, deliberately: no
  consumer acts on the difference, and the one signal that could tell them apart is already spent
  telling both of them from the arm below.
- **A failure carries the operating system's number as well as the kind.** The standard library
  flattens most of what a pseudoterminal can answer into one bucket whose entire printed form is the
  word «uncategorized», while the number survives. Measured by running, not read: errno 5, 70 and 75
  all come out as that bucket and all keep their number. A sentence built from the kind alone would
  have said nothing at all.

  **And `EIO` is NOT the example, which this entry had wrong two bullets from where it says why.**
  `EIO` never reaches the failing arm at all: the pseudoterminal library maps it to end of file, on
  both platforms, which is the bullet below. The errnos that do arrive and do flatten are the other
  ones — a device or driver answering something `std` has no name for — and they are the reason the
  number travels. The field stands; the example given for it was refuted by the same delivery.
- **«Nobody was listening» is its own ending, and it is the one this vocabulary exists for.** A
  delivery that fails has two reasons and they mean opposite things: the stop flag being up is
  somebody closing this console, while the far end of the channel disappearing on its own means the
  program inside may be alive and writing into an output that goes nowhere. **Answering one failure
  for both published that second case as «the program finished»** — the exact silence the ticket is
  about, arriving through the mechanism built to report it. A review found it; it had shipped in no
  release.
- **A reader that dies reports nothing**, and there is no variant for it. That is stated in the type
  rather than papered over: `None` from the accessor means «nothing was recorded», which also covers
  a reader abandoned at the closing deadline (**D32**).

### What was discarded, and why

| | |
|---|---|
| **An enum payload on the output channel** (`Bytes` / `Ended(reason)`) | It changes the type every console, every test and three modules pass around, for a value that arrives once |
| **A payload on the channel that already announces the thread's end**, whose own comment invites exactly that | Right shape for a reason only the destructor learns, wrong one for a reason the host asks about on every frame: receiving consumes, so the second asker finds nothing |
| **One accessor answering both what the program has not taken and how the reading ended** | Two lifetimes behind one accessor: the backlog changes every frame, the ending once per console |
| **Inferring a panicked reader** from «the channel closed and nothing was recorded» | That is the app deciding what somebody else's failure means, which **D6** forbids. Catching the panic so the thread reports its own death is a separate decision |
| **`Ended`, and other shorter names for `EndedWell`** | The word is the one the decision about what closing means is written in, and one vocabulary for both is worth more than one syllable at the point of use |
| **Naming the reading loop `pump`** | In this workspace pumping means *draining* that channel with a budget per frame — the pane's own method and the number D13's reports publish. This end fills it, and one word for both directions is how a vocabulary starts lying |

### What it leaves open

- ~~**An interrupted read still ends the console.**~~ **Closed by #194, and the reason it is retried
  is not the one first written down.** The obvious argument — «a signal interrupts the blocking
  read» — was checked and refused: nothing in this process installs a handler. What justifies it is
  `io::Read`'s own contract, which says a reader may answer this for no reason and that the call
  should simply be made again; nothing under the pseudoterminal library retries it, and that library
  absorbs only the one errno that means end of file. So the failure mode was not a silence, it was a
  **false sentence** on the glass about a console whose program was still writing.

  **And the retry consults the stop flag, which is the half that is not obvious.** A retry is a wait,
  and the rule this crate has been bitten by twice is that a thread which waits is not looking at
  what would wake it: without the flag inside it, a source answering «interrupted» back to back is a
  loop at full speed that closing cannot reach, and `Drop` would abandon a **spinning** thread for
  the life of the process. Its test makes the call on a thread of its own with a deadline, because a
  test for that shape otherwise hangs the suite instead of failing it.
- **On Windows no end of file can arrive while the console lives, and that is the opposite of the
  risk this entry first recorded.** The first draft said the ordinary-exit arm rested on a Unix-only
  translation and might narrate every normal exit on Windows. **It was refuted by reading the
  dependency**: `portable-pty`'s Windows side builds two anonymous pipes and hands the reader a
  `filedescriptor::FileDescriptor`, whose own `Read` maps a broken pipe to end of file exactly as
  Unix maps `EIO` — so the arm is reached the same way on both. What the same read turned up instead
  is real and belongs to **F2.7**: the pipe's write end is owned by the pseudoconsole and shared with
  the master through one `Arc`, so dropping the slave closes nothing and the read never ends while
  the `Pty` is alive, whether or not the program exited.
- **Telling a close from a vanished listener DOES rest on the host's field order, and this entry
  said the opposite for a while. The arc is worth more than the conclusion.** A review recorded the
  dependency; a skeptic refuted it by reading that the bridge *moves* the receiver into its own
  thread, which parks in a receive and cannot let go until the reader's sender closes; and a second
  reviewer refuted the refutation by finding **the bridge's other exit**. It breaks out of its
  pick-up loop when its own stop flag is up — the flag its destructor sets. At the time this was
  recorded that path was reachable **only while the queue was at its ceiling**, precisely the jam
  that also parks the reader inside a delivery. A test in that module demonstrated the receiver
  being dropped with the console alive.

  **Amended by native Windows testing in November 2026 (#187).** A dropped bridge now checks that
  same flag before enqueuing every chunk, not only while waiting for capacity. Otherwise it keeps
  the receiver until a full megabyte accumulates: a streaming Unix writer hides this, while
  ConPTY renders changes in its buffer and need not deliver the child's original byte volume.
  The regression keeps the queue below its ceiling, drops the bridge and supplies one more byte;
  the thread must release its receiver and captured wake callback. It still needs that next chunk
  to leave a blocking channel receive; no polling thread or new silence guarantee was added.

  So: a pane declares its console **before** the bridge, and Rust drops fields in declaration order,
  which is the only reason an ordinary close raises the flag first. **Reverse those two lines and
  every ordinary close of a jammed pane is recorded as «nobody was listening»** — the terminal
  telling somebody a console went silent on the pane they just closed. `term-core` cannot detect the
  violation, so the dependency is stated where the delivery makes it and the test belongs in the
  consumer, in whoever first reads the distinction. **The comment in `deliver` says exactly that and
  is the one to trust**; this bullet is what was wrong twice.

  The transferable half: a refutation is a claim too, and «the receiver cannot be dropped first»
  needed the same exhaustive read of the exits that the original claim needed. One exit was missed,
  and it is the one that matters, because the state it requires — a jammed queue — is the state a
  pane closed mid-dump is in.

---

### Windows test witnesses — November 2026

A raw consumer of a Windows console's output must answer the initial cursor-position request
before expecting the child to write. The test consumer uses the same incremental `Screen` reply
path as the application, with one parser retained across reads. A nonempty screen proves no child
execution: ConPTY can draw before the program starts. A listener-loss witness therefore establishes
child-produced output before disconnecting its receiver, leaves the child producing output, and
requires `NobodyListening` exactly. Waiting longer, killing the child or accepting any abnormal
ending would test a different event. D111's drain can continue after that ending is recorded.

The error kind belongs to the platform too: raw error 5 is not `Uncategorized` on Windows. The
synthetic failure witness must preserve both the original raw number and its platform's kind;
a Unix mapping is not a portable oracle.


## D39 — A notice takes at most half its console, and it may not be drawn at all rather than drawn unreadable

**A held paste's notice grew to fit its text and was only clamped to the console.** At the width two
panes get — a console of 168 pt, measured — that meant the box ran the console's **whole height**, so
**any click anywhere in that console discarded the paste**. The entire console was a dismiss button,
which is **#198**.

### What this one decided

- **Half the console, and the half is what it buys, not a taste.** A console's last output is at the
  **bottom** of its grid; it is the first thing somebody wants to mark and copy; and marking begins
  with a bare press — the same press that answers a notice. So the bottom half stays theirs. It is
  also what leaves room for a second thing to say about one console, which is why the delivery that
  needs that room is where this was asked for.
- **The floor is measured against what the ceiling leaves, not against the box the text asked for.**
  Those were the same question only while the box grew to fit its text. Under a ceiling they are not:
  a three-row console gets a box of ≈26 px over a text area of ≈10 — under one line — which is
  stroked, swallows the press and shows nothing. That is precisely the failure the floor has always
  existed against, recreated by the ceiling.
- **And the floor asks for the whole answer, not for a line.** At this application's narrowest
  console the line saying how to get out wraps to about three, while a five-to-seven-row console has
  room under the ceiling for one. «A line fits» let through a box showing a third of an instruction:
  legible enough to look deliberate, useless for getting out.
- **So a console too small gets no notice at all, and that silence is accepted with its eyes open.**
  A held paste with nothing on screen saying so is the failure **D31** exists against, and it is
  still the better of the two: an unreadable rectangle that eats the press is worse than nothing,
  because it looks like a working control.
- **The sentence is clipped to its own box**, and that is the half that was missing rather than a
  detail. Without it the ceiling shortens the box and nothing shortens the text, so the lines past
  the ceiling are drawn **outside** the rectangle, unbacked, over the grid's bottom half — and
  outside the rectangle a press is judged by, so a click on the visible words starts a selection
  instead of answering. **The argument that chose the ceiling over the whole sentence — «a clip takes
  the tail, never the answer, because the keys are first in the galley» — was true of a clip that did
  not exist**, and it was published in two docblocks and in the answer to the question that decided
  it before a review read the painting.

### What D39 discarded, and why

| | |
|---|---|
| **Letting the box grow past the ceiling where the sentence needs it** | On a console of twenty cells and forty rows the sentence needs 400 px and half is 312, so the exception would be the common case at the width two panes get — which is #198 coming back |
| **Shortening the sentence until it fits at twenty cells** | It changes what the notice *says* to solve a geometry problem, and what it says is the part somebody has to act on |
| **Keeping a box on any console, unreadable or not** | A stroked rectangle that answers a press and shows nothing readable is a control that lies. The silence at least does not |
| **A floor of «one line of text»** | Measured: it admits a box showing a third of the way out |
| **A ceiling expressed as a number of rows** | It would have to be re-derived every time the font changes; a share of the console does not |

### What the second notice added to it (**#192**'s last delivery)

The room this ceiling was asked for is now taken, and what fills it is the sentence about a console
that stopped receiving. Two things follow from the ceiling itself; what that sentence *says* and what
a click does to it are **D41**'s.

- **The question keeps the top and the statement gives way.** Both notices share this ceiling, so on
  a console with room for only one the held paste is drawn and the silence is not. The order is not
  alphabetical: a held paste is a **question**, with bytes waiting on an answer and a key to press,
  and the other is a statement about something already over that nothing can undo.
- **The statement is drawn whole or not at all**, which is the opposite of what this ceiling does to
  the question, and the difference is exactly the thing above. A clipped question still shows its
  answer on the first line, so the ceiling can win there; a clipped statement protects nothing.
  **What enforces it is the ceiling and not a floor of its own**: a box the console cannot hold is
  clipped to the console, whose bottom is past the half by definition — measured, and the separate
  clause that used to say so was dead code.

### What D39 leaves open

- **A console between three and about six rows holds a paste in silence.** Three rows is a shape the
  split gesture deliberately allows, so this is reachable rather than theoretical. The honest answer
  is probably that such a console should not accept a held paste at all — refusing at the moment of
  pasting, where there is still something to say — and that belongs to whoever owns the paste, not to
  the painter.

## D40 — «Finished» means the reading ended well, so the window stops closing itself over a failure

**A console that broke used to count as one that had finished.** What tells the window a console is
over is its bridge closing, and a read that failed, a reader that died and a program that exited all
close it the same way — so a window whose only console broke **closed itself**, taking with it the
output that would have explained what happened. **D38** made the reason a value; this is the first
thing that reads it.

### The decision

- **A pane is finished when its reading ended well, its bridge is gone and nothing it wrote is left
  unshown.** The three conditions together, in `pty_output::is_finished`.
- **Nothing recorded is not «finished» either.** It is what a console still reading answers — see
  **D38** for the two things that answer belongs to — and the honest reading of it is «this console
  is not over». Erring towards a window that stays open is erring towards output somebody can still
  read. (The reader abandoned at the closing deadline never reaches this question: by then the pane
  and its pseudoterminal are gone.)
- **It is asked of the pseudoterminal, not of anything the frame keeps.** So what the window does
  with a console that ended does not depend on whether the pump happened to run this frame.
- **The decision lives outside `crates/app/src/lib.rs`, and the reason is what a test can reach.**
  A pseudoterminal whose *read* failed cannot be built from outside `term-core`, so the two failing
  arms — a failure and a panicked reader — are reachable only through a value, which is what makes
  the decision exhaustively testable as a function and not as a method. The pane's own question is a
  different test and needs no failure: **a bridge with nothing behind it** leaves the reading with no
  listener, which is an ending that is not «ended well» and reaches the predicate exactly as a
  failure would. Both tests exist; a review found the second after this entry claimed no test could.
- **What the window does when a console ends badly is nothing, for now**: it stays open with the last
  output on screen and copyable. The sentence that says why is the delivery that follows this one.

### What was discarded, and why

| | |
|---|---|
| **Leaving `is_finished` alone** and letting the sentence over the console be the whole answer | The sentence would be painted over a console the window is about to close itself over. A window that vanishes cannot say anything, and this is the half that can be had on its own |
| **A latch beside `some_closed_by_hand`** — «somebody's console broke, never auto-close again» | Three lines in the file whose ratchet (**D28**) forbids growth, for a bit that is already derivable from the endings the panes report. And it would outlive the pane that set it, so closing the broken console by hand would not give the behaviour back |
| **Asking the bridge's own record of the ending** instead of the pseudoterminal | The bridge learns the reason on the frame the pump runs, and closing must not depend on that. Two readers of one fact, one of them a frame behind |
| **Treating «nothing recorded» as finished**, which is what the code did before | It is the answer of a console that is still reading. Reading it as «over» is the inference **D6** forbids, and it is the exact defect this entry closes |

### What it leaves open

- **This entry said no test could catch the pane forgetting to ask, and that was wrong.** It is
  worth keeping because the reasoning looked sound: the state the question distinguishes needs a
  reading that failed, and that cannot be built from outside `term-core`. What it missed is that
  **the distinction does not need a failure** — any ending that is not «ended well» will do, and a
  bridge constructed over an empty channel produces one with the program still alive. Reverting the
  pane's body to «the bridge closed and no queue is left» now fails
  `a_console_whose_reading_ended_badly_does_not_close_the_window_on_its_own`. Two reviewers refuted
  the claim independently; the mutation was re-run before and after.
- **And one cross of the decision's own three inputs was covered by nothing**, found the same way: a
  reading of `!alive && (!queue_left ^ (ending != EndedWell))` passed all four of its tests and
  declared a console **broken with a queue pending** finished — the window closing over the exact
  bytes that explained the failure. Its test walks every ending with a queue left.
- **A console whose program left a survivor still never ends**, so it is never finished and the
  window never closes over it. Unchanged by this, mechanism in **D32**, and the obvious fix is worse
  than the silence: `Child::try_wait` answers «the child is gone», not «this console is over».

## D41 — What a console that stopped receiving says, and what a click does to it

**D38** made how a reading ended a value, **D40** stopped the window closing itself over an ending
that was not ordinary, and **D39** left half a console free for something to be said there. This is
what is said, and it is the only part of **#192** a person sees.

### The decision

- **One sentence on the glass, the cause on the developer channel.** *«This console stopped receiving
  its program's output. The program may still be running.»* Asked and answered by the user, against
  three sentences over the console: from the chair the two reasons are acted on identically, and the
  difference between a read that failed and a reader that died is actionable in the channel that
  carries the `ErrorKind` and the errno, not over the grid.
- **It names no key.** There is nothing to press to answer it — a click only takes it away — and
  naming a key nobody needs is the defect **#170** closed one notice earlier.
- **It says «what you type still reaches it» only where that is true.** The clause is a second
  sentence, added when the console can still be typed into and withheld when the writer has given up
  — because then **D29**'s own line says «this console can no longer be typed into», and the glass
  promising delivery in the same window where standard error denies it is the kind of falsehood this
  project treats as worse than silence. The reading's ending says nothing about the input side: they
  are two threads with two endings. **A review caught the promise being made unconditionally**, and
  it had been published in the design's own contract.
- **Any ending that is not an ordinary one earns it** — a read that failed, a reader that died, an
  output nobody was left to take — which is the same partition **D40** closes the window by. Nothing
  recorded is a console still reading and says nothing.
- **A click dismisses it, for that console, for good, and undoes nothing.** How the reading ended
  stays exactly as the pseudoterminal recorded it and the window still will not close itself over
  that console; what goes is the sentence, not the fact.
- **The sentence never enters the grid.** It is painted over the console, not written into it: the
  grid is what the program wrote, and a sentence of the application's put there would survive in the
  scrollback as if the program had said it.

### What was discarded, and why

| | |
|---|---|
| **Three sentences on the glass**, one per reason | From the chair the two reasons are acted on identically. The user answered this one, and the difference stays in the channel where it can be used |
| **Saying it in the pane's status panel** | That document belongs to whoever does the work, and `attention` is theirs to declare: **D6**, **D17** and **D18** all forbid the application writing into it |
| **A key to dismiss it** | It would have to be named in the sentence, on every pane, while the keys reach one of them — the defect #170 closed. A click is the gesture that already means «not this» here |
| **Leaving it until the console is closed** | The box is what swallows a press, and the console's last output — the thing somebody wants to mark and copy — is underneath. A notice with no way out is the whole console becoming a button, which is **#198** |
| **Re-arming it on focus, or on any other event** | A dismissal that comes back is a notice that cannot be got rid of. What it costs is written in «what it leaves open» |

### What it leaves open

- **Nothing says which console.** Neither the sentence over the grid — it is over the console it is
  about, so it does not have to — nor the developer line, which goes to a standard error shared by
  every pane and says «this console». With several panes open, the line says what failed and not
  whose. It is the shape all twenty-eight of this crate's lines have, so it is one change over all of
  them.
- **Once dismissed there is no way back**, and the only other surface that would say it is the
  developer line — which is written for a **failed read** alone. So for a reader that died or a
  listener that vanished, a stray press near the console's top-left corner takes away the only sign
  there was. Candidates, none of them free: re-arming on focus, or the tab's own card, which the
  application does own.
- **A console too short or too narrow for the whole sentence says nothing at all**, which is
  **D39**'s accepted price and is worth naming twice: on such a console what a person sees is a
  console that stopped and a window that stayed open.

## D42 — A button is painted with its command underneath, and one click sends it

**Status: taken.** Delivered in the second PR of the milestone that gives the panel buttons, F3.

The panel has been a screen to look at since F1.6: whoever does the work publishes what is
happening and the application paints it. This is the decision that turns it into a place to work
in — a producer declares buttons and pressing one writes into its own pane's program.

The rule this has to satisfy predates it by a year and is in
[03-architecture.md](03-architecture.md) → Security: *a file that declares commands and a button
that runs them is code execution in the user's shell, so the command must be visible before it
runs.* Everything below is how that sentence is made true rather than asserted.

### The decision, in four parts

**1. The literal command is painted under its label, always, and one click sends it.**

Not a confirmation on press, and not a button that arms on the first press and fires on the second.
The command is on screen **before** the pointer arrives, which is what the rule asks; and Part I of
the phase promises answering the agent **without typing**, which a chord would take back.

**2. Painted and sent are the same text, character for character — so a dirty command is refused
when the document is read, never cleaned.**

This is the load-bearing half. That painted text *is* the confirmation, so a filter that removed
characters on the way out would mean the bytes that ran are not the bytes that were read. Refusing
at the read makes the property true by construction: the type holds two private strings and its only
constructor checks them, so there is no path from a JSON file to a pseudoterminal that skips the
gate, and a reviewer checks one function instead of every call site.

What is refused is **every control character** — the kernel's line discipline acts on them before
the program sees them, and that is measured in this repository rather than assumed — plus **a line
break and a tab**, which a paste keeps and a button must not: a line break makes a button that runs
two commands while painting one, and a tab in a line editor is the completion key, so the same
button would do something different depending on what is on disk. And **the zero-width and
direction-changing scalars**, which are painted as nothing or paint the rest backwards.

**3. A press is refused unless the panel is settled, and a button that is not fully on screen is not
pressable at all.**

The panel is the one surface in this window that repaints because **somebody else wrote**, with no
user action. So between the frame a person read and the frame their click lands in, a producer can
have replaced every button — and it needs no attacker, a progress panel reordering its own buttons
does it. A press is therefore only accepted on a frame whose painting reused the layout it already
had. Refusing costs nothing: the panel is drawn again, it is read again, it is pressed again.

And what does not fit is dropped from the bottom of a short panel, so a label can survive with its
command scrolled away. Such a button is **absent from the frame's reach** rather than refused by a
check — there is no branch anywhere that has to remember.

**4. The application appends the terminator. A button always executes.**

The text cannot carry a line break, so whether a button runs is a property of this application and
not a character a producer can get wrong. One meaning, nothing to misdeclare, and the other kind —
a button that only writes the text — can be added later as a new field without breaking a single
producer.

### What was discarded, and why

| | |
|---|---|
| **A confirmation dialog on press** | The command is already on screen. A second gesture buys nothing the painting does not already give, and Part I promises answering without typing |
| **A button that arms on the first press and fires on the second** | Two presses per button is the ergonomics of a dialog with none of its clarity, and the press that answers a painted question in this window is already a bare click |
| **Reusing the held paste's question** (`Ctrl+Enter`) | It is keyboard-only, it holds one pending per console, and it drags [#170](https://github.com/mashware/ivy-history-private/issues/170) with it — every pane painting a question only the focused one can answer |
| **Cleaning the command instead of refusing it** | It makes the painted text a lie, which is the whole mitigation. And a remove-the-bad-characters pass is one refactor away from a search-and-delete, which this repository has already measured being slipped past |
| **Keeping the paste's two exceptions** (tab, line breaks) | A paste is a document somebody copied and its lines are the point. A button is one command |
| **A per-action flag choosing between running and only writing** | A second thing a producer can get wrong, for a case the roadmap's own three examples do not include |
| **Painting a button inert when its command would arrive truncated** | Written, and then removed: a command is at most 512 characters — 2 049 bytes in the worst encoding — against a ceiling of 4 095 on the only platform that answers the question, so the branch could not be reached by any input. What replaced it is a test asserting that the worst a command can compose fits under the platform's ceiling, whose failure message says the mechanism has to come back. **A mechanism with no present scenario is what this project's own design rules forbid** |
| **An origin check on the document** | There is nothing to test: `PANEL_DIR` is created by the application, `0700`, under the user's runtime directory with a name drawn at random, so whoever writes there is a process of the user's own uid. Deducing an origin would be **D6**. What is enforceable is the premise, and it is now written in the security table: the application never reads a panel document from a path it did not create |
| **A hover highlight on a button** | A new painted path woken by a new event, to buy an affordance the symbol and the colour already give (**D13**) |
| **A new theme colour for the command row** | The palette is twenty-two colours somebody writes by hand (**D26**), and a milestone that adds one for decoration is a milestone changing the manual |

### Measured

Batch **`R`** of `scripts/measure-frame.sh`: two consoles, both dumping, **eight buttons** — the
ceiling the contract accepts — seeded on each panel, and a bare press on the first one over and
over. **Its control is `C`**: same consoles, same load, and the only difference is the buttons.

**Two runs of `R`, which `compare` says describe the same window and can be averaged**, against one
of `C`:

| | `R`, first run | `R`, second run | `R`, after the review | batch `C` (control) |
|---|---|---|---|---|
| the walk that decides what a press reaches, left pane | 1.015 µs mean, 1.853 µs p95 | 904 ns mean, 1.503 µs p95 | **1.137 µs mean, 1.884 µs p95** | 244 ns mean, 481 ns p95 |
| the same, right pane | 623 ns mean, 1.182 µs p95 | 574 ns mean, 1.293 µs p95 | 575 ns mean, 1.283 µs p95 | 135 ns mean, 281 ns p95 |
| buttons | 8 416 drawn whole, 173 pressed | 2 456 drawn, 51 pressed | 20 112 drawn, 415 pressed | 0 drawn, 0 pressed |

**The third run is the one the delivery publishes**, and it is here beside the other two rather than
replacing them: the review added a check of the ink's real width to every row drawn, so the number
had to be taken again. It is inside the spread of the first two.

**The two runs differ by about a tenth and the counts by a factor of three, and that is the injector
rather than the work**: a gesture takes two frames and the run lasts as long as the load does, so how
many presses fit is a property of the machine that minute. What is comparable is the per-frame span,
and the two agree.

**The control's figure is not zero, and that is honest rather than a defect**: the walk runs every
frame over every visible row, finds that none of them is a button, and costs what it costs.

**It is subtracted out of the panel's span and not folded into it.** That span means six named
things and its docblock enumerates them so nothing creeps in; laying out and drawing a button's rows
*is* painting, which it already covers, while deciding what a click can hit is not.

**The report's line is read in both directions**, like the emoji and the fonts: a batch that declares
buttons and draws none was never seeded, one that draws them and presses none never landed on a
rectangle, and one that declares none and draws some is being seeded by something other than its own
load.

> **The batch invalidated itself four times before publishing, and every one was a real defect.** The
> symbol first chosen for a button's label was not in the built-in fonts, so every frame that painted
> one sent the window off to search the machine — which breaks the rule that only the Japanese batch
> may do that, and would have cost the comparability of every published series. A symbol nobody can
> see costs one glyph; one that triggers discovery costs the whole table.

### What it leaves open

- **A `send` lands in the shell's history** — `~/.bash_history`, `~/.zsh_history` — outside anything
  this application controls and surviving the pane. It is written in the contract, because a producer
  author will not guess it, and it is not fixable from here.
- **A fully printable command is still a destructive one**, under a label that says something else.
  That is the premise rather than a gap: the painted command is the defence, and there is no second
  one.
- **No batch measures opening or closing a pane**, by hand or over the channel that arrives in the
  next PRs. It was listed in `docs/DEBT.md` — deleted by #150, still in its history — in the same
  class as «no batch draws a block element».

## D43 — A connection holds a pane, the file is not read while it does, and the hold dies with the connection

**Hook publication amendment (November 2026).** Hook publication is the constrained exception in D90: it repaints for the holding invocation without taking its connection or enabling the file transport.

**Status: taken.** Delivered in the third PR of the milestone that gives the panel buttons, F3.

**[D17](#d17--the-contract-is-the-document-not-the-file-the-source-of-truth-is-the-apps-state) said the panel's contract is
the document and `panel.json` is one of its transports, and it left a question open with its name on
it: with two writers of the same place, who rules?** Until this delivery the question was academic
because there was one transport. It is not academic now: a producer can connect to its pane and
publish without writing anything.

The answer is not a rule about ordering. It is a rule about **authority**.

### The decision, in three parts

**1. A connection claims the pane on its first publication, under a source it declares.**

The source is a name the producer chooses and the panel shows — `flow`, `ci`, whatever it calls
itself — and it is what a refusal names, so a caller can tell somebody else's connection from its
own. What is compared is **the connection and never the source**: two connections calling themselves
the same thing are two producers, and letting the second one through because the name matched is
exactly the collision this exists to stop.

**2. While it holds the pane, its `panel.json` is not read at all, unless the holder yields file
refresh after declaring `attention: done`.**

Not read and compared — **not read**. The file cannot identify its producer, which is the property
that made D17 necessary in the first place, so authority cannot be handed to it: with both being
read, who wins is decided by the operating system's scheduling. What it skips is the read and the
parse, and only those: the watcher stays armed on that pane and its notice is still raised — and it
has to be, because that notice is what the pass that ends the hold goes back for.

And because refusing in silence is indistinguishable from being broken, a file publication that is
actually refused is **confessed** in the application's own row, naming the source that is driving the
panel. Nothing is said until a file has really been refused: a standing warning over a panel that is
working perfectly is noise.

The done state yields only file reads; it does not release the connection reservation or its source.
The file can take over only when its filesystem modification time is strictly newer than the last
successful channel write. Each successful publish or patch stamps that cutoff. A later channel
write reclaims the document and resumes suppressing files. Once a valid newer file is painted,
patches are refused until a full channel publication; the holder still retains retirement authority
and other connections are still refused by the holder's source. Disconnect drops the reservation
but keeps the cutoff with the painted channel document, so an old file cannot replace it. A valid
strictly newer file may take over after disconnect.

**3. The hold dies with the connection, and there is no sequence number anywhere.**

A hold that outlived its holder would freeze that pane's panel for the life of the window, and the
file would never be read again — which is what an agent that crashes does. So the release is sent by
the connection's own thread on its way out, and it can be refused by nobody.

**A sequence number was in an earlier shape of this design and was cut.** With one endpoint per pane,
and the file unread while the pane is held, there is exactly **one writer**; a connection's
newline-framed requests cannot reorder themselves. A `seq` would detect only a reordering the
transport cannot produce — and without a release it would be worse than useless: a restarted agent
beginning again at zero would be refused for ever.

**What stays on screen when a hold ends is the last thing that was published.** The agent is gone and
what it last said is still the truest thing anybody knows about that pane; blanking it would throw
away information in order to be tidy. The file takes over from its next publication, not from that
instant.

### What was discarded, and why

| Discarded | Why |
|---|---|
| Comparing dates between the two writers | The file does not know who writes it. Giving authority to the transport that cannot identify its producer is the defect D17 named |
| A sequence number per source | It detects a reordering that one endpoint per pane makes impossible, and a restart at zero would be refused for ever |
| A hold released by an explicit message | A crashed agent sends no goodbye, and that is precisely the case the release exists for |
| Blanking the panel when a hold ends | Throwing away the last thing anybody knows about that pane in order to look tidy |
| A window-wide endpoint with the pane named in the message | Then the server decides which pane a message is for from the message's contents, and the directory's permission stops being the access control |

### What it leaves open

- **A connection that holds a pane and never publishes again keeps it**, and the panel goes stale
  where a person can read that it has. The date is stamped **on arrival** precisely so that happens:
  a delivered document carries none of its own, and leaving it undated would make a held panel
  immortal.
- **On Windows nothing binds yet**, so a Windows pane publishes by file exactly as before. A named
  pipe gets a default security descriptor that lets any process on the machine open it, and the
  delivery that wrote this cannot compile or run a line of Windows code to prove otherwise. **F2.7**
  is where the pipe, its descriptor and the assertion over both arrive together.
- **A caller that gives up waiting still gets its document applied.** Undoing a document already on
  screen is not something the window can do, so the reply is worded for what is true: the window did
  not answer in time, never that nothing was published.

## D44 — An agent names the panes it opened with an opaque handle, and the person's first byte takes them back

**Status: taken.** Delivered in the fourth PR of the milestone that gives the panel buttons, F3.

**D43 answered who rules a pane's document while a connection holds it. This answers a different
question: what may a connection name.** `split_open`, `split_read` and `split_close` act on consoles
other than the one whose channel asked, which is the first time a message has to point at a pane —
and pointing is exactly what the transport refused to allow.

**The three verbs keep `split` in their name, and that is not a slip against D27.** D27 gives
`split` to the gesture and `pane` to the thing; `split_open` *is* the gesture — it splits the pane
that asked — and its two siblings keep the prefix so the three read as one surface. They are names
on the wire, declared by the channel's crate, and nothing in D27's rules turns them away. Renaming
them would change the wire contract, not correct a translation, so on 28 September 2026 they were
kept ([#147](https://github.com/mashware/ivy-history-private/issues/147)). *Discarded:* `pane_open` /
`pane_read` / `pane_close`, which buys consistency with the noun at the price of a change across the
wire, the MCP binary, the application and four documents, for a first verb that already reads right.

### The decision, in four parts

**1. The handle is sixteen random bytes, and it is scoped to the connection that opened the pane.**

Not a pane index, and not `PANEL_ID`. The environment is inherited by every descendant of a pane and
readable out of `/proc/<pid>/environ` by any process of the same user, so it is a name and not a
credential; a small integer would be reachable by a typo. What is checked when a handle is used is
**the pair (the pane whose channel asked, the connection)**, so a second connection cannot use a
handle it overheard, and a pane with no addressable identity answers `null` instead of inventing one.

**2. The keyboard does not move.**

Opening a console for an agent is not the person asking for one. The chord's split moves the focus
because whoever presses it wants to write there; a request over the channel is a machine reading, and
stealing the person's keyboard for it is the wrong direction. Everything else about the new console is
ordinary: it takes its place in the tree like any other, and its program is launched with the pane's
own identity — **no environment travels in the request**, and there is no field for one.

**3. The person's own input ends the access, at the doors the user's input already has.**

A keystroke or a paste (the keyboard's single door), a wheel turn, a mouse report, or a held paste
sent from its notice's button (**D31**, amended again): the places the application writes what a
person produced into a pseudoterminal. The fourth is the same bytes `Ctrl+Enter` sends through the
keyboard's door, reached by the mouse instead. From that byte on, the
handle answers **with the reason** and not with a bare error, and the console's own row — the
application's, which `header: false` cannot suppress — says the same sentence to the person. **A press
on a panel's button does not revoke it**: that door is the application writing on the person's behalf,
not the person taking the console over.

**4. It is an application boundary, and it is written down as one.**

`split_open` chooses the program, so the process it launched keeps reading whatever the person types
into that console afterwards. What the agent loses is **the application handing it that buffer**, not
the program's own read: the promise an agent author is given is *«the app stops handing you that
buffer; the program you opened does not»*. Removing `program` was not the alternative — it would make
`split_open` useless for the one case the roadmap names.

### What was discarded, and why

| Discarded | Why |
|---|---|
| `PANEL_ID`, or any pane identifier, as the handle | It is in the environment of every descendant and readable by any process of this user: a name, not a credential, and a field somebody would start trusting |
| A small integer handle | Reachable by a typo, and it would name a pane that the connection did not open |
| Moving the focus to the console that was opened | The person did not ask for it; splitting from the chord moves it because that gesture is theirs |
| An `env` field on `split_open` | The console's environment is the pane's identity and nothing else; a caller contributing variables would be a second, unchecked way to shape a child process |
| Revoking on any user act — a focus click, a selection, a scrollback wheel | None of them writes a byte into the program, and none of them is the person taking the console over. The wheel only revokes when it goes *to the program*, not when it scrolls the history |
| Revoking on a panel button's press | That door is the application acting on the person's behalf — a remote control they were shown and pressed, not their hands on this console |
| Killing the consoles a connection opened when it dies | They are visible consoles of the person's; an agent that crashes must not take a person's screen with it. The handles die with the thread, the consoles do not |
| A cleanup pass over the panes a dead connection opened | Same reason, and a pass would be a mechanism with no scenario: nothing reads a handle after its connection is gone |
| Refusing to close a console the person has taken over, in silence | The reason travels, and the row carries it: refusing in silence is indistinguishable from being broken |

### What it leaves open

- **The scrollback read is as large as the console's ceiling** — 3 500 lines per console (**D21**) —
  and nothing bounds a loop of `split_read`s. It is not a control: an agent can loop, and what it
  reads is a buffer it was already allowed to see.
- **Revocation does not reach the program the agent opened** (part 4), and that is the decision rather
  than a gap to close here.
- **A pane that closes leaves its handles naming nothing**, and using one is refused as gone rather
  than as somebody else's: the window forgets them when the console goes, and the sentence says the
  console is gone.

---

## D45 — Every console carries a strip you drag it by, and a measured run does not paint one

**Decision.** A **strip over every console**, in every tab and with however many panes there are, a
single console included. It is grabbed with the **primary button and no chord** to take the pane
somewhere else, which is **D15**'s gesture reached through a second door: the chord of **D20** goes
on working, unchanged. And **a measurement batch does not paint it**, vetoed by the same gate that
already vetoes the settings file (**D26**) and forces the cursor's blink off (**D37**); a batch that
wants one asks for it with a knob of its own.

**Why a strip at all, when the chord already moves panes.** `Ctrl+Shift` is a chord you have to be
told about: nothing on screen says a console can be moved. The strip is what says it. That is the
whole of what it buys, and it is why it is the milestone's subject rather than a decoration.

**Why it costs what D20 refused to pay, and what changed.** D20 discarded a header **as part of
F2.6** for a stated price: a header takes height from every console, always, and with one **the
published control batches stop comparing with anything** — the same reasoning that keeps the tab bar
unpainted with a single tab. The arithmetic has not changed; what changed is who pays it. **The bench
switches the strip off**, so the eighteen published batches keep the grid they were measured with,
and what the strip costs **will be** measured in a batch of its own against its control — a batch
that does not exist yet, which is what the closing paragraph of this decision says. D20 is amended
here, not reopened: its reason stands and this is the answer to it.

**The price of that answer, said rather than hidden.** With the gate on, every published batch
describes a console **one row per pane taller** than the one a person sees. The numbers stay
comparable with each other and stop being comparable with the product, and the new batch is the only
place the difference is visible. Nothing forces a later milestone to re-baseline.

**Why the knob is an opt-in outside the five measurement variables.** The script exports the window
size on **every** batch, so the gate holds in every run: with the gate alone deciding, the batch that
exists to measure the strip could never paint one. It is the shape the script's per-batch switches
already have — the ones that turn on the Japanese dump, the emoji dump and the panel's buttons.

**Why the strip is a second door into the same gesture and not a gesture of its own.** Everything
that hangs off moving a pane — the identity the drag is held by, the half previewed before dropping,
the tree transformed by value or not at all (**D15**), D20's latch — is earned once. A widget of its
own would have to earn every one of them again, and two mechanisms that must agree about one gesture
is how they stop agreeing.

**What it drags along, and it is in the code with its tests.**

- **The row is reserved wherever it is painted, and nowhere else.** One value decides both, resolved
  once at startup beside the settings the same gate vetoes. There is no regime where a row is
  reserved and not drawn, or drawn and not reserved.
- **A pointer is measured against the console's rectangle, never the pane's.** Otherwise every
  selection and every mouse report slides by a row and a press on the strip reaches the program as
  row 0. The seam was moved first, in a delivery of its own that changed no behaviour.
- **Splitting counts the strip.** Each daughter owes one, so a pane that fits two consoles of three
  rows no longer fits them: it is refused instead of producing two consoles below the floor.
- **The cut is bounded.** `Rect::split_top_bottom_at_y` builds an inverted rectangle when the cut
  falls past the bottom (emath 0.36.1), which a window dragged shorter than a pane's strip reaches.
- **What it costs the frame is declared on its own line** (**D13**): a path that is not declared is
  published as the framework's cost, which is the figure **D3** judges egui by. It is zero in every
  published batch, and declared all the same.

**What was discarded, and why.**

| Discarded | Why |
|---|---|
| Re-running the seventeen published series with the strip on | Days of machine time to restate what is already known, and the two halves would still not be comparable with anything published before |
| Splitting the history into «pre-header» and «post-header» | Two series that cannot be compared, and every later reader having to know which one they are holding |
| A setting to switch the strip off | The decision taken is «always»; the only regime that needs it off is the bench, and that is the gate. A key would also have to join the manual's table and its floor |
| Showing it only where a pane has siblings | A console that is alone can still be dragged onto another tab's layout later, and a strip that comes and goes teaches nobody that the gesture exists. **The button the strip carries is the opposite case and it is decided the other way** — see **D46** |
| Chrome overlaid by proximity, with no row of its own | **D20** already discarded it: it has to be discovered and leaves no room for the close button. Kept as the fallback if the measurement makes the row unaffordable |
| A widget of its own for the strip | See above: it would have to re-earn the identity, the preview, the atomicity and the latch |

### Measured — what the strip costs (18 September 2026)

**Two runs with the binary that merged**, on `main` after every code PR of this milestone had
landed: `scripts/measure-frame.sh T` and `... C`, window of 1280×800 points, two panes dumping,
neither run touching the other. `scripts/measure-frame.sh compare` says both pairs of runs describe
the same window and can be averaged.

| Run | Frames with a strip | Header total | Per frame | Grid, `T` | Grid, `C` |
|---|---|---|---|---|---|
| 1st | 436 | 228.712 µs | 524 ns | 48×40 | 49×40 |
| 2nd | 378 | 388.846 µs | 1.028 µs | 48×40 | 49×40 |

**Averaged over both runs: ~759 ns per frame** — (228.712 + 388.846) µs over 814 frames. Against the
60 Hz slot that is under a thousandth of 1 %.

**The range is wide on purpose, and it is said rather than rounded away.** The two runs differ by a
factor of two, 524 ns against 1.028 µs, on the same machine and the same binary — the signature of a
strip that is one flat rectangle plus a stroke, cheap enough that the frame's own noise dominates it.
Publishing a single figure to three digits would be feigning precision D45 does not have.

**`T` and `C` do not share a grid, and the asymmetry is declared rather than subtracted.** Every run
of `T` measures 48 columns per pane; every run of `C` measures 49 — one narrower, with the same 40
rows and the same window. Both directions of the guard hold across both runs: `T` never painted zero
strips and `C` never painted one, so the report's two-way invalidation (**AC8**) is exercised by the
very runs this annex cites. The column lost to `T` is not folded into the per-frame number above; it
is a fact about the grid the reader compares, not about what the strip costs to paint.

## D46 — The strip carries the button that closes its console, and a console with no sibling carries none

**Decision.** At the right-hand end of every strip, a cross on a target two cells wide: pressing it closes that
console and its siblings reabsorb its place, which is what `Ctrl+Shift+X` already does. **A console
that is the only one of its tab carries no button**, only the handle — and neither does one whose
strip has no room for the handle and the button at once. There is no confirmation.

**It closes the console by name, not «the focused one».** The first shape of this moved the focus
onto the pressed console and reused the chord's door, and that is wrong in a way only a third console
shows: a button is pressed on the console it belongs to, which is very often **not** the one being
typed in, so forcing the focus across handed the keyboard to the neighbour of the console that left.
Somebody working in a third pane lost their place to a gesture aimed at neither. The by-name door
moves the focus only if the console that leaves was holding it. For the same reason the ordinary
«a click gives this console the keyboard» rule skips the press spent on the button: a press and a
release inside one frame — a quick click — would otherwise focus the console on its way out.

**Why the lone console has none, which is the decision inside the decision.** With no sibling there
is nowhere to give the place back to, so closing it closes the **tab** — and on the last tab, the
window, with whatever an agent was doing inside it. That is one click on a target two cells wide,
against the three keys the chord asks for, and it is a different act from the one this button
promises. The precedent is already in the window: **the tab bar is not painted with a single tab**
either. What is lost is nothing — the chord and the tab's card still close it.

**Why no confirmation.** This terminal has no dialogue and D45's strip is not the place to grow one.
What limits the damage is the rule above and the fact that a destructive press is a press on a
declared, visible two cells, rather than a modal surface every console would then have to answer.
Said plainly rather than left implicit: **pressing it ends a program without asking**, and the
milestone accepted that.

**Why its own hit list and not the panel's buttons.** A panel button runs **the producer's** text
under **D42**'s visible-before-it-runs rule; this one acts for the person, on the window's own
chrome. Putting the second into the first's list gives one consumer two owners and two rules.

**Why it acts on the press.** It is what the panel's buttons already do, and the press is the one
instant whose position can be trusted: the queue's own event carries where the button went down,
while egui's pointer state carries where the pointer **got to** — a frame that batches a press with
a move after it would close a console nobody aimed at. Acting on the release would need the pressed
button's identity kept across frames, which is a second latch of the kind `pointer.rs` already
documents as fragile.

**Why the name is noted and the console closed at the foot of the pass.** Not a borrow — the same
walk already transforms the tree inline when somebody drops a console. It is the **rectangles**: the
pass is walking a tiling computed before the close, and closing reabsorbs a pane's place among its
siblings, so from that instant every one of them describes a console that has moved. And the pass
**says the tree changed**, or the console that stayed is painted — and its grid announced to the
program inside — in the half of the window it had while its neighbour was alive.

**One click closes one console, and that needed saying in the code.** A pointer press **cannot be
consumed** — egui needs it to decide who is being dragged — and egui can run the interface pass twice
over the same input, so the second pass reads the same press again. The program's own presses already
defend themselves by counting the frame; without the same count here, a tab of three consoles with
the rightmost one's button pressed lost **two** programs to one click: the survivors refill the area
and the new rightmost strip puts its button on the same abscissa the press is still sitting at. Two
consoles hide it, because the survivor of the first close carries no button at all.

**Amended on 26 September 2026** ([#290](https://github.com/mashware/ivy-history-private/issues/290)):
**the second pass does not read the same press.** egui 0.36 moves a frame's events out of the raw
input before each pass (egui 0.36.1, `data/input/raw_input.rs:137`, called at `context.rs:854`), so a
pass repeated after a discard receives no events; the pointer's position and held buttons, the keys
held down and the modifiers are kept, and a drag delta is zero there. What does run twice is the whole
interface closure. **The two programs lost to one click were reproduced by the test's hand-made
second mouse pass inside one egui pass, not by egui** — the paragraph above describes the mechanism
it guards against, not something this egui was seen doing. The frame count on
this button is **kept, as defence**: it costs one frame compare, and an egui upgrade that started
replaying events would turn one click into two consoles closed, with only the tripwire test
`egui_hands_a_frame_s_events_to_its_first_pass_only` to warn. Discarded: removing the guards, which
saves a few lines and leaves that upgrade silent; and keeping only the destructive ones, which
splits one rule into two for the same price.

**A strip with no room for both keeps the handle and loses the button.** Nothing stops a window being
dragged narrow — no minimum size is asked of the desktop, which is why the strip's own cut is already
clamped — and a button merely squeezed would take a narrow strip **whole**: every press on a console's
top row ending its program, and the band that exists to be grabbed grabbable nowhere. The chord still
closes that console.

**The button does not take the strip's right-hand corner.** That corner is where a folded panel's
handle sits, and **D45**'s delivery paid a review finding to keep it grabbable. The first version of
this one put the cross flush against the edge, and the test that presses at that abscissa caught it
at once: the console **closed** where somebody had reached to move it. The button starts where that
corner ends, taken from the two constants that build it, and the one line the two share belongs to
the grab — the same tie the strip already decides on the line it shares with the console.

**What was discarded, and why.**

| Discarded | Why |
|---|---|
| A confirmation before closing | A surface this terminal does not have. What answers the risk is the lone console's exception, not a dialogue |
| A button on a console with no sibling | It would close the tab, and on the last tab the window. See above |
| Reusing the panel's `PaintedButton` list | Ownership: that list runs the producer's text under **D42**, this acts for the person |
| Closing inside the pointer's walk | The walk's rectangles would describe a tiling the close has just made false |
| Acting on the release rather than the press | A second latch across frames, for a gesture whose position is only trustworthy at the press |
| Closing through the chord's own door, «the focused one» | It makes the argument travel as a side effect on the keyboard, and on three consoles it moves the keyboard off the one being typed in |
| Squeezing the button into a strip too narrow for it | The narrow strip becomes one big close button, and the handle disappears |
| A guard so that the press which closes does not also arm the drag | **Asked for by the design and cut after measuring.** Its reason was that egui cannot consume a pointer button event. What is true is that egui does not decide a press is a drag **in the frame the press arrives in**, and by the next frame the console has been closed at the foot of that pass and nobody interacts with its widget again. Written, mutated away, and no test moved: a guard that fires in no frame that exists |

**What it did to three published batches, said rather than left silent.** The scan for a pressed
close button walks the panes on any frame carrying a bare primary press, inside the mouse's already
declared span (**D13**, so nothing is mislabelled). Batches `K`, `M` and `N` — the three that inject
gestures — gained that scan **after** they were measured and published: their figures are from
before this decision, and the milestone does not republish them. Rejected: hours of machine time to
restate what the scan does not change enough to matter, and it would turn D45's annex into a new
measured series nobody asked for.

---

## D47 — A line carries an address its producer chose, and a patch is all-or-nothing over it

**Hook publication amendment (November 2026).** A holder patch applies over the current committed document, including an admitted hook overlay (D90); the hold and patch authority remain connection-local.

**Status: taken.** Delivered in the second PR of the milestone that lets a producer read its panel
back and change one line of it, on top of
[D48](#d48--the-application-can-write-the-document-back-out-and-what-the-writer-promises-is-a-round-trip-not-the-producers-bytes)'s
writer.

**The verb that existed was the whole document or nothing.** A producer with a six-hundred-line panel
whose third line changed re-sent six hundred, and for the producer this terminal exists for — an
agent paying per token for what it says — that is the cost of the panel and not the cost of the
change. Nothing in the contract could say *this line*, because nothing in a document named a line.

### 1. The address is a field of the document, not of the channel

`key` goes on a line, so it reaches **both transports** and a document that has been through one is
readable by the other (**D17**). The alternative — an index into `lines` — was discarded before it
was written: a position is invalidated by every insertion above it, so a producer would have to read
the document back before every patch, and the verb that saves a round trip would cost one.

**It is deliberately not `ref`.** That field is painted, clipped to the width of its column and
legitimately repeated — `#1` twice in one panel is ordinary — so an address built on it would change
whenever the text did and would name two lines at once.

**It carries no bound of its own, and the bound it first had was justified by a false reason.** The
design gave it `MAX_KEY = 128` on the grounds that *every other field of this contract carries one*,
which is not true: `text`, `phase`, `updated_at`, `ref` and `link` are all unbounded strings. The
three that are bounded — `indent`, `send`, `actions` — are bounded against what **painting** them
costs, and a key is never painted. `MAX_BYTES` holds it as it holds every other string.

### 2. A repeated key keeps both lines and refuses whoever addresses it

The first answer was that the first line keeps the key and the rest lose it. The challenge showed
that contradicts the rule this whole verb rests on: **a patch must never silently mutate the
producer's document**, and stripping an address while reading one is exactly that. It also puts a
pass over every line inside `parse`, on the **file** path as well, in a crate that publishes an
allocation table nobody was going to re-measure.

So both lines keep it, and the op that names it comes back
`{"ok":false,"error":"the key \"mr-2\" names two lines of this panel"}` — told to the party that can
act on it, at the moment it matters, by the transport that can answer.

### 3. All or nothing, by value, and the precedent is D15

`Panel::patched_with` returns a **new** panel and the caller installs it or drops it. Three reasons,
and the third is not an analogy:

- the acceptance criterion demands it — *refused with the reason, and the panel does not change*
  cannot be true of a request whose other ops applied;
- the transport already draws the line between a **summary** tolerated per field and an
  **instruction** that has no partial reading;
- **D15** transforms the pane tree by value and replaces it only if the transformation reaches the
  end, so no path leaves a live console outside the tree. Same shape, same reason.

**And the ops within one request see each other's effects**, applied in order to that working copy.
Resolving every op against the document as it arrived makes two `after` ops on one key ambiguous
about their final order and makes *insert a line, then replace it* unexpressible. It costs nothing in
atomicity, because the copy is one nobody is painting.

### 4. `MAX_OPS = 64` bounds work in a frame, not bytes

The bytes are already held by the transport's own ceiling, so a second byte bound would be a
constant with no job. What this bounds is the walk: without it, an unbounded list of ops over a
document of tens of thousands of lines is unbounded work **in one frame**, per patch. The keys are
resolved in **one pass** — the index is built once and repaired as ops insert and delete — and the
ceiling is what makes that pass worth its constant.

**What that pass actually costs, corrected in review and written here rather than quietly fixed.**
The first version of this section claimed the result was `O(lines + ops)`. It is not: repairing the
index on an insert or a delete walks every position it holds, so a patch is
`O(lines + ops × keyed lines)`. At the document ceiling that lands in the same order of magnitude as
the figure `MAX_OPS` was chosen against, so the number still does its job — but the claim that was
published was false, and a false complexity is the same error as a false measurement.

**And what a patch costs in a frame is more than the walk**, which the same review found stated
nowhere: applying one clones the panel, builds the index over it and writes the result out to weigh
it against the ceiling. All three scale with the document and not with the patch, so a one-op patch
on a panel at the ceiling costs about what publishing that panel costs. That is the verb's price and
not a defect — what it saves is the bytes on the wire and the producer's own tokens, which is what it
was asked for — but it is the kind of thing that has to be written down before somebody measures it
and calls it a regression.

**Surplus ops are refused and never truncated**, which is the opposite of what `actions` does and
deliberately so: a truncated button is a button nobody sees, and a truncated instruction is an edit
that silently did not happen with the producer told `{"ok":true}`.

### 5. What an op says is not tolerated the way what an op carries is

A `line` an op carries **is a document line**, read by the same reader `parse` uses, so a `mark`
nobody knows is dropped and the line stands. A field named by a `set` **is part of the instruction**,
so `{"op":"set","field":"attention","value":"waiting"}` refuses the patch — where the same word
inside a `panel_set` document is dropped and the panel still paints.

**This is the sharpest point of the design and it is written into the published contract** rather than
discovered: a document is somebody's summary and losing it over a field nobody looks at would be the
worst thing to do with it; a request is an instruction, and one nobody can read has no sensible
reading at all. The document did not change; what can be said *about* it diverges.

### 6. `actions` is refused by name, as scope and not as security

The security framing was false and the challenge was right to say so: a holder already sets any
buttons it likes through `Action::new` with a `panel_set`, so a patch bypasses no gate and adds no
reach. What it would add is a second path into the one shape of this contract that reaches a shell,
for a convenience nobody asked for. The refusal says so in its own sentence, so a producer does not
read it as a typo.

### 7. What it costs in the frame, and what the run has to declare

A patch enters the frame, so **D13** applies: `patched` and `patches_refused` are counted, the report
names them on its `documents:` line beside the read's, and `largest_serialized` is fed by the ceiling
check the patch already pays rather than by a second write. Batch `U` is the only one that patches,
its control is `S`, and the script invalidates the run in **three** directions rather than the usual
two — the third being **no patch may be refused**. That third fits no pair, which is why
`crates/app/tests/batches.rs` guards it by name rather than through the symmetric loop (it is not the
script's only asymmetric guard: `ACTIONS=1` with nothing pressed is another). It is the one load this
bench can promise: the publisher holds the pane it patches and every op names a key it published
itself, so a refusal means the run measured the parse and the answer instead of the walk, the index
and the install — and left on screen exactly the panel a successful patch would have. **And the
numbers it reads are pulled out of the report by the words beside them**, so an empty capture is
invalidated rather than read as a zero, and a second guard holds those words against what the report
writes.

**Measured at the ceiling, 25 September 2026** (#248). Until then the ceiling had only been reached
through a refusal and the largest document the bench ever patched was 457 bytes, so «it scales with
the document» was a sentence with no number behind it. Two figures now stand, taken on the same
machine the same day, and they are two because the bench can only give one of them.

- **Per patch — the verb itself.** `measure_a_patch_at_the_ceiling`, an ignored release-mode test in
  `crates/app/src/control.rs`, times `PanelState::patch` with the ops `U` sends over the two
  documents the bench publishes. Three runs: **4.13–4.68 µs** on `U`'s panel (456 bytes written out)
  and **1.823–1.835 ms** on `Y`'s (249 462 bytes, 1 597 lines and 1 591 of them keyed) — about
  **four hundred times** more for about five hundred and fifty times the bytes, so the cost follows
  the document and not the patch, as the section above said. At 60 Hz that is **eleven per cent of
  a frame** for one patch, before the re-layout it forces, which is painting and is not in this
  figure. It does include dropping the panel the patch replaced, which in the window can happen
  later, on whoever held the last reference to it.
- **Per frame — what the bench can see.** Batches `X` (a document of 249 464 compact bytes published
  over the channel once a second) and `Y` (`X`, then `U`'s ops every turn), two runs of each, which
  `compare` says can be averaged. The channel's span goes from **4.51–5.62 µs** in `X` to
  **25.0–35.6 µs** in `Y`, and the panel's from **63–98 µs** to **101–124 µs**; the second is where the
  whole-panel re-layout a patch forces lands, and the ranges do not overlap. Against eight lines the
  same verb cost +347–385 ns in the channel's span (`U − S`), so the ceiling moves it by fifty to
  ninety times per frame too.

**Why the per-frame figure cannot be turned into a per-patch one**, which is what the design of #248
first planned: the channel's series is recorded **only in frames that carried console output**, and a
publication or a patch can land in a frame that carried none — `Y` ran about 510 frames for 358 with
output. What those frames spent is subtracted from the rest of the frame and averaged nowhere, so
`(mean × frames) ÷ patches` undercounts by an amount the report does not state. **And the two
batches did not publish the same number of times**: `X`'s runs applied 28 and 44 documents, `Y`'s 56
each, so its per-frame means carry more publications as well as the patches. Both are why the per
patch figure comes from the release-mode test and the bench is quoted per frame only — the same split
**D49** made for the read.

**Two things the large pair needs that nothing else in the bench did.** The document travels to the
publisher on standard input, because Linux refuses a single argument longer than `MAX_ARG_STRLEN` —
128 KiB with 4 KiB pages — and every channel batch takes that path so the pair and its neighbours load
their document the same way. And `X` feeds no written-out size of its own — only reads and patches
do — so its size is checked before anything is published, on a lower bound of the compact form, and
the suite holds the batch table so that `X` is `Y` minus the patch and nobody else carries the filler.

**What was discarded, and why.**

| Discarded | Why |
|---|---|
| Addressing a line by its index in `lines` | Every insertion above it invalidates it, so a producer would have to read the document back before each patch — the round trip the verb exists to save |
| Addressing by `ref` | Painted, clipped and legitimately repeated: an address that changes when the text does |
| A repeated key keeping only the first line | It rewrites somebody else's document while reading it, and puts a pass over every line inside `parse` on the file path too |
| A bound on `key` | Its stated reason was false — the unbounded fields of this contract outnumber the bounded ones, and the bounded ones are bounded against painting |
| Applying the ops that work and reporting the rest | The acceptance criterion, the instruction-versus-summary line, and **D15**'s precedent all say the same thing |
| Every op resolved against the document as it arrived | Two `after` ops on one key become ambiguous, and *insert then replace* becomes unexpressible |
| Truncating at `MAX_OPS` | An edit that silently did not happen, with `{"ok":true}` on the wire |
| A `source` on the patch request | The hold already names its holder and a patch cannot exist without a hold: it would reach nothing, or rename a holder mid-hold |
| A second error type for a refused op | `serialize` answers in `Problem` and the ceiling is `Problem::TooBig`; two vocabularies for one verb makes every caller translate |
| A field beside its value — `Set { field, value }` | Two parallel enums the parser has to keep in step and no type can: a `header` paired with a phase string was constructible, and the applying code, obliged to answer for a pair the parser cannot produce, absorbed it into «as if unsaid» and cleared a field nobody had named. One enum carrying its own value leaves four total arms and no impossible pair. **Caught by two reviewers blinded to each other and to this document** |
| Accepting an empty `ops` list | Applying none of no ops succeeds by definition: the pane would answer `{"ok":true}`, count a patch and stamp the date, so a producer looping on empty patches keeps a panel looking fresh without ever publishing — the exact silence the stale notice exists to break |
| Wrapping a refusal in `Problem::Invalid`'s «not understood» | The patch was understood exactly and refused, and the contract publishes the sentence the producer reads |
| Serializing the patched document a second time to measure it | The ceiling check already wrote it out, and paying it twice would publish this path as costing about double |
| Making `panel_patch` take no hold, like `panel_get` | A read exists for the producer that has *not* claimed the pane; a write by a second producer is the collision the hold exists to stop |
| Giving the patching batch a document of its own | Its control would publish different bytes, which is not a control. The key goes on all three channel batches and on no file-seeded one — **which does leave `S` publishing thirteen bytes its own control `C` does not**, said in the batch table and in the script rather than left to be found: a key is never painted and never read by the application, and putting it on every batch would move all eighteen measured runs instead of three |
| Setting `phase` as the patching batch's scalar op | **Caught in review, and it is D13 exactly.** `phase` is painted on the identity row, so the batch would have drawn «· patched ·» where its control draws nothing — ten cells laid out every frame, and a row that wraps differently on a narrow panel — and the difference published as what a patch costs. The expiry is read and never drawn, and the op sets it to what the document already declares |
| Letting an insert carry a key the document already has | It makes that key name two lines, so every later op addressing it is refused for the life of the hold and the only way out is republishing the whole document — the cost this verb exists to remove. Reached by an ordinary copy-paste, and by retrying a patch whose answer timed out |
| One sentence for «nothing published» and «the file rules this panel» | **The design's premise was refuted by its own sibling verb.** It argued the two cannot be told apart from outside, and a read of the same pane tells them apart, answering *nothing has been published on this console yet*. A producer that had just read its document back would have been told, one request later, that the document was the file's |
| Leaving the ops unbounded on the wire | `MAX_REQUEST` is the document's ceiling plus an envelope sized for fixed keys and a name. A patch rewriting sixty keyed lines of a near-ceiling document exceeds it — and that is `Unrecoverable`, so the connection ends and the hold with it, and the panel falls back to its file. The bound is on the encoded bytes, the same shape `source` already has, and the refusal leaves the connection alive |
| Deriving the per-patch cost from `Y − X`'s per-frame means | The channel's series only counts frames with console output, and a patch can land in one without; the undercount is not in the report. The per-patch figure is timed on the verb itself, in release, with the bench's own documents and ops |
| Measuring the large patch against `S` | `S` publishes seven lines, so `Y − S` would be the large document and not the patch. `X` publishes the same document as `Y` and nothing else changes between them, which the suite holds on the batch table |
| One long line to reach the ceiling | It measures the painter's layout — 65.8 ms the first time for a line that size — instead of the patch. The large document is 1 590 lines of a working panel's length, every one keyed so the index a patch builds is at its widest |
| Passing the large document as an argument | Linux refuses it with `E2BIG`: one argument may not exceed `MAX_ARG_STRLEN`, and the document is about twice that |

---

## D48 — The application can write the document back out, and what the writer promises is a round trip, not the producer's bytes

**Status: taken.** Delivered in the first PR of the milestone that lets a producer read its panel
back and change one line of it.

**[D17](#d17--the-contract-is-the-document-not-the-file-the-source-of-truth-is-the-apps-state) said
the source of truth is the state the application holds per pane, and until now nothing had to test
that literally.** The application read documents and never wrote one; while the only verb was
`panel_set` it could, in principle, have kept the bytes as they arrived and handed them back. It
cannot any more: the milestone this belongs to goes on to change a document a line at a time, and a
patched document has no bytes of its own. So the writer is the piece of real work, and the whole
decision is what it is allowed to promise.

### 1. It cannot promise the producer's bytes, and pretending otherwise would be the mistake

The parse is lossy **on purpose** — that is the contract's first rule and it is the right one: a
document is somebody's summary, so an unknown field is dropped, an indentation past 64 is clipped, a
list of buttons past eight is cut and a word none of the vocabularies knows falls away, rather than
the whole panel being lost over a field nobody looks at. A writer that claimed fidelity to what
arrived would therefore be claiming something the reader already threw away.

**What it promises instead is that the round trip is closed:**

```text
parse(serialize(p)) == p          for every p the parser can produce
serialize(parse(serialize(p))) == serialize(p)
```

which is the property a producer actually leans on — read it, edit it, publish it, and the panel is
what the document says.

### 2. It never grows a document, and that is a promise about bytes rather than values

**This came out of the review of the design and it had already broken an acceptance criterion on
paper.** The first shape wrote `"text"` on every line. A line published as `{"indent":1}` — twelve
bytes — came back as `{"text":"","indent":1}`, twenty-two; a document of some twenty thousand such
lines is legal going in and **over the 256 KiB ceiling coming out**. A panel painting perfectly would
have answered *over 256 KiB* to a producer asking what was on it, for a document the contract's own
parser had produced.

The rule that fixes it is one line and it is the writer's whole shape: **emit no field the document
did not carry**, empty `text` included, defaults included. Every other difference between what
arrived and what goes out shrinks it — whitespace gone, escapes resolved, unknown fields dropped — so

```text
len(serialize(p)) ≤ len(the bytes p was parsed from)
```

and the tests that hold it are written over **compact** sources on purpose: a pretty-printed document
shrinks by so much that the assertion would pass however much the writer added.

**The price, named here so it is not read later as a bug**: a producer that published an explicit
`"header": true` reads back a document without it. After the parse there is no difference between
*the producer did not say* and *the producer said the default*, so writing one would assert something
the application does not know.

### 3. One line, and no newline anywhere

The reply that carries a document is one line on a newline-framed channel, and it is **the only reply
that is interpolated raw instead of being escaped** — because escaping a document would roughly
double a large one and make the channel refuse an answer the file transport would have accepted,
which is the two-transport drift **D17** forbids, seen from the reply side. What keeps that safe is
not the reply: it is this writer, whose every string goes through the same escaping and whose
structure carries no newline of its own. There is a test on the emitted bytes for exactly that, and
it is the one property the round trip cannot see — a raw newline between two tokens re-parses
perfectly.

### Discarded

| Discarded | Why |
|---|---|
| Keeping the producer's original bytes and handing those back | There are none once a document has been patched, which is the whole reason this exists |
| A `Serialize` derive on the document's types | A dependency feature to buy the half the channel's reply already buys by hand, in the one crate that exists in order to survive the interface being rewritten, and whose manifest carries a written reason per dependency |
| Writing through a generic JSON value | The clipping, the truncation and the per-field tolerance do not survive a round trip through a value that knows none of them, and the drift would be per field — which is exactly the failure the one-parser rule exists to prevent |
| Emitting the defaults, so a reader need not know them | It asserts something the application does not know: after the parse, *absent* and *the default* are the same document |
| Emitting `written_at` | It is the file's own date, held by the application, and the parser does not read it back: it would invent a contract field |
| Dividing the expiry by sixty | The parse saturates at `u64::MAX` seconds, and a plain division comes back fifteen seconds short of it and does not saturate again. Rounding up overflows into the saturation and lands where it started, and is exact everywhere else |
| Asserting on the emitted text rather than on the re-parsed panel | **D30**: a writer that emitted the expiry in seconds would satisfy a text assertion and still hand back a different document. The two exceptions are the two properties a round trip cannot reach — the newline and the growth |

---

## D49 — Reading a panel does not depend on the window, and reads only what it has committed

**Status: taken.** Delivered after the read-and-patch milestone exposed that its recovery verb stopped
working precisely when a producer needed it most.

**`panel_get` used to be put in the same bounded inbox as a write and answered only when an interface
pass drained it.** A minimised or occluded window may have no such pass, so the request waited for
five seconds and timed out while the document it asked for was already in memory. Re-attaching after
a producer restarted is the verb's stated use, and a window nobody is looking at is an ordinary time
to do it; visibility cannot be its availability boundary.

### 1. The connection reads one immutable committed document

`PanelState` remains the sole owner of publication, patching, file refresh and painting. Beside that
state it publishes a shared pointer to the same immutable `Panel`, replaced only by the transitions
that successfully replace `last_good`. A connection upgrades a weak pane capability, clones that
pointer under a short mutex, releases the mutex, then serialises and answers on its own thread.

The clone is the read's consistency point. A concurrent commit may land wholly before it or wholly
after it; no reader can observe half of each document. The size-dependent work is outside the frame,
and the interface thread pays only a pointer replacement inside the publication work it already
declares.

**Queued is not committed.** A `panel_set` or `panel_patch` accepted by the bounded window queue is
still invisible to a read until the interface thread applies it. This keeps the existing order: a
document that has not been accepted and cannot yet be painted is not reported as the pane's document.
Failures, release and unchanged file refreshes replace neither representation.

### 2. Closing admits a read at the weak capability, and never waits for serialisation

Connection threads are deliberately not joined when their pane closes. A read that upgrades the
pane's weak capability first may finish successfully after closure: the upgrade admits that read
while the pane still exists, but it does **not** choose a document version. The later pointer clone
under the mutex is the separate consistency point that selects one complete committed document. A
read that starts after the strong capability is gone gets the existing `this pane is gone` refusal,
and that refusal is counted like every other refused read. Closing never waits for a ceiling-sized
answer to be written.

Read totals and the largest serialised size therefore belong to one concurrent accumulator created
with the window and shared by every pane: an accepted read may finish after its pane leaves, and no
closed-pane tracker needs to be retained. The final report closes admission and waits only for the
callbacks already admitted before it reads that accumulator; it does not wait for their socket
replies. Ordinary pane close remains non-blocking. The remaining per-pane publication and patch
counts keep their close banking. This preserves the existing `documents:` vocabulary while making
its counts survive the lifetime the decision permits.

### 3. What is measured, and what is not

Batch `V` remains the bidirectional execution guard: it requires reads and a non-zero serialised size
in `V`, and forbids reads in every other batch. It no longer claims that the frame delta
against `S` is serialisation latency, because serialisation and socket output happen on a connection
thread outside every frame span. The frame report covers only the pointer clone or replacement that
actually occurs in frame-owned work.

The off-frame callback has measurements of its own. These numbers include weak-capability upgrade,
pointer clone, serialisation and metric locking; they exclude listener dispatch, reply encoding and
socket output. Measured on the release binary on 19 September 2026,
on an AMD Ryzen AI 9 365 (10 cores, 20 threads), five runs each: the ordinary and ceiling cases run
20,000 and 1,000 sequential reads respectively; the concurrent case runs 250 reads on each of eight
threads. The range is across the five whole runs, not samples selected from inside one run.

| Case | Evidence |
|---|---|
| Ordinary committed document, one reader | 47 bytes: **740 ns–1.018 µs per callback**. |
| Document at the 256 KiB ceiling, one reader | 260,110 bytes: **375.612–382.219 µs per callback**. |
| Document at the ceiling, eight concurrent readers | Per-thread mean **348.261–361.043 µs per callback**; wall time **98.411–108.150 ms** for 2,000 completed callbacks. |

### Discarded

| Discarded | Why |
|---|---|
| Serialising eagerly on every successful commit | It puts size-dependent work into file refresh and `panel_set` even when nobody reads; `panel_patch` already serialises for its own ceiling but the other paths do not |
| Draining a second read lane from `App::logic` | It creates another queue and ordering path, walks panes while hidden, and still makes availability depend on what a platform schedules for an occluded window |
| Putting all panel state behind a lock or actor | Painting, presses, file refresh and writes already have one interface owner; moving them all changes the ownership model to make one read concurrent |
| Letting a read see a queued write | A queued request may still be refused and is not what the pane shows; exposing it would make `panel_get` stop meaning the committed document |
| Making close wait for in-flight reads | It puts up to one ceiling-sized serialisation and reply on the interface close path, despite the reader already having selected an immutable value |
| Keeping a strong pane capability in each connection | A forgotten connection would extend the pane's lifetime; a weak capability says exactly which side wins when the read has not started |
| Treating batch `V` minus `S` as read latency | The expensive work is deliberately off-frame, so subtraction would attach a connection thread's work to a frame number that never measured it |

## D50 — An intentional session retirement clears what its own connection controls; a disconnect still preserves it

**Hook publication amendment (November 2026).** Retirement also revokes the shared hook hold generation. Queued hook publications cannot resurrect a retired panel (D90).

**Status: taken.** Delivered after the panel's bidirectional channel exposed the difference between
an agent disappearing and the same agent deliberately ending its session.

**D43 deliberately leaves the last document on screen when a connection closes, and D44 deliberately
leaves the consoles it opened alive.** Both are right for a crash: destroying the only evidence of
what happened, or closing visible consoles because their agent died, would turn failure into data
loss. They are wrong for `/clear`, which is an explicit statement that this session and its work are
over. A transport close cannot carry that distinction, so retirement is a separate request.

### The decision

**1. `panel_retire` belongs to the connection that already holds the panel.**

The request names no pane, source, document or console. Its endpoint addresses the pane and its live
connection is the authority. A fresh connection is refused even if it presents the same source or
inherits the same `PANEL_ID`: those are names, not credentials. `/compact` sends no retirement — it
is the same session, and `panel_get` remains its recovery path.

**2. One logical transition removes the session's visible state and only the consoles it still
controls.**

The document, actions, attention and hold disappear together. Auxiliary consoles still owned by the
retiring `(origin pane, connection)` leave the tree; a console the person took over stays, and every
handle issued to the retired connection is forgotten. The reply is sent after that logical commit
and before the detached pseudoterminals are destroyed, so D32's close delay cannot turn completed
retirement into a five-second channel timeout. Destruction proceeds afterwards through ordinary pane
and PTY destruction. Its transition and reaping costs are named separately in reports; **this decision records
no measured figure for either**.

The retired connection cannot open another auxiliary console until `panel_set` begins its next
generation. Otherwise an idempotent retry would truthfully return the old counts while leaving new
work from the supposedly retired session alive.

Retirement is also a barrier in that connection's request order: a later `panel_set` remains queued
until the window has committed or refused the retirement. A timeout changes who is still waiting for
the answer, not the order in which accepted requests take effect.

**3. The observed file snapshot retired with the session stays suppressed without deleting
`panel.json`.**

The file cannot identify its producer, so the holder has no authority to delete it: it may contain a
publication from somebody else that D43 refused while the connection held the pane. The terminal
keeps a bounded snapshot of the file's metadata and bytes while that pane lives. That unchanged
snapshot cannot resurrect the retired panel; a later valid file whose observed snapshot differs is
read normally even if its timestamp was preserved. Pane closure already deletes its private directory
and a restored pane receives a fresh identity, so there is no retirement marker to persist across an
application restart. The marker is application state, never producer content.

**4. Only an explicit harness event may use this operation.**

A crash, EOF or accidental disconnect still follows D43 and D44: release the hold, preserve the
document and preserve the consoles. This repository supplies the connection-owned operation, not a
harness adapter. Automatic `/clear` support is claimed for a harness only after a real run proves its
lifecycle event invokes this tool on the existing connection; a command hook, fresh connection,
empty-document sentinel or terminal parsing does not pretend to be that route. On Windows the
control endpoint is still absent until F2.7 proves the named pipe's permissions and cancellation
path, so this operation is currently POSIX-only.

### What was discarded, and why

| Discarded | Why |
|---|---|
| Treating an empty panel as retirement | Empty content is valid producer data; interpreting it as lifecycle would violate D6 and make data a hidden command |
| Letting any new pane connection retire the holder | `PANEL_ID`, `PANEL_DIR`, process ancestry and same-user files are names or same-user data; none proves ownership of D43's live connection |
| A bearer token for command hooks | It creates a second authority system and makes console destruction available outside the connection that opened them |
| Deleting `panel.json` | The file's producer is unknowable and may not be the retiring connection; exact-generation suppression prevents resurrection without destroying somebody else's publication |
| Reusing ordinary connection close | A crash and an intentional clear would become indistinguishable, throwing away the evidence D43 exists to preserve |
| Closing every auxiliary console | D44 gives the console back on the person's first input; retirement cannot take it away again |

## D51 — A link belongs to the pixels that named it, and only web addresses reach the desktop

**Status: taken.** The console and the status panel already paint addresses, but neither has owned
the act of opening one. Adding that act makes three old boundaries meet: the mouse already belongs
to selection, the hosted program or pane movement; panel documents and terminal output are
untrusted; and the pane's shell is an execution boundary rather than a desktop service.

### The decision

**1. The console opens with `Ctrl` plus the primary button; the panel opens with the primary button
on its own link label.**

A bare console press keeps selecting text or reporting the mouse to the hosted program, and
`Ctrl+Shift` keeps moving panes. The panel has no competing text-selection contract, so its explicit
link label needs no modifier. Both effects cross one once-per-frame barrier before selection or
mouse reporting: a second egui pass cannot turn one physical press into two browser requests.

**Amended on 26 September 2026** ([#290](https://github.com/mashware/ivy-history-private/issues/290)):
that sentence states what the barrier guarantees, and it stays true — but egui 0.36 does not hand a
repeated pass the frame's events at all (egui 0.36.1, `data/input/raw_input.rs:137`), so today the barrier is
**defence** against an egui that starts replaying. It is kept for the reason D31's and D46's
amendments give: one frame compare, against one click opening two tabs after an upgrade with only
the tripwire test `egui_hands_a_frame_s_events_to_its_first_pass_only` to warn. Removing it was
discarded.

The console recognizes two sources from the exact `View` and rectangle used for its last paint. OSC
8 is semantic and wins when its sparse span covers the cell. Otherwise the terminal core recognizes
the plain `http://` or `https://` range containing the cell, across engine-marked wrapped rows and
style changes. The lexical boundary is a contract rather than an accident of a parser: whitespace,
controls and quote/angle delimiters end the raw range; ordinary sentence punctuation is left out;
and a closing round, square or curly bracket is left out only while it is unmatched inside the
candidate. A range that leaves the visible grid is not active.

**2. The target comes from the last painting, never from a producer reread at click time.**

The application pumps program output and can commit a panel document before it routes the mouse.
Looking up the current cell or current line from geometry painted in the preceding pass could
therefore open a value the person never saw. Each pane retains the exact engine-independent `View`
and console rectangle its painter just used, without cloning them. Wrapped-row facts and sparse OSC
8 spans travel beside its unchanged `ViewCell`s; plain URL parsing still runs only for the clicked
logical line. A settled panel link carries its exact target in its own painted region. The later
press never indexes the new terminal grid or `last_good` through stale geometry, and the first
`Ctrl+click` works without requiring a prior hover frame.

The panel follows the existing prototype: it paints a deterministic compact label beside the line,
or below it when there is no room, and exposes the exact destination on hover. OSC 8 exposes its
exact target under Ctrl-hover. A hidden, clipped or unsettled region is inert. A full duplicate URL
is not required to fit in the pane: doing so would make a valid labelled link disappear precisely
where the panel is narrowest.

**3. One application boundary accepts only complete absolute `http` and `https` URLs with a host.**

Plain text, OSC 8 and `line.link` all pass through the same `WebTarget` value. Relative references,
missing hosts, controls, whitespace, invisible formatting characters, malformed values and every other
scheme are inert: nothing reachable is painted for them, so no hand and no tooltip either. **What is
kept is the parser's form of the address, not the producer's spelling**, because the desktop's opener
parses it again and loads that form — host lowercased, an international host in punycode — and the
tooltip has to show what will load. The characters a command interpreter reads — `|`, `"`, `<`, `>`
and a `%` that is not an escape — are percent-encoded in that form, because under WSL the opener
passes the address to `cmd.exe /c start` and escapes only `^` and `&`; for a browser the address is
the same one. Two gaps under WSL are known and left: two valid escapes can still spell a `%NAME%`
that `cmd.exe` expands, and when `cmd.exe` cannot start the opener falls back to
`powershell.exe Start`, which reads `;`, `$` and more of its own syntax. That one string goes to the desktop's own opener (the `webbrowser` crate, the one
egui's `links` feature would call), on a thread of its own, since on Linux it asks `xdg-settings`
on every call and waits for a text browser to exit. This application never concatenates it into a
command, sends it to the PTY, interprets it as a template or fetches it. Redirects belong to the
browser.

**A click the opener could not act on says so, on standard error**: when the opener reports that it
found nothing to start or could not start it, one line names the address and its error — the channel
**D38** already uses for a console that stops receiving. That is why the opener is called directly
rather than through egui's `links` feature: that feature reports a failure only through `log`, and
this binary installs no logger, so a person with no configured browser clicked and nothing happened,
with no trace anywhere. **What it cannot say** is a browser that started and then failed — a locked
profile, a launcher that exits with an error: the opener returns as soon as the process starts, and
past that nothing reaches this application. Nothing is painted over the console
for it: the person asked for the answer on the terminal they launched from, not for another notice
competing with D41's for half the console.

**4. Link work is declared inside the measurements that already own it.**

Plain URL recognition runs only on hover or click, not while rebuilding every ordinary visible
snapshot. Building a `View` does record one wrap fact per row and only the OSC 8 spans that exist,
without enlarging `ViewCell`; retaining it replaces its old drop rather than cloning it. Panel link
layout stays in the panel's cached layout and visible-row walk. Reports count the sources the bench found and
the activations per source; the work stays inside the existing console, panel and mouse spans, and
in particular a panel label's reachability is part of the panel's painting, not of the buttons' walk
the report subtracts from it. Batch `W` exercises all three sources with desktop launch suppressed;
both of its consoles carry the same link rows and panel label, and only the one its injector clicks
takes the gesture. The links sit on the top two rows, outside the scroll region the load runs in,
so every gesture lands on a link and the guard can demand an opened link per gesture. Finding where
to click is the bench's own work: it walks a console's top rows inside that console's span, and only
in `W`, because outside every span it would be published as the framework's cost.

### What was discarded, and why

| Discarded | Why |
|---|---|
| A bare console click | It would steal the gesture D20 gives to selection and D23 can give to the hosted program |
| Semantic links only | Ordinary shell and agent output prints plain URLs; excluding them leaves the reported failure in place |
| Copying a URI into every `ViewCell` or indexing every plain URL | Almost every cell has neither; sparse OSC spans and click-time plain parsing keep the hot cell type unchanged |
| Resolving the live terminal cell or current panel document on click | Output and documents can change after the pixels under the pointer were painted, retargeting old geometry |
| Requiring a Ctrl-hover frame before Ctrl-click | Keydown and click can arrive in one egui input batch; the approved first gesture must not be inert |
| Painting every complete panel URL in full | Long targets consume scarce pane rows and can make the link inert in the narrow layout where the compact label is needed most |
| Reusing panel shell actions or adding an `open` action | A URL and executable shell text have different payloads, gestures and trust boundaries; `line.link` already names this contract |
| Passing the target to `xdg-open`, `open`, `start` or a pane shell | Command construction reintroduces shell interpretation and platform branching that the opener crate already owns |
| Opening through egui's `links` feature | It calls the same opener and swallows its error into `log`, which this binary never installs: a failed click would say nothing anywhere |
| A notice over the console when opening fails | Another notice competing with D41's for half the console, for a failure that belongs to the desktop's configuration; the person chose standard error |
| Keeping the producer's spelling of the address | The opener re-parses it, so the tooltip would show one address while the browser loads another — the homograph case is exactly that |
| An opener worker, queue or retry | A human-rate command needs none of their lifecycle; what it does need is not to run on the interface thread, and a thread per click, detached, is all of that |
| Supporting `file:`, `mailto:` or producer-chosen schemes | The feature is web navigation; widening the scheme set widens the untrusted capability without a present requirement |
| Dedicated nested timing spans from the first version | The existing panel and mouse spans already own the work; counters and batch `W` declare it without risking double subtraction |

## D52 — Home, End, Insert, Delete, Page Up, Page Down and the arrows go to the program in `xterm`'s form, modifiers included

**Status: taken.** The keyboard milestone (F1.3) translated Enter, Tab, Backspace, Escape and the
four arrows, and every other named key went silent — held that way by a test. Six of them are on
every keyboard and used daily in a shell, an editor and a pager, and pressing them did nothing with
nothing on screen saying why.

### The decision

**1. The sequences are the ones the advertised `TERM` describes.** The terminal launches its
programs with `TERM=xterm-256color`, so what a curses program expects is what that terminfo entry
says, and `xterm`, Alacritty and WezTerm send the same: Insert `CSI 2~` (`kich1`), Delete `CSI 3~`
(`kdch1`), Page Up `CSI 5~` (`kpp`), Page Down `CSI 6~` (`knp`), Home `CSI H` and End `CSI F`.

**2. Home and End follow application cursor keys (DECCKM); the four tilde keys do not.** A program
that sends `CSI ?1h` — curses does it through `smkx` — is told by terminfo that Home is `SS3 H` and
End `SS3 F` (`khome`, `kend`), so while the screen holds that mode they go out in that form. The
mode is read from the screen once per keyboard pass, like bracketed paste, because only the program's
own output can change it and that is interpreted outside the pass.

**3. A modifier travels as `xterm`'s parameter, and this is a choice to widen, not something the
ticket forced.** `1 + Shift + 2·Alt + 4·Ctrl`: `Ctrl+Delete` is `CSI 3;5~`, `Ctrl+Home` is
`CSI 1;5H`, and with a parameter Home and End keep the `CSI` form in either mode, because `SS3` has
nowhere to carry it. The free option was on the table — the code already sends the plain key when
Ctrl accompanies something that is not a letter, and silence for Alt — and it was turned down for
the reason `key_bytes` already gives about Alt: for these keys the modifier changes the sequence, so
the plain one is a different key (`Ctrl+Delete` would delete one character instead of a word). The
macOS command key still silences them.

**What that left inconsistent, and how it was closed.** As first taken, `Ctrl+Home` carried its
modifier and `Ctrl+Left` did not — the arrows kept sending the plain sequence with Ctrl and kept
ignoring DECCKM, so word-jump did not work in a shell and a curses program that called `keypad()`
was told by terminfo that Up is `SS3 A` (`kcuu1`) while it received `CSI A`. It was left out on
purpose, because the arrows also feed the wheel-as-arrows path of the alternate screen; point 5
closes it (#261).

**4. The scrollback shortcuts keep the keys first.** The keymap is consulted before translation, so
`Shift+PageUp`, `Shift+PageDown`, `Shift+Home` and `Shift+End` still move the scrollback and send
nothing. What changed is what happens when one of them is given back with `none`: it now reaches the
program as `CSI 5;2~` and so on, where before it was silent. The same goes for chords nobody binds —
outside Windows, `Shift+Insert` and `Shift+Delete` now send `CSI 2;2~` and `CSI 3;2~`, where
they did nothing.

**And a bare one is typing now, with typing's price.** A console scrolled back into its history
that receives any of the six keys bare sends it to the program, and — like every key that reaches
the program here — brings the view back to where the program writes. Before, the key did nothing
and the view stayed where it was. This is this terminal's rule for typing, not a precedent borrowed
from `xterm`, whose own snap-on-key (`scrollKey`) is off by default; `Shift` is still how to move
through the history.

**What Windows does before any of this, for F2.7 to decide.** The desktop layer (the vendored
`egui-winit`, **D33**) turns chords over two of these keys into clipboard requests on Windows only,
before they are keys, looking at one modifier and ignoring the rest: **any** chord with Ctrl over
Insert is a copy, **any** with Shift over Delete is a cut, and **any** with Shift over Insert is a
paste — so `Ctrl+Shift+Delete`, `Ctrl+Alt+Insert` or `Ctrl+Shift+Insert` are caught as well, and
the last one copies the selection. They arrive without their modifiers, the same wall the annex of
**D26** describes for `Ctrl+Shift+C`, and what reaches the program is what `bytes` does with a bare
request: a copy is `0x03`, an interrupt, and a cut is `0x18` — `nano` exits on it. The one that
still becomes a key is the paste over a clipboard with no text, because D33's patch lets it fall
through, and that one does go out as `CSI 2;2~`. None of this is new with this decision — it held
while these keys had no bytes of their own — and none of it can be seen from Linux, where the suite
runs. **Decided in D115**: Shift+Delete is a key on Windows too, Ctrl+Insert copies and Shift+Insert
pastes only when they are exactly those chords, and every other chord over the two keys takes the
form above.

**5. The four arrows follow the same two rules, and the wheel sends them in the program's mode.**
Bare, an arrow is `CSI A..D` normally and `SS3 A..D` under DECCKM (`kcuu1`…`kcub1`); with any
modifier it is `CSI 1;m A..D` in either mode — `Ctrl+Left` is `CSI 1;5D` — the same parameter and
the same table as Home and End, so Alt stops silencing them too. The wheel on the alternate screen,
which sends arrows to a program that did not ask for the mouse (**D23**), takes its bytes from that
same translation, bare and in the mode the screen holds: `less` and `vim` switch DECCKM on as they
start, so a wheel with a table of its own would send them the other shape on every notch. WezTerm
does the same — its alternate scroll presses the arrow through its own key encoder. Alacritty does
not: it always sends `SS3 A` / `SS3 B`, which in normal mode is a sequence the advertised terminfo
never promised. The terminal's own chords are untouched — the keymap is consulted before
translation, so `Ctrl+Shift+←/→` still switches tabs — and a chord given back with `none`, or
pressed with a single tab, now reaches the program with its modifiers (`CSI 1;6D`) where before it
arrived as the bare arrow.

**What a person sees change in a shell.** A modified arrow the shell has no binding for is no longer
a plain cursor move: the `/etc/inputrc` Debian and Ubuntu ship binds `CSI 1;5C/D` to word motion, but not
`Shift+Up` (`CSI 1;2A`) or `Alt+Left` (`CSI 1;3D`), and readline answers an unbound sequence by
inserting its tail (`;2A`) into the line. It is what the same key does in `xterm`, WezTerm and
Alacritty, which send the same bytes, and the remedy is the shell's binding, not a plainer key.

### What was discarded, and why

| Discarded | Why |
|---|---|
| `CSI 1~` / `CSI 4~` for Home and End (the VT220 «Find» and «Select» keys, what rxvt sends) | Not what `xterm-256color` describes, which is the `TERM` this terminal advertises |
| Dropping the modifier and sending the plain key, as the arrows did until point 5 | A different key: `Ctrl+Delete` deleting one character, `Shift+PageUp` given back and arriving as a bare `PageUp` |
| Silence under a modifier, as Alt gets for every other key | Six keys dead again as soon as a modifier is held, which is the defect this decision exists to remove |
| A complete extended keyboard protocol (kitty's, `modifyOtherKeys`) | A protocol a program has to ask for, covering every key; the parameter form above is part of the plain PC-style function-key encoding and needs no request |
| Moving the arrows to DECCKM and to the modifier parameter in the same change | The arrows are also what the wheel sends on the alternate screen; changing them is a change of its own — made in point 5 |
| The wheel keeping the plain `CSI A` / `CSI B` whatever the mode | A second table that contradicts the keyboard's, in the one mode the wheel's programs almost always hold |
| The wheel carrying the chord held while turning it | The chord on the wheel already means something else here (`Shift` is the scrollback, **D23**) |
| Reading the mode per key event | It cannot change inside a pass; one read, where bracketed paste is already read, is the established shape |

## D53 — The stretch from the desktop's key to the framework's event is verified by hand, on every platform the change reaches

**Decided.** Nothing in this repository tests what happens between the operating system's key event
and the `egui::Event` the terminal receives, and that stays so on purpose. A change that can alter
that stretch is verified **by hand**, on **each platform it can reach**, by pressing the list below
and reading what arrives in the program. The pull request says, under «How to test it», which
platforms it was pressed on and what came out; a platform nobody could press on is named as
unverified, never left out.

**What counts as touching that stretch.** Any change to the copy in `vendor/egui-winit` (**D33**); a
version change of `egui`, `eframe`, `egui-winit` or `winit`, the patch section of the workspace
manifest included; and a change to the keyboard translation that rests on what the desktop does or
does not deliver — which chord arrives as a key, which one arrives as a clipboard request, which
one arrives at all. A change to the bytes a key produces once it has arrived as an event is **not**
this: that half is the terminal's own translation and is tested in the ordinary way, the asymmetry
of the two paste chords being the example (**D33**).

**Why it is not tested, said once so nobody has to find it again.** Two walls of `winit`'s own, and
either one is enough:

1. `KeyEvent` carries a `pub(crate)` field and has no constructor, no `Default` and no builder, so
   no crate but `winit` can build one. Being inside the vendored `egui-winit` does not help: the
   privacy is `winit`'s.
2. `on_window_event` demands a real `Window`, which `winit` creates only from a running event loop
   bound to a compositor, and `on_keyboard_input` — where the branch that failed in **D33** lives —
   is private and reachable only through that method.

`egui_winit::State::new` is **not** a wall — it takes a display handle, and a dummy one can be
built — which is exactly the trap: getting a `State` looks like progress and both walls stand
behind it. So every keyboard test in `crates/app` builds `egui::Event` by hand, and its green says
nothing about delivery. That is the third disguise of **D30** in the variant a library imposes:
**#203** lived below that line for **26 days**, with `Ctrl+V` promised by **D11**, tests all around
it, and the key reaching the program as nothing at all.

**The list.** Each chord, the situation it is pressed in, and what the program has to receive. The
observer is a program that prints the bytes it gets: `cat -v` on Linux. **D30** already records
that `stty` and `cat -v` are a Linux assumption the other two platforms do not share in that shape,
so F2.7 names the observer it uses on each of them.

| Chord | Situation | The program receives |
|---|---|---|
| `Ctrl+V` | text on the clipboard | `0x16` (`cat -v`: the first press prints `^`, the second completes `^V` — the line discipline's literal-next, **D33**) |
| `Ctrl+V` | an **image** on the clipboard, and then an empty one | the same `0x16` |
| `Ctrl+Shift+V` | text on the clipboard | the text, pasted, and no `0x16` |
| `Ctrl+Shift+V` | an image on the clipboard | nothing at all (**D33**: the terminal's own shortcut never reaches the program) |
| `Ctrl+Shift+C` | text marked | nothing; the marked text is on the clipboard (**D22**, **D26**) |
| a letter, `Enter`, `Ctrl+C` | a shell prompt | the letter, `\r`, `0x03` |
| `Ctrl+Left`, `Ctrl+Delete`, `Shift+Insert` | `cat -v` | `CSI 1;5D`, `CSI 3;5~`, `CSI 2;2~` (**D52**) |
| `Ctrl+Shift+X`, pressed once and then held | three consoles | nothing reaches any program; each press closes the focused console, and holding closes only one (**D33**'s second change) |
| `Ctrl+X` | `cat -v` | `0x18` (`^X`) — still a cut, still the program's |

**Where the list is expected to disagree, which is what F2.7 comes to decide.**

- **macOS**: the framework's «command» is ⌘ there, so ⌘+V would produce the paste request and
  `Ctrl+V` arrive as a key; the two shortcuts of **D11** stop being what this table says. The copy
  row goes the same way: `Ctrl+Shift+C` is not a copy request there, it arrives as a key, and
  `key_bytes` lets Shift through with Ctrl, so it reaches the program as `0x03` — the terminal's
  copy chord interrupting instead of copying. F2.7 rewrites the paste and copy rows for that
  platform rather than reading them as failures.
- **Windows**: the three chords over Insert and Delete are decided in **D115**, and the list there
  is the one below plus these rows, which replace the `Shift+Insert` one:

  | Chord | Situation | The program receives |
  |---|---|---|
  | `Shift+Delete` | `nano` open; then `cat -v` | `nano` stays open; `cat -v` prints `^[[3;2~` and no `^X` behind it |
  | `Ctrl+Insert` | text marked, `cat -v` running | nothing, and `cat` still running; the marked text is on the clipboard |
  | `Ctrl+Shift+Insert` | `cat -v` | `^[[2;6~` |
  | `Shift+Insert` | text on the clipboard | the text, pasted |
  | `Shift+Insert` | an image on the clipboard | nothing at all |
  | `AltGr+2`, `AltGr+´` | Spanish layout, a shell prompt | `@`, `{` |
  | left `Ctrl+Alt+2` | Spanish layout, a shell prompt | `@` once; native verification pending for #206 |
  | `Ctrl+Alt+A` | US layout, a shell prompt | no text; native verification pending for #206 |
  | `AltGr+Q`, `AltGr+7` | German layout, a shell prompt | `@`, `{` |
  | `Ctrl+Insert`, then `Shift+Delete` | text selected in a Settings field | copied; then cut |
  | `Ctrl+Insert` | lines selected in the drawer's code viewer | copied |

  The observer there is a console whose program echoes what it receives — `cat -v` inside Git Bash or
  WSL, or `nano`/the prompt itself where only the effect is read.

**What was discarded, and why.**

| Discarded | Why |
|---|---|
| An integration test under a virtual display (Xvfb, or a headless Wayland compositor) with operating-system key injection (`xdotool`, `ydotool`) | It is the one route that exists — `winit` builds the event itself from a real platform event, and the virtual display supplies the window — and it covers **only Linux**, while what makes the hand check expensive is macOS and Windows, where the injection is a different tool on each. It would also be the first test here that needs a display, on a machine that has neither tool installed and a repository with no continuous integration to run it; and tests of that kind are slow and flaky, which is why they get ignored rather than fixed |
| Leaving the question open until F2.7 | F2.7 is where it gets expensive, and arriving there without a list means deciding on the day which chords matter, on two unfamiliar machines |
| Building a `KeyEvent` some other way, or testing through `egui_winit::State` | The two walls above; `State::new` constructs and leads nowhere |
| Driving our own `winit` loop, so the translation becomes ours and testable | The same rewrite of the application's shell that **D33** discarded for the fix, now for a test |

**What reopens it.** A regression on this stretch that the list would not have caught, or the
repository gaining continuous integration with a display; the first says the list is incomplete, the
second removes most of what the virtual-display test costs. Either one is the new signal, and the
question it reopens is the one #206 asked: one hand check per platform per change, against building
the machinery once.

## D54 — A limit is written down with the evidence for it: «it cannot be fixed» names the attempt, and «it is rare» names who meets it

**Decided.** A decision often ends by accepting something it does not fix, and two kinds of sentence
close that question for everyone who reads it afterwards — whoever wrote it included. So both carry
more than the sentence itself, from here on:

1. **«It cannot be fixed» names what was read or measured to reach it, or says «not attempted».**
   The second form is honest and perfectly acceptable. The first is a claim, and it needs its
   evidence like any number in this file: the routes looked at and why each one fails — the
   function, the hook, the option — or the measurement that ruled the fix out, and whether the
   conclusion was **observed** on the running application or only reasoned from the source.
2. **«It is rare» names the technical condition, the situation of the person using the terminal
   that reaches it, and how it is known that this situation is the only one.** Naming the situation
   is not enough on its own, and D11 is the proof: it did name one. What is judged rare is the
   situation, so the claim stands only if no other ordinary situation reaches the same condition —
   and that is the half that gets skipped, so it is the half that has to be written.

**Where both came from.** **D11** recorded that with an empty clipboard `Ctrl+V` sends nothing, that
this is barely noticeable *because an empty clipboard is rare*, that it means `vim`'s visual block
does not open in a freshly started session with nothing copied, and that *it cannot be fixed from
here*. Both halves were wrong, each in its own way:

1. **The unfixability was reasoned, never tested.** **D33** read four routes and ruled them out —
   `eframe`'s `raw_input_hook`, `on_window_event` and `event_loop_builder`, and `egui-winit`'s
   `clipboard` feature — and watched the defect on a running window with an image on the clipboard,
   which showed something the reading had not predicted: the key's release survives. The fix was
   then checked by hand with `cat -v`. None of that had been done when the sentence was written.
2. **The rarity was judged against the wrong condition.** D11 named a situation — a session with
   nothing copied — and never asked whether it was the only one. It was not: the real condition is
   a clipboard that **holds no text**, and a clipboard holding a screenshot for an agent is an
   ordinary state in a terminal built to host agents.

It sat filed as a limitation for **26 days** — from 13 August to 8 September 2026, in this file and
in a knowledge card — and was fixed the day it was met in use.

**What was discarded.**

- **Re-auditing every existing decision against the two rules.** The rules apply from here on; an
  existing decision is looked at when something else brings you to it, which is exactly how D11 got
  corrected.
- **A guard that checks the rule.** «Not attempted» and «rare in what» are judgements about prose,
  and a test that looked for the words would be satisfied by the words — the same disguise **D30**
  names, in a document instead of a terminal.
- **Two decisions, one per rule.** They come from the same failure, one sentence each; split, each
  reads as half an argument.
- **Only the header of this file, unnumbered.** It is where a writer of a new decision looks first,
  so it points here — but a rule with no number cannot be cited from a review or from another
  decision, and it would carry neither its reason nor what was discarded, which is what this file
  asks of every entry.

## D55 — Closing a console hangs up its program itself, looking every millisecond, and keeps the library's grace

**Decided on 25 September 2026** ([#196](https://github.com/mashware/ivy-history-private/issues/196)).
`Drop for Pty` no longer calls `portable-pty`'s `kill` on Unix. It sends SIGHUP to the child's pid,
looks with `try_wait` every **1 ms** for up to **200 ms**, and sends SIGKILL if the program is still
there. Windows keeps the library's `kill`, which is `TerminateProcess` at once and never had a grid.

**Why.** The library's Unix `kill` looks once right after the SIGHUP and then **sleeps 50 ms** between
four more looks. Every live program needs *some* time to die, so every one missed the first look and
paid the whole step: 50.72 ms median per console, paid inside the interface pass, and in series for
the consoles of a tab (~101 ms for two, ~200 for four). What the programs actually need was measured
on a real pseudoterminal with the same session and controlling-terminal setup the library uses: bash
with its rc files 1.30 ms, zsh with its rc files 1.84 ms (max 2.14), a Node program in raw mode
1.16 ms, `vim -u NONE` through this crate's own `Pty` 3.4 ms. So about 48 ms of every close was
sleep. **After**, with `examples/closing.rs` over three runs: a live program 1.20–1.37 ms (was
50.88), a survivor with echo off 101.35–101.68 ms (was 151.01 — D32's deadline, untouched), a program
ignoring SIGHUP 201.54–201.99 ms (was 201.94 — the grace, untouched).

**The grace is kept at the library's 200 ms on purpose.** A program that catches the hang-up to save
its state is owed that time, and nothing measured here says how much of it a real program needs — so
the number stays the one the library chose rather than one invented here. What changed is only how
often somebody looks.

**The signals go to the pid, never the group**, exactly as before: who else closing a console should
reach is **#193**'s question, and this does not answer it. And a signal follows a `try_wait` that said
the child is still unreaped, because a reaped pid can belong to somebody else's process by the next
instruction.

| Discarded | Why |
|---|---|
| **SIGHUP then an immediate SIGKILL** — the cheapest, and the issue's first option | It takes away the grace a program uses to save its state on hang-up. The cost was never the grace: it was the grid |
| **Moving the close off the interface pass** — a thread, or a queue of dying consoles | Nothing joins at process exit, so a program being closed as the window goes could outlive the app instead of being killed; and `Pane` drops its `Pty` before its panel directory, so a deferred kill lets the directory be deleted with the program still alive and able to write `panel.json`. D32 had discarded it for its own reason too: the abandonment becomes unbounded and invisible |
| **Asking `portable-pty` for a non-polling kill** | It fixes nobody here until a release ships it, and the whole change this crate needs is twenty lines over two system calls the library already makes |
| **A blocking wait with a deadline** instead of looking every millisecond | Unix offers none for a child: `waitpid` either blocks without a bound or does not block, and a timer signal or a `pidfd` would be more machinery than a loop that costs two hundred non-blocking calls in the worst case |
| **Looking less often as the grace goes on** (1, 2, 4, 8… ms), which a review argued for against D32's own objection to a poll inside the interface pass | D32 objected to *waking a hundred times where a blocking wait existed*: `recv_timeout` wakes on the event. Here none exists, the window is frozen for the program's dying either way, and every step of backoff after the first is added straight onto the common case — a shell dying at 1.3 ms would be reaped at 3. The worst case is two hundred non-blocking calls, in a close that is already paying 200 ms |

**One hole it does not close, and did not open.** ~~If the system refuses the SIGHUP, nothing
further is sent and the `wait` that follows is unbounded — exactly as behind the library's `kill`,
which returned at the same refusal. **Reasoned from the source, not observed, and no fix
attempted.** The kernel lets a process signal another when their user identities match on the real
or saved side. The first candidate a review named, `su`, was withdrawn: its waiting parent keeps the
user's real identity. The second, **`sudo` once authenticated**, is widely reported to refuse a plain
`kill` from the user who ran it, and it would be the pane's direct child in `sudo vim …` — **not
measured**, because `sudo` asks for a password on the machine this was written on. So the case has a
plausible subject, and it is tracked as work of its own rather than as a curiosity. The bounded
alternative a review proposed — look with `try_wait` up to a deadline and then abandon the child, as
D32 abandons the reader — belongs to that work, because abandoning a live child is a trade D32 only
accepted with its survivor measured.~~ **Closed by D56**, and the candidate was the wrong one:
measured, both sudos accept the hang-up and die of it in about 3 ms, and what refuses it is
`exec pkexec`. Nor was the deadline taken: nothing sent can end such a program, so the child is left
at once and waited for on a thread of its own.

**Still paid, and declared.** A program that ignores the hang-up — or a stopped one — still costs the
whole 200 ms, and a survivor holding the pseudoterminal still costs D32's 100 ms on top of the dying.
Both are paid on the interface pass and both are already declared into the frame's separately-timed
work (**D13**). No batch closes a console by hand, so no published frame figure moves.

## D56 — A program the system will not let us signal is left running, waited for off the interface pass, and said once

**Decided on 25 September 2026** ([#271](https://github.com/mashware/ivy-history-private/issues/271)).
When closing a console, the hang-up (D55) is **refused** by the system — any failure of `kill` but
«no such process» — `Drop for Pty` no longer waits for the child inline. It hands the child to a
thread that does nothing but `wait` for it, writes **one line to standard error** naming its pid and
the operating system's refusal, and carries on with the rest of the close: the reader's deadline
(D32), the panel directory. ~~Nothing else changes: a program that accepts the signals is waited for
where it always was, with the same grace.~~ Since the amendment of 19 November 2026 (below), a
program that accepts the signals is waited for at most 100 ms, then left running the same way.

**Why.** Measured on 25 September 2026, on this machine: a console whose shell was replaced by
`exec pkexec sleep 60` became a program with every user identity root, `kill(pid, 0)` from the
terminal answered `EPERM`, and closing that console froze the real window until the desktop offered
to force-quit it. The cause is the wait, not the signal: a parent can always reap its own child
whatever its uid — only signalling is gated by identity — so `waitpid` blocked for the program's whole
life on the interface pass, and nothing this process can send shortens it. The kernel's own hang-up
would not rescue the wait either: it needs every descriptor of the master closed, and the reader thread
still holds a clone at that point.

**Amended on 26 September 2026** ([#288](https://github.com/mashware/ivy-history-private/issues/288)):
**«left running» is what happens at the moment of the close, not what happens after it.** The rest of
the close drops every copy of the master — the reader's once it ends, the console's own, and the
writer thread's, which is never joined and can be the last. That last close makes the kernel hang the
terminal up and send SIGHUP to its **session leader**, which is the console's child, and the kernel
checks no identity to do so. So a program that keeps the default action for SIGHUP ends a moment
after the close anyway, root or not. It stays alive only if it ignores or handles the hang-up, or
while some copy of the master is still open — an abandoned reader (D32) holds one until the survivor's
next byte. Measured with a forged refusal: a program that trapped SIGHUP logged it after a close in
which this crate sent nothing, and holding a duplicate of the master across the close kept it alive
until the duplicate closed. **The real `exec pkexec` case is not measured**: that pkexec keeps the
pid and the controlling terminal is read from how it works, not run, because it needs an
authenticated prompt. The line on standard error says «may keep running» for this reason, and the
regression test's program ignores SIGHUP, which it did not before and which made it fail now and
then.

**Who meets it, and who does not** (D54). **`sudo` does not**, which is the opposite of what D55 and
the issue said. Both sudo-rs 0.2.13 (Ubuntu's `/usr/bin/sudo`) and classic sudo 1.9.17p2 leave the
parent that is the console's child with the **invoker's real uid**; the kernel allows the signal, sudo
catches it and relays it, and the whole chain was gone in 2.65 and 3.55 ms. What «sudo refuses a kill
from its invoker» describes is signalling *the command* sudo runs. **`pkexec` does**: it executes the
command itself as full root, with no parent left. A console runs `$SHELL`, so `pkexec cmd` typed at
the prompt is a grandchild and harmless; the path is `exec pkexec cmd`, or a shell set to such a
program. Uncommon, and a window that never comes back when it happens.

**The line is the channel D41 already uses** for a console that stops receiving: whoever ran the
binary from a shell learns that a root program was left running, with nothing to find it by
otherwise — the console that would have shown it is gone. Decided by the user at the fix's gate.

| Discarded | Why |
|---|---|
| **Waiting with a deadline, then abandoning** — the option the issue carried over from D32 | A deadline is for something that may still end: here nothing sent can end it, so any deadline is time the window is frozen for no possible gain. Abandoned at once is the same outcome, without the wait |
| **Dropping the child without waiting** | It leaves the program a zombie for the life of this process once it ends. The thread is the smallest thing that reaps it |
| **Saying nothing**, as D32 does for an abandoned reader | That abandonment leaves nothing behind that anybody could act on. This one leaves a live program running as root with no console, and the person who closed it is the only one who could know |
| **Moving every close off the interface pass**, which would take this special case and D55's inline grace with it | D55 discarded it and its reasons still stand: nothing joins at process exit, so a program being closed as the window goes could outlive the app, and `Pane` drops its `Pty` before its panel directory, so the directory would go with the program alive. Here the program is left running only because nothing can end it — the one case where both costs are already paid |
| **Closing the master before waiting**, so the kernel hangs the terminal up with its own authority | The reader's clone keeps the master open, and the last close only sends SIGHUP to the session leader — a root program that ignores it survives anyway. Not needed: the fix does not rest on the kernel. (It used to say «and the foreground group» too: Linux signals the leader only. And the hang-up does arrive once the close has dropped every copy of the master — see the amendment above) |

**What it costs, declared.** One thread per such console, parked in `wait` for as long as its program
lives, the same shape of leak as the writer's and the reader's. If that thread cannot be created, the
child is dropped unwaited and becomes the zombie above: the price of not freezing. The regression test
forges the refusal — an unprivileged test cannot create a child it may not signal — and runs the rest
against a real program. The real window was checked by hand on the same day, with the build before
and the build after: the one before froze until the program's minute ran out, the one after closed
the console at once, kept answering, and printed the line.

**Amended on 19 November 2026** ([#138](https://github.com/mashware/ivy/issues/138)): **a program
whose signals were accepted is waited for with a bound too.** After the hang-up and its kill,
`Drop for Pty` now looks whether the child is reapable every millisecond for at most 100 ms
(`REAP_GIVE_UP`), and if it is not, hands it to the same `left-running` thread and writes one line
to standard error with its pid. Until then that wait had no bound, on the claim that a kill always
makes a program reapable.

**Why.** On macOS that claim is false, and closing froze the window for good. Reported on
19 November 2026 from macOS 27.2.1 (ARM64, 0.1.0-alpha.12): a `sample` of the frozen app showed every
main-thread sample in `wait4` under `Drop for Pty`, with every other thread idle. The waited pid was a
live, unreaped child of ours (`wait4` on anything else fails at once), so the signals had reached it
and it still did not become reapable. **The most probable reason, not reproduced**: XNU's
`proc_exit` calls `ttywait` on a session leader's controlling terminal, which sleeps with no timeout
while the terminal holds output nobody has read. The close stops reading the master before it hangs
up, and keeps the master open while it waits. A kill arriving after the program is already exiting
cannot interrupt that sleep. Linux has no such drain at exit, which is why every closing measurement
(D32, D55, this decision) missed it. The fix does not rest on that explanation: whatever keeps the
child unreapable, the window no longer waits with it.

**On Linux nothing measurable changes**: a killed program is reapable about a millisecond later
(D55), well inside the bound, and a unit test pins that a program reapable a few looks later is
reaped inline. **On Windows** a refused `TerminateProcess` takes the same path, so the freeze of
[#273](https://github.com/mashware/ivy-history-private/issues/273) is bounded too. That path was
type-checked for Windows and never run there. The program that is left over on macOS is reaped by its thread once
the close drops the last copy of the master, if the explanation above is right. If it is wrong, the
thread waits for as long as the program lives, which is the cost declared above, now for one more
case.

| Discarded | Why |
|---|---|
| **Keeping the master read until the child is reaped**, so the drain at exit can finish | It fixes only the explanation above, which is not reproduced, and it keeps the wait on the interface pass |
| **Waiting with no bound and relying on the kill** | That is the defect. A kill accepted by the system is not a program gone |
| **A longer bound** | Only time the window is frozen for. A program that is not reapable a tenth of a second after a kill is not going to become reapable by waiting more |

**Amended on 20 November 2026** ([#163](https://github.com/mashware/ivy/issues/163)): **on Windows a
refusal is seen, and nothing is left waiting on the interface pass.** `portable-pty`'s `kill` read
`TerminateProcess`'s result the wrong way round and discarded it, so the refused path above was never
taken on Windows. This crate now calls `TerminateProcess` itself and writes this decision's line when
it fails. Windows takes neither the 100 ms bound nor the `left-running` thread: there is no zombie to
reap, and the thread that watches the program's exit (D111) already waits on it. The line is
written by the thread that closes the console, after its grace.

## D57 — Every blocking wait in production code is bounded or declared abandoned, and a test enforces it

**Decided on 25 September 2026** ([#190](https://github.com/mashware/ivy-history-private/issues/190)).
`crates/app/tests/waits.rs` walks the production source of **every crate** under `crates/` and fails
on any call named `recv`, `wait` or `wait_while`, and on any argument-less `join`. Such a call passes
only when a comment **opens** with one of two cases, its reason on the same line. That comment is
either a paragraph of the block ending right above the call (its first line, or the first after an
empty `//`), or a comment trailing on the call's own line. Only the first wait on a line can be
declared, so a line with two waits has to be split:

- «Bounded:» — what ends the wait.
- «Abandoned on purpose:» — who is left behind, and why that trade is better than waiting.

Deadline forms (`recv_timeout`, `wait_timeout`) have other names and are never judged. The number of
declared waits is pinned as an equality, five on the day it was decided. The walk has floors of its
own on the crates, files and lines it reaches.

**Why.** A thread that waits is not looking at the thing that would normally wake it, so every wait
needs its own way out. The rule was in the knowledge store before this, and `term-core` broke it
twice anyway. The first was the writer thread, which could block for ever and so is never joined
(#159 — since that issue, only off Linux). The second was the reader thread: it had its stop flag
and still hung the window, because it was parked inside a `read` where the flag is never consulted
(D32). A comment is read once, so a
third reminder would not hold. The three guards that already walk source and break the build
(`shape.rs`, `prose.rs`, `glossary.rs`) are what showed this can be enforced.

**The scope is every crate, not the one that broke it**, and the user decided it at the design. A
guard that watched one crate is the mistake the scripts guard nearly repeated (D30). The cost on the
day was one declaration outside `term-core`: the condition wait in `app`'s control channel.

| Discarded | Why |
|---|---|
| **One phrase for both cases** («Unbounded on purpose:») | Four of the five waits it was written for are bounded: the writer's receive, ended when its only sender is dropped; the kill that follows a hang-up's grace; a join reached only after the thread ended; and reads that only touch memory. The declarations would have been false, and this project treats a mislabelled claim as worse than none |
| **An allow-list of `file:line` inside the guard** | It goes stale with every line that moves, and nobody reading the call sees it. The declaration belongs where the wait is |
| **Stopping at `mod tests`, like the line counters** | Production code written after a test module would be invisible, and `keyboard.rs` already has some. Test modules are removed by counting their braces instead, with literals and comments blanked first so a `"{"` cannot throw the count off |
| **A floor on the declared count** | A floor also catches a matcher that stopped matching. But it goes stale the day a wait is removed, and an equality fires |
| **Judging test code** | Out of the ticket. A test's unbounded wait hangs the suite, not the window |
| **Clippy's `disallowed_methods`** with an `#[expect(…, reason = "Bounded: …")]` at each call — raised in review, and it resolves types, so it sees calls written as paths and aliases | Code behind `cfg(not(unix))` is never type-checked on Linux, so the lint would miss exactly the Windows paths F2.7 is about to exercise, which a walk over text reads. Every test module holding a wait would need an allowance of its own, about thirty waits' worth on the day. And the reason's wording would still need a test. It could complement this guard later; it does not replace it |

**What it does not see, declared.** A lock held across a wait: a walk over text cannot see how long a
guard lives, although a condition variable's `wait` is still caught by name. A wait it cannot name:
a `for` over a receiver or its `iter()`, the listener's blocking `accept()`, a blocking channel
`send` (unbounded channels use the same name and never block), and the pseudoterminal's own `read`,
which is the parked side of D32. Nor waits under other names: running a command to completion
(`output()`, `status()`, `wait_with_output()`), a thread scope that joins at its end, a call written
as a path (`JoinHandle::join(h)`), or parking a thread. Turning a declared wait into one of those
lowers the declared count, and the equality is what notices. Test modules are recognised in one
shape only: a `#[cfg(test)]` line of its own, other attributes after it, then a `mod … {` header on
one line. Any other shape is read as production: an undeclared wait in it fails the guard, and a
declared one counts toward the pinned number. Nor does it see whether
a declaration is true. It checks that a reason is written, and the pinned count is what makes every
new declaration an edit that a reviewer reads.

**One declaration was true only in part, and is gone.** The close's `child.wait()` used to be
declared «Bounded:» by the kill a hang-up falls through to. That was false for a program in
uninterruptible sleep, for a refused kill on Windows
([#273](https://github.com/mashware/ivy-history-private/issues/273)), and for a shell exiting on
macOS ([#138](https://github.com/mashware/ivy/issues/138)). It is now a `try_wait` loop with a
deadline, and anything left over goes to the abandoned `left-running` thread (D56, amended), so the
pinned count fell from 16 to 15.

## D58 — A publication over the channel that lost a word is still taken, and its reply says which

**Decided on 26 September 2026** ([#255](https://github.com/mashware/ivy-history-private/issues/255)).
When `panel_set` or `panel_patch` carries a value none of the three closed vocabularies knows —an
`attention`, a line's `mark`, a line's `style`— or a value there that is not even a word, the
document is read exactly as before: the value is dropped, the line stands, the panel paints. What
changes is the reply. A taken request that lost something answers
`{"ok":true,"dropped":"<sentence>"}` instead of `{"ok":true}`, and the bridge an agent speaks to
answers `published; 4 lines carry an unknown mark "doing"` instead of `published`. The tool's own
description names the three vocabularies word by word. The literal sentences are in
[the contract](04-panel-contract.md#what-fell-away-is-said).

**Why.** A session driving a pane published five lines with `"mark":"doing"` on four of them and was
answered `published` — the same word a perfect document gets. Four lines lost their symbol, colour
and column, and nobody noticed until a person compared two panes. The producer had the document's
shape from the tool's schema and its vocabulary from nowhere: the description spelled out
`attention`'s three words and called the six marks and seven styles «the rest of the contract». D17's
own table already said the channel *warns the producer that its document is broken* and the file
does not; for a document that was tolerated rather than refused, it never did.

**The tolerance is not what changed**, and that is deliberate: a panel that paints four fifths of its
lines beats a refusal, which is what D6 and the per-field reading have always said. `panel_patch`
already refuses an unknown word in a `set`, and that stays too — an instruction nobody can read has
no sensible reading, a summary with one bad word does.

**Three bounds on the sentence, all for the same ceiling.** The count is of lines; at most three
distinct values are named, the rest become «and others»; each value is shown up to 32 characters.
Every value is escaped once into the sentence and again into the reply line, and the client refuses a
reply past the document's ceiling plus one kibibyte — which drops the pane's hold with it. A document
of garbage marks, legal going in, would otherwise break the connection it was published on.

| Discarded | Why |
|---|---|
| **Refusing the unknown word**, the way a patch's `set` does | Loses the whole panel over one field; D6 and the issue both keep the tolerance |
| **A structured list on the wire** (`[{field, value, lines}]`) | The only consumer turns it into text for an agent, and the contract already fixes words rather than shapes for what a producer reads (`Problem::Refused`). A structure would be a second contract to keep in step with the sentence |
| **Reusing the reply's `text` field** | `text` is a console's screen (`Reply::Text`); a second meaning is how a reply gets misread |
| **Changing `parse_document`'s return type** | Some sixty call sites, nearly all of them tests, and none of them has anybody to tell. A second entry point, `parse_document_noting`, reads with the same function and keeps the note; the file's reader, inside the frame, records nothing at all, so the frame pays nothing for a record nobody reads (**D13**) |
| **Letting the window build the reply** | The parse already happens on the connection's thread, where the note is in scope; the window has no use for it, and `crates/app/src/control.rs` sits at its line ceiling |
| **Documenting `panel_get` as the way to check** | It works — it returns the document as the application understood it — and no producer does it. The reply reaching the producer unasked is the point |
| **Reporting every other silent drop** (unknown fields, a clipped indent, a refused button, a line that is neither text nor an object) | Out of the ticket. Those are not a vocabulary a producer guessed from nowhere |

**What it does not do, declared.** The file transport stays silent: a `panel.json` on disk has nobody
to answer. A null is not reported, because it reads exactly like the field left out. Only the three
vocabularies are reported: any other known field of the wrong type, an unknown field, a clipped
indent or a refused button still fall silently, so a bare `ok` promises that no vocabulary value was lost and
nothing more. Two values that agree up to the 32-character cut are named as one, though each of their lines is counted. And a
publication the window takes after its five-second answer deadline has already been answered *the
window did not answer in time*, so what it lost is never said.

## D59 — A named theme is a built-in base; explicit colors always override it

**Decided for [#179](https://github.com/mashware/ivy-history-private/issues/179).** The first catalog
ships two complete named themes, `tokyonight` and `gruvbox-dark`, inside the binary. Names ignore
ASCII case, as `font` already does. A `theme = name`
line chooses the base for all twenty-two colors. Every explicit `color.N`, `background`,
`foreground`, `cursor`, `divider`, `focus`, or `panel` line overrides its own slot regardless of where
the theme line appears. If `theme` occurs more than once, the last valid name wins and the existing
repeated-key warning says so. An unknown name is reported with its line number and the available
names; the previous valid name and every other valid line remain in force. With no `theme` line, the
existing factory theme remains the base.

**Why.** D26 reserved a named catalog after F2.10 while keeping the configuration file tolerant
line by line. `Settings::Colors` already holds each explicit color as an optional override, and
`Theme` already applies those overrides one slot at a time. Treating a theme as a base fits that
model and lets a person reorder the lines of their file without changing their explicit tweaks.
Keeping the catalog in the binary avoids a new startup path lookup, directory convention and file
failure mode for two static palettes. One name alone would offer no choice; these two give distinct
cool and warm dark palettes alongside the existing dark factory theme. The sixteen terminal colors
follow [Tokyo Night's Night terminal palette](https://github.com/folke/tokyonight.nvim/blob/main/extras/windows_terminal/tokyonight_night.json)
and [Gruvbox dark's terminal palette](https://github.com/alacritty/alacritty/wiki/Color-schemes#gruvbox),
respectively; the exact forty-four values are pinned in the work's design and its tests. The six
colors for the terminal's own interface are choices made here.

| Discarded | Why |
|---|---|
| Theme files on disk, or imports from other files | D26 already discarded imports. A disk catalog needs search paths, missing-file behavior and startup I/O before the first two themes need any of them. |
| A theme line resets colors written above it | Moving a theme line would erase an unrelated explicit override, despite `theme` and each color being distinct keys. |
| Rejecting the whole file for an unknown name | D26's line-by-line tolerance would be broken and valid font, shortcut and color lines would be lost. |
| Silently falling back to factory colors | The person who typed the name would see a different appearance with no explanation. |
| Shipping one theme or a large collection first | One has no choice; every additional palette has twenty-two values to check. Two make the first catalog useful without making it a palette import project. |

## D60 — The input queue's honest ceiling refuses, it does not block; and it refuses on what is already owed, never on a write's own size

**Decided for [#161](https://github.com/mashware/ivy-history-private/issues/161).** `Pty::write`
(term-core) refuses once `MAX_OWED` (256 KiB) is already owed to the program, checked **before**
this write's own bytes are added — so a single write, however large, is always accepted whole when
the queue has room, and only a write arriving once the queue is *already* full is turned away.
Nothing owed is ever discarded: what was already handed over stays queued and still lands if the
program comes back to read it. The refusal is a new fact, not a new shape: `Backlog` gains
`refused: bool`, edge-triggered — set the moment a write is refused, cleared the moment a later one
is accepted — and `refusals: u64`, how many writes have been refused so far, both read the same
way `gave_up` already is. The application shows it the same way it already shows a console that
stopped receiving (D40/D41): a sentence over the top half of the console, dismissed by a click,
sharing the same half-console ceiling (D39) — but its own sentence, never the output-ending one,
since a refusal is routinely reached while the program is reading its output perfectly normally.
**A dismissal is for the refusals it saw, not for the state**: it records the count, so the next
refusal — a retry into a queue that is still full included — brings the sentence back. When the
output has ended too, only the ended-output sentence is shown, and a press dismisses only the
reason on screen; one function answers which reason that is, for the painting and for the press.

**Why.** #50/D29 deferred this exact ceiling, on purpose, pending an answer to one product
question: what does the window do when it cannot accept a keystroke? Discarding was already refused
there, for the same reason D8 refuses it on the output side — a key or a mouse report that vanishes
in silence is the failure hardest to understand from the other side of the screen. Refusing,
visibly, is the only third path, and #161's own text already names it: "a limit past which
`Pty::write` **refuses** instead of accepting."

**The asymmetry with D8 is deliberate, not an oversight.** D8's ceiling blocks-and-retries its
producer — the reader thread — because blocking a background thread costs nothing anybody is
watching. `Pty::write` is called from the console's own input path, the same thread D29 already
freed from blocking on this exact pseudoterminal; making it block again here would quietly reopen
the freeze D29 was written to remove. Refusing is the only shape that keeps D29's guarantee intact.

**The ceiling is checked against what is already owed, not against what this write would bring the
total to.** The difference matters for one real case: a large, legitimate paste sent to a program
that is only momentarily behind. Checking after adding would refuse that paste for its own size,
which is not what "somebody's hands produced it" (D29's own reason for not discarding) was arguing
against. Checking before means a queue with room left always takes a write whole, and only what
arrives once the queue is already full is turned away — which is also what makes the ceiling
harmless to the ordinary, healthy case: nothing about typing or pasting into a program that is
actually reading its input changes at all.

**256 KiB is a reasoned default, not a measured one — declared as such rather than borrowed from a
number that only looked like a precedent.** An earlier draft anchored it to `GENEROUS_MARGIN`
(term-core's own `without_consuming_it_what_arrives_is_bounded` test, `crates/term-core/src/
lib.rs`), which reads like a production safety margin but is not one: it is a deliberately loose
(10×) assertion bound, chosen so that test does not depend on this machine's exact kernel slack.
The design's challenger caught the citation; the number now anchors instead to the one *real*
measured ceiling this repository has on the input side's neighbour — the output side's
`READS_IN_FLIGHT × READ_SIZE` plus the kernel's own buffer, on the order of 90-100 KiB — rounded up
for headroom. No input-side flood benchmark exists to measure a number of its own against; this is
recorded as a followup (F1 of the work that shipped this decision) rather than hidden behind false
precision.

| Discarded | Why |
|---|---|
| A second bounded channel, imitating D8's `sync_channel` + `try_send` + retry-on-`Full` shape literally | `Input.owed` already counts what a second queue's length would count; a second counter would only risk disagreeing with the first. Capping the existing counter inside `Pty::write` is the smaller, more honest change |
| Blocking the caller once the ceiling is reached, matching D8 | `Pty::write`'s caller is the console's input-handling thread, the one D29 already freed from blocking on this exact call. Blocking here reopens that freeze |
| Checking `owed + bytes.len() > MAX_OWED` (after adding this write's bytes) | Refuses a single legitimate oversized write — a big paste to a program that is merely behind — for its own size, punishing exactly the case D29's "no discarding" reasoning protects |
| A public enum for the refusal, mirroring `Ending`'s shape | No caller branches on *why* a write failed today — all five write call sites already log the message and return on any `Err`. `Backlog.refused`, polled the same way `gave_up` already is, needs no new type on the return path |
| A third, independent box in the notice layer for the refusal | Would need a real three-way stacking design under D39's half-console ceiling, which #161's design scoped out (recorded as a risk, not solved). Reusing the existing "console has something to say" box's trigger and sentence needed no new geometry at all |
| Reusing `QuietConsole`'s sticky `dismissed` latch for the refusal too | An ending never reverses, so that latch is permanent by design. Refusals keep coming; a sticky latch would silence every later one |
| A dismissal cleared by observing `refused` false (the first shape this delivery built) | A read that wrote, whose safety rested on a claim about threads nothing checked — and the flag stays up across a retry into a full queue, so the retry was refused in silence. Both reviews of this delivery found it. Keying the dismissal on the `refusals` count makes the question a pure read and gives each refusal its sentence |
| Dismissing both reasons on any press on the shared box | When the output had ended *and* the queue was refusing, only the ended-output sentence was shown, and the press silenced a refusal nobody had seen. The built-in review found it; the press now dismisses the reason `notices::what_the_silence_is` says is on screen, the same function the sentence is chosen by |

## D61 — Searching the scrollback: the focused console, literal text, and the match is the selection

**Decided for [#177](https://github.com/mashware/ivy-history-private/issues/177), F2.13.** `Ctrl+Shift+F`
(`key.search`, and `none` hands it back to the program) opens a search **in the focused console**.
What is typed builds the query, which is **literal text** — a `.` or a `(` means that character —
with the engine's case rule: insensitive unless the query holds an uppercase letter. `Enter` walks
toward older lines and `Shift+Enter` toward newer ones; each step scrolls to the match and **makes
it the selection**, so `Ctrl+Shift+C` copies it through D22's four rules with nothing of its own.
Every match on screen is marked in a colour of its own, **already while the query is typed**. A
one-row line is painted **over** the console's last row with the query and how to leave; `Escape`,
the chord again, or **any key that sends bytes to the program** closes it. While it is open the
search keeps typed text, `Backspace`, `Ctrl+W`, `Ctrl+Backspace`, `Ctrl+U`, `Enter`, `Shift+Enter`,
`Escape` and a paste — which goes into the query, with its control characters dropped so a copied
line's newline does not make it unfindable — and nothing else. Modifiers riding on those keys are
not looked at beyond the ones named: `Ctrl+Enter` is a step older, `Alt+Backspace` deletes a
character.

**Why.** Part I of Phase 2 promises search by name and WezTerm, Alacritty and Ghostty all answer
the questions #177 left open the same way: per console, with every visible match marked and the
current one distinct. This project measures itself against them (D19), so their answer is applied
rather than re-asked.

**The match is the selection because two mechanisms would disagree.** A separate «current match»
would need its own copy path and its own place to remember where the next step starts. The
selection already has both: the engine rotates it as output arrives and drops it when its lines
leave the history, so the next press starts from the same text however much the program wrote in
between, and nothing outside the engine holds a position that could go stale. The price is that a
step replaces a selection somebody had made, which is what the reference terminals do too.

**The step starts one cell past the current match.** The engine counts a match that sits exactly on
the point it is handed, so starting on the match finds it again on every press. Alacritty steps past
it the same way.

**The line is painted over the console, never cut out of it.** Cutting a row would resize the
program every time the search opens and shift every pointer-to-cell mapping by a row. To keep the
line from covering what it found, the step scrolls the match off the last row when the view is not
already at the bottom; when it is — nothing newer to scroll in — and the match the last press found
is on the last line, the line is painted on the top row instead. Before any press has found a match,
a selection made with the mouse does not move it.

**Literal, not a pattern.** A pattern can build an automaton large enough that the engine gives up
mid-walk after three cache clears and reports no match where there is one — and it says so only in
a log. A query too large for the engine is refused and the line says so.

**A press that finds nothing walks the whole history, and that walk is not split across frames —
a limit accepted, written down the way D54 asks.**
- **What it costs**: measured with `cargo run --release --example search -p term-core`, a history
  of 3 549 lines of 180 columns and a query that is not there, two runs of twenty presses each way:
  **a median of 11.8–13.6 ms, and a slowest press of 13.8–27.1 ms** (the raw output is kept with the
  work that measured it). On top of a frame of two dumping consoles that is one or two late frames
  at 60 Hz, **once per press**, never per frame.
- **Who pays it**: the person pressing `Enter` on a query that is not in a full, wide console.
  A match that exists is found sooner.
- **Why it is not fixed**: splitting the walk means carrying an engine position across frames, and
  those positions shift under new output — the stale point the selection-as-cursor exists to avoid.
  The design's line was 4 ms; the measurement crossed it and the trade was taken knowingly.
- **What would reopen it**: a press measured above one frame on the console's own batch (`E`), or a
  ceiling of history raised past D21's.

**Marking is per frame, so it is bounded.** It walks only what is on screen, plus the rest of a
logical line a wrap carries across an edge — and never more than `MARKING_REACH` (a hundred) lines
beyond the screen, following the wraps itself rather than asking the engine, whose own walk to a
line's ends has no bound: one line wrapped across the whole history would otherwise be walked on
every frame. Its cost is taken out of the console's painting span and published on the report's
`search:` line, with how many paintings marked a match.

**Measuring it closed a declared hole.** The keyboard's pass was left untimed because no batch
typed; the search's batch (`E`) types, so on a frame somebody typed in the pass is now timed whole
and subtracted from the rest of the frame (D13). A frame nobody typed in declares nothing, as
before. That moves what every typing frame publishes as the framework's cost by the pass's own
cost, and no batch published before `E` types.

**What it does not do**, each on purpose: search every console at once, take regular expressions,
remember earlier queries, or survive a restart. The line takes no clicks: a press on it marks the
grid under it. The chord is checked on Linux only; macOS and Windows meet it in F2.7. **And a held
paste answered with `Ctrl+Enter` while a search is open goes to the program and leaves the search
open** — the pane's keyboard pass has no room left under D28's ratchet for the line that would close
it, and the paste reaching the program is the outcome the person asked for either way.

**Measured (batch `E` against its control `C`, 10 October 2026, same binary, runs interleaved E·C·E·C,
1280×800, both consoles dumping 44 MiB, release, at `8afc00f`).** `compare` says the two runs of each
batch can be averaged. The walk's own bound (`MARKING_REACH` followed rather than clamped) landed
after these runs; it can only shorten the marking, and the load's lines wrap over two rows at most
— far inside the reach — so on this load both shapes walk the same cells.

| | E run 1 | E run 2 | C run 1 | C run 2 |
|---|---|---|---|---|
| presses (found) | 1 199 (239) | 2 094 (238) | 0 | 0 |
| a press, mean · slowest | 51.5 µs · 377 µs | 87.7 µs · 409 µs | — | — |
| paintings with a search open (with matches marked) | 1 499 (269) | 2 618 (268) | 0 | 0 |
| marking, per painting | 59.7 µs | 64.5 µs | — | — |
| left console, enqueueing the paint | 544 µs | 569 µs | 500 µs | 546 µs |
| right console, enqueueing the paint | 414 µs | 423 µs | 430 µs | 464 µs |
| frames | 1 796 | 3 139 | 455 | 464 |

- **Most presses found nothing, and the report cannot say how much each kind cost.** The load
  starts at the next multiple of thirty seconds (`host-load --at 30`) while the gesture starts at the
  first frame, so presses ran on a console with no text yet — 80 % and 89 % of them found nothing.
  The report publishes one mean over both kinds and does not split them, so **neither mean is the
  cost of a press during a dump**, and the two runs' means (51.5 and 87.7 µs) cannot be decomposed
  from these reports. What they do bound: **no press of either run took more than 409 µs**, far
  under the 11.8–13.6 ms the bench example measures for a full history with no match.
- **The marking is 60–65 µs a painting**, subtracted from the left console's painting and published
  apart. The left console's enqueueing span — which keeps the search's row and the matches' fills —
  reads 544–569 µs against C's 500–546: the ranges touch, so this is **no difference measurable from
  two runs**, and the right console, which never searches, moves by as much between runs.
- **Per-frame means do not compare between E and C**: E runs four to seven times more frames, because
  injected keys keep the window awake (D15's annex has the same trap with drags). Its «rest of the
  frame» is not published as a comparison.
- **C's second run carries two keystrokes nobody's batch sent** — a key pressed during the run. They
  are declared by the run's own report and change no figure above beyond its noise.

| Discarded | Why |
|---|---|
| Opening a pager (`less`) over the scrollback in a split, as Kitty does | Part I promises search in the terminal, in place; `less` does not exist on Windows (D1); and the console's text would be written to a temporary file, which is a copy of the screen outside the window |
| Searching every console at once | No reference terminal does it, and it needs a list of results nobody asked for |
| A «current match» mark separate from the selection | Its own copy path and its own stored position, which goes stale when output arrives; the selection already has both |
| Regular expressions | An oversized automaton makes the engine give up and report no match where there is one, saying so only in a log |
| A row cut from the console for the line | Resizes the program on every open and shifts every pointer-to-cell mapping by a row |
| The line moving to the top whenever a selected cell is on the last row | The step can usually keep the match off that row; moving the line is only the answer when the view cannot scroll further — and a mouse selection is not something the line has to avoid |
| A cap on the query's length | A literal's automaton grows with its length far below the engine's limit, and the only way in is typing or a paste; the engine's own refusal is said instead |
| Stepping to a match as you type | Each keystroke would pay the walk over the history — up to the figures above — while typing. Marking the matches on screen as you type is kept: it walks the screen and at most `MARKING_REACH` lines beyond it |

**Amended 18 November 2026 — #120: a search button per console, a box, and three options.** With
several consoles the one search button at the top of the window did not say which console it
searched, and the one-row line took no clicks, so pressing the button seemed to do nothing.

- **Each console header carries the search button** (magnifier, «Search this console»), in the
  base group beside review and the panel (D99). It opens or closes the search **on that console**,
  focused or not, and hands it the keyboard so what is typed next lands in its query. The titlebar
  button is gone; `key.search` and the application menu's «Search console» still act on the
  focused console. Leaving the bar moves D96's compact boundary down by the button and its gap:
  **570 points on Linux and Windows, 533 on macOS**.
- **The line becomes a box at the console's top-right**: the query, next (`Enter`, an up arrow,
  since it walks toward older lines), previous (`Shift+Enter`, a down arrow), an options button and
  close (`Escape`), **painted over the console and never cut out of it**, as the line was, and with
  every part kept inside the console — a part clipped away would still take presses there. The
  options list unfolds on the far side of the box from the console's edge, to the box's left when
  there is no room for that, and over the box — whose parts under it then give way — only when
  there is room for neither; a list that would cover a notice is not shown. **A step keeps its
  match off the rows the box takes** (and the list's, while it is unfolded), at the edge the box
  sits on, while the view is in the history and can still scroll, which replaces the keep-off-the-last-row scroll
  that served the line: walking older the engine lands a match on the top row, and a box that moved
  there would leave the next click on «next» to fall on the console. A match on the live screen
  leaves the view where the program writes, and at the oldest line the history has no room: there
  **the box moves to the bottom-right** when the match a step found lies under it — a selection made
  by hand before any step found one does not move it, so the box never jumps under a drag. It does
  the same for the held paste's question and the «stopped receiving» sentence, which keep the
  console's top (D41); **a console too short to hold the box beside them closes the search**, since
  a box nobody can see must not go on taking keys. Its place comes from the console, that match and
  those notices only, never from whether the options list is unfolded. A link hidden under the box
  is not offered under the pointer: the box takes the click.
- **The box takes presses, and what lies under it hears none.** A press on it is claimed before
  the console's own routing — no selection, no mouse report to a program that asked for the mouse,
  no context menu or paste from the hidden text — and a press on its field hands that console the
  keyboard. As with D99's header controls, **only a box painted in the frame before answers**, so one
  recorded on a console that stopped being painted cannot take a press unseen. A press anywhere
  else folds every unfolded options list it did not land on, before the header controls and the
  consoles' own routing take it, and goes on to what it was aimed at.
- **The field is painted, not egui's text field**, and that keeps every key rule above: a text
  field takes every key it is given, and this decision gives every key that is not the search's to
  the program, which closes the search. So the query is still edited with the keys listed above;
  clicking inside it does not move a caret, and input-method composition is not offered.
- **Three options, belonging to the console** and kept while it lives (not across a restart):
  **Match case** — off by default, and **off means case is ignored always**: the engine's own rule,
  case-sensitive the moment the query holds an uppercase letter, is retired, because a checkbox
  that says «off» must not be overruled by what was typed. It is set with an inline flag ahead of
  the escaped literal. **Whole word only** — a match counts only when each end that is itself a
  letter, digit or `_` (in any script) does not touch another one on its logical line — the
  boundary rule of `\b` and of the reference editors, so `foo(` is a whole word in `foo(bar)`;
  checked on each match the engine
  returns rather than with `\b`, because an ASCII boundary takes every non-ASCII character for a
  non-word one and the engine's lazy automata do not take a Unicode one. The check steps over
  wide-character spacers and follows wraps, since the engine's two walks leave a match's ends on
  different halves of a wide character. A rejected candidate is skipped **inside the engine's own
  walk**, not by asking the engine again past it: the engine's step rescans from the start of the
  origin's logical line every time, and stepping past rejections that way was quadratic — over a
  history that is one wrapped line, minutes for one press. Whole word is judged before wrapping, so
  «no more matches» is only said of a match that counts. Matches do not overlap, as the engine's
  never do: a rejected candidate's cells are not searched again, so a whole word that overlaps a
  rejected one — `1.1` inside `v1.1.1` — is found walking one way and missed walking the other, and
  marking (which walks rightward) may not light the match a step older selected. Accepted: it needs
  a query that overlaps itself. **Wrap around** — on by default, which is what
  the search did; off, a step past the last match says «no more matches» and moves nothing.
  Changing an option re-marks the screen and takes back what the box said, as editing the query
  does.
- **Whole word costs one walk, like a query that is not there.** Measured with `cargo run --release
  --example search -p term-core` (3 549 rows of 180 columns, two runs of twenty presses each way,
  18 November 2026): a press that finds nothing **11.4–12.2 ms** at its median as before; a
  whole-word query rejected on every line **14.2–14.9 ms**, slowest 16.1; the same over one line
  wrapped across the whole history **14.1–14.5 ms**, slowest 16.4. One frame at 60 Hz once per
  press, never per frame — the trade above, unchanged in kind. The first version stepped past each
  rejection with a fresh engine call: **33–42 ms** a press over lines, and over the wrapped line a
  run of twenty presses did not finish in ten minutes.

| Discarded (#120) | Why |
|---|---|
| egui's text field inside a floating area | It takes every key, so arrows, `Ctrl+C` and the rest would edit the query instead of reaching the program and closing the search |
| The field inside the console header | The ticket asked for a floating box; the header keeps 30 points of title and hides its controls on an unfocused console |
| Keeping the smart case as the unchecked state | Tilix, GNOME Terminal and VS Code's find read an unchecked box as insensitive; under the smart rule «Needle» would be case-sensitive with the box saying otherwise |
| `\b` or `(?-u:\b)` in the pattern for whole word | ASCII boundaries match `日本` inside `日本語`; Unicode boundaries are not available to the engine's lazy automata over non-ASCII text |
| Only moving the box, never scrolling | Walking older the engine lands a match on the top row, so the box would jump on nearly every press and the next click on «next» would fall on the console |
| Only scrolling, never moving the box | At the oldest line nothing can be scrolled, and a match on the live screen must not take the view away from where the program writes |
| Dropping «Wrap around» because the engine always wraps | The ticket asks for it; off is decided after the engine answers, by where the match lies against the step's start |


### Amended 19 November 2026 — query navigation and insertion caret (#149)

The floating search looked like an editable field, but arrows were deliberately handed to the
program and closed search. The user's report supersedes that part of D61: while search owns input,
Left/Right move the insertion point, Home/Up and End/Down reach the ends, Ctrl/Alt+Left/Right move
by whitespace-delimited words, and Delete removes ahead of the insertion point. On macOS,
Command+Left/Right reach the ends. PageUp/PageDown are consumed by the single-line field. Global
configured shortcuts retain precedence; Enter/Shift+Enter still step matches and unrelated program
keys still close search and reach the program. Shift movement moves the insertion point without
adding text selection; mouse caret placement and text selection are outside this change.

Typing and paste insert at a UTF-8 boundary; Backspace and the existing word deletion act before
that position. Pure navigation keeps the compiled search and result. The query is painted without
synthetic text and a separate vertical caret follows galley geometry, staying visible when clipped.
It uses the existing focus-aware blink and its 600 ms halves, 15-second idle stop and measurement
ceiling. The terminal cursor is an outline while search owns input. The pane's existing blink
outcome includes the visible focused search caret in both blink halves, so no new alarm is needed;
opening or focusing search by mouse resets the same anchor. Existing keyboard and console paint
spans account for this work; no new frame-cost figure is claimed.

Discarded: suppressing arrows without moving an insertion point cannot deliver query editing;
a general TextEdit would also change interrupt and global-shortcut ownership; an independent timer
would duplicate the existing focus, idle and measurement gates. The earlier exclusions above
remain historical rationale and are superseded only by this amendment.

### Amended 20 November 2026 — regular expressions, in the console and in Files (#121)

This supersedes, above: the «Regular expressions» row of the discarded table and «take regular
expressions» among what the search does not do; the «Literal, not a pattern» paragraph's reason; in
#120's amendment, case «set with an inline flag ahead of the escaped literal», the rejected candidate
«skipped inside the engine's own walk», the rescanning and the spacer-stepping that explained, and
the accepted miss of a whole word overlapping a rejected candidate; and the second half of the
`\b` row of #120's discarded table — the meta engine does take a Unicode `\b`, and whole word stays
a check on the match only so that a literal and a pattern follow the same rule.

- **A fourth option, Regular expression**, in the console's options list, off by default and kept
  with the other three. Off, the query is literal exactly as before. **The Files name search gets the
  same choice** as a `.*` toggle beside its field, matched against each entry's name, case-insensitive
  as that search always was; an inline `(?-i)` makes it sensitive. Both modes now go through the
  same engine as the console, so Files' text search folds case the engine's way instead of
  lowercasing: at the edges of Unicode a listing changes — `i` no longer finds `İzmir`, `σ` now
  finds `ΟΔΟΣ`. A pattern's own spaces are part of it; text is still trimmed.
- **The engine that gave up is no longer used.** `alacritty_terminal`'s search runs a lazy DFA that,
  on a pattern whose states do not fit its cache, clears it three times and stops, and the crate
  turns that into «no match» with only a log line — which made «not found» and «gave up»
  indistinguishable. The search now extracts each **logical line** (rows joined through a wrap) into
  text with a map back to its cells, and runs `regex-automata`'s meta engine on it, which falls back
  from its lazy DFA to engines that never give up. **Literal and regex share this one path**; a literal
  is the escaped query.
- **What the person is told instead of «no matches»**: a pattern that does not parse shows the
  parser's message; a pattern too complex to build says so. The box's field leaves about twenty
  cells for that, so where the whole of it does not fit its short form is shown — «invalid
  pattern», «too complex» — and a refusal is never the part that gives way to the query. The build limit is 2 MiB of automaton,
  the same the old engine used, and it refuses short patterns with large Unicode counts — `\w{50}`
  (2.8 MB) is refused.
- **What stays as D61 decided**: the match is the selection, the step starts one cell past it, wraps
  are followed and hard line breaks are never crossed, wide characters take their two cells, Whole
  word keeps its rule, and marking walks only `marking_bounds()` — the screen and at most
  `MARKING_REACH` lines beyond it, one logical line at a time.
- **What changes, declared**: walking older, a match is now chosen left-to-right inside its line, as
  marking always chose it — `aa` over `aaa` selects the first two cells where it used to select the
  last two. The row's padding is still part of the text, so `$` matches after it, as before. **Case
  is set on the engine's syntax**, so a pattern's own `(?i)` or `(?-i)` overrides the box. **A
  candidate whole word rejects gives its characters back**: the search goes on one character past
  where the candidate started, so `1.1` at the end of `v1.1.1` is now found walking either way and
  marked, where #120 accepted missing it one way. Retrying past a rejected candidate skips the rest of a run of word
  characters, so a long run is read once; a candidate rejected only at its end is retried one
  character on, and a pattern built for that (`-+\w` over a long run of dashes) is quadratic — one
  more hostile pattern, accepted as below. **Marking reads one row past each of its cuts**
  as context it never marks, so a line cut by `MARKING_REACH` is not taken to start or end at the
  cut by whole word, `^`, `$` or `\b`.
- **A hostile pattern is accepted as a pause, by the user's decision — asked twice.** A pattern
  with a large count over text that varies (`\p{L}.{170}~` over random letters, digits and a wide
  character) is the engine's slowest case: **0.32–0.33 s a press** at the median over a full history of lines,
  **3.2–3.4 s** (slowest 4.5 s) over one logical line wrapped across the whole history, and **3–13 ms
  of marking per frame** over lines, **70–100 ms** over that one line (median to slowest, one run) — for as long as the search stays open, paid
  while typing and while output streams, not only on `Enter`. The person was asked first with the
  design's estimate (0.6 s, measured over a synthetic grid) and again with these figures, and kept
  the answer: a pattern like that is not what this terminal is for. Over repeated text the same
  pattern costs no more than an ordinary query, which is why the bench draws random text for it.
  Discarded: a per-press deadline and cutting a huge line into chunks — chunks would bring back the
  silent miss for a match across a cut, which is what this amendment removes; and a per-frame marking
  budget, which would also need the huge line cut to bound it.
- **Measured** with `cargo run --release --example search -p term-core` (3 549 rows of 180 columns,
  twenty presses each way, three runs — the last after the review's fixes — 20 November 2026; ranges are
  medians across runs). A press that finds nothing: **2.81–3.14 ms**
  walking older, **3.48–3.90 ms** newer — down from 11.4–13.6 ms, because the engine reads each line
  as text instead of a byte per cell through the grid. Whole word rejected on every line
  **4.47–4.64 ms** older, **5.60–5.71 ms** newer; over one wrapped line **7.14–8.10 ms**. An ordinary
  pattern that finds nothing (`absent\s+\d+`) **2.69–2.88 ms**. The hostile figures are above.
- **Measured in the window** (batch `E` against its control `C`, 20 November 2026, runs interleaved
  C·E·C·E, 1280×800, both consoles dumping 44 MiB, release; `compare` says each pair can be
  averaged; a first `E` was thrown away by the script's own guard — no panel was published — and
  the next run of the same binary was valid). Marking: **44.8–58.9 µs a painting** with a search
  open, against **55.0 µs** for one run of `E` on `main` the same day and D61's 59.7–64.5 µs — no
  measurable change. A press: **23.1–35.4 µs** at the mean, slowest 96–189 µs (`main`: 34.6 µs,
  slowest 396 µs). The left console's enqueueing span reads 270–586 µs in `E` against 552.6 µs in
  both `C` runs; `E` runs several times more frames, so — as D61 already warned — its per-frame means
  do not compare with `C`'s.

| Discarded (#121) | Why |
|---|---|
| Keeping the old engine and checking with the meta engine when it says «nothing» | Giving up stops the old walk midway, so a press can also return an earlier, wrapped match or mark half a screen — not only «nothing» |
| The old engine for literals, the meta engine for patterns | Two walks with two sets of wide-character and wrap rules, so the same text could be found in one mode and not the other |
| Removing the old engine's give-up limit | Trades a false «no match» for a window frozen for as long as the cache keeps thrashing |
| Patching `alacritty_terminal` to expose the error | A fork carried until upstream takes it |

---

## D62 — Pane controls share the strip's geometry and own their entire gesture

**Decision.** The existing strip gains Split right, Split down and Show/Hide information panel
on the left; close stays on the right and the centred grip keeps its space. The controls act on
their pane by name. Splitting successfully focuses the newborn; toggling leaves focus where it was.
An unavailable split is visibly disabled using the same minimum-size rule execution checks.
(The integrated header omits an unavailable split instead, by **D99**; this strip keeps the rule.)

**Why a press needs an owner until release.** D46 could rely on a closed widget disappearing before
egui starts its drag. A panel toggle leaves that widget alive, and a held control press would
become a pane move on a later frame. The header retains ownership through release, including a
quick press and release in one frame. Releases for earlier program gestures still reach their owner.

**Narrow panes use a compact actions menu.** Resizing can shrink an existing pane below the split
minimum. Before three buttons would reach the grip, one menu button replaces them. If even that
button cannot coexist with the grip, a right-click on the strip opens the same menu. Menu actions
retain their pane name, and menu clicks, dismissal and navigation keys do not also reach the
program underneath. The last console still has no close button (D46).

**Hiding is presentation, not retirement.** Toggle changes only the pane's saved folding value;
its width, producer connection and document remain. Availability follows existing panel visibility,
including a warning-only panel: a failed read must not make a visible panel impossible to hide.

**What was discarded.** A second row would take another line from every console. Squeezing all
buttons at narrow widths would overlap the grip or destructive target. A permanent menu would hide
frequent actions at ordinary widths. Temporarily focusing a pane to invoke its shortcut would move
the keyboard even when the operation fails. Separate painting and hit geometry would let the two
locations disagree. New visibility fields would duplicate what the saved layout already carries.

**Measurement.** D45's opt-in and published control grids remain. Header control geometry,
painting and hover work belong to the header span; pointer decisions and their mutations belong to
the already-timed mouse pass. A split invoked there does not also subtract the shortcut's timing.
The compact menu's painting is part of the header span; its key handling is timed separately.
No cost claim is made until the measurement runs are valid and recorded.


### Measured — 7 November 2026: the strip with its controls

Two valid T runs against two C controls, all from release binary `0393cb4`, one tab, two consoles,
44 MiB per console and a 1280×800-point window. The environment was Xvfb with
`LIBGL_ALWAYS_SOFTWARE=1`; these are software-renderer measurements, not a comparison with the
hardware-rendered figures in D45. The comparator accepted both same-batch pairs.

| Batch/report | Grid per console | Header cost per frame, both strips |
|---|---|---|
| T / batch-T-190839.txt | 48×43 | 9.671µs |
| T / batch-T-190934.txt | 48×43 | 10.652µs |
| C / batch-C-190903.txt | 50×43 | Not painted (measurement gate) |
| C / batch-C-191003.txt | 50×43 | Not painted (measurement gate) |

T includes both strips, their three control icons, availability geometry and hover registration.
C paints no strip. T and C have different grids, so subtracting their whole-frame means would not
isolate the controls. The menu and clicks were exercised functionally; this batch does not measure
an open menu or continuous clicking, and these figures do not claim that cost.


**Gesture brightness clarification (9 November 2026, Ivy #13).** Retaining a pane-header
press until its release blocks interaction without adding a disabled appearance to the shared
console painter. egui's `Ui::disable` both denies interaction and multiplies painter opacity;
the input guard therefore preserves the opacity that existed before disabling. This keeps
close and information-panel controls from briefly fading unrelated consoles, while preserving
existing modal opacity and the complete gesture ownership required above.

Discarded: removing the input guard (release could reach a console or another control), forcing
opacity to one (would erase an intentional inherited fade), and changing the global disabled
style (would change the appearance of genuinely unavailable controls). The regression observes
rendered glyph vertex colors during press, hold, release and idle; text presence alone cannot
prove stable brightness. Native desktop observation remains separate from that rendering proof.

## D63 — Graphical settings stage the shared file and apply on restart

**The restart-only Save boundary is superseded by D64 when #324 is delivered.** The staged editing, shared-file preservation and historical measurements below remain their original record.

**7 November 2026 — F2.15, [#322](https://github.com/mashware/ivy-history-private/issues/322).** Settings
belongs to the whole window, so its entry is global and visible with one tab. Its dialog groups
appearance, cursor and shortcuts, while every console keeps its existing pump budget.

**One boundary for every setting.** Open reads a draft of the saved configuration. Cancel discards
it, Restore defaults stages removal, and Save validates and writes. Every saved change applies at
restart, stated before and after saving. Startup merges settings and registers fonts before the
first frame; reusing that path avoids inventing partial live reset and font-activation rules.

**The plain-text file remains shared with hand editing.** Patch only edited semantic keys, removing
all their assignments before writing a single replacement. Unsetting/defaults remove aliases and
invalid assignments too, so an older value cannot reappear. Preserve comments, unrelated lines,
untouched entries and retained line endings. Reuse the startup parser's values and chord rules;
show conflicts instead of silently displacing a binding. Copy remains its independent switch.

New or blank saved configurations declare `version=1`; existing version declarations survive.

**A failed Save is not success.** Snapshot comparison detects observed external edits; a sibling
exclusive temporary file is flushed and replaced only after rechecking. Failure preserves the old
file and retains the draft. Existing permissions survive. Non-regular files, symlinks and whole-file
vetoes disable graphical saving, rather than silently overwriting unknown contents or a link.
An arbitrary editor can still write between comparison and rename; no atomic compare-and-swap
across unrelated editors is claimed.

**Input belongs to the dialog before terminal dispatch.** Ownership covers opening and closing
frames, repeated passes and a held pointer's release. Releases of earlier program-owned presses
remain routable through the existing header interception. The entry does not take keyboard focus
from the shell; typing Tab then Enter in a console must remain terminal input. Header actions recheck
the same content rectangle as shortcuts after resize; a cached enabled menu item is not a witness
of the space available now.

**Work is named.** Entry, editor, configuration I/O/validation, font enumeration and input ownership
accumulate across passes and close once per frame; child spans are excluded from parent spans and
the combined work is discounted once. The global row is an explicit exception to D13's single-tab
geometry; historical batches keep their old regime. Dedicated settings variants witness the shipped
visible entry, the open dialog and first/repeated font-selector requests at matched geometry. They
never lift the user's config veto. Font selection enumerates only on request and registers nothing
until startup. Real desktop validation remains separate from headless interaction and software-renderer
measurements.

**Discarded alternatives.** Per-pane settings buttons repeat a global action in every pane. A hidden
shortcut alone fails one-tab discoverability. A separate workspace adds navigation. Full live
application needs a resettable settings/font lifecycle; mixed application makes Save ambiguous.
Rewriting the entire file loses human text, appending cannot restore inherited values, and a second
store disagrees with hand editing. Advisory locks do not coordinate arbitrary text editors;
platform-specific compare-and-swap would add portability work without changing that editor contract.


### Measured — declared settings work

**7 November 2026.** Release binary SHA-256
`e93dcd07ddad140f7b09a61554f8de17545a9ebb6547c968f4746d7721af3fb1`, built from the
`e944242` tree. Two runs each of `scripts/measure-frame.sh C 44`, `settings-closed 44`,
`settings-open 44` and `settings-fonts 44`, on the same machine and a 1280×800-point Xvfb display
with software rendering (`glow`). Every pair passes the script's `compare` command; none is
marked invalid. These are virtual-display measurements, not desktop keyboard/clipboard evidence.

The table gives **the range of the two reported per-frame means** for each separately declared
span; discovery has one sample per run. The editor excludes its separately named children.

| Regime | Global entry | Editor | Config I/O / validation | Font enumeration | Input ownership |
|---|---|---|---|---|---|
| Historical `C` | 0 | 0 | 0 | 0 | 0 |
| `settings-closed` | 34.015–34.626 µs | 0 | 0 | 0 | 0.564–0.713 µs |
| `settings-open` | 37.467–43.963 µs | 291.510–307.002 µs | 5.608–6.227 µs | 0 | 1.640–1.816 µs |
| `settings-fonts` | 41.489–41.529 µs | 312.000–312.513 µs | 6.224–6.419 µs | 29.098–30.386 ms, once | 1.763–1.805 µs |

Each fonts run accepts **two explicit requests and enumerates once**, retaining 259 family names;
opening the dialog alone discovers none. This script does not expand the family dropdown, so the
cost of painting the complete inventory is **not measured**. Save and real configuration reads are
also not measured: the configuration veto remains active. Explicit Open/Save/discovery have no
latency bound established here.

The historical control has 50×43 console grids; all settings variants have 48×43. The new global
row changes geometry, so whole-frame subtraction against `C` cannot isolate its cost. The named
spans above are used directly. The report pairs are `214328/214334` (C), `214404/214435` (closed),
`214504/214536` (open) and `214606/214636` (fonts), under `target/measurements` with their batch
prefixes. One final publisher write races directory removal at shutdown in the last run; both
panels had already published and the report passes every witness guard. No failed run is used.


## D64 — Saved settings replace the complete runtime configuration

**7 November 2026 — [#324](https://github.com/mashware/ivy-history-private/issues/324).** The user approved
the Compact Settings proposal and asked to remove the restart required by D63. The global gear
shares the tab strip, including a single-tab window; the system titlebar remains native.

**Save is the application boundary.** Opening, editing and Cancel change neither the active settings
nor the file. Restore defaults stages removal. Save resolves the complete document against factory
defaults, prepares any changed font assets, persists through the existing Store and only then commits
the prepared runtime settings. A failed validation, new-font preparation or persistence leaves both
file and active settings unchanged. The non-cooperating editor race in D63 remains an explicit limit.
No second settings store or automatic watcher is introduced.

**Omission means reset, not retention.** Palette/theme, font size, cursor ceiling, copying and a fresh
default keymap are resolved as at startup; applying only present overrides leaves old values active
when somebody removes them. Changing or clearing the selected family must likewise replace the old
choice. An unchanged family request reuses its current active result, including an existing startup
fallback, so changing a colour cannot become an unrelated failed font lookup.

**Prepared fonts do not become active before Save.** A newly selected missing/unreadable family
blocks persistence with a visible error. After persistence, keep the chosen assets pending through
that pass's ordinary fallback discovery; queue the final combined definitions last so another writer
cannot overwrite the selection. Commit the selected family, coverage/readiness and complete settings
at the next begin-pass, before measuring cells. Keep fallback faces and unresolved characters; reset
old-family coverage. Request repaint through the next logic round so quiet visible and hidden panes
receive their new grids. Programs, identities, histories, panels and pump budgets survive.

**Presentation and evidence remain explicit.** Categories, theme samples, colour swatches and a
collapsed ANSI palette replace the raw form. Draft controls do not apply to running consoles. New
preparation/commit work has named spans; work performed inside egui outside an app stopwatch is not
claimed as an isolated app duration. Compact and historical chrome regimes cannot be averaged as if
their geometry were identical. Config-free apply probes retain the configuration I/O veto and never
claim to measure persistence; actual desktop checks use temporary settings.

**Discarded alternatives.** Retaining restart-only application contradicts the approved scope.
Overlaying changed fields cannot restore omitted defaults. Broad startup reconstruction couples
one-time resources to updates unnecessarily; rebuilding consoles would additionally destroy running
programs. Applying before persistence violates Cancel/failure semantics, while fallible file/font
work after persistence can leave a saved but unusable configuration. A custom titlebar, external-file
watcher, new keys and a parallel settings store are outside this delivery.


### Measured — compact Settings and transient application

**8 November 2026.** Release tree `621e679`, binary SHA-256
`23fa8fcdd7434fd28bc2d233de78645fe2b339175e30ccf03cee0483c03aae4e`.
Two runs each of `scripts/measure-frame.sh C 44`, `settings-closed 44`, `settings-open 44`,
`settings-fonts 44` and `settings-apply 44` (the apply run sets
`MEASURE_SETTINGS_FAMILY='DejaVu Sans Mono'`), on a 1280×800-point Xvfb display using software
rendering and `glow`. All ten runs are valid and all five same-variant pairs pass `compare`.
These are virtual-display performance results; native desktop functionality was confirmed
separately by the user, with a screenshot.

Ranges below are the two separately declared per-frame means; enumeration occurs once.
The entry is the gear; card painting remains in the existing tab-bar span. Editor work excludes
its separately timed validation and enumeration children. No whole-frame subtraction across
historical and compact chrome regimes is used.

| Regime | Entry | Editor | Family enumeration | Chrome partition |
|---|---|---|---|---|
| Historical C | 0 | 0 | 0 | 0.096–0.139 µs |
| Closed | 12.626–15.783 µs | 0 | 0 | 0.095–0.099 µs |
| Open | 14.495–14.749 µs | 325.587–328.407 µs | 0 | 0.087–0.118 µs |
| Font selector | 13.479–14.385 µs | 324.118–333.801 µs | 29.214–32.239 ms, once | 0.097–0.100 µs |

Each selector run accepts two requests and enumerates once. It does not open the complete family
dropdown, so full-inventory rendering is not measured. Opening alone enumerates none.

The transient apply probe uses two tabs/four consoles and the explicitly requested canonical
family **DejaVu Sans Mono**. Each run observes four changed kernel-independent screen grids,
six settings commits, three font-bearing commits, three queued definitions, four preparation
attempts and one intended missing-family failure. Colour, changed size, active family,
unchanged-family reuse, reset and repeated selection all have positive effect witnesses.
The measurement blink ceiling remains off and persistence remains vetoed throughout.

| Direct preparation attempt | Range across the two runs |
|---|---|
| Missing family, including the first collection discovery | 29.445–31.281 ms |
| First selected face, collection already warm | 0.863–1.004 ms |
| Clear selected face | 0.370–0.601 µs |
| Repeated selected face | 0.180–0.192 ms |

These attempt durations are **subsets** of the named preparation span, not additional frame
costs. That span also includes definition requests and totals 30.528–32.524 ms over the whole
sequence. The six runtime-commit frames average 2.219–3.221 µs; probe observation averages
11.860–12.803 µs per frame and is measurement-only work. Internal font-engine activation is not
assigned an invented isolated duration. No Save-I/O latency or speedup over D63 is claimed.

Reports under `target/measurements`: C `000616/000635`, closed `000705/000736`, open
`000806/000837`, fonts `000907/000937`, apply `001008/001035` (each with its batch prefix).
The comparator additionally requires matching nonempty effective families for apply reports,
even when grids match; missing-family reports cannot justify an average.

## D65 — Theme browsing stays in the settings draft; palettes are converted before shipping

**Decided for [#327](https://github.com/mashware/ivy-history-private/issues/327).** A named theme remains a
base for all twenty-two colors, with explicit overrides above it (D59). The catalog is still part of
the binary. Additional terminal palettes are converted from a pinned external source during
development, with their original license notices and attribution shipped alongside them. The
application reads no theme files and contacts no theme service at startup or while browsing.

**Why now.** The two-card gallery in Settings does not scale to a useful collection. A compact
selected name and one console-and-panel sample keep Appearance the same size when the catalog grows;
a bounded searchable picker provides All, Dark and Light filters. Keyboard and mouse selection
change only the existing draft. The preview resolves that base plus each valid explicit color
setting, so a person's overrides are visible before saving, even if an unrelated setting is invalid.
The factory/inherit choice remains available. Escape dismisses the picker before Settings.

**Color provenance.** The external sixteen ANSI colors, background, foreground and cursor remain
those of the pinned source. The application-only colors are local choices: the divider moves the
background 25% toward foreground, the panel moves it 5% toward foreground, and focus uses ANSI blue.
These rules apply to the added palettes; the two previously shipped themes keep their exact colors.

**Save and close shares the existing transaction.** Save changes keeps Settings open. Save and
close dismisses it only after validation, font preparation and persistence succeed; failures keep
the draft and error visible. The save operation returns its actual outcome, including across egui's
multiple passes in one frame. The saved pending runtime update survives dismissal and applies through
D64's existing begin-pass boundary without restarting any console.

| Discarded | Why |
|---|---|
| A permanently expanded gallery | Its height grows with the catalog and pushes everyday font/color controls away. |
| A dropdown containing only names | It hides the palette and makes discovering unfamiliar themes harder. |
| Runtime theme imports or a plugin ecosystem | Search paths, file errors and downloaded executable plugins are not needed for this offline collection; imports remain a separate product step. |
| Changing the running terminal while browsing | Cancel must discard the draft; browsing must not affect running consoles. |
| Clearing overrides on theme selection | A base selection must not silently erase personal colors (D59). |
| Inferring save success from the notice text | Notices describe outcomes; the actual transaction result must decide whether Settings closes. |

## D66 — Integrated, theme-derived window chrome and transient pane maximization

**8 November 2026 — #326.** The user approved the interactive HTML prototype: one global titlebar carries split-right/down, new tab, search and the application menu; tabs sit below; each pane carries an explicit label, information-panel toggle, maximize/restore and close. This approval is the new signal superseding D64's exclusion of custom titlebars. (Amended 18 November 2026 by **D99**: each pane also carries split right and split down. Amended 18 November 2026 by **D61**, #120: search moved from the titlebar to each pane's header.)

Every chrome surface and ink derives from the effective configured Theme after named-template selection and explicit overrides. Live Save updates chrome alongside terminal colors. Copied prototype RGB literals and an independent chrome theme are discarded because they would ignore the user's settings.

Maximization is a transient view of a stable pane identity. The weighted tree and saved geometry remain unchanged; restoring reveals the original order and proportions. All consoles retain full pump budgets. Hidden siblings use their original tree geometry for resizing and panel refresh; they are not painted into collapsed rectangles. A zoom target becomes the keyboard target. Structural operations reconcile transient view state.

Pane labels are explicit inert user data, never inferred from a TUI or used as process arguments. Optional bounded label metadata extends the v2 session format without changing geometry or process launch; old readers may discard labels on saving. No live process identity is persisted.

Native window gestures use viewport commands and explicit edge/corner resize targets. Platform behavior requires real desktop checks, and unverified platforms remain unverified. Historical measurement regimes keep their geometry and native decorations; integrated chrome is opt-in for measurement and its work has a named nonoverlapping span. No performance claim is made from source inspection or headless tests.

Discarded: rebuilding the tree for zoom (loses restoration), reducing hidden pump budgets (blocks programs), deriving names/state from output (D6), separate hard-coded chrome colors (ignores configuration), and relabelling historical measurements as integrated chrome (D13).

**10 November 2026 — #34, the first sight of it on a Mac.** The integrated titlebar keeps macOS's own close, minimize and zoom buttons, and on a real Mac two things were wrong: AppKit still drew the window title over the header's own logo and name, and it centred the buttons in its ~28-point titlebar, about ten points above the header's icons. The title is hidden (`titleVisibility`), and the buttons are moved into the header: AppKit's titlebar container is grown to the header's height and each button centred in it, on a whole device pixel. AppKit lays that container out again on its own, so the frames are compared every pass, written only when they differ, and a correction asks for one more pass, a few in a row at most. Full screen is left to AppKit, and any hierarchy other than the expected one leaves the buttons alone. The work is chrome work and is timed in its span (D13). Confirmed by the owner on a real Mac with the 0.1.0-alpha.4 DMG: one title, the buttons level with the header's icons, the header's own buttons clicking across its whole height, a double-click that zooms once, the hover glyphs, and the layout holding through resize, focus changes and full screen.

Discarded: a unified `NSToolbar`, which centres the buttons by itself but whose view hit-tests above the content and would take the clicks meant for the header's buttons; shrinking the header to AppKit's 28 points on macOS only, which would make the approved header differ per platform — kept as the fallback if the Mac shows that the grown container takes those clicks; and observing AppKit's layout notifications, which needs an Objective-C observer for a correction a compare-on-every-pass already makes.

## D67 — Appearance groups a theme with its overrides and keeps saving in a fixed footer

**Decided for [#331](https://github.com/mashware/ivy-history-private/issues/331)** after the user approved
`prototypes/settings-appearance.html`. Theme selection and explicit terminal colors are one visual
group. The selected theme shows how many colors are customized; the editable colors stay behind
Customize colors, with the ANSI palette still expandable inside it. The effective console/panel
sample sits beside the selector at wide widths and stacks at compact widths. Typography follows.

**Why.** The previous screen presented a theme and its color overrides as unrelated sections. A
custom purple background survived selecting Tokyo Night, correctly under D59, but the screen did
not make the relationship clear. Its sample consumed space needed by controls, and the bottom
helper text could be clipped. The approved design establishes one hierarchy and bounded proportions.

The main-column footer reserves space for its actions and bounded notices before assigning height
to scrollable settings content. Save and close is primary; Save changes remains secondary and keeps
the dialog open. Both retain D64/D65's existing transaction and failure behavior. Navigation adapts
to compact widths; font discovery remains lazy. This is presentation, not a configuration change.

| Discarded | Why |
|---|---|
| Separate Theme and Terminal colors sections | Hides that explicit colors override the selected base. |
| Clear overrides automatically when choosing a theme | Would erase personal choices; D59 remains unchanged. |
| Large full-width preview and permanently expanded swatches | Pushes routine font controls out of view. |
| Footer and notices sized after an unconstrained content row | Long content or errors can place save controls outside the viewport. |
| New appearance direction during implementation | The concrete HTML hierarchy and proportions were explicitly selected by the user. |

**Typography preview after the native review.** A specimen that stays at one built-in face and
size cannot show what choosing a font does. It now uses the valid draft point size and an isolated
font family. Opening Settings reuses the active face without enumerating the machine; an explicit
font selection prepares and caches the specimen face. The terminal's live family and icon chain
remain unchanged until Save. Every definitions rebuild includes current live faces and the preview,
so fallback discovery or a pending save cannot erase either. Cancel drops only specimen assets and
recomposes current definitions; it never restores an old snapshot. Font-list arrows change the draft
and keep its row visible; Enter closes that list without saving.

Rejected: activating the terminal's font to draw a draft sample (breaks Cancel), restoring a captured
font-definition snapshot (loses later discoveries), and reloading the font file on every frame
(unnecessary I/O). The native review also chose readable15-point body text, aligned40-point font/size
controls, a compact20-point override badge, and padded cursor/shortcut controls.

> **Superseded in part by D98 (November 2026).** The Settings half of the paragraph below no longer
> holds: Settings is painted with the roles of the theme it previews, like the chrome's dialogs.

**Integration with D66.** The shared chrome dialogs, window controls and pane controls retain their
effective-Theme-derived style. The separately approved Settings presentation keeps its neutral editor
palette, including its popups, so selecting a console theme does not silently replace the approved
Settings design. The console/panel preview still shows the draft theme and explicit overrides.

## D68 — Overview projects live panes and preserves console geometry

> **Superseded by D85 (November 2026).** The Overview screen is replaced by the board. Its input
> rules — activation by stable pane name, vanished targets, maximized layouts, no PTY geometry
> change — carry over to the board unchanged; the excerpt of three panel lines does not.

**Decided.** Overview is a read-only window screen over the console area. Each live pane appears
once, identified by its existing stable name and its current tab. Its display name is the name
already used by the console strip. The first three nonempty producer lines are shown literally;
neither the phase nor body marks are used to infer a task's state (D6/D18).

Ordering reuses AttentionState: wait before block before done, a backed claim before an unbacked
claim of the same class, then current tab and tree order. A missing attention value says no claim;
a missing document says no published state. Existing warnings and the done silence exception apply.
Freshness deadlines include background and folded panes while Overview is visible because their
warnings are then visible too; closing it restores the existing narrow alarm scope.

**Input belongs to the screen for the entire opening and closing frame.** Selection and pointer
activation retain stable pane names through list reordering. Activation resolves the pane's current
tab, restores a maximized arrangement when necessary and focuses that pane; a vanished target never
redirects to its former neighbour. The screen exposes no producer actions or active links. Settings
and Overview cannot open over one another; opening or closing Overview changes no PTY geometry,
panel ownership, search, selection or saved layout. Every console keeps its full pump budget.

The global control and configurable `key.overview = ctrl+shift+o` open the same view; `none` gives
the chord back to the program when the view is closed. Fixed-height scroll rows have bounded slots
for names, phase, warnings and three excerpt lines so long valid metadata cannot expand one row
over its neighbours. The active runtime theme supplies all colours.

**What was discarded, and why:**
- A separate task registry or queued-task board: the existing pane is the identity and state unit;
  queuing work without a console is outside Phase 4.
- A drawer or tab-card enhancement alone: the roadmap promises a global screen and the panel body
  explains what a claim is about; a tab's single maximum cannot show every console.
- Replacing the console layout: hiding it beneath a screen preserves its current grids, refresh
  paths and measurements instead of resizing running programs just to inspect their state.
- A second watcher, polling loop or summary cache: existing panel state, watcher and deadlines
  already provide the required transitions.

Overview's projection, input and paint-enqueue work is named in the frame report and subtracted
exactly once, including repeated passes. New matched open/closed batches declare their chrome
regime and witness the view's actual effects; they do not change historical batches. No performance
number is claimed before valid repeated measurements exist. Cross-platform proof remains F2.7.

## D69 — A pane without a manual name follows its program's terminal title

**Decided for [#334](https://github.com/mashware/ivy-history-private/issues/334).** The user wants pane
names to follow agent session names and explicitly accepted the terminal title each client publishes,
including its prefixes and shortening. This is the new signal extending D66's manual-only display
labels: standard OSC title metadata is an explicit declaration, not meaning extracted from a TUI.

An existing nonblank manual label wins. Without one, the pane uses its latest usable terminal title, then
`Console N` when no title is available. The header and Overview use the same rule. Naming a pane
manually selects the override; clearing it or choosing **Use terminal title** resumes following.
Only manual labels are saved. A restored console starts with a fresh title, because the application
starts a new program rather than resuming the previous agent session.

The existing VT parser supplies title and reset events. Received titles are inert text, trimmed,
limited to 128 Unicode scalars and refused when they carry control characters. An empty or unusable
latest title clears the automatic value. Nothing changes stable pane identity, attention, commands,
tab names, provider credentials or provider settings. Other programs, including a shell, can publish
titles too; this is no claim that a title authenticates an agent or a session.

Codex needs its terminal title to include the thread; Claude Code can publish its renamed session;
OpenCode can decorate and shorten it. Client configuration and actual version compatibility are
documented and tested separately. A silent exit or a terminal reset that produces no library title
event cannot establish that a session ended: the last observed title remains until replaced or the
console is recreated. In particular, the pinned engine's RIS handler clears its private title without
emitting `ResetTitle`; this change does not infer a reset from an unrelated event.

**Discarded, with reasons:**
- Reading screen painting or private session files: it would reconstruct meaning from a client
  implementation, contrary to D6, and couple naming to each client's internal storage.
- Client-specific adapters or model-authored names for this delivery: the user accepted standard
  terminal titles, and an agent may neither know the canonical name nor observe an external rename.
- A new persisted follow-mode flag: the existing optional manual label already supplies the choice.
- Saving observed titles: a newly launched shell would inherit the name of a session it did not resume.
- Stripping client prefixes or guessing process lifecycle: a terminal title is the literal value the
  producer chose; transforming it would introduce client-specific semantics that the terminal lacks.

**Amended for [#227](https://github.com/mashware/ivy/issues/227), 23 November 2026.**
The header's folder/program fallback hid an AI-published title whenever the separate activity
integration had no declaration, while Rename still showed that title. A usable published title
therefore takes precedence over the folder/program fallback without requiring an activity signal.
This includes shell-published titles: the terminal has no authenticated producer identity for OSC
0/2. Requiring activity signals was discarded because naming and activity are independent;
guessing the producer from title text or executable names would violate D6. Manual names still win.

Title events are coalesced into one latest-update slot and consumed within the existing parser
span. Name resolution belongs inside the existing chrome and Overview spans. There is no new pump,
watcher or lower budget for hidden panes, and no new performance claim without valid measurement.



## D70 — Optional programming ligatures reuse the font shaper without changing terminal cells

**Design for [#335](https://github.com/mashware/ivy-history-private/issues/335), 8 November 2026.** The user
explicitly requested programming ligatures after discussing the terminal's adoption priorities. That
is the new signal revisiting D19's exclusion for this feature alone; it neither changes the scope
of a completed Phase 2 milestone nor introduces Kitty's graphics protocol.

`font.ligatures = on` enables the option; `off` or absence keeps it disabled. Preferences uses the
same configuration key and existing Save/Cancel/live-application transaction. A compatible explicitly
selected monospaced font supplies the actual drawing. The terminal does not replace source text
with Unicode approximations or select another font on the user's behalf.

The console painter shapes bounded same-style printable ASCII runs through epaint's existing
harfrust path and font atlas. A private single-face family uses the already selected font bytes and
collection index, so fallback cannot mix advances inside one shaped run. The normal per-cell path
remains responsible for other characters and incompatible font metrics. Enabling the preference
alone does not enumerate system fonts.

Equivalent canonical selections reuse the existing selected-face allocation during preparation.
Installed aliases can have different requested names but resolve to the same face bytes/index;
epaint deliberately skips equal definitions, so creating a new allocation would make the readiness
identity check refuse a face the engine correctly kept active. Equality is checked once during
selection, never per painted row. A regression prepares the same canonical fixture twice and
asserts that actual shaped drawing continues; a separate transition test covers genuinely changed
font definitions.

The source cells remain authoritative for columns, rows, copy, search, selection and cursor
position. Run boundaries include rows, spaces, color/style changes, selection/search highlights
and the cursor cell. Moving a cursor or changing a highlight may therefore separate a ligature
into its original characters. That is an explicit presentation rule, not a source-text change.

**The grid remains D34's.** Its cell step is still floored to device pixels. Each
visible shaped glyph is translated from its natural pen position to its source cell's origin;
its raster quad keeps its dimensions and atlas texels. Continuation glyphs in a substituted
cluster contribute no duplicate ink. Logical row extents follow the cell grid while ink bounds
include natural ligature overhang. Coverage, fixed isolated advances and the unwrapped row's
expected total advance are checked before this path is used.

The private ligature face disables subpixel raster bins, so splitting a plain-text run at a
selection, search or cursor boundary does not select different raster texels. The normal,
preview and fallback faces keep their existing configuration. Tests compare plain ASCII with
per-cell drawing and split runs across fractional sizes/scales, and separately prove actual
ligature substitutions.

The original horizontal mesh squeeze resampled already rasterized glyphs and let selection
boundaries change their spacing and raster phases. Translation replaces it without changing
terminal cells or selection-color inversion. This geometry regression is independently
reproducible; the original Ubuntu Sans Mono screenshot still needs a desktop comparison.

Epaint's cached galleys are shared. The transformed mesh lives in an owned cloned galley with consistent horizontal bounds and row
placement, submitted as an ordinary text shape. Texel UVs remain unnormalized until epaint's final
tessellation, because later layouts in the same frame may grow the atlas. No cached shared layout
is mutated. There is
no separate text atlas or app-level text cache. Font, size and scale changes use the current layout.
The optional path's processing is declared as a subset of painting, so it is never subtracted twice
from the framework residual. A matched on/off scene supplies the font and witnesses shaped runs;
historical control batches retain their configuration veto and geometry.

**Discarded, with the reason:**
- Natural-width row drawing: its fractional advance accumulates against the terminal's floored step.
- Reconstructing substituted clusters from visible quad counts: continuation glyphs do not each
  emit a quad; placement instead follows the source glyph entries and their vertex ranges.
- Forking epaint to expose cluster metadata: solves that unnecessary intermediate reconstruction.
- Another monochrome rasterizer and atlas: duplicates facilities the selected engine already has.
- Unicode substitutions for `!=` or `=>`: change the characters and cannot represent font features.
- Always-on shaping, implicit font replacement or eager font discovery: ignores the optional nature
  of the request and the user's selected font.

This records the design, not a measured performance claim. Delivery requires the real-font visual,
source-text, grid, runtime-settings and controlled-measurement checks defined in the work's design.


### Measured — controlled ligature on/off scene, 8 November 2026

Four valid post-extraction renderer-build `scripts/measure-frame.sh ligatures-on|ligatures-off 44` runs used the same pinned
JetBrains Mono v2.304 Regular face, 14.7 points, scale 1, two consoles of **42 × 43** cells in a
1280 × 800 window, and the same ASCII programming load of 44 MiB per console. Both same-mode
pairs passed `scripts/measure-frame.sh compare`; on and off are deliberately distinct regimes and
are not averaged together.

**Environment:** Linux, Xvfb display, glow with Mesa software rendering (`LIBGL_ALWAYS_SOFTWARE=1`,
`GALLIUM_DRIVER=llvmpipe`). These are new-batch comparisons in that environment; they do not compare
with the historical native GPU series and prove no desktop input-to-pixel latency.

| Report | Mode | Left console: mean enqueuing paint | Right console: mean enqueuing paint | Shaped runs | Included ligature work, entire run |
|---|---|---|---|---|---|
| `batch-ligatures-off-200710.txt` | off | 383.417 µs | 341.116 µs | 0 | 0.000 µs |
| `batch-ligatures-off-200735.txt` | off | 374.803 µs | 336.898 µs | 0 | 0.000 µs |
| `batch-ligatures-on-200805.txt` | on | 612.016 µs | 526.465 µs | 164,152 | 298,366.326 µs |
| `batch-ligatures-on-200834.txt` | on | 682.318 µs | 520.697 µs | 164,632 | 317,042.714 µs |

The per-console numbers are the existing **enqueuing paint** span, not the complete frame. The
last column is cumulative work already inside painting and is not subtracted a second time.
The on runs report zero fallback runs; the off runs report zero eligible runs, shaped runs and
vertices. This scene therefore exercises the new path while showing that the disabled path pays
no shaping work. Font read/validation/registration is reported separately before the first frame;
cold and steady shaping distributions are **not** separately measured.

One earlier off run was automatically marked `.INVALID` because `XDG_RUNTIME_DIR` was absent and
the panel publisher could not find its directories. It is excluded. Valid runs used the runtime
directory expected by the existing measurement script and retained every publication/configuration
and geometry guard.


The later equal-alias preparation correction is outside this scene's explicit-font registration
path; these reports cover the same unchanged renderer. Alias handoff is covered by its real-font
regression and the full suite, not by a claimed timing for selecting an alias.


## D71 — Ivy is the public product name

**Internal-name retention superseded by D87 on 10 November 2026.**

**Decided for [#339](https://github.com/mashware/ivy-history-private/issues/339).** The project owner
selected **Ivy** and approved the green-to-teal logo with three hanging leaves. Keep that SVG as
the editable source and embed a transparent PNG rendering for the application title bar and native
window icon. The integrated title bar shows the logo beside the name, without moving its controls
or taking space from the consoles. Decode the PNG and create its texture before the first frame;
paint enqueueing stays inside the existing integrated-chrome timing span.

The original branding delivery retained repository, binary, Cargo package, panel contract and
configuration identifiers. A public name must not reset a person's terminal or
break existing scripts. Native task-switcher and launcher icon support belongs to the platform:
X11 and Windows accept the window icon, while Wayland needs desktop-entry integration and macOS
needs an application bundle for the Dock. Packaging those is separate work, not something a
viewport icon alone proves.

**Discarded, with reasons:**
- A global replacement of the former product name: it would mix public branding with persisted paths,
  shell integration and historical measurements, requiring an unrelated migration.
- Loading or rasterizing SVG inside the frame: the approved PNG can be embedded and decoded once,
  without adding an SVG renderer or disk access to the painting path.
- Replacing any controls with branding: window and console actions keep their existing geometry.

## D72 — Document viewers share a transient sidebar and never become consoles

**8 November 2026 — #337; amended for #73 on 15 November 2026, for #93 on 17 November 2026, for
#178 on 20 November 2026 and for #198 on 22 November 2026.**
Markdown and Git changes open in a transient surface beside the consoles, together with Files and
the global Board; since #93 that surface is Files on the left and a drawer below (see the last
section). The
console tree and its running programs retain their identity; while open, Files reserves width from
them and the drawer height. The sidebar is closed on restart and its width and tool state are not persisted.
The user chooses the source explicitly: a local Markdown path, or the review control of a console.
Git opens that console's working directory without a path form. On Linux and macOS, the live
foreground process directory takes precedence over a previous local OSC 7 report, which may predate a
`cd`. Local OSC 7 metadata remains the fallback when native discovery is unavailable, which today is
Windows. Native discovery reports the resolved path (macOS `/tmp` is `/private/tmp`), so a console
inside a symbolic link is shown under its target rather than the name the shell printed.
*Amended for #116, 18 November 2026:* macOS was added because its default zsh sends no OSC 7 outside
Terminal.app, so with OSC 7 as its only source no macOS console had a directory, and Files and project
colours never worked there. Discarded: asking users to enable OSC 7 in their shell, which leaves
the default install broken. An
explicit unusable/remote directory is never replaced with another console's or the app's directory.
The terminal does not infer task state. The viewer has no PTY, producer identity, attention claim or MCP endpoint.
It is not persisted. The view owns its gestures and keyboard input until focus returns to a console;
its dismissal consumes the input that closed it. Tool state remains retained while another tool is
shown. Viewer, Board and sidebar host frame work are separately named and discounted once from the
framework remainder; the host span excludes viewer and Board spans.

**One-click return — 22 November 2026, #214.** A bare primary press on a visible console's body
returns the keyboard from the drawer and focuses that console in the same input pass, before any
typing is dispatched. This also releases a note or review field's editor focus. A gesture begun
inside the drawer stays there until it ends; headers, panels, links, dividers and modal surfaces
keep their own input rules. Discarded: requiring a second click, which makes returning from a note
or review unlike switching consoles; relying on the console's later egui click response, which
cannot choose the destination of text already dispatched earlier in that pass. No new frame span
is introduced: the handoff stays in the existing viewer input span.

Until #93 the sidebar reserved a 36 point navigation rail, a 6 point divider and a preferred 480
point body of at least 320 points. The rail is gone and the geometry is the one the last section
states. What stays: the console tree keeps its metric-derived floor (at least 320 points wide),
a surface that cannot fit beside it is temporarily suppressed with its selection kept and a tooltip
saying how to bring it back, and neither suppression nor resizing changes pane identities, PTYs or
tree weights.

Markdown renders headings, lists, tables and code as native widgets and refreshes when its selected
file changes. HTML and embedded assets are inert. Loading is bounded and runs off the window thread;
errors and oversized content are visible rather than silently truncated.

Git presents files and hunks in unified or side-by-side form. Local changes compare tracked content
with HEAD and include untracked files; a user-selected base provides another comparison. Git runs
without a shell, pager, external diff, text conversion or fsmonitor, with optional locks disabled and
its ownership checks intact. Inherited Git environment variables cannot redirect the selected
worktree. A folder outside Git gets a clear non-repository message; actual Git failures remain
visible. File and process limits are enforced away from the UI thread. Untracked
symlinks are not read.

Review comments can be drafted against the displayed changes. A preview shows the complete text
before insertion into the console that opened the review, pinned by identity; insertion requires an
explicit user action. A missing origin is reported instead of silently choosing another console.

Discarded: general plugin runtime (no second extension contract needed for two built-in readers),
new console types (would entangle PTY identity, session restore and MCP controls), webview HTML
(unneeded scripting/network surface), shell-composed Git commands (paths are data), and automatic
repository guessing from task text, other consoles or the app's working directory (the user chooses
the source console explicitly).

Native desktop behaviour remains a separate verification from headless input and rendering tests;
no performance figure is claimed from source inspection. Historical `board-4-*` and `board-16-*`
runs remain overlay-era evidence and the current script refuses to relabel them. The
`dock-v1-4-closed/open` regimes (frozen since #93, which measures `dock-v2-4-closed/open`) use matched layouts and must
publish the selected tool, effective width, per-pane grid and pane identity count before comparison.
The dock-open grid intentionally differs from its closed control, so no painting speedup is inferred.

### Explorer interaction refinement — 15 November 2026, #88

(Its rail icon and inline search field were replaced in #93, and search results no longer stand in
for the tree: they are a floating list whose choice is revealed in it; see the last section.)

The owner rejected the explorer's extra close/pin/navigation controls and filled triangles,
then supplied native feedback rejecting the source/cwd-change rows, large chevrons, console
flicker during scrolling and inactive Markdown links. Files keeps one root caption and a compact
name-search field. Its rail icon is a recognizable folder with the adjacent tools' vector style;
that button or Escape dismisses Files. Folder arrows were six-point open chevrons within their
existing hit areas; since **D103** they are Phosphor's carets in a 12-point square, same hit areas. Normal tree activation expands/collapses without replacing the root.

Files follows usable console-directory changes automatically, including the shell's transient
startup directory. The same source retains navigation; remote/unavailable metadata suspends local
browsing. Keep one top-right Refresh icon for explicit cache invalidation. Its ink and hover fill
come from the effective theme. Files gestures claim input without applying modal console dimming;
other modal owners retain their existing behavior. Files key handling, result reconciliation and
painting are counted in the existing sidebar application timing span, including hidden-result
processing. Console Ctrl+click reveal remains within the existing pointer span, counted once;
viewer and Board work remain separate.

A single click on `.md` opens the existing viewer, including an explicitly selected symlink.
The reader already opens nonblocking on Unix, validates the opened regular file, bounds its size
and requires UTF-8. Directory traversal still does not follow symlinks. Other files select quietly;
there is no unsupported-preview notice. A general text viewer remains separate work.

The owner chose **Show more** after review reproduced two valid 4096-directory levels filling the
8192-row budget before an expanded descendant. Each click admits another 8192 rows for that console,
retaining root, selection and scroll. A new root resets the initial budget. Existing listing/cache
limits and virtualized painting remain; extra work requires an explicit click.

Search matches file and directory names beneath the displayed root, not contents. It runs on the
existing single cancellable worker, with generation/epoch-tagged replies. Queries are limited to
256 characters, scans to 100000 entries and 4096 pending directories, and results to the existing
4096-entry / 4 MiB payload bounds. Partial/unreadable results say so. Changing/clearing a query
rejects old replies; Escape clears search before closing Files. Matching directories expand within the results, without replacing the search root. Clearing an ordinary query restores that console's tree.

Ctrl+click on a local console path reveals it in Files: directories open directly and files open
their parent with the file selected and scrolled into view (PNG and JPEG files open in the drawer
instead; see «Images in the document tab» below). It uses the clicked console's explicit
local context, never the app's directory or a remote file host. The clicked console becomes the
active source for its Files state. Metadata and directory reading run on the same worker; missing
paths remain visible errors, not shell commands. Web links and ordinary selection retain their
existing gestures. The source's line suffix is accepted as path annotation, not executed.

Discarded: duplicate close/pin/navigation controls; filled or oversized arrows unlike the supplied
IDE reference; fixed-root traversal without a way past its row budget; permanently dimming consoles
for nonmodal Files scrolling; refusing a Markdown symlink the existing reader already validates;
content search, a second filesystem worker, unbounded recursive scans, following directory links,
a general text renderer, and treating a displayed path as executable code.

### Markdown reading mode (owner amendment, 15 November 2026, #89)

(Replaced in #93 by the drawer's maximize, which serves Markdown and Git alike; see the last
section. The Files companion, the rail and the document header described here are gone.)

The owner requested consistent representative icons, removal of the redundant Markdown path
picker, and maximize/restore for Markdown alone. In maximized mode the document occupies the
content below the global chrome and tabs, while the tool rail stays reachable. Files can open as
a lateral companion without replacing the document. Activating another Markdown file updates the
same reader and keeps Files available. Restore returns to the ordinary Markdown dock at its prior
preferred width; a separately sized Files companion cannot overwrite that width. Git keeps its
ordinary dock and Board retains its independently selected full-size presentation. This is transient presentation, never a new console or saved layout.

The host keeps Markdown selected and tracks the Files companion separately so opening Files does
not suspend the visible document's refresh or swap its bank. One common geometry supplies distinct
document, Files, rail and divider rectangles to input and painting. When the two bodies cannot
meet their existing minimum widths, Files is temporarily suppressed without losing its selection.
Covered consoles retain the ordinary valid PTY layout and their full output-pump budgets; a hidden
surface never becomes a zero-sized terminal. Dismissal and tool transitions consume their input.

Markdown's rail and document cue share their outline painter. Maximize/restore reuse the console
header's painter and paths, with a larger 24-point icon box suited to the document header, as
requested during native visual review. The document icon and title share a centered row. Controls
retain explicit action tooltips and hover/pressed feedback; the empty header omits the redundant
"Markdown file" subtitle. The empty view directs the person to Files. The manual path form and title-click path editor are removed, while
explicit local Markdown links and the existing bounded loader remain available.

Discarded: a generic docking framework or document workspace model, because only Markdown with
Files is requested; maximizing the whole sidebar, because Files must remain lateral; zero-sized
hidden PTYs, because the programs remain alive; keeping the duplicate path picker, because Files
already supplies explicit document selection; Unicode icon mixtures or another icon dependency,
because the existing vector style can represent these actions deterministically (reversed by
**D103**: the owner judged the hand-drawn set amateurish, and an icon font is as deterministic). No frame-cost
improvement is claimed; host, viewer and explorer work retain their named spans.


### Files on the left, Git and Markdown in a drawer below — 17 November 2026, #93

The design kit (`Files.dc.html`, `Git.dc.html`, `DockB.dc.html`) and the owner's answers at the
build gate replace the right sidebar. The owner chose to remove the rail as the mock-up does, to
keep no loose floating window, and to put Markdown in the drawer as a tab.

- **Files** opens on the left edge of the content area, 264 points by default, at least 200, with
  a 6 point divider that resizes it. It is independent of the other tools: opening Git, Markdown or
  a Markdown file leaves it where it is. Its header shows the title, Search, Refresh and Collapse
  all, the root, and that it follows the focused console. Its rows take the theme's roles.
- **Search** is a floating list over the window, anchored under Files' header: the existing worker
  finds names beneath the displayed root, Enter reveals the chosen match in the tree (its folders
  expanded, selected, scrolled into view) and Ctrl+Enter (Cmd+Enter on macOS) also opens it: a
  Markdown file in the drawer, a folder in the tree; any other file is only revealed, and Files
  says it opens Markdown alone. Typing changes the query without dropping the tree's listings. A
  press on the list, as it is laid out, never reaches the console or the divider beneath it. The
  search belongs to the focused console's Files and closes when focus moves to another console.
  Escape closes the list first, then Files. It is drawn to `Files.dc.html`: a field outlined in the
  accent with the search icon and an Esc key cap, 30-point rows with file and folder icons and the
  matched part of the name marked, and a footer; the header's search button stays lit while it is
  open.
- **The drawer** takes the bottom of what Files leaves, 370 points by default, at least 160, with a
  divider above it and 6 points kept free on its left, right and bottom. Its strip names the source
  console's project in its ink (or, with no project, the folder it reads: the terminal's own title
  said nothing useful there), then a Changes tab (with the file count once the diff has loaded) and
  one document tab — the last Markdown file opened replaces it — each with a cross that closes it;
  closing the shown tab hands the drawer to the other, and closing the last closes the drawer. A
  review with unsent comments is not closed and says why. On the Git tab the strip also carries the
  comparison picker and a refresh button (the owner kept both, compacted, rather than lose the
  comparison with another branch), then Unified / Side by side, maximize/restore and close; a row
  under the strip appears only to type a base or clear comments, and the hunk arrows are gone. A
  narrow strip gives way in this order: the source's label, the Git controls, the diff toggle; one
  that leaves something out says so on its maximize button. Every place the host resolves has a
  fixed width, so a click is resolved before the frame's tiling against the rectangles the painting
  uses. Review rows show the file's name first, its folder dimmed and its counts on the same row;
  diff rows touch, with dim line numbers and the change's mark in its colour. Side by side divides
  its two columns where the person drags the handle between Before and After.
- **The active drawer tab has priority in a narrow strip** (#215). After the source label,
  Git controls and diff toggle have given way, inactive tabs give way before the selected one.
  Retained tabs keep their Changes / Document / Image order; a tab and its close target are
  retained or omitted together. If the selected tab alone cannot keep its usual width, its label
  is shortened inside the remaining tab space. Maximize and drawer close keep their places.
  This replaces dropping the last tab by position, which could hide the view being read and
  leave only its close button. Reordering the active tab first was discarded: selecting a tab
  should not shuffle the strip when the retained tabs fit in their original order.
- **Not modal.** Neither the drawer, Files nor a press on the title bar dims the consoles: only a
  modal surface does (a console's rename or menu, the title bar's menu, a dialog). Dimming on the
  drawer shaded the window beside it, and dimming on a title-bar press painted every console at half
  opacity for the frames the button was held — the blink the owner reported on every button.
- **Maximized**, the drawer covers the consoles edge to edge at the size they already had beside
  it, so toggling it resizes no program and no strip of a hidden console is left to take a press;
  Files stays beside it, and restore returns the previous height. The Board covers the whole content
  area, Files included, and the consoles keep the size they had beside Files; the Files button over
  an open Board leaves the Board for Files.
- **Stepping aside.** Files needs its minimum beside the consoles' width floor, and the drawer its
  minimum above their height floor. Past that, the real placement decides: Files steps aside when
  it is what leaves a console under its width floor, the drawer when it is what leaves one under
  its height floor, and neither for a squeeze the tree has without it. The tool keeps its selection
  and its title-bar button explains how to bring it back.
- **The doors** are title-bar buttons: Files at the left end, Board and Markdown left of the
  right-hand group (D96 amended); a compact bar lists them in its menu. Git still opens from a
  console's review control.

Discarded: keeping the right rail (the kit draws none, and it cost every console 36 points for
a column of tool buttons); a floating, detachable Git window (the owner asked for a drawer that grows, shrinks,
maximizes and restores, not a loose window); maximizing by re-tiling the consoles into the space
left (it would resize every program on each toggle, which the Board already avoids); a Markdown
reader of its own beside Files (two document surfaces to keep in step, where one drawer with tabs
serves both); hiding a surface because of a squeeze the tree has without it (it punished the tool
for what it did not cause). No frame-cost figure is claimed: the host, viewer and explorer work
keep their named spans, and `scripts/measure-frame.sh` has not been re-run on this geometry. Its
dock regime is versioned `dock-v2-4-closed/open`; `dock-v1-*` measured the rail, a different
console grid and a width drag, and the script refuses it rather than relabel it.

### Images in the document tab — 20 November 2026, #178

Agents print the paths of the screenshots they take, and reading one meant leaving the terminal.
A `.png`, `.jpg` or `.jpeg` path (any letter case) now opens in the drawer's document tab: Ctrl+click
on it in a console, or activating it in Files. Ctrl+click on an image no longer reveals it in Files,
which would show a name and not the picture; every other local path is revealed as before. A
relative path is joined onto that console's directory, as its review is, and refused with a reason
only when the console has none. Files says «Only Markdown, PNG and JPEG files open in the drawer.»
for anything else.

- **One document tab, as before.** *(Superseded by «The image leaves the document tab», #198.)*
  The image and a Markdown file replace each other there: opening
  one forgets the other, and closing the tab forgets both, through one replacement every entry
  uses. The image is pinned to the console it was printed in, not to the focused one, and the strip
  shows that console's project or folder.
- **Decoded once.** The file is read and decoded on the viewer's worker when it is opened and
  never again: switching to Changes and back, or closing and reopening the drawer, shows the same
  picture, and a program that overwrites the same screenshot name does not change what is on
  screen. Markdown's one-second refresh does not apply.
- **One read at a time.** *(Superseded in part by «The image leaves the document tab», #198: the
  bound is per open console, and a closed console's stuck read is let go.)* A read stuck on an unresponsive filesystem cannot be cancelled, so
  replacing or closing an image whose decode has not finished keeps its handle, and another open
  while it runs is refused with «Still reading <path>…» in the drawer. Repeated opens cannot pile
  up readers, the rule the Markdown state already keeps. Errors name the path they are about.
- **Bounded before it is decoded.** The path is opened nonblocking on Unix and refused unless it is
  a regular file, as Markdown's is; at most 32 MiB is read; the format comes from the bytes, and only
  PNG and JPEG are decoded; the decode may allocate at most 256 MiB (`image::Limits::max_alloc`),
  counting the RGBA copy the viewer keeps when the file is not already RGBA. Each refusal is a
  visible message. The GPU gets a copy no side of which exceeds its `max_texture_side`, made after
  the decode and outside that cap; the original is kept at full resolution.
- **Seen as a person reads a screenshot.** It fits the drawer's width and is never enlarged past one
  physical pixel per pixel, so on a display of two pixels per point it is shown at half a point per
  pixel rather than blurred. The image has a viewport of its own, a scale and an offset the viewer
  keeps, painted clipped to the drawer's body with no scroll bars: Ctrl+wheel, or Ctrl + two-finger
  scroll on a touchpad, zooms around the pointer; the wheel and two-finger scroll pan it on each
  axis where it is larger than the drawer, and Shift+wheel arrives from egui as a horizontal
  scroll. On an axis where it is smaller and whole inside, the wheel leaves it and only a drag or
  a zoom moves it there; where a zoom carried it across an edge, the wheel brings it back and,
  like a pan, never pushes it further out. The wheel is the image's only while the pointer is on it
  with nothing of a higher layer above, and the part of a turn that moved it is taken, so a menu or
  popup open over the drawer keeps its own wheel and nothing later in the frame scrolls with the
  same turn; an axis it does not apply, and the part of a turn an edge stopped, are left for whoever
  reads the wheel later. Over the blank beside the image
  the point kept is the image's nearest one, so a zoom there grows the image from its near edge and
  never carries it out of sight; anchoring the empty point under the pointer, as a canvas does, was
  discarded because under a fitted screenshot most of the drawer is blank and four notches there
  pushed the image out. Where the image may sit is one rule per axis. Where it is larger than the
  drawer, no gap opens between its edge and the drawer's, after a zoom, a pan or a resize. Where it
  is smaller, a zoom never moves it, so the point under the pointer stays there even when the
  image crosses the drawer's edge; a pan may only bring it back toward whole inside and never push
  it further out; a resize of the drawer puts it whole inside. While no zoom was chosen, a resize
  changes the fit's scale, and the offset is scaled with it before that rule, so the part being
  read stays at the top of the view. The image is painted as the body's own rounded shape,
  textured with it and clipped to where the image is, so it takes the body's bottom corners
  wherever it reaches them; the image's own rect with those corners was discarded because epaint
  caps a radius at half a rect's shortest side. When a zoom makes an axis larger
  than the drawer, the first rule applies from that frame on. Pinning a smaller image inside on
  every change was discarded: a screenshot fitted to width and shorter than the drawer sits at its
  top, so every zoom pushed its top edge out, the pin brought it back, and the image grew downward
  from its top edge instead of around the pointer, the symptom the owner reported. Centring a
  smaller image on open was discarded too: it moves the screenshot away from where a person starts
  reading and still pins it once a zoom reaches an edge. winit 0.30.13 emits `PinchGesture` only on
  macOS and iOS, so pinch zoom is a macOS-only behaviour until F2.7 verifies it there. egui reports
  Ctrl+wheel as a zoom and not as a scroll, so the drawer counts the zoom as pointer activity over
  it to keep the gesture. The drawer stays not modal: nothing dims. With a mouse, Space + left drag
  and a middle-button drag move the image, with a grab hand while they do. A plain left drag does
  not pan, because the next step draws marks with it. Space + drag is Figma's convention and the
  middle button needs no key. Whether a drag pans is decided when its button goes down, by where
  it went down: on the image with nothing of a higher layer above that point, wherever the pointer
  is by the end of that frame. It lasts until that button is released: a press of another button meanwhile neither stops the pan nor
  takes it over, and a pan is forgotten when another image opens, when the image is closed, and
  after any frame the image was not painted, so a release that happened while it was hidden cannot
  leave a pan behind. On a touch screen the image zooms with two fingers and does not pan:
  egui-winit turns a touch into touch events, and the one gesture it turns into a wheel event is
  winit's `PanGesture`, which winit 0.30.13 emits only on iOS. On Linux, macOS and Windows two
  fingers therefore give egui's multi-touch zoom, which the zoom reads, and no scroll; a one-finger
  touch drag arrives as the primary button, which is the plain left drag the marks take. No
  touch-specific pan exists. Space is the drawer's only when it went down after the drawer held
  the keys, which is after opening the image or clicking the drawer, or at any time while the
  drawer is maximized, since a maximized drawer reads every key without a click. It is worked out
  again every frame and forgotten whenever the drawer does not take the keys, so a Space typed into
  a console still reaches its program and never pans, also when the drawer comes back with that
  Space still held. On GNOME with "disable touchpad while typing" on, which is the default, holding
  Space blocks touchpad clicks, so with a touchpad the image is moved with two fingers and zoomed
  with Ctrl + two fingers.
- **Cost.** Decoding runs on the worker. The texture is uploaded once per open, by the backend
  after the pass, on the first frame after the decode arrives, so that upload lands in the framework
  remainder, not in the viewer's span. No measurement batch opens an image, so no published figure changes.
- **Marked and sent (21 November 2026).** A plain left drag draws on the image, and Send hands a
  copy with the marks to the console the image came from: **D113**. Marks not yet sent keep the
  image in its console's slot, as unsent review comments keep their review.

Discarded: an image `Kind` inside the existing viewer state, because the document tab's click
forces the Markdown kind and a kind change clears that state, which would erase the image on the
way back from Changes; a third `viewer::State`, because the two that exist are swapped with a
single boolean (the image keeps a state of its own instead); a separate image surface beside the drawer, because it would bring a second geometry and a
second set of input rules for what D72 already gives one place; refreshing the image from disk,
because what was opened would change under the person reading it; a separate side limit, because
the allocation cap already refuses the headers that would need one; GIF, WebP and SVG, because
screenshots are PNG or JPEG; driving the image through an egui `ScrollArea`, which was the first
version: its offset only runs from zero to the content's far edge, so holding a zoom around the
pointer meant laying blank space before and after the image, and that broke it four ways — a
trailing blank left stale when the drawer grew, scroll-bar drags remapped against content that
shrank every frame so the thumb ran ahead and jumped, guessing where egui puts its bars to keep a
pan off them, and its own drag-to-scroll joining the pan after a touch.

### The image leaves the document tab — 22 November 2026, #198

Images get a drawer tab of their own, **Image**; the document tab is Markdown or code only again,
single and shared, and opening one no longer forgets the other. Each console holds one image with
its zoom, its marks and its notes, kept on the console and dropped with it, unsent marks included,
with no dialog. The Image tab shows the focused console's image and follows focus: clicking another
console shows that console's image, and coming back shows the first one as it was left. On a
console with none the tab stays selected and says so, since closing or switching the drawer on a
focus click would resize every console. A Ctrl+click on an image path, or Files activating one,
fills the clicked console's slot and moves the focus to that console. Unsent marks refuse a new
image in their own console only; they no longer refuse opening Markdown. Decoding stays lazy and
once per opening; one live and one abandoned read are bounded per open console, so a stuck read
never refuses another console's image. A closed console's stuck read is let go with its slot,
unlike a replaced one, which its slot keeps: its thread finishes unwatched and nothing waits on it,
so a hung mount costs one reader per console opened and closed on it. A pan latched on one console's image is forgotten when the drawer paints another's. Send
addresses the slot of the console it was clicked for, never the focused one: its line, its «Sent
to» and a cancelled worker's last word all go there, and what it has to say about a console that
closed meanwhile, whose slot went with it, goes to standard error only. No title-bar button opens
the Image tab: the File door opens the drawer, and the tab is in its strip while the focused
console holds an image or the tab is the one shown.

Discarded: keeping the image in the document tab with a per-console «front» flag (the tab flipped
between an image and a Markdown file not that console's); per-console Markdown (nobody asked); a
map of images on the window (a prune on every way a console leaves; the field needs none); showing
a closed console's last word in the focused console's slot, which would put one console's news
under another's image.

## D73 — Connection help is global and reports observed publication

**Selected by the project owner.** A **Connect agents** icon in the integrated title bar opens an
in-application setup guide. It is highlighted while no console has an accepted, non-retired panel,
and remains available without highlighting once any console publishes. File and control-channel
producers count equally. The menu provides the same entry, including compact windows. Since #93 the
icon is the kit's link and the highlight is its colour alone: the owner removed the dot beside it.
Since **D103** the icon is Phosphor's plugs.

The primary action copies a setup prompt for the user's AI client inside Ivy. It asks that client
to find the bridge, preserve existing settings, register local stdio if necessary, and verify by
publishing a panel. It explicitly handles a required client restart without claiming success.
The expected executable path is JSON-encoded data; per-console identity is never persisted.
Manual setup and diagnostics remain collapsed until requested. They explain the separate bridge,
client registration, inherited environment, tool discovery and instructions to publish progress. It reports accepted panels and
live publishing holds, never whether an arbitrary client is installed or configured. Checking the
bridge only reports whether its expected sibling file was found at startup or on an explicit
recheck; a binary installed elsewhere is not declared absent. Commands, paths and agent instructions
can be copied, but nothing is installed, executed or written into client configuration by the guide.

Opening, closing and copying in the guide own their input before console dispatch, including the
closing frame and a held mouse release. The guide is exclusive with Settings, Overview and pane
rename dialogs. Consoles keep running with their original grids, histories and pump budgets. The
indicator is painted after active and background panels refresh so a quiet first publication updates
it in the same pass. Its projection, painting and guide work belong to the existing integrated-chrome
span; historical measurement mode adds no guide. No performance result is claimed here.

**Discarded, with reasons:**
- An alarm saying MCP is not installed because no connection exists: a configured client may not
  have called a pane tool, a file producer needs no MCP, and a regular shell needs no panel.
- Reading third-party client files or terminal output to discover configuration: those sources do
  not declare producer state to Ivy and would add client-specific inference, contrary to D6.
- A warning inside every empty console: the owner selected one global indicator with a guide.
- Automatic client configuration or command execution: this delivery explains the existing setup
  and lets the person copy explicit text; changing external settings is separate behavior.


**Codex setup clarification (issue #6, 9 November 2026).** The copied prompt and manual guide
explicitly document `env_vars = ["PANEL_ID", "PANEL_DIR"]` in the existing stdio server table.
These are names to forward, never persistent console values. A Codex shared background server
can lack the originating console's environment even when its terminal client is already inside
Ivy, so the missing-identity branch must reach that diagnosis instead of only asking to reopen.
Check the installed launch or resume help before recommending `--no-daemon` inside the intended
console; an unsupported flag is reported as a client-version limitation. Codex 0.167.0 advertises
both command forms, but this capability check is not native two-console verification.

Why: the reported session discovered both bridge tool sets while calls still returned a missing
pane identity. Forwarding is explicit in Codex's MCP configuration, and its CLI help describes
`--no-daemon` as bypassing the shared background server. Discovery, usable connection and correct
console observation are three different proofs; distinct statuses in two real consoles are needed
before claiming independent routing. The guide continues to copy text only.

Rejected workarounds include persisting one console's identity (stale or wrong-pane binding),
restarting a global daemon for each console (does not establish independent per-console identity),
and adding a pane selector to MCP calls (changes the existing endpoint-scoped contract without
evidence that it is necessary). Automatic client configuration remains outside this guide.


### Codex console-local startup

Ordinary supported shells install a console-local `codex` function after user startup and delegate to the bundled `ivy-codex` launcher. The launcher checks client capability before adding `--no-daemon` to local interactive launches. Existing aliases and functions retain precedence; explicit bypasses remain available. MCP identity forwarding is still required. This avoids changing global shell or Codex configuration and leaves existing recovery intact.



**Code-document navigation — 25 November 2026, #245.** A text selection held beyond the top or bottom advances the viewport until the person returns inside, releases, or reaches the document boundary. The anchor stays in document coordinates. Overflowing code documents use opaque scrollbars with their own space; wrapped text reserves the vertical bar and disables horizontal overflow. Discarded: enabling scrollbar visibility without changing the floating style, whose dormant opacity still hides the position indicator, and relying on ScrollArea to infer autoscroll from a custom selection widget. The code-file menu restores comfortable item padding and an outlined Wrap lines checkbox locally because the generic menu style removes both; the shared menu system stays unchanged. Toolbar alignment remains an observed report to verify visually: centered headless fixtures are not evidence that the native symptom is fixed.

---

## D74 — Ivy is public free software under GPLv3

**Decided on 9 November 2026.** Ivy's original code and documentation use GNU General Public
License version 3 only (`GPL-3.0-only`), inherited by all five workspace packages. The complete
license is at the root; COPYRIGHT records Alberto Vioque and Ivy contributors. Third-party
dependencies, themes, the test font and vendored egui-winit retain their own licenses and notices.

**Why:** the owner wants distributed derivatives to provide their corresponding source under the
same license instead of allowing modified closed-source products. GPLv3 supplies that condition
while allowing private use, modification and commercial redistribution. Public source and releases
share one repository, so update discovery requires neither credentials nor a separate releases repo.

**Discarded:** permissive licensing that allows closed-source derivatives; AGPL's additional
network-use source obligations, which were not requested for this desktop application; returning
the source to private visibility, which would not implement the requested public GPLv3 project.

**Boundary:** public visibility and this license do not publish a binary release, claim platform
certification or change any third-party license.

---

## D75 — Stable release discovery is global and does not interrupt consoles

**Decided on 9 November 2026.** Ivy checks the public GitHub releases of `mashware/ivy` once per
ordinary launch on a background thread. A dot on the application menu announces a newer stable
semantic version. The menu offers **Check for updates** and **What's new**; neither starts a program
inside a console. A user can dismiss the dot for this launch and still reopen the notes.

The dialog includes bundled notes for the installed version, available offline, and the twenty most
recent published releases returned by GitHub. Drafts, prereleases and tags that are not semantic
versions are excluded; versions are compared numerically, ignoring build metadata. No releases is
not the same as up to date. Failed requests are visible in the dialog and retried only on request.

**Why:** people using an older binary need to discover releases and understand new features without
an agent, running command or active terminal losing focus. Published releases give the maintainer an
explicit publication boundary; ordinary commits and tags do not become user announcements.

**Boundaries:** one request at a time, a ten-second global timeout and a one-MiB response limit.
Release notes are bounded and use the existing inert Markdown viewer: remote images, links and HTML
do not execute. Explicit download and release-page buttons use fixed HTTPS GitHub repository URLs
through the desktop's existing opener. No provider credentials, console output or pane identities
are sent. The update check runs on all supported desktop platforms and requires no external CLI.

Every measurement launch suppresses the automatic check, including integrated-chrome measurements.
Polling and dialog work is attributed to integrated chrome and subtracted from framework cost.

**Discarded:** automatic installation, which requires platform packaging/signing and restart policy
that this request does not define; periodic polling, which would wake idle terminals; a latest-only
endpoint, which cannot supply history; browser-only notes, which are unavailable offline and leave
installed features undiscoverable without a published release; a second Markdown renderer.

**Accepted limits:** release history and dismissed notifications last for the current launch only.
Offline users retain bundled installed notes, but remote history is not cached. A single request
reads twenty recent releases, so older history remains on GitHub. Source builds identify themselves
by their Cargo version. Publishing a release requires bumping that version and updating the bundled
notes; this feature does not publish releases or build installers.
## D76 — Native packages make Ivy available for explicit experimental trials

**8 November 2026 — #343.** Installers wrap the existing terminal and MCP bridge without changing
Cargo target names or the user-data namespace originally retained by D71 (superseded by D87). The first delivery
is an Ubuntu 24.04-baseline x86_64 `.deb`, usable on the approved Ubuntu 24.04 and 26.04 targets;
macOS 15+ ARM64/Intel and Windows 11 x86_64 follow as separate coherent deliveries. Native runtime
and desktop evidence, including upgrade and uninstall, decides what is actually supported.

Linux installs `app` and `panel-mcp` under `/usr/lib/ivy-terminal`, exposes `ivy` and `panel-mcp`
commands, and supplies an Ivy desktop entry and the approved icons. Package metadata owns no
configuration or layout, and uninstall has no script deleting them. ELF inspection determines
linked dependencies; dynamically loaded desktop/font libraries are declared separately because
ELF dependency scanning cannot see them.

Versioned CI artifacts and explicit prerelease publication are different acts. Builds have read-only
repository access; a manual publication on a matching version tag aggregates the complete declared
asset set and checksums in a draft before making it visible. The first alpha has no publisher
signature. No signing identity, certificate purchase, provider key or updater is assumed.

**Discarded, with reasons:**
- Source-only installation: it requires a compiler on every tester's machine and ties daily use to
  a development checkout instead of an identifiable package.
- A new GUI wrapper or target rename: packaging needs desktop resources, not a replacement for the
  existing application, and D71 preserves internal identifiers.
- A release-framework-only first delivery: it supplies no terminal the owner can install. Shared
  machinery instead ships with Linux and is reused by both later platform deliveries.
- A single all-platform delivery: an unvalidated Mac or Windows assumption would delay usable Linux
  trials; platform packages can each stand alone with their actual limitations declared.
- Mandatory signed distribution before owner-led trials: identity provisioning is a separate
  credential/cost decision, while explicitly experimental packages enable the native testing.
- Treating package creation as portability completion: the Windows listener deliberately rejects
  control connections until F2.7, and desktop integration cannot be proved by compilation alone.

Release versions admit stable `MAJOR.MINOR.PATCH` and numbered `alpha.N`, `beta.N`, `rc.N`
prereleases only. A direct hyphen-to-tilde conversion of all SemVer labels was discarded: Debian
orders `rc10` after `rc2`, whereas SemVer orders these lexical identifiers the other way, and Debian
equates `alpha01` with `alpha1`. The supported numbered subset preserves ordering and identity.

The package retains the current GPLv3/COPYRIGHT and third-party asset notices. Cargo source
license/notice texts are collected from resolved release dependencies; known Apache-2.0 choices
may use the repository's preserved canonical text when published crates omit it. Unknown missing
texts stop packaging. This preserves existing licenses rather than choosing new ones.

The owner selected Arch x86_64 as the next delivery before macOS/Windows. Arch uses native makepkg metadata and pacman lifecycle checks around the same verified Ubuntu-baseline Linux payload. Reusing binary bytes avoids independent builds producing different executables within a release, while explicit Arch dependencies and actual installed-runtime checks establish whether that payload works on Arch. AUR/official repository submission remains separate.

Public alpha1 upload demonstrated that GitHub rewrites '~' in asset names to '.', breaking manifest/checksum filenames. Asset names therefore retain SemVer spelling (`ivy_<V>_amd64.deb`); Debian control metadata still maps the prerelease hyphen to tilde. Public transport-safe names and package-manager version syntax are separate contracts.

The Mac delivery supplies separate Apple Silicon/Intel DMGs for macOS15+, preserving Ivy.app,
`io.github.mashware.ivy` and the existing user-data namespace. A DMG with an Applications shortcut
was chosen over an installer package because the app owns no system services or user configuration.
Native builds and architecture/minimum-OS/dependency checks prevent shipping a renamed Linux binary
or an executable requiring an undeclared Homebrew installation. Original license texts travel inside
Contents/Resources. Python's plist library handles bundle metadata directly rather than interpolating XML.

Local ad-hoc signatures seal the Mac bundle's nested executable and resources, but the release remains
unsigned in the publisher-identity sense: no Developer ID or notarization is provisioned. Per-app
approval follows Apple's normal Privacy & Security flow. Global Gatekeeper bypasses and quarantine
removal were discarded because an experimental installer must not weaken the person's system.
Native runner lifecycle/MCP checks remain distinct from actual desktop and quarantine evidence.

Offline export of both Mac dependency graphs exposed published Objective-C binding crates without
license files. The build retains pinned upstream MIT text and licensing context, original Cargo
attribution and source revisions for the explicitly verified versions. Provenance and the dispatch
MIT fallback distinction are recorded in resources/licenses/macos/README.md; unknown versions still
stop packaging. This preserves declared terms and available attribution rather than changing licenses.

The Windows delivery uses a per-user Inno Setup executable for Windows 11 x64, with a stable
application identity, a Start menu shortcut and an uninstaller. It installs under LocalAppData,
requests no elevation, changes no PATH or default terminal, and owns no preferences or layouts.
Inno Setup's established per-user lifecycle was chosen over a portable ZIP (no installer or
uninstaller) and a machine-wide MSI (unnecessary administrator requirement). Cargo target names
stay unchanged; the installed GUI filename is Ivy.exe. The existing PNG supplies a multi-size ICO.
Native MSVC builds, PE architecture checks and installed-file/notice hash comparisons gate artifact
upload; installation, same-version replacement and removal run only on a disposable Windows runner.
An older-to-newer version upgrade and real desktop behavior remain separate evidence. The installer
is unsigned and has no automatic updater. Packaging does not implement the Windows control listener.
The complete release manifest now requires this installer as well as Linux and both Mac assets.

## D77 — Console text has its own inner spacing

**Decision.** Every console reserves 8 logical points on all four sides of the terminal grid.
The shared console rectangle determines painting, PTY rows/columns and pointer coordinates,
including hidden tabs and the immediate retiling while dragging a status-panel divider.
Panel allocation and split feasibility include the extra 16 points before preserving the existing
20-column and 3-row split minimum. Tree-divider dragging retains its existing smaller floor;
padding yields before the last available cell in a collapsed console.

**Why.** Text against the console border is harder to read, as shown in the user screenshot.
Changing only the drawing origin would make selection and application mouse reports disagree with
what is painted. Padding is presentation space, not blank terminal cells a program can overwrite.

**Discarded.** A glyph-only translation breaks input alignment. Empty terminal rows or columns
are controlled by the program and cannot reserve a stable margin. A new configurable preference
adds a product surface the requested fixed spacing does not need.

**Measurement boundary.** Padding changes the available grid, including measurement runs; no
frame-cost improvement is claimed against older grids. Its geometry remains in the existing
timed panel-tiling path, and grid measurement and painting consume that one result.


## D78 — User splits inherit their source console's local working directory

**Decision.** Toolbar, compact-menu and keyboard splits resolve the source console by name,
after the geometry check. A latest unusable OSC 7 report vetoes local process discovery, so a
remote shell path is never mistaken for the local ssh process's directory. Otherwise the live
foreground directory supplied by term-core takes precedence over a valid but potentially stale
local OSC 7 report. When native discovery is unavailable, the local report supplies the path.

The path goes to the new child's command builder, including the fallback shell. It never changes
the application's current directory and never becomes a `cd` command written to a PTY. A known
path must be absolute and an existing directory; opening fails through the ordinary split failure
path if validation fails. Without directory information the ordinary launch directory remains,
with a diagnostic on standard error. No discovery is added to an idle frame: the complete
operation remains inside the existing shortcut or pointer stopwatch.

**Why.** A second console for the same project should start where its source console is working,
after navigation as well as at first launch. Header controls can address an unfocused console
(D62), so resolving global focus would inherit the wrong project.

**What was discarded.** Parsing the prompt is unreliable and violates D6. Changing process-global
cwd races sibling launches. Sending `cd` mixes application setup with program input and shell
quoting. Always trusting the latest local OSC 7 ignores a subsequent cd before another report.
Installing shell hooks changes the user's setup and is outside this feature. New tabs, saved
restart directories and agent-requested explicit programs keep their existing behaviour.

**Limits.** Live native discovery is currently Linux-only. macOS needs local OSC 7 metadata;
Windows drive-letter file URIs are converted to native paths before launch, but real host
validation remains F2.7. Shell startup scripts may themselves change directory. portable-pty
0.9.0 silently substitutes home for a configured directory that is not a directory, which is why
validation happens before invoking it. A directory removed between that check and the dependency's
own check can still fall back to home; this is not an atomic directory-pinning guarantee.

**Fresh Windows agent amendment (26 November 2026, #185).** A new explicit agent launch (D90)
opts into a required working directory at the command-builder boundary. The chosen path is passed
to Windows without the dependency's `is_dir()` filter or home substitution; an unavailable target
therefore fails the launch. Ordinary shells/splits and Unix behavior retain the policy above.
The published portable-pty 0.9.0 source is vendored with licensing, original byte hashes and a narrow
patch guard because its public API provides no strict directory option. The new default-false mode
preserves legacy builders and serialized input. Only the fresh Windows entry point selects it.

Discarded: another pre-check (the race follows it), changing HOME/USERPROFILE (changes client
configuration and authentication), directory rename guards alone (later metadata errors can still
select home), process-global cwd, a shell wrapper and silently weakening the displayed-directory
promise. This is an exact-path-or-error rule, not immutable directory identity against namespace
replacement by the same user.


## D79 — Project identity is a theme-aware header cue independent of the console title

A console's entire header uses its project's color. The existing manual/program title, focus outline and producer-declared attention keep their semantics. The project key is a canonical common Git directory, so a repository's subdirectories and linked worktrees share it. An exact canonical folder identifies a positively established non-repository. Unavailable or remote directory metadata stays neutral; a failed Git launch is not evidence of a non-repository.

The project name is plain CamelCase text followed by ` | ` and the session title, using the same text size. Hyphens, underscores and spaces become word boundaries; existing internal capitalization is preserved. Name collisions are checked after formatting and use path suffixes, with the raw full path as a final disambiguator. A filled name badge was discarded after visual feedback: its background and fixed width added a heavy block and an unnecessary gap before the title.

**New signal, 10 November 2026 ([#48](https://github.com/mashware/ivy/issues/48)).** The owner reported that the narrow color strip was almost invisible and requested PhpStorm-style coloring of each individual pane header. After viewing the first solid-color version beside a PhpStorm reference, the owner requested the reference's subtle tint rather than the solid block. The assigned accent now blends into the neutral header background, from 28% at the left to 6% at the right. One four-vertex mesh covers the whole header; hover clips a matching gradient to the target instead of restarting it inside each button. Ordinary text and icons use opaque white on dark header backgrounds and opaque black on light ones. Tint strength is reduced for a custom palette only when needed to retain at least 4.5:1 contrast: component-wise bounds include the entire gradient and its hover shades, not just the endpoints. Names, numbers and separators stay opaque. Same-project headers retain the same gradient across focus changes.

Agent logos keep their textures and freshness opacity on small neutral backings; the waiting marker keeps its semantic color and freshness treatment on its owner-approved small neutral backing. Project assignment, unknown-project neutrality and the final whole-pane focus outline are unchanged. Discarded: retaining the reported strip, a flat saturated header (rejected in the native preview), tinting terminal content, and independent gradients inside each control. A low-intensity full-header tint is visible over a much larger area than the original 3-point stripe without competing with the terminal contents.

Directory selection follows D78: unusable published metadata vetoes discovery; otherwise native local information precedes the local report. Native observations occur at most once per second on existing frames, including hidden tabs; the cue adds no periodic idle repaint. Git and canonicalization run in one bounded background worker, coalescing the latest request per live pane. Source/generation checks reject results after navigation or closing. Identity is transient and changes no program or saved launch directory. Directory reports carry an observation revision: comparing path text alone misses A → native B → newly reported A, because the final text repeats. Resets advance the revision too; they do not discard a newer usable native sample.

Accents derive from the effective configured palette, including overrides. Live project assignments survive focus and layout changes; distinct suitable resulting colors are used while available. A finite or monochrome palette cannot encode unlimited identities, so names and full-path hover text remain available. A narrow header clips project and session text inside its existing label region without moving controls or changing console geometry.

**Why.** Several projects can share one tab, and their task titles do not reliably identify them. A shared border has two neighbors; each pane’s own header unambiguously belongs to one console. Shared Git metadata groups worktrees without guessing from prompts, session titles or remote URLs.

**Discarded.** Replacing the session title loses task context. Full-console tint alters program colors. Manual project catalogs and per-project settings duplicate directory metadata. Hash-only color assignment collides even with two projects. Per-frame Git blocks painting; a permanent cache keeps stale directory identities. Recurring repaint timers wake an otherwise resting window for a decorative cue.

**Limits and accounting.** Outside Linux, automatic directory information depends on local OSC 7 reports. An unreported silent directory change is noticed on a later ordinary frame. Native metadata/source/result/color bookkeeping has its own named frame span; header painting stays in the existing strip span. Background subprocess time is never subtracted from a frame. Historical measurement batches disable discovery; the explicit project probe injects source directories but resolves real temporary repositories and paints real badges.

The size guard measures the core at 647 production lines, unchanged. Moving watcher construction to the resource factory lowers the aggregate Window implementation from 1617 to 1611; project refresh remains a free function rather than growing that already-ratcheted type.

## D80 — Agent signals are declared independently of the panel publisher

**Hook publication amendment (November 2026).** Hook publication changes the panel document only. It does not submit or reinterpret agent signals (D90).

**Decided for [#16](https://github.com/mashware/ivy/issues/16).** Client identity, activity and
human-input requirements belong to a transient pane-local signal, separate from the panel's
producer-owned document. Enabling an integration is explicit per client in Connect agents.
A signal never claims, patches or retires somebody else's panel. Its verified input-required
claim combines with panel attention using the existing priority order; withdrawing it cannot
withdraw the panel's claim.

**Why.** A one-shot hook cannot patch the connection that already owns Flow's panel, and the
file transport is deliberately suppressed while that connection holds it (D43). Replacing a
published summary with a permission notification would discard the information this app exists
to show. Hooks know lifecycle, not the work's semantic progress (D6): a completed turn or an
error alone therefore does not become task done or task blocked.

Codex 0.167.0 already publishes explicit foreground lifecycle snapshots through OSC7501,
including permission/question/auth blocking kinds and native withdrawal. This is structured
producer metadata, not interpretation of its title or screen. Native frames update one bounded
pane-local register; untagged clear withdraws that register only. It does not identify separate
sessions sharing a PTY or authenticate a client. Missing cleanup remains stale information,
never an inferred answer or successful completion. Disabled integrations consume no signal.

Claude Code 2.1.319 uses the same native root snapshot protocol after an explicit capability
reply; that negotiation also removes its duplicate title activity prefix. OpenCode 1.18.31 uses
an official TUI plugin scoped by the visible route and authoritative local SDK pending lists,
with complete snapshots serialized on a separate connection. Their concrete contracts must
be verified before adapter code and their supported behavior proved with the real client before
being advertised. A connection ending is loss of its reporter, not proof that a question was answered; retain
its last wait as stale/unconfirmed. A connection alone cannot take presentation authority: a
complete eligible snapshot is required. Incomplete child/pending hydration cannot publish a clear.
No transcript reader, process-name recognizer, generic daemon or automatic client-config editor
is introduced. The guide exposes reviewed setup instructions and preserves existing hooks.

Automatic capability replies reuse the existing PTY write queue only when it has no backlog;
at most one reply is deferred. They cannot spend or clear the person's refused-input state.
A fresh enabled native declaration can replace a disconnected TUI reporter; an untagged clear or
capability probe cannot withdraw that reporter's unconfirmed wait. Transient signal channel queue-full/timeouts carry an explicit retryable flag and retry
on the same connection; legacy tool replies keep their shape. Repeatedly reconnecting while a
window is not draining would fill its finite connection slots with undelivered close notices. A permanent adapter refusal stops reconnect attempts until the client is explicitly restarted, so a disabled integration
does not wake an idle window forever.

**Presentation.** Supported identity selects a bundled logo; only explicit active state animates
it, and hidden/inactive windows arm no logo-only repaint. Removing a duplicate spinner is a
supported, explicitly reviewed client title setting, never a guessed substring rewrite (D69).
Manual names retain priority. All braille patterns U+2800–U+28FF in title text are drawn from
their dot masks, so the observed U+2838 is independent of proportional-font coverage. This is
rendering and does not interpret braille as activity.

Pane-title and tab-label point sizes are independent settings, default 12, range 8–32. Their
resolved metrics drive both drawing and shared geometry, including hidden-console PTY resizing.
They reuse the existing preserving save/apply boundary rather than a second configuration path.
The console number remains unchanged.

**Discarded.** Forced hooks for Codex duplicate a more precise native source. A custom OSC for
all hooks assumes terminal access command hooks may lack. Directory-wide OpenCode server
events do not identify which session the person is looking at, so they cannot replace TUI state. Changing only title font sizes
while retaining fixed strip heights clips text and separates paint from input. Machine-only
proportional fallback leaves the reported braille fix dependent on installed fonts.

**Evidence boundary.** This records the design, not completed integration. Per-client versions,
fixtures, native checks and measurement accounting travel with each delivery; headless tests or
source documentation alone do not establish a working desktop integration.

## D81 — Startup recovery is optional and only the initial window owns its snapshot

**Decision.** `startup.restore` defaults to `none`: normal startup opens exactly one tab with one
fresh shell, even when a saved layout exists. Settings offers `none`, `layout` and `directories`;
the latter includes layout recovery and each console's last known usable local directory. The
preference changes the next launch and whether this initial window records its state; it never
restarts a live console. This intentionally supersedes D16's unconditional startup restoration.
The user's explicit request on 9 November 2026 is the new signal: “one tab with one console” by
default, configurable recovery, and independent additional windows without duplicated sessions.

Read Settings before deciding whether to load the snapshot. Disabled recovery neither reads nor
replaces it. Enabling recovery during a primary run checks the existing file before writing, so a
newer-version veto cannot be bypassed by starting with recovery off. The enabled primary saves its
current arrangement for the next launch. Switching to layout removes directory metadata on the
next successful save; switching to none preserves the last successful snapshot. Format v2 gains an
optional `cwd` per leaf; old documents remain usable, and an older binary can discard the optional
metadata if it saves again. Labels and paths never supply a program, arguments or shell commands.

A short exclusive startup coordinator classifies windows using process-lifetime OS file leases.
Every window retains its active lease; only a launch with no other live lease is primary. Later
windows always open one fresh console and never read or replace the primary snapshot. Roles do not
transfer: a secondary that outlives the primary still prevents a third launch from duplicating its
workspace. Process death releases the lock; stale files are removed only after acquiring their OS
locks, never by guessing from a PID or file age. On Unix the locks are process-scoped POSIX record
locks: descriptor-scoped locks can survive a parent's crash in a concurrently forked child before
exec, even with CLOEXEC. The application has one main window per process, and tests use real
processes for that ownership boundary. Secondary windows are intentionally transient in this
delivery, approved explicitly before implementation. This replaces D16's last-writer-wins
limitation without adding a workspace registry or selector.

**Lease inspection amendment (#225).** On Unix, existing leases are opened read-only and probed
with a nonblocking POSIX read lock. Every live owner still retains an exclusive write lock, so the
probe preserves active leases while allowing a readable obsolete file owned by another user to be
removed from the user's writable lease directory. Requiring write access to inspect an old file
disabled both restoration and saving after a root-owned `0644` lease was left behind on macOS.
No owner or permission is changed: unreadable leases and failed cleanup still refuse ownership.
Discarded: deleting files after a permission error, which cannot establish whether a lease is live,
and changing permissions automatically, which requires authority Ivy should not need.

Directory recovery uses D78's local sources and precedence, reusing the ordinary-frame project
observations instead of introducing a polling timer. A final observation before clean exit covers
recent navigation. An explicitly remote or unusable report clears local eligibility. A temporarily
missing native observation preserves the last known local path. Paths must be absolute, bounded
and free of control characters; they go to the child's cwd builder, never through a typed `cd`.
Native discovery is Linux-only; other systems need usable local directory reports and native host
validation before parity can be claimed.

An unavailable saved directory opens a fresh shell with a console-local explanation. Its intended
path remains separate from the fallback shell so an unrelated geometry save does not erase it.
Typing into that console, clicking its notice or closing it explicitly takes over the fallback and
forgets the failed target. No repeated attempt runs inside that launch. The existing 64 KiB document
ceiling is checked against the final serialized bytes before any temporary or replacement is
created: individually bounded directories can otherwise exceed it when combined. The previous
snapshot survives a size failure.

Historical measurement controls retain their explicit tab/pane requests, seeded layout batch and
overwrite-veto witness. Automatic directory capture is disabled there; ordinary capture has its own
named report row and is discounted from the frame. No frame-cost result is claimed from unit tests.
Moving startup selection out of the core keeps D28's module limit; persistence execution and restored
leaf construction use focused free functions instead of expanding already oversized type impls.

**Discarded.** Unconditional recovery contradicts the requested default. Global last-writer-wins
can erase another live window's arrangement. Promoting a secondary after the first closes makes its
role depend on close order. PID-file expiry can mistake a recycled process for a live window or an
idle window for a dead one. A named-workspace registry and restoring every window require a product
selector outside this delivery. Prompt parsing and replaying shell commands violate the existing
metadata/input boundary. AI recovery is a separate delivery of #19: this change neither advertises
a sessions mode nor installs client integrations.

## D82 — Context menus follow their invocation and keep the console visible

Rename dialogs are centered on the viewport. Console context menus retain the opening pointer
position with the pane identity and are constrained inside the viewport. They keep the existing
pane actions and add Copy for a selected terminal range and Paste for the ordinary clipboard.
The menu has no heading or Cancel row. Two adjacent split buttons at the top reuse the titlebar
icons; text actions occupy full-width rows with a hover highlight. Opening or dismissing a menu
preserves the selection. Escape and outside clicks dismiss it, with
the complete gesture retained until release; input ownership does not multiply console opacity.
This extends D62 with the user's explicit request for an ordinary pointer menu, rather than a
floating dialog in a location chosen by the interface engine.

Linux mouse selections publish PRIMARY, independently from CLIPBOARD. A middle press pastes PRIMARY
into the clicked console. Mouse-reporting programs keep right and middle gestures unless Shift is
held, the existing terminal escape hatch. Both paste sources use the existing long-line gate and
current bracketed-paste mode; they also revoke auxiliary-console control on successful personal input.

An app-owned adapter uses arboard for native reads and PRIMARY publication. On Linux its
Wayland data-control support is enabled: it performs capability setup before accepting operations,
and uses X11/XWayland when native data-control initialization is unavailable. This avoids creating
a second focus-based smithay subscription after the gesture whose serial that subscription needs.
The pinned smithay client has no readiness or publication acknowledgement API; its first store
could otherwise silently disappear. Arboard owns its connections and needs no borrowed display
or retained native window. A pure Wayland compositor with neither data-control nor reachable
XWayland is explicitly unsupported; PRIMARY is never substituted with CLIPBOARD.

One worker reads desktop data outside the interface thread, retains the requesting pane identity,
and wakes the reactive window on completion. Delivery expires after two seconds; late text never
becomes input to another console. No replacement read/worker starts while an owner is still
answering. PRIMARY publication coalesces to the latest selected text and expires after two seconds; a queued publication is discarded after the window loses focus, so a stalled read cannot later replace another application's selection. **D57 declaration:** shutdown
disconnects and abandons this worker because external reads and library destruction may stall;
no interface thread joins it. The idle receive ends when its sole Service sender is dropped.

No adapter worker runs in historical measurement batches. Delivery is counted within the existing
integrated chrome span and selection work within the mouse span. Enabling the arboard feature is
Cargo-global: egui's existing native clipboard construction also gains data-control capability
setup before the first frame. This is not measured here and no startup/frame-cost claim is made.

Discarded: an untagged RequestPaste interpreted through keyboard modifiers (loses the requested
pane and mouse-paste intent), replacing CLIPBOARD on selection (PRIMARY has its own desktop
contract), external clipboard tools (installation/backend requirements), a new framework/native
protocol fork (arboard already provides the required selections), a second asynchronous smithay
client (first-operation readiness race), and replacement workers for stalled owners (retains native
resources without a ceiling). Native inter-application exchange is checked separately from
headless geometry and routing tests.

The native startup resource factory now belongs to opening.rs, keeping clipboard construction out
of the window core. The stateless primary-press reader is a free pointer function; the measured
Window implementation ratchet falls from 1611 to 1605 production lines.

## D83 — PR checks validate source; release preparation validates native packages

**Decision.** Ordinary pull requests run formatting, workspace lint and tests, documentation-link
checks and portable packaging-logic tests on Ubuntu 24.04. Full Ubuntu, Arch and macOS packaging
and installation checks run only through explicit manual dispatch on the selected revision.
The same workflow retains its complete platform matrix and publication dependencies; publishing
still requires the explicit `publish` input, matching version/tag and unchanged integrity checks.

**Why.** Full release compilation, package creation and installation on every PR update delays
iteration even though merging does not publish a release. The owner explicitly accepts that
platform incompatibilities can reach main and surface during release preparation, when attribution
may span multiple merged changes. Source checks preserve fast feedback without claiming native
package compatibility. No elapsed-time improvement is claimed without hosted measurements.

**Discarded.** Keeping the full matrix on PRs preserves earlier detection but retains the delay
this change addresses. Removing platform checks from publication would allow unvalidated packages
to reach users. Separate release workflows or repeated conditions on dependent install jobs add
configuration duplication: dispatch-gating the existing independent build roots reuses their
dependency graph. Reusing artifacts from earlier runs would separate publication from the selected
revision's fresh complete validation.


## D84 — AI recovery uses declared IDs and fixed client launches

**Hook publication amendment (November 2026).** Panel hook authorization reuses native client recognition independently of the Linux-only recovery setting. macOS adds native peer audit-token, process unique identity, PTY ancestry and kqueue lifetime evidence; it does not enable session recovery (D90).

Startup sessions mode extends D81 only when explicitly enabled. Save an allowlisted client and
validated exact ID beside the local directory; never save a program path, argument list, environment,
prompt or transcript. Restore with the installed official client’s fixed resume arguments and
existing authentication. Resolve trusted absolute PATH entries before applying the restored cwd.
An unavailable target remains inspectable, with a notice and a retained retry target until takeover.

**Why.** A person wants the same conversation across restarts, while a saved file is not authority
to execute an arbitrary command. Titles and painted output are display data, not canonical IDs.
Registration is separate from panel publication and never claims or retires another producer’s panel.
The endpoint scopes the pane; native peer/ancestry/foreground/birth/lifetime evidence scopes its
reporter. Unsupported native or remote topologies remain unverified rather than guessing.

Codex 0.167.0 communicates SessionStart at a turn, not every visible conversation selection. The user
explicitly accepted last-hook recovery on 9 November 2026: switching without sending a message may
restore an earlier reported conversation. Settings and setup instructions disclose that limitation.
No hook birth ordering is presented as a canonical Codex UI revision. Root death still invalidates
that root’s registration; stale roots cannot overwrite a newer invocation.

Claude 2.1.319 and 2.1.321 use an explicitly installed official Mods live-session getter; OpenCode 1.18.31 uses
its official TUI route getter. Serialized producer reads assign their own monotonic sequence before
asynchronous delivery. A bounded client-side probe publishes only changed state or a requested
final challenge; unchanged probes do not request an Ivy repaint. A clean exit requests a fresh
acknowledged read before tearing down PTYs, with one shared deadline. If it cannot be confirmed,
keep the usable directory rather than recording an unconfirmed conversation. Crash recovery uses
the last successfully saved snapshot. No HUD/StatusLine replacement or silent client setup occurs.

**Discarded.** Latest-server-session and title guessing can reopen unrelated work. Generic hooks
do not prove current UI selection; Claude’s pre-spawn awaits can reorder helper births and Codex
can emit no hook on an idle switch. Provider APIs change the subscription path. Per-provider keys,
permission bypasses, prompt replay and arbitrary persisted arguments are outside this feature. A
modified Codex client would expand scope beyond the user’s accepted last-hook limitation.

**Evidence boundary.** Source contracts and installed versions establish the selected approach;
real packaged adapters and client lifecycle probes must pass before functional support is claimed.

The helper is located through an absolute `IVY_SESSION_HELPER` path supplied by Ivy, not a bare
name searched from a client’s potentially relative PATH. It is a transient executable-location hint,
not a credential and never saved in the snapshot. The existing PANEL_ID/PANEL_DIR endpoint resolver
remains the routing contract. A reporter attach proves a process binding, not a sampled identity;
only a subsequent live sample can confirm a pending restoration. Verified client-declared cwd takes
precedence over a launch wrapper’s stale directory in sessions mode, with the explicit remote or
unusable-directory veto preserved. Ordinary-shell D78 precedence remains unchanged.

Codex ordinary direct TUI configuration flags are structural options, not dispatch selectors in
0.167.0. Capture accepts up to four `-c` / `--config` value pairs after `--no-daemon`, optionally
followed by exact-ID resume. Values are streamed with a byte bound and discarded; unknown modes,
flags and later prompt payloads remain rejected. Restore still uses only its fixed arguments. This
permits explicit per-run client setup without modifying personal configuration. Codex hook output
is a different strict protocol from the native helper's JSON reply: the observational static guard
discards that reply and cannot block a turn when optional recovery is disabled or unavailable.

Official Codex command hooks start a new OS session. The reporter helper therefore needs bounded
descendant attestation, separately from strict foreground inspection. Only a recognized native
client ancestor in the pane's actual foreground group, SID and terminal grants authority; its root
is rechecked strictly. Detached helpers cannot promote foreign, background or nested roots.

A failed live client is distinct from an already opened fallback shell. The former is replaced on
explicit takeover, including clipboard and mouse paths; stale TUI input modes cannot reach the new
shell. A held paste remains a pending user question until answered or dismissed explicitly.

## D85 — The board replaces the Overview: project roadmaps from files, blocks owned by their console

**Decided (November 2026).** The Overview screen (D68) is replaced by one global board, grouped by
project identity (D79). Each project section starts with that project's roadmap and holds one card
per live console working in it. The board shows three kinds of content, and each kind has exactly
one owner:

- **A project's roadmap comes from one of two sources.** A plan published by a producer over MCP
  wins as a whole; the file named in settings (`roadmap.<name> = <path>`) is used only when there is
  no published plan or it is retired. There is no merge. The published plan is kept in Ivy's state
  per project and restored after restart. Otherwise, Markdown task items from the configured file
  form the plan; the first unchecked item is in progress. The board shows a window around it
  (`roadmap.window`, two done and three next by default; retired by D93 on 17 November 2026). File changes repaint the board. A project
  named in settings has a section even with no console open; a stored plan alone does not create a
  section.
- **A console's card starts from its existing panel document**, so every console with a panel is on
  the board without publishing anything new.
- **Blocks are published by producers**, over MCP or as a file in `PANEL_DIR`, and belong to the
  console that published them. A producer reaches only its own blocks. A block leaves the board when
  its console closes, and an old block is dimmed by its declared freshness.

**Why.** The Overview repeated each panel cut to three lines; it knew nothing the panels did not
already say. The user wants a surface that shows what they chose to watch. The design follows from
three facts:

- MCP is scoped by `PANEL_ID`, so a shared canvas several agents edit in turns would need either a
  hole in that rule or a lock nobody can enforce. One owner per block needs neither.
- A roadmap belongs to a project, not to a console. Several consoles in one project share the
  published plan; the repository file remains the fallback for projects without a publication.
  Concurrent edits to that file are git's problem, not the app's.
- An agent acts only on its own turns. State that changes outside it — a pull request merged on the
  forge — would go stale in an agent's block. A script on a loop keeps it current at no token cost,
  which is why the file transport is part of the contract and not an afterthought (D17).

The app learns that a console closed from its own close path and from the reading's end (D38); no
producer has to report it. A console whose agent exited while its shell stays open is the case that
freshness covers.

**What was discarded, and why:**

- **An embedded web view the agents control.** It runs content somebody else wrote inside the app,
  with network access, which D72 already refused for the document viewers, and it adds a heavy web
  engine dependency on three platforms. Blocks are rendered natively and inert.
- **Turn-based editing of one shared canvas.** It contradicts MCP scoping and needs a lock with no
  owner. Per-console ownership removes the need.
- **Per-tab boards.** Most tabs hold one or two consoles; the board would be nearly empty.
- **A roadmap published by an agent without a shared project store.** Project-scoped publication
  avoids competing per-console copies; persistence in Ivy's state keeps the plan after its producer
  exits and across restart.
- **Showing a large roadmap whole, or one chosen section.** A whole file does not fit a block; a
  section needs a convention every repository would have to adopt. A window of milestones around the
  one in progress works on any task list.
- **The app running a refresh command for a block.** It would be code execution from a published
  document. A script the user starts in a console does the same job inside the existing trust model.
- **Charts.** No case yet; they can come later as one more block kind.

**Limits.** Project identity outside Linux depends on OSC 7 reports (D79); without one, publishing
is refused until the project resolves. `plan_set`, `plan_get` and `plan_retire` act on the caller's
pane project; the optional `repo` argument is ignored and replies name that project. Published plans
use a bounded JSON document with five item marks (`done`, `current`, `pending`, `wait`, `block`),
while the Markdown fallback has no stable item ids. A roadmap with no items shows a notice, not a
guessed progress. Board work is named in the frame report and subtracted once (D13); no performance
figure is claimed before repeated valid runs.

**B1 — how the first board is built ([#40](https://github.com/mashware/ivy/issues/40)).**

- **The Overview is renamed in place, not rebuilt beside it.** Its input layer — the screen owns
  the keyboard and pointer for the whole opening and closing frame, closing restores the original
  focus, activation goes by stable pane name — is D68's hard-won part and is kept as it is. What
  changes is the projection (a grouping axis) and the painting (sections of cards instead of rows).
  `key.overview` becomes `key.board` with no alias: Ivy has no users to migrate.
- **A section is a Git repository.** The grouping key is the project key D79 already resolves, not
  its display name, which is a presentation rule. A console whose identity is a plain folder keeps
  its header badge (D79) but sits under «Other consoles»: D85 defines the project as the repository
  that contains the roadmap file, and a folder has none. The mismatch between header and board is
  accepted on purpose.
- **Order.** Sections by the strongest claim among their consoles, with D68's order of claims; ties
  by the first console in tab and tree order; «Other consoles» last. Inside a section, D68's order.
- **Cards have a fixed size.** D68's rule that long metadata cannot grow a row over its neighbours
  carries over to cards. A card shows the console's name and tab, the claim only when there is one,
  the phase, the freshness warning and the first three nonempty body lines literally. The filler «No
  attention requested» is gone; «No published state» stays, because an empty card would say nothing.
- **Arrows follow the grid.** ←/→ move one card in reading order, ↑/↓ one grid row in the same
  column, clamped to the last card of a shorter row.
- **Measured with two projects.** The board batches turn on the existing project probe so the board
  they measure has two repository sections with headers — the shape it has in use — rather than
  one. The report counts repository sections only («Other consoles» never passes for one), as the
  most any single frame grouped, and declares as `ungrouped` the frames projected before the
  projects resolved. The grouping cost is inside the board span either way. The probe never
  produces a folder identity, so that path is covered by tests, not by the bench.

Discarded for B1: a new module beside the Overview (it would re-prove the input rules), grouping by
display name, sections for folders, and variable-height cards (they arrive with B3's blocks).

## D86 — Terminal queries return audited protocol replies through the console's ordered input

**Decided.** The host returns the engine's DSR status/cursor reports, primary and secondary device
attributes, character dimensions, and numeric mode reports through the same `Pty::write` path as
keystrokes. Each console pumps its own replies, including consoles outside the visible tab. The
work stays inside the existing interpretation stopwatch, so it is not attributed to the framework.

**Why.** The engine already produces these replies, but the original screen used a no-op listener
and the later title listener still discarded them. A raw, echo-disabled program received nothing
for six query families; positive controls through the existing writer delivered the exact bytes.
Programs waiting for those responses cannot complete the protocol exchange.

**Trust and bounds.** Only the audited numeric reply shapes are accepted, even if a future engine
adds another `PtyWrite` origin. No program-controlled title or answerback string returns as input.
Color, clipboard and pixel-dimension requests remain unanswered. Collection is enabled only during
the host's reply-aware parse call; passive screen parsing never retains replies. A temporary batch
has a 16 KiB ceiling. Overflow or an unsupported shape refuses the entire batch; the host reports
that error through one shared worker with a one-message channel, shared with the existing
input/output diagnostics: automatic replies can themselves trigger a backlog notice. The frame never waits for stderr;
when the sink stalls, additional diagnostics are dropped. A worker creation failure disables this
diagnostic channel. A refused writer reports its existing input refusal and is also logged. Nothing is
retried later or stored in an unbounded second queue. The 16 KiB limit is a conservative resource
bound, not a measured performance claim; a query flood can exceed it and receive no replies.

**Discarded:**

- Writing from the PTY reader or directly to the master: bypasses D29's ordering and writer.
- Returning every event/string the engine provides: turns a future reflection feature into input
  injection without a host decision.
- An unbounded pending-reply queue or blocking the frame until space appears: reintroduces the
  resource growth or freeze D60 and D29 prevent.
- Keeping overflow for another frame: needs a separate backpressure protocol and ordering policy;
  this correction keeps the existing writer's explicit refusal model.

**Evidence and limits.** Regression tests assert bytes written by the raw-mode child into its own
file, through the application's console pump; screen output cannot impersonate those bytes.
Chunk-boundary, resize, refusal, flood and non-reflection checks cover the bridge separately.
No application-specific failure or macOS/Windows behavior is claimed from the Linux reproduction.

## D87 — Ivy owns its runtime and configuration names

**New signal, 10 November 2026:** the owner found the former product name in `PANEL_DIR`
and requested that it disappear throughout Ivy. This supersedes D71's retained internal names.

New consoles use `ivy-<PANEL_ID>` runtime directories and the Windows endpoint uses the same
prefix. MCP identifies the server as `ivy panel`; diagnostics, examples and Settings use Ivy.
The open contract keeps `PANEL_ID`, `PANEL_DIR`, `panel.json` and `control.sock` unchanged:
producers discover their endpoint through the inherited directory, not a hard-coded prefix.
Already-running consoles retain the directory they were launched with until they close.

Configuration and layout use `$XDG_CONFIG_HOME/ivy/` (or `~/.config/ivy/`). Before normal
startup reads either file, if the Ivy directory is absent, move the former product's entire
configuration directory into place. This preserves settings, layout, recovery and other files
without interpreting or rewriting them. An existing Ivy directory wins and is never merged or
overwritten. Failure to move stops startup with an error rather than opening factory-fresh and
risking a subsequent overwrite. Measurement sessions skip migration and retain their existing
settings and persistence gates. Migration is the only production compatibility use of the former
name; historical knowledge and negative regression assertions are not product branding.

**Discarded:** retaining old paths (the reported defect); starting with empty preferences (data
loss); persistent aliases or reading both namespaces (keeps the old identity alive); merging two
configuration directories (no safe rule for choosing between conflicting settings or layouts).
Historical issue links point to `ivy-history-private`, where those original numbers belong,
rather than unrelated issues with the same numbers in the current public repository.

## D88 — The MCP bridge asks its agent to publish, only where it can keep the promise

**Signal, 12 November 2026 (#51):** an agent connected to `panel-mcp` learned nothing about the
panel unless it loaded the tools, and harnesses may **defer** MCP tools — listed by name, with no
schema — so an agent that never loaded them published nothing and the person saw an empty panel.
Bellwether, which paints the same contract inside Herdr, had already measured and fixed this.

**Decision.** The bridge answers `initialize` with the protocol's optional `instructions` field,
which harnesses that honour it place in the agent's own instructions. The text asks the agent to
publish with `panel_set` at the start of a task, before long stretches and at every stop; to keep
`attention` true; to key the lines it will patch and patch only after publishing on the same
connection; to publish again after a restart before patching, without a claim nobody holds any more,
and only when the document read back was its own; and to pass through as printed a panel document a
workflow it follows renders, without the buttons the agent would not run itself — `actions` are
commands for the person's shell, and a document printed by a script of unknown origin is the
untrusted panel the hard rules name. It is sent only when the bridge starts with a **whole
identity** — `PANEL_ID` present, non-empty and UTF-8, `PANEL_DIR` present and absolute — the same
rule its tool calls apply, where the control channel binds, and metadata says `PANEL_DIR/control.sock`
is a socket. This metadata check is presence only: it does not connect, check liveness or path
length. Outside those conditions, `instructions` is absent. On Windows the channel does not bind
until F2.7 (**D43**). A tool call with a broken identity —
one variable carrying something but the identity not whole — is refused with its own message, naming
the client's forwarding rather than «not in a pane»; both absent or empty is a program outside any
pane. And because Ivy must never hand out a directory its own bridge refuses, the runtime directory
ignores a relative `XDG_RUNTIME_DIR`, as the XDG specification requires, and falls back to the
system temporary, itself held to the same rule (a relative `TMPDIR` falls back to `/tmp` on Unix); a
variable somebody set and Ivy had to ignore is named once on standard error, with the directory used
instead. Where no channel binds, a tool call says so rather than blaming the console. A whole
identity that cannot be reached is refused naming the directory it could not reach.

**Two rules the text carries because of how harnesses use it.** It names *this server's* tools,
because a second panel server may be registered beside this one with the same names. And it tells
an agent working for another agent in the same console to leave the panel alone. A subagent shares
the parent's connection, which holds the pane (D43): one subagent publishing `done` would erase the
question its parent is waiting on. A worker process the parent starts in the same console brings a
bridge of its own: it would be refused at every stop or, publishing first, hold the pane against its
parent. Only that case: an agent another agent started in a console of its own has its own identity
and panel.

The Connect guide asks for this manual prompt only `if this client did not pass you the bridge's
server instructions`. The copied setup prompt and the README quote both carry the same-console
subagent rule and say that an agent started in its own console keeps its own panel.

**Discarded:**
- **Naming all seven verbs** — the `split_*` and retirement descriptions already explain those, and
  the goal is publishing.
- **Presence-only identity, as the tool path had** — two rules in one process: the handshake said
  «no console» for an empty identifier while the first call connected with it; on Windows an empty
  identifier names one machine-wide endpoint. One function, `whole_identity`, now decides for both.
- **Checking that the endpoint answers before promising a panel** — a stale or borrowed identity
  (a multiplexer reattached after Ivy closed, an editor or daemon started from another console) is
  real but rare. A stale one fails at its first call, naming the directory it could not reach; a
  borrowed one from a console still open publishes into that console, and nothing the bridge can
  see tells it apart from its own: it is the launcher that carried the wrong variables. Probing on
  every handshake would buy a message for the first case and nothing for the second.
- **Telling the agent never to publish while a file producer paints the pane** — it would mostly
  stop agents publishing at all. What the text asks instead, after a restart, is to republish only
  a document it recognises as its own: `panel_get` names no source, so the agent judges by content.
- **Reading the environment where the reply is built** — a unit test would inherit the identity of
  the console running the suite; the read stays in `main`, covered by a test that starts the binary
  with its own environment.
- **Accepting an empty or non-text identifier on Unix, where the endpoint ignores it** — the pane's
  identity is the pair (**D7**), Ivy always sets both, and a rule that depends on the platform is
  the two-rule split this decision closed. The session helper shares the same strict whole-identity
  rule. Runtime-directory selection is shared through `panel_contract::base_dir()`.
- **Withholding the instructions under a terminal multiplexer** (`TMUX`, `STY`, `ZELLIJ` set) —
  it would mechanise «workers keep off», but a person running their one agent inside tmux in an Ivy
  console would lose the instructions, and the hold already settles who owns the pane: the first
  producer keeps it and every other is refused by name (D43).

## D89 — The block's fold belongs to the reader

A section header's `collapsed` is its initial state. Clicking its row sets a per-console choice,
in memory only, identified by `key` or, without one, its text. That choice wins across whole-document
republishes. A file producer has no return channel, and flow republishes its folded default, so
producer-owned state would prevent the reader from keeping the train open. Saving it with the
window's geometry would change the layout format for a preference nobody asked to persist.

The last painting owns the header identity and fold value a press toggles: a republish cannot
retarget old pixels. Only a whole painted row is reachable. Unlike a command button, a fold runs
nothing, so it does not require a settled frame; otherwise each toggle would swallow the next click.
Header painting and relayout remain inside the existing panel paint span (D13); no measured bench
batch currently paints these headers. Fold choices are kept until the console closes.


## D90 — An attested hook publishes over the hold it works for

**Decided (November 2026).** `ivy-session publish` accepts a bounded panel object on stdin
and submits `panel_publish` to the inherited pane endpoint. A hook is admitted only when kernel
peer and bounded process ancestry evidence identify the same live native client invocation as
the holder. Source names and PANEL_ID/PANEL_DIR select or label; they never authenticate.
A script can still hold the panel with ordinary tools, but its hold cannot receive hook updates.

**Windows fresh-launch amendment (26 November 2026, #185).** A person may explicitly open
a fresh native agent from Connect agents, after seeing its selected executable, fixed arguments and
working directory. Only that direct PTY child, retained through its actual process handle and checked
against the selected image path, is a Windows hook root. The holder and publisher must have verified
native ancestry to that same live root. Ordinary shell-launched agents keep MCP without this new
hook authority; Windows diagnostic ancestry is not relabelled as Unix foreground evidence.

The launcher starts collapsed after the existing connection instructions. This preserves the
positions of both the observed panel status and the manual setup controls. An expanded form ahead
of that status was discarded because it pushed existing connection help outside the initial viewport;
optional launch configuration must not displace the help already there.

The selected working directory uses D78's opt-in required Windows command-builder mode. No
failed metadata check can silently replace it with the person's home directory. The OS receives
the requested path and rejects it when unavailable; no provider environment or ordinary launch
policy is changed. The dependency patch and native mutation proof belong to this explicit path.

The native recognition catalogue is the finite set of explicitly selected client images, frozen
for each invocation. A second recognized client below the root, including the same executable again,
is a nested invocation and refuses. Generic hook/shell descendants may remain in the chain.
Unsupported or unselected installations and interpreter-hosted agents are outside that recognition
guarantee; this does not isolate malicious descendants or authenticate vendor code under D84.

Windows workers retain concrete bounded holder snapshots and use the existing native wake rather
than application callbacks. A hook reserves the holder connection's existing one-outstanding delivery
slot through apply/discard. Contention refuses promptly; it neither waits for the window nor widens
that ceiling. Apply rechecks holder incarnation, root lifetime and pane revocation; helper exit
after admission alone does not invalidate the hook.

Canonical image resolution can wait on a filesystem beyond named-pipe cancellation. Therefore
native ancestry/image inspection runs in an auxiliary task outside the joined control workers.
Admission allows one unfinished inspection per invocation and eight per application process, with
no backlog. A caller waits at most 750 ms, checking pane revocation every 10 ms. A timed-out or
revoked caller discards its result; the inspection retains its permits until it actually ends.
Only native evidence and a result sender enter that task, never publication or window capabilities.
Shutdown joins connection workers, not these inspections. An unavailable filesystem may retain a
bounded inspection for the process lifetime; process-wide unfinished/timeout/busy/cancel counters
report that residual separately. The limits are engineering bounds, not native timing measurements.

Discarded: removing canonical image matching (does not establish alias/junction equivalence),
waiting for filesystem cancellation before joining, terminating native threads, admitting unlimited
replacement inspections after timeouts, and restricting legitimate hooks to direct/local helpers.

**Why.** Windows already has private MCP, verified peers and native diagnostic ancestry, but none
of those proves Unix foreground client ownership. The user chose only live hooks for explicitly
opened fresh agents, leaving recorded-conversation recovery on macOS/Windows and Windows shared
Codex relay deferred. The explicit root supplies the needed invocation boundary without importing
installation discovery or saved-session machinery. Codex's distinct local-hook launch previews
`--no-daemon` and requires that capability; ordinary commands are never rewritten.

**Discarded:** inferring a client from source/environment/title; adopting all diagnostic descendants
as authority; another peer handshake or secret; generic npm/Volta/script discovery; a direct-child or
single-shell restriction without provider evidence; a comprehensive PE parser or vendor certification
for a person-selected executable. The OS's explicit native loader and the actual retained child image
supply different evidence from a file claiming an executable format. Native installed-client and
mutation proof remain required; compile checks are not runtime verification.

**Native Claude discovery amendment (19 November 2026, #142).** A desktop launch may lack
`~/.local/bin` even though its console shell adds it. Discovery and fixed launches therefore
search the absolute PATH entries first, then `$HOME/.local/bin/claude` for Claude only when HOME
is absolute. The first executable candidate keeps precedence, including refusal of unsupported
scripts; the fallback is canonicalized and must pass the same native-executable check. The
launch PATH is unchanged. This recognizes an installation candidate, not vendor authenticity:
exact executable, native peer ancestry, foreground, lifetime and same-invocation checks remain.
Custom install locations and running older versions after an update are not covered by this rule.

Discarded: adding a directory to the application's global PATH (changes unrelated launches),
accepting any process named Claude (weakens recognition), and retrying another installation after
an unsupported PATH candidate (silently changes the invocation the user selected).


The connection thread admits into the same bounded control queue as holder publications and
patches, wakes the window and replies immediately. This promises admission, not painting.
Application rechecks the live root and hold incarnation. Root death, foreground takeover,
release, republishing and retirement invalidate queued authority. The helper itself may exit
after admission. Late refusal is visible on the identity row.

A hook replaces lines, phase, attention and freshness. Holder header, roadmap identity and other
metadata remain. Incoming actions never introduce commands. Existing actions survive only when
the hook still declares `wait`; otherwise they are cleared. The merged document must fit the
normal ceiling before any state changes. The identity row displays `<source> · hook` without
changing ownership. Holder set replaces all, patch uses the current document, disconnect retains
it and retirement clears it.

Native macOS evidence is isolated from Linux recovery. Unsupported or inconsistent proof is
refused. Same-user code injection and modified hooks remain within D84’s same-user trust limit;
this authenticates process lineage, not vendor code. Native Mac client scenarios remain a required
manual verification; cross-compilation alone establishes no runtime claim.

`overlaid` counts committed overlays; `overlays_refused` counts admitted overlays refused in the
window. Admission refusals cost no frame and are excluded. Application belongs to the existing
control span. No hook workload has measured its frame cost yet.

## D91 — A borrowed publisher is labelled by ancestry, never refused

A daemon or multiplexer can carry another console's panel identity. Environment values select an
endpoint; they do not show whether its publisher belongs to that console. On Linux and macOS the
listener therefore captures its kernel peer once and asks a revocable, pane-bound ancestry callback
off the frame. A stable walk to the original PTY child is inside; a stable chain reaching the native
root without that child is outside; failed or unstable evidence is unknown. The walk is bounded to
64 hops, checks parent and birth identities, and rechecks peer/root liveness before returning.

Only outside adds `published from a process outside this console`. Unknown allows publication
without a label. Session authorization is unchanged. The label belongs to the accepted document,
not its connection reservation: disconnect preserves it, patches keep it, a newer valid file or
full publication replaces it, and retirement clears it. It is an application row even when the
producer hides its header, and it is absent from serialized producer documents.

**Accepted limits.** An editor still descended from console A can borrow A's identity without a
label. A multiplexer detached from that ancestry can label an intended publisher outside. Native
snapshots cannot make the whole process tree atomic, especially on macOS; detected instability is
unknown. A same-user ptrace-capable process may counterfeit evidence and can already write the
unlabelled panel file. These are diagnostic limits, not authority grants. Windows remains file-only.

**Discarded:** refusing foreign ancestry (breaks intended detached publishers); using foreground
session authorization (rejects ordinary background descendants and conflates failure with outside);
producer-supplied PID or labels (untrusted); storing the label on the holder (loses it on disconnect);
process scans or per-frame inspection (unbounded or frame-blocking work without a consumer need).


## D92 — Codex mobile and panel tools share one conversation owner

**Amended 25 November 2026 — #173.** Private Codex route storage is supported on Windows
independently of the shared-conversation relay. Route directories and each opened/created file
use a current-user-owned protected user/SYSTEM DACL, checked through the handle used for data;
capability files and the directory reject reparse points. Persistent lock files are separate
from synced, atomically replaced records and thread mappings. Existing storage with an unsafe
ACL is refused rather than silently repaired. Runtime capability files deliberately do not
support cloud/filter-backed reparse storage; this does not change the document viewer's policy.
The pathname namespace and other processes of this user retain the existing per-user trust boundary.

Windows shared-panel routing remains unsupported, with that reason printed at ordinary interactive
launch. Official Codex documents a Windows AF_UNIX daemon endpoint and a proxy that accepts it;
upstream transport support alone does not identify the intended Ivy pane. A route does not carry
the expected app PID required by Windows producer authentication, and the shared daemon cannot
supply it from its inherited environment. Native relay console lifetime and daemon/proxy attachment
also remain unverified. The launcher preserves the original command and status, never adding
`--no-daemon`. This delivery keeps route v1 unchanged and does not port session recovery.

**Discarded:** enabling the relay after storage alone (omits per-route authenticated app identity
and native lifetime proof); declaring the upstream Windows transport unsupported (the official
client documents AF_UNIX); inheritance-only file privacy (does not validate an existing file);
truncating snapshots in place or deleting before replacement (exposes incomplete state to readers);
copying a second ACL implementation (would diverge from private pane-directory policy);
automatically fixing existing ACLs (changes ownership/access without the caller choosing it).


**Amended 25 November 2026 — #251.** The official-server connection accepts an
individual WebSocket frame up to the existing 64 MiB message ceiling. The official
Codex 0.172.0 proxy emitted the plugin catalog as one 48,300,053-byte frame; the
relay's default 16 MiB frame limit rejected its header even though the complete
message was within the supported envelope. Frame and aggregate-message limits are
independent, so setting only a message ceiling does not make that response usable.

The upstream limit remains finite and independent of Codex version numbers. The
local client's incoming-request limit, authentication, routing proof, shared owner
and byte-progress health checks keep their existing boundaries. A large response
must still pass through normal routing to the official client; accepting its header
alone does not establish that it was delivered.

**Discarded for the catalog amendment:** disabling remote plugins or editing user
settings to shrink the catalog (removes available functionality); an unbounded
frame/message size (removes the transport's memory envelope); extending health
timeouts (does not affect header-length rejection); a version allowlist (catalog
size can change without a CLI update).

**Amended 23 November 2026 — #224.** On macOS, the bounded official
`app-server daemon start` launch disclaims Ivy's native process responsibility at exec.
The reporter closed Ivy and confirmed that its main process had exited, while two
official Codex daemon processes still reported Ivy's executable as their responsible
path. The surviving MCP bridges instead reported ChatGPT. A daemon's responsible PID
being itself after its original parent exits does not remove the cached application
attribution, so PPID 1 or `setsid` does not establish an independent application lifetime.

Only this persistent-service launch uses the native adapter. Argument/environment storage,
absolute PATH candidates and spawn attributes are prepared before fork; the final hook replaces
the already-prepared child with `posix_spawn(POSIX_SPAWN_SETEXEC)`, retaining its PID,
stdio, directory, signal setup and the existing timeout/reaping path. The dynamically
resolved `responsibility_spawnattrs_setdisclaim` SPI follows LLVM's spawn-time interface;
an unavailable capability or failed setup refuses the launch rather than silently
claiming separation. Ordinary shells, remote TUIs, proxies and explicit admin commands
retain their launch behavior. New permission requests belong to the resulting service's
native identity; no permission is granted or authentication setting changed by Ivy.

The official shared server remains the conversation owner. Attaching to an existing
server does not repair its old attribution, restart it or stop other clients. A server
already attributed to Ivy can retain the Dock entry until it is explicitly restarted.
Native tests must observe effective responsibility through the actual launch path;
cross-compilation does not prove the packaged Dock outcome or mobile interoperability.
Those acceptance checks remain required on macOS.

**Discarded:** killing all Codex daemons or bundled MCP helpers on exit (disrupts shared
and external clients); adding `--no-daemon` (loses live mobile ownership); merely
detaching the Unix session (does not clear native attribution); relocating the MCP
bridge (the observed bridges belonged to ChatGPT); installing a new persistent launch
service (introduces lifecycle and installation behavior beyond this fix).

**Amended 23 November 2026 — #223.** Relay health counts inbound byte progress, including an incomplete WebSocket frame that yields through read timeouts. Counting only complete messages falsely closed a valid slowly delivered MCP inventory response after the five-second probe deadline: a Pong cannot interrupt the bytes of an already-started frame. Idle connections still receive the five-second Ping probe, and an unanswered probe with no inbound progress still closes after five further seconds. A per-connection failure emits its fixed, payload-free reason before closing; conversation contents, protocol payloads and authentication tokens are absent.

The unchanged relay reproduced the cutoff at 10.045 seconds with 100-byte writes every 500 ms; the same response sent immediately and a healthy twelve-second wait succeeded. Separately, actual Codex 0.171.0 reproduced both reported unavailable/read-only messages after an induced disconnect on a fresh main conversation: its server rejected reattachment because no rollout existed before the first turn. This establishes the portable failure paths, not native macOS incident identity. Byte-progress regression tests use a real decoder and virtual time; silence and route-proof semantics remain unchanged.

**Discarded for the health amendment:** increasing or removing the deadline (hides a dead proxy); requiring a completed Pong while a frame is still arriving (the Pong cannot be decoded yet); automatic independent fallback (loses shared mobile control); changing Codex history or writer ownership in Ivy (those remain upstream).

**Amended 19 November 2026 — #140.** Ordinary supported Codex starts on macOS now select the same authenticated shared relay as Linux. A fresh-window report showed the second console reading the first console's panel even with shell integration present: excluding macOS from the relay while executing Codex unchanged allowed the shared daemon's inherited identity to select the panel. Missing capabilities, failed shared setup and unverified route proofs retain the existing refusal path; Ivy never automatically adds `--no-daemon`. Explicit bypass invocations and ordinary direct panel producers retain their meaning. Shared-launch and refusal fixtures now run on macOS as well as Linux, with native arm64 and Intel panel-control tests required by the PR workflow. Native two-console and phone history/input interoperability remain separate acceptance checks; Linux fixtures and a native package build do not establish them.

**Discarded for the macOS amendment:** retaining unverified ordinary-launch passthrough (reproduces cross-console publication); automatically selecting independent mode (loses live mobile input); treating all detached producers as unauthorized (changes D91's deliberate diagnostic-only boundary). Windows remains outside this relay's supported platforms; the #173 amendment separates private storage from the unverified relay identity and lifecycle.

**Amended 18 November 2026 — #130.** Updating the official CLI to 0.170.0 exposed the exact-version guard as a product regression: the installed launcher silently selected independent mode and mobile could only read saved messages. Shared eligibility now uses bounded CLI capability checks and the actual daemon/proxy protocol, retaining the immutable MCP proof before route binding. Neither CLI nor managed-server version numbers control eligibility. Ivy never adds `--no-daemon` automatically; an explicit user argument retains its meaning. Missing capabilities or failed shared setup stop the launch with a diagnostic. Other platforms and unsupported invocation shapes preserve the original command and diagnose unverified local panel routing. This removes version-based rejection, not the possibility that a future incompatible protocol fails. Native and phone interoperability observations remain version-specific evidence below.

**Discarded for the amendment:** extending the version allowlist (repeats the regression at the next update); automatic independent fallback (silently removes live mobile input); bypassing MCP proof to accept unknown protocols (risks publishing into another console). Active independent conversations are not killed or transferred by this change.

**14 November 2026 — #69.** On the verified Linux/Codex CLI 0.167.2 with app-server 0.167.2 or 0.169.0, ordinary compatible starts use the official shared app-server through a private authenticated local relay. Each conversation's existing Ivy MCP configuration receives a private runtime route capability; user configuration is not edited. Successful starts, resumes and forks bind that capability to the originating console. A loaded MCP client resolves the stable capability again on each tool call, so rejoining from a new console does not depend on Codex replacing loaded-thread configuration.

**Why:** Codex 0.167.2 explicitly excludes command-line MCP overrides from shared-daemon mode. Its loaded-thread resume can acknowledge a request while ignoring its new MCP settings. An isolated real daemon/TUI probe confirmed the first behavior and a saved-history protocol probe confirmed the second. A transparent relay prototype preserved start/resume/fork IDs, directory and model. One shared owner is necessary for live mobile history/input without a second writer.

**Boundary:** route capability is private runtime state, never a pane selector in an MCP tool. Route changes and complete pane calls share a lock; old attachment cleanup cannot revoke a new attachment. Failed, unknown or revoked routes never fall back to daemon-inherited identity. Direct env-based MCP integrations and explicit launcher bypasses retain their meaning. The relay uses the official client and subscription, never a model API or provider key. Version-specific compatibility and actual phone checks must be reported separately.

**Discarded:** simply removing --no-daemon loses independent panel identity; MCP -c overrides select embedded ownership; per-thread direct overrides alone do not update a loaded MCP; a metadata thread ID alone is not authorization; separate remote servers introduce enrollment and lifetime semantics that the shared official owner already handles; guidance-only delivery was explicitly rejected by the owner.

The relay discovers MCP configuration for each requested project, using the explicit launch directory or the saved thread's directory. It merges only its environment leaf; normalized config readback is never reinserted as input. Before binding, a normal read-only MCP call returns an immutable public proof nonce from the actual running bridge. The relay compares it with the pending capability's proof, so a loaded-thread race cannot turn an ignored config override into a false successful binding. The nonce is independent of the bearer path and grants no panel authority. Missing or mismatched proof refuses the attachment without choosing another pane.

Route data is atomically replaced while a separate persistent lock inode serializes complete calls and cutover. Each attachment scans validated atomic route snapshots under the registry lock and releases every prior active route for that exact pane, including an orphan left before its thread mapping committed. Only matching routes are locked; unreadable unrelated records cannot poison another pane. This costs one scan per attachment, outside frame and MCP-call work. The MCP process also releases an obsolete Producer while otherwise idle. Missing or corrupt unrelated records cannot prevent another pane from binding. The original delivery retained an independent safeguard on macOS and Windows pending native verification; the amendments above supersede that launcher policy. Windows private atomic route storage is covered by the #173 amendment; Windows relay support is not claimed.

On launcher exit, an immutable private cancellation marker closes its exact attachment epoch before any route lock or relay join is attempted. A tool operation already in flight can finish, but subsequent operations see that epoch as inactive even if a slow operation delayed ordinary record cleanup. Cancellation takes no route or registry lock, so it remains effective under contention and cannot revoke a newer epoch. The stable public proof remains readable while inactive, permitting a later verified resume. Process reapers are detached and explicitly declared; relay and reader joins have a grace deadline. Cleanup failure is diagnosed rather than presented as successful revocation.

The host manager can install an app-server newer than the CLI. The actual official CLI 0.167.2 starting managed server 0.169.0 was separately exercised for start, exact saved resume with a new epoch, fork and matching MCP proof. That initial delivery refused other server versions; the #130 amendment above replaces that version restriction with capability/protocol checks. Real phone history/input and panel publication after resume were observed with CLI/server 0.167.2; the mixed-server protocol check is separate evidence.


## D93 — The Board follows Bellwether, with independent native reader state

**New signal, 14 November 2026 — #71.** The owner requested the same Board presentation,
information and behavior as Bellwether's Herdr plugin, with executable action buttons deferred.
The reference is Bellwether commit `995b18da65f3bb6775cf18b157aa05410efb07eb`, specifically
`mods/bellwether-board/main.js`; its executable classification takes precedence over older README
prose. This supersedes B1's fixed cards, three-line excerpts and attention/tree ordering in D85.

**Decision.** Keep D85's native global Board and project-plan sources, but present project rows
across Waiting for you, Stuck, Working, Finished and Idle columns. Repository rows sort by their
strongest effective column, then name; Other consoles remains last. A card exposes its producer's
supported content, with Bellwether's initial eighteen-line expansion and compact finished/idle
cards. Reader-owned section and roadmap folds do not alter the published document. Roadmaps show
completed summaries, started milestones and later milestones, with controls to inspect all items.
The configured roadmap window remains the initial detail window and never makes attached work or
remaining content unreachable. Existing published-over-file precedence and configured projects
without consoles remain unchanged.

**Visual reference clarified by the owner.** The supplied folded/expanded Bellwether screenshots
are the presentation target, not merely an example of five-column behavior. Each project shows its
summary, then its expanded roadmap, then its terminal-state headers and cards. Column badges are
local; global Board totals share the first gray summary row, replacing its project name/count. Later project rows retain their names. The separate toolbar and global roadmap button are removed at the owner's request. Plan-only projects omit an empty terminal grid. Use monospaced
reference spacing, state colors, progress tracks, right-aligned count pills and wrapping milestone
cards; retain every pictured item, active ID, attachment, dependency, outside count and source.
The comparison fixture uses the actual reference plan and panels, with executable actions excluded.
The owner further chose yellow Waiting, orange/red Stuck, blue Working and green Finished; console navigation uses one outward diagonal arrow with a “Go to console” tooltip, and the branch and its icon sit at the card's left edge, following the owner's subsequent correction.
Native warnings remain; console identity and honest observed age are disclosed on hover. Vector
marks avoid relying on missing glyphs, without changing D9's sidebar or terminal cell policy. Since
**D103** the arrow and the branch are glyphs of the interface's own bundled icon face, which no
machine can be missing; D9's policy is still untouched.

**Retain string plan items.** Bellwether's reference has four deliverables and one outside item as
plain JSON strings. Dropping them silently changed 4/14 to 4/10 and hid one pending outside task.
Accept a string as literal item text with no invented mark or identity. This is a compatible syntax
addition, not a new field or version. A valid milestone retains a console whose deliverable ID is
not in its item list at milestone level; absent/unknown milestones are off-plan.

**State is explicit.** In first-match order: published wait unless the client explicitly says busy;
published block; published done or explicit native Done; explicit client waiting; stale panel;
explicit busy; otherwise idle. Reuse supported native client declarations, including preserving
ProgramState::Done through the internal view. No phase/body/output parsing or public activity field
is added. Silence alone never moves a waiting or finished card to Idle; warnings and unconfirmed
backing remain visible. This is a Board projection, not a rewrite of sidebar/tab attention.

**Time is observed, not guessed.** Ivy currently receives no authoritative wait-onset timestamp.
Track the effective transition into waiting with a monotonic clock for all live panes, including
while Board is closed. Republishing the same wait retains its start; departure, a fresh shell or
pane closure clears it. Warm begins at five observed minutes and hot at fifteen. After restart the
prior duration is unknown; label the duration as observed instead of treating file age as history.

**Two views share primitives, not interaction authority.** Reuse bounded native wrapping,
color-emoji and link primitives while the Board owns its reference mark/ref/section presentation,
but give Board its own caches, fold choices and painted child hit geometry. The sidebar's mutable
layout and drawn records describe its own pixels. Child links, folds and expansion consume their
click before card navigation; no producer action command is routed from this Board. Available Git
branch text is resolved per worktree directory off the frame, not from the repository grouping key.

**Discarded:** keeping excerpts behind a sidebar drill-down (fails the requested information);
duplicating the wrapping, emoji and link-trust engine (would diverge across surfaces); calling
the sidebar painter a second time (overwrites its input geometry); guessing wait age from publication
mtime or producer dates (republishing is not a new wait); classifying activity from terminal output
(violates D6); replacing existing plan transport/storage (presentation needs no new source).

**Verification boundary.** Observation while Board is closed and all added layout/input/paint work
must be named and subtracted once. Native visual and interaction parity needs a real desktop run;
headless tests alone cannot establish it. No new performance figure is claimed by this decision.

**Presentation correction, 15 November 2026.** At the owner's request, Board fills the content
surface below the tabs (beside the tool rail until #93 removed it; since then over Files too), at every window width. There is no reduced or
resizable Board dock. The covered consoles retain their geometry and pump budget. The duplicate
top-toolbar shortcut is removed; the keyboard binding remains available, and since #93 the Board
is opened from its title-bar button instead of the rail.
Discarded: opening Board in the shared narrow dock, which hides it in narrow windows and makes
its project overview unnecessarily small. Other sidebar tools keep their existing dock behavior.

**Two arrangements, 17 November 2026 — #93, fourth PR.** The design kit (`Board.dc.html`)
lays each project out by milestone: one column per milestone with its progress and «after
<milestone>» resolved to titles, each console's card under the deliverable its panel declares, and
an «Off the plan» column. The owner asked for **both** layouts, behind two buttons at the top of
the Board — «By state» (this decision's five columns, restyled to the kit) and «By milestone». It
began as a trial whose loser would be deleted; after using both on a real desktop the owner kept
them **for good, as the person's choice**, and the last one chosen is remembered as
`board.arrangement` in the configuration file (written when the choice changes, only by the window
the person launched: a measured run and every test window have no file to write to). With the plan
laid out in «By milestone», **«By state» no longer carries the roadmap**: its project rows show the
project, its consoles and its claims, a project known only by its roadmap appears only in «By
milestone», and `roadmap.window` — which sized the roadmap those rows folded open — is retired and
reported as unknown rather than read as a roadmap file called «window». Superseded above by that:
the roadmap folds and the configured detail window, the inspect-all controls, and the
summary-roadmap-columns order of a project row with its plan-only rows; «By milestone» keeps the
milestone cards, every item, each milestone's **done when** and the plan's outside work, which it
lists at the top of «Off the plan». Dropped with the strip, deliberately: the plan's title (the
project header names its source instead) and the fallback that listed producers' section headings
for a project without a plan — the cards already carry those headings. **The arrangement lives in
the configuration file, not in the saved layout**, because it is a preference a person may write by
hand and keep across layouts; the saved layout (D16) is the window's shape. The Board applies the
file's value only when the file changed since it last applied it, so an unrelated Settings save
never undoes a choice made since, and removing the line returns «By state». A file that cannot be
written — a symbolic link, which the store refuses — keeps the choice for the session and says so
at the Board's foot, not only on the console that launched Ivy. Placement reads only the panel's declared
`milestone` / `deliverable` (D6): a console that declares neither lands in «Off the plan», which on
today's producers is most of them. Both arrangements share a kit top bar — totals (which leave the
first project's summary row), a «Claiming you» narrowing, a filter over console, tab and task
names, and Close — and a key hint at the foot; a narrow Board stacks the bar's controls on more
rows rather than hiding the field while its query still narrows. **One projection decides every
state on screen**: the effective column of the rule above paints the card's state row (the kit
pill for wait, stuck and finished; the column's mark for working and idle), counts the project
header's claims and decides «Claiming you», which counts finished because D18 makes «finished,
nobody looked» a claim. The count is taken over the whole Board, so a query can leave fewer cards
than it says. The bar's controls are pressed through the Board's own hit list, like a card's fold,
so one input pass decides what a press reaches; the filter field edits its text in that same
pass, Enter or the first Escape leaves the field and the next Escape closes the Board. The console
headers the Board covers do not take its presses: the header pass ignored the Board's claim on the
frame, so a press on the Board also pressed the header button beneath it — the Git button swapped
the Board for the diff viewer, and the close button closed a console nobody could see. An unpublished console is a dashed card and
an old state nobody refreshes is veiled. The roadmap's source is **named, not linked**: it is a
file path or a publication, and the Board opens only web addresses; an unreadable roadmap says why
in the project's header. Discarded: a per-project choice of arrangement
(doubles the surface for no signal); remembering the arrangement in the saved layout (it is a
preference, not the window's shape, and could not be written by hand); inferring a milestone from the branch or the tab name
(D6); painting the declared `attention` on the card while the columns follow the effective state
(a card read «waiting» in the Working column).

**Large-roadmap correction, 24 November 2026 — #235.** The supplied Board milestone specification
and screenshot show why wrapping every milestone and every item stops locating live work, and why
unbounded child cards can cross a neighbouring column. This replaces only the By milestone reader
presentation above. By state and the saved arrangement preference remain.

Every project plan has one equal-width, non-wrapping, non-scrolling milestone strip, a legend, then
In progress, Ready to start, Off the plan and Opened by you groups. Completion is the literal done
count equalling the item count, including 0/0. Otherwise a declared current item or an exact
milestone/item console attachment activates it; an attached effective claim uses Wait > Block >
Done. Partial progress alone is not activity. All dependencies done makes an inactive milestone
ready; unknown dependency IDs and cycles are unmet and diagnosed once. No title, translation,
branch or panel body establishes a console attachment or claim.

Active columns show relevant items plus at most two following pending items; ready columns show
one pending item. Headers and remaining-item summaries expose all items and the closing criterion.
The strip reveals/expands existing columns, or explicitly opens done/later ones; a second click
restores their default or removes an explicitly opened column. Show all displays the roadmap order
and Off the plan last. It overrides the claiming-column restriction while enabled. Reader disclosure
is per project for the app session; class changes clear temporary full expansion and retain explicit
opens. An all-done message does not hide outside tasks or unmatched consoles.

Expanded columns keep 240 points, collapsed ones 190, with a 20-point gap within a group and
24 points between groups. Expanded columns share surplus width, with card-bearing columns weighted
1.2. When minima do not fit, only the column row scrolls horizontally. Both painted content and hits
are clipped to their column and viewport. Item cards are indented 20 points; milestone-only cards
remain above items, and Off the plan cards precede outside tasks. Plan-less projects use a responsive
card grid capped at four columns. Text filtering affects cards only, after classification; Claiming
you restricts columns while leaving the strip intact.

The owner explicitly resolved three specification gaps: Tab traverses strip, columns/items and
cards without adding the nonexistent next-claim shortcut; fourteen milestones at 1440 points is
the overview witness, while physically insufficient widths/counts truncate labels with full hover
text; compact cards show the title and at most three literal non-title producer lines, prioritizing
explicit wait/block marks, rather than inventing universal Decision/Now/Next fields from arbitrary
content. Those marks choose excerpt presentation only. Cards retain effective attention, freshness,
branch, phase and warnings. Escape closes By milestone immediately, including while editing its
filter; By state's existing two-step filter escape remains. A declared resolvable local roadmap
source opens in the existing read-only document viewer; unknown sources report unavailability on
hover. Old publication age is highlighted after 24 hours.

Discarded: fuzzy matching translated task titles (ambiguous identity under D6); changing the public
panel schema to force three labels (unnecessary for a reader arrangement); another strip scrollbar
or wrapped strip (loses the overview); drawing the whole producer body in each milestone card
(reintroduces the supplied crowding); a preliminary layout PR followed by a replacement (an extra
serial delivery with no durable independent part). No native desktop or performance claim follows
from synthetic geometry tests; existing Board spans account for the new work once.

**Reader controls correction, 25 November 2026 — #250.** The owner's annotated screenshot asks
for consistent header padding, removes roadmap preview rows because the header already exposes
them, and makes caption groups adjustable. Milestone headers inset title, badge, count and progress
by eight points. Default bodies retain every attached console card and its literal producer excerpt;
only explicitly expanded headers (or Show all) expose roadmap item rows and closing criteria.
The preview counts below cards are removed. Discarded: retaining two pending previews (the reported
duplication) or removing access to roadmap details (the header is the requested access).

In progress, Ready to start, Off the plan and Opened by you are reader-owned groups. Adjacent
separators transfer width between their two neighbors, bounded by existing column minima. Hidden
groups have named restore controls outside the horizontal scroller. Maximization is an override
that leaves normal widths and visibility untouched; restoring recovers that state. These choices
have Reader's session lifetime, independently per project. Show all retains its flat roadmap order
and temporarily bypasses these group preferences. Discarded: per-milestone controls (the screenshot
marks group boundaries), a second saved layout snapshot (duplicates normal state), and a new disk
format (not requested). New input/layout/paint stays inside the existing Board timing spans;
no performance or native validation claim follows from this decision.

## D94 — Ivy Night is the factory theme, and the interface's colors are roles derived from any theme

**New signal, 15 November 2026 — #93.** The owner commissioned a redesign in Claude Design and asked
for it to be implemented. Its first artboard defines two default themes, Ivy Night and Ivy Day, and
states that every interface color is a *role* derived from the selected palette, never a fixed brand
hex. This amends D59 (what «no theme» means) and D65 (the derived focus of converted palettes).

**Decision.** `ivy-night` and `ivy-day` join the catalog as hand-written themes of all twenty-two
colors; with no `theme` line, Ivy Night is the base, and Settings names that choice «Ivy Night
(default)». The interface's colors come from `Roles` (`theme/roles.rs`): chrome, base, panel, raised, hover, sel,
line, line2, three text inks, accent and its ink, the three claim colors and their soft fills,
diff backgrounds, scrim and one shadow. Each is a stated rule over the theme's colors — for example
`chrome = mix(background, black, 22 %)` on a dark theme, `accent = focus`, `done = palette[4]`.
Only text is corrected for contrast: secondary and muted text are brought back toward the foreground
until they read at 4.5:1 on every surface text is set on (the console background, the panel, raised
surfaces and hover), and a collapsed configuration falls back to black or white so Settings stays
legible. A person's explicit colors are used verbatim. `Roles` is a pure function of a theme rather
than a cache, because the themes it will be asked about differ (the live one, the Settings draft, the
Board's derived one); egui's style is written from it into both of egui's styles, with the preference
pinned, only when the effective theme changes.

**Green means accent, blue means unread.** The `done` claim moves from ANSI green to ANSI blue, a
panel's `current` step moves to the accent, and every theme's focus is a green, so the accent never
equals the `done` claim: Ivy Night, Ivy Day and Tokyo Night use their normal green, Gruvbox its bright
green, and converted palettes their normal green, else their bright one, else normal green moved until
it reads at 3:1 as a mark on the background — Catppuccin Latte's reaches only 2.96. The Board keeps
D93's owner-chosen blue for Working by naming ANSI blue rather than the focus. The producer's `accent`
style stays ANSI blue, which the design paints as information.

**Discarded:** a fixed brand palette for the chrome (ignores the person's theme, as D66 already
discarded); a 23rd configurable `accent` color (`focus` already is the application's own color);
contrast floors on every role pair (they recolor arbitrary themes until they no longer look like
themselves); one cached `Roles` refreshed on theme change (Settings paints a draft theme and the
Board a derived one, and a single cache serves neither); keeping the old gray factory palette as a
named theme (no published users rely on it).

**Measured boundary.** The derivation blends in gamma space, so Ivy Night's mixed roles land within
ten per channel of the design's hand-tuned CSS, not exactly; the slot-backed roles are exact. The
style install runs inside the existing theme-derived refresh and compares one theme per frame.

**Factory bright red, 18 November 2026 — #107.** With the owner's actual Claude Code settings,
the `dark-ansi` theme requests ANSI bright red (`ESC[91m`), rather than an exact RGB value.
Ivy Night's former slot 9, `#f59a89`, therefore made its mascot salmon. A controlled RGB-theme
comparison alone missed that difference. The owner asked for a shared default rather than a
personal configuration change, using the orange seen on another computer as the reference.
Slot 9 is now `#d77757`, the exact RGB `(215,119,87)` measured from Claude Code's RGB theme.
This also changes other bright-red output and the normal red that bold raises to slot 9.
The other palette entries, named themes and explicit color overrides keep their values.

**Discarded:** editing the owner's configuration (does not reach other installations); matching
Tilix's darker `#cf6a4c` (the owner's final reference is the RGB-theme orange); identifying Claude's
mascot in the buffer (D6, and terminal colors describe output rather than the program drawing it);
rewriting explicit RGB or other themes' reds (breaks the program's or the person's color choice).
This is a palette preference, not a rasterizer correction, and makes no claim about text sharpness.

## D95 — Geist and Geist Mono ship inside Ivy; Geist Mono is the terminal's default font

**New signal, 15 November 2026 — #93.** The design kit sets the interface in Geist and the terminal
and panel in Geist Mono. Asked whether to keep egui's monospaced face for the terminal, the owner
chose Geist Mono as the default.

**Decision.** Geist (400, 500, 600) and Geist Mono (400, 500) are embedded from the `geist` 1.9.2
package under the SIL OFL 1.1, notice in `resources/fonts/Geist-OFL.txt`. They are the base of every
font definition: Geist leads the proportional family, Geist Mono the monospaced one, each weight a
named family because egui has no weight axis, and egui's own faces stay behind them. Every rebuild of
the definitions — start-up, a configured family, fallback discovery, a Settings preview, clearing the
font — starts from that base, so none of them can drop it. A configured `font` still goes first.
Embedding needs no enumeration of the machine, so D24 stands.

**Header symbol fallback, 24 November 2026 — #240.** Claude's published `◐` (U+25D0)
rendered as the replacement box: neither Geist nor egui's proportional fallback faces cover it,
although the already-bundled monospaced Hack does. Append egui's monospaced fallback faces after
the existing proportional fallback chain, including the medium and semibold UI families. A face
already in that chain is not duplicated. The existing faces still win every character they cover;
the added fallback only supplies missing characters and survives every definition rebuild.
Discarded: requiring a system font (machine-dependent coverage and discovery), adding another
font asset (coverage already ships), and drawing or replacing the published symbol ourselves
(the text font already contains it). This changes neither terminal font selection nor title text.

**D9's symbols over the new face.** The six body marks are all in Geist Mono and measure one cell within
a hundredth of a point. Two of the three claim symbols, `⊘` and `∎`, are not in Geist Mono and come from the faces behind
it; `⊘` comes from egui's Hack, whose advance is 1233/2048 of the size against Geist Mono's 600/1000,
0.03 points apart at 14. The claim symbol test bounds the difference to a hundredth of a cell, which
still fails on a missing glyph (zero) and on an emoji glyph (almost two cells), the two failures D9 was
written against; both symbol tests also pin the cell to Geist Mono's, so they cannot pass over Hack.

**Discarded:** keeping egui's monospaced face for the terminal (the owner's choice, against the
three advisors' recommendation, because the design names Geist Mono); registering the bundled faces
as the `chosen` family (clearing `font` or the first discovery round would drop them, and a chosen
face arms the ligature family); the variable Geist files (one face per weight is what epaint can
use).

**Measurement.** The cell changes with the face, so a batch measured before this decision and one
after it lay out a different grid in the same window and are not a comparable pair (D13). The first
series after D95 is a new baseline, not a delta.

## D96 — The tabs live inside the title bar, and a project tints its console header

**New signal, 15 November 2026 — #93.** The redesign D94 started draws the window with one 40-point
title bar that holds the tabs, and console headers that carry the project as a low tint of its color,
a 3-point stripe and its name in that color. This amends D66 (tabs sat in a row of their own under the
titlebar) and replaces D79's 28 → 6 % gradient with a flat tint. The owner chose to keep D66's panel
toggle in the console header against the design, and not to add a Board button to the title bar.

**Decision — tabs.** In the integrated regime the title bar is one row: split right, split down, the
logo, the tabs on the row's bottom edge (34 points, equal widths of at most 190), the add button, a
gap of at least 48 points (16 when the bar is compact) that still drags and maximizes the window, and
the right-hand buttons; the six points above the tabs drag too. The consoles start under that row
instead of under two, so every console gets the old tab row back. One partition (`chrome::strip`)
feeds the title bar's painting, its claim on the pointer — which leaves the tabs and the add button to
the tab strip — and the tab geometry. A larger configured tab label grows the tabs and the bar with
them. The bar turns compact — no split buttons and no logo, the window buttons in the menu — when the
full bar would leave its tabs no room for one tab with its pill whole and a character of its label,
plus the add button: about 456 points wide on Linux and Windows and 426 on macOS. The active tab is
the console background with a separator border open at the bottom; the others are transparent until
hovered, and their hover covers the whole card short of the bar's bottom line. Each tab shows its
claim as an 18-point pill in one column (smaller only where its card is): wait and block solid, done
soft, a claim nobody holds up dashed, no claim a muted dot. Small dots after the name carry the
project ink of each of its consoles. A label that does not fit is not clipped: an unnamed tab shows
its number instead of «Terminal N», and a named one is cut with «…» while two of its characters still
show, its number below that. With two tabs or more, the close cross is painted on the active tab and on
the one under the pointer, and its target exists exactly there; on any other tab that slot is the
tab's body, so a press there selects the tab instead of closing its consoles unseen. egui resolves a
press against the previous pass's targets, so the cross must have been under the pointer for one pass
before it can be pressed, which a pointer moving onto it provides. The left-hand «new tab» button left
the row, because the add button after the tabs is the same action a few points away and the design has
only that one; the word «Ivy» left with it — the native title still carries the name. The add button
stays after the last card, so the tab it creates lands under the pointer: the second click of a double
click on it is taken as a plain click on that tab, not as a rename.

**Decision — headers.** A console header is 28 points. The focused console's is the panel color mixed
with 18 % of its project's ink, with an accent outline around the pane; the others' are the chrome
color mixed with 14 %, with a separator outline. A console without a project keeps the plain panel or
chrome and the roles' text. A project also paints a 3-point stripe in its ink down the left edge and its name, semibold,
in that ink brought toward the text until it reads at 3:1 on the tinted surface (inks are chosen
against the plain panel), then « / » and the task. The task and the separator are the theme's text
roles brought back until they read at 4.5:1 on that surface, and black or white where the text role
cannot. The number «01» is gone. Agent logos and the waiting marker stay, without the neutral backings
D79's 28 % gradient needed: a 14–18 % tint stays close to the panel and the chrome they were drawn for.
The branch is not shown: the only off-thread resolver is refreshed by the Board while it is open, and
refreshing it with the Board closed would add a worker, periodic `git` runs and wake-ups outside any
named span.

**Project colors.** A project takes cyan, magenta and their bright twins (slots 6, 5, 13 and 14), never
the accent or a claim's color: green, red and yellow (1, 2, 3 and their twins 9, 10, 11) read as focus
and claims, and blue (4) is the done claim, with 12 too close to it to tell apart at a glance. So
there are at most four distinct project colors instead of six, fewer where a theme's slots collapse
into each other. Past them a project gets no color of its own: its stripe, name and dots take the
theme's text color (D79's «while available»), and the name in the header and the details on hover
stay what tells two projects apart.

**Why this reverses part of D79.** D79 replaced a narrow stripe the owner found almost invisible with
a strong gradient. The tint stays, flatter, and remains the cue that answers that report; the stripe
comes back on top of it, with the project's name painted in the same ink beside it and the same ink on
the tab's dots. It is still a change to an owner signal, and the first look on a real desktop decides
whether the flatter tint is enough.

**Discarded:** a second tab row under the title bar (costs every console a row for one strip);
keeping D79's 28 → 6 % gradient with white or black text (the design keeps the theme's text roles on
a low flat tint, and its focused header differs from the others by the surface it is mixed from); a
close target live on every tab (a press on an inactive tab's right edge closed it and its consoles
with no cross painted there); an add button anchored at the band's right edge (it stays put, but the
design draws it after the tabs, and at full width it would sit far from them); showing the branch from
the Board's resolver while the Board is closed (stale values with nobody refreshing them); keeping the
compact threshold at 300 points (below about 380 points the full bar left the tabs a band of zero, and
up to about 456 one too narrow for a pill, so cards, pills and the add button vanished or were cut).

**Amended 17 November 2026 — #93, sixth PR.** With the right rail gone (D72), the title bar takes
the kit's buttons in the kit's order (`Window.dc.html`), with one change the owner asked for at the
desktop check: the logo opens the bar and Files stands a little apart after it. Right of the tabs
come the Board as a framed button with its name, split right and split down, a separator, Markdown,
connect and the menu (search sat between Markdown and connect until #120 moved it to each console's
header, D61), a separator, and the window's own buttons. Buttons are the kit's
28 points (32 for the window's), drawn from its 16-unit line icons in one module (`ui::icons`) that
the console headers and the drawer use too. Since **D103** that module draws Phosphor glyphs rather
than the kit's paths; the buttons, their sizes and their order are unchanged. This reverses the earlier choice not to put a Board
button in the title bar, and moves the splits from the left group to the right — the rail that held
the tools no longer exists. The compact boundary moves from about 456 to **600 points on Linux and
Windows and 563 on macOS** (570 and 533 since the search button left the bar, D61 amended); a compact bar lists Files, Markdown and Board in its menu, before
«Connect agents», so a short menu never pushes that entry out. A tool that is on but suppressed says
on its button's hover how to bring it back.

At the same check the owner removed the 3-point project stripe from the console header (the tint
and the ink name stay), asked that the focused console be outlined in its project's ink rather than
the accent (the accent stays for a console with no project), and asked for the kit's diagonal arrows
to maximize a console, in the header and in the drawer alike. A header no longer shows the shell's
`user@host:path` title: with no name given and no agent titling the console, it shows the foreground
program, after its folder when no project names the place («TaxPlatform / Bash», «tax-platform /
Bash»). The program's name is read from the operating system (`/proc/<pid>/comm` on Linux) in the
same once-a-second sample as the directory, off no extra thread; elsewhere the title stays. It is a
name, not a state: nothing infers what the program is doing (D6).

**Measurement:** every integrated-chrome batch (`chrome`, `chrome-zoom`, `projects`, `dock-v2-*`)
now paints this bar, and a full bar's tab band is 144 points narrower than before this amendment;
their reports from before it are not averaged with later ones.

**Roles.** Hand-painted surfaces read `Roles` through `ui::style::roles`, which memoizes the last
theme it was asked about and its roles in egui's memory. It answers any theme correctly, so it is not
the single live-theme cache D94 discarded; it only saves re-deriving the contrast search every frame.
Secondary and muted text are checked on the title bar's chrome as well as on the console background,
the panel, raised surfaces and hover; a tinted header is not a role surface, and brings its own text
back on it.

**Measurement.** Only the integrated regime changes. The historical regime — no integrated chrome,
the numbered tab cards and their single-tab rule — keeps its geometry and painting, so the measured
series stay comparable (D13); the new tab painting runs under the same tab-bar stopwatch.

**Amended 18 November 2026 — #113: a tab shows each console's claim on that console's dot.** One
pill per tab showed only the most urgent claim among its consoles, so with several of them it kept
switching and said neither which console was asking nor how many were. The pill and the muted
"no claim" dot left of the name are gone; the label starts at the card's padding. Each console's dot
keeps its fill — the project ink, or the hollow muted outline of a console with no project — and,
while that console claims, gets one whole ring 1 point wide around it, in its class's full colour:
wait and block as before, **done blue** and not green (D94: green is the accent and focus; the owner
confirmed it against the mock that drew it green). **Every ring is the same, held or not**: the tab
says which console claims and what; whether anybody still holds the claim up is said by that
console's own panel, whose strip keeps the dashed pill and the dimmed words. The owner settled it at
the desktop check after seeing three ways to mark an unheld claim on the ring — the dimmed colour
`attention_color` gives it turned the red into a brown, and a thinner or a dashed ring read as
noise — and asked for every ring thin, vivid and continuous. The price is stated: on a tab you are
not looking at, nothing on the card says a claim has gone stale; the pill used to dash it there.
Dots are 16 points apart — 4 points from one ring's band to the next, 4.5 between the painted
strokes — whether or not anything claims, so a claim arriving never moves the label; at 1 point
apart the owner read the rings as touching. **A card without room for all its dots keeps one dot's
slot**: a muted dot ringed with the tab's loudest claim, wait over block over done — chosen by the
owner at review, once it was measured that «show nothing» left no claim on the bar with eight
two-console tabs in a 1280-point window. That slot outranks the label, which keeps only the room
for the tab's number beside it, so **every card the bar keeps — down to the narrowest, before the
bar turns compact — can show a claim**; a first version that let the label win made the claim blink
on and off as the window narrowed, through the widths where the card's air and its cross change. The
slot is reserved whether or not anything claims, so the label does not move when a claim arrives.
With the cross's room (two tabs or more), four consoles keep their dots down to 128 points of card
and two down to 96; a tab of eight consoles or more never shows them all, at any width, and gives
up its project dots for the summary dot. The compact threshold does not move: the narrowest card
keeps its old width, written without the pill.

*Discarded:* nothing at all when the dots do not fit (the owner's first choice, reversed at review on
the numbers above); the label outranking the summary dot (the claim blinked while resizing); the
label giving way to all the dots (a tab of many consoles still had nothing); a green done ring; a
1.5-point ring (too heavy); for an unheld claim on the tab, the dimmed colour (brown red), a grey
ring (loses the class), a thinner ring and a dashed one; 1, 3 and 6 points between rings. The class
reads by colour alone: the pill's glyph does not fit a 6-point dot, and the console's own claim
strip still carries it.

*Measurement:* a card counts as marked when it painted at least one ring, so «tabs marked» still
counts what was drawn and not what was claimed. The batches that paint this bar — `chrome`,
`chrome-zoom`, `projects`, `dock-v2-4-*` — paint rings instead of pills from this change on, and
their reports from before it are not averaged with later ones; `measure-frame.sh compare` cannot
tell the two apart on its own, so that line is the record. `F` and `G` paint the historical strip
and do not change.

**Amended 20 November 2026 — #141: projects take named colours, and green is the focused
console's.** With four consoles in four folders the owner saw two cyans and two pinks: slots 6, 5, 13
and 14 are two hue families. A project now takes, in this order, **violet, turquoise, pink, brown,
white, maroon, sand and lavender-grey**, then the same names in a second tone — fixed values, one set
for a dark panel and one for a light one (`theme::project_colors`). A value is kept for a theme only
when it reads at 3:1 on the panel **as written**, sits at least ΔE 20 (CIE76) from that theme's live
red, yellow and blue claims and its focus green, and is distinct from those already kept; one that
fails is dropped, never brought toward the text — which is what washed the old inks to near-white.
The owner named the colours: brown, maroon and white differ from violet and pink by lightness, which
no rule over hues alone offers. Ivy Night keeps nine, Ivy Day nine, the pastel Catppuccin Mocha five,
because its own claims sit where pink, maroon and sand would. Past the list a project still takes the
text colour, as before.

**Green belongs to the focused console.** Its header tint, project name and outline turn the focus
green, with or without a project; when focus moves, the console that gets it fades to green and the
one that loses it fades back to its project colour, in 150 ms. This reverses the 17 November
amendment's «outlined in its project's ink». The tab's dots and the Board keep the project colour:
the badge's ink never changes. Only the ink fades; the header's ground still switches between panel
and chrome on the first frame. **Measured runs do not fade** — the frames a fade wakes would be
published as the framework's cost (D13) — so `Chrome::start` allows fades only outside a measured
run, and with fades off the mix is 0 or 1 without touching egui's animation, which requests a repaint
even with no time left to animate.

*Discarded:* the green as the first project's colour (the ticket as written — it made one project
read as focus); colours generated around the hue circle avoiding the claim hues (measured over the
twelve themes: four hues on Ivy Night, and a second lightness that collapsed into the first or failed
3:1); lifting a failing colour toward the text (the near-white of the report); keeping the claim
colours as project colours (a project in blue reads as finished work); outline-only green on focus
(the owner wanted the whole colour); fading in HSL (150 ms of RGB mix through grey is not seen).

## D97 — A panel's claim is a strip across its top, and its controls look like controls

**New signal, 16 November 2026 — #93.** The redesign D94 started draws the status panel with a band
for the claim, the phase as a chip, framed action buttons, section headers with a chevron and step
marks in two inks, in the order identity, title, summary, buttons, sections. This amends how D9 and
D18 present the claim — not what it says or when it dims — and how the identity row of F1.6 reads.
The choice between the two panels travels as one value (`panel::Chrome`) in the layout's key.

**Decision — the claim.** In the integrated chrome a declared claim is a 32-point band across the
panel's top: the claim's 15 % tint, D96's 18-point pill, and the claim's words in the colour
`Theme::attention_color` already answers with. The words are D18's three phrases, word for word; a
band cannot wrap, so a narrow panel cuts them inside the band and ends the cut with «…» — «stuck on
something» alone would lose the «not you» that sets it apart — and shows only the pill when not even
one letter fits beside the «…». A claim nobody holds keeps D18's dimmed look: no tint, the dashed pill
and the dimmed words; `done` stays whole while its file is read, which is the same `backed` answer,
not a second rule. No claim, no band. The rows, the buttons' reach and the fold headers are all
measured from what the band leaves. On a panel too short for the band and one row, the band shrinks
to leave that row — where «… N more» says what is below — down to one row of its own; a panel shorter
than one row paints nothing, band included. The design shows how long ago Ivy observed the claim
beside the words; that is not painted, because the panel holds no moment at which a claim was first
seen — the file's date is the last publication, which a producer republishing every minute would keep
at «just now» — and the only observed-since time, the Board's, exists for waits alone.

**Decision — identity.** The integrated identity row paints the producer's `phase` on a chip (hover
fill, secondary text, chip radius) and who drives the panel — the connection or hook that holds it —
beside it in muted text, on one row; a part that does not fit ends in «…», and a part that cannot show
even that is left out rather than drawn as an empty chip. The pane's hexadecimal identifier is no
longer painted: a person finds the console by its header (D96), and the identifier is a name for
programs. With neither a phase nor a driver there is no row.

**Decision — order and buttons.** When there are buttons to place, a leading `title` line, at most
one unmarked line under it — the design's summary — and the producer's own gap closing them, if one
does, go above the buttons, as the design reads; title and summary are cut to one row each with «…»,
so the first label starts at most four rows below the application's own rows: title, summary, gap,
and the blank row its frame reaches into. A marked line under the title is a step of a list, and a
list is never split. Without buttons the body is painted exactly in the producer's order, with no
row added. An action is a rounded, bordered frame: the label,
without D9's `>` sign, in the interface face at medium weight on the raised surface, and the literal
command below a rule on the console's own background; a `›` prompt is drawn at the label's inset and
the command starts after it and a space — the prompt is never part of the text that is sent. Every
button stands between blank rows its frame reaches into, which gives the label and the command room
inside the frame and leaves two buttons six points apart. The border is the muted text at half
strength: the kit's `line2` all but vanishes against the panel on Ivy Night. The frame **is** the
reach: one function gives both, half a cell in from the strip's sides, so the edge a person sees is
the edge a click obeys; a frame counts as drawn only when the blank row under it survived the clip,
and a row whose ink passes the frame's right edge leaves its button unreachable, as a row past the
strip's edge always did. A frame's place is reserved under the text and filled only once the ink has
said the button stayed reachable, so a button the clip cut or whose command ran past its frame looks
like plain text. The label stays in the cell grid's row and goes through the same colour path as
every other row, so its emoji stay in colour; it is wrapped by character count for the grid but drawn
in a proportional face, so each of its rows is fitted by what that face measures and cut with «…»
where it does not fit — a wide label never inks past its frame and takes its button away. The command stays in ordinary ink, not the design's secondary one, for the reason the
layout already gives: it is the one sentence that must be read before pressing. No shortcut number is
shown: actions have none.

**Decision — sections and marks.** A section header paints a chevron (down open, right folded) where
the old `▸`/`▾` stood, the title in the text colour, a one-point rule in the separator colour and the
summary muted at the right; its ink still decides whether it can be clicked. A line's mark is its own
glyph in the first cell, so it takes its own ink: done muted on both, current the accent on the glyph,
pending a muted glyph and secondary words, wait and block in their claim colour. Words that read as
ordinary text — current's and info's — take the same ink as a line with no mark. The glyphs are still
D9's measured characters. A style still rules over a mark, glyph included.

**Discarded:** keeping the claim as a row and only restyling it (the band is what reads from across
the window, and a row wraps where a band cannot be read at a glance); painting the observed age from
the file's date (it would claim an observation Ivy never made); cutting the claim's words with no mark
(a cut phrase reads as a whole one); painting the frame on the strip's whole width as the old reach
was (the frame would touch the panel's edges and two buttons would read as one block); a frame inset
inside its own rows (the label and the command touched its borders); a frame for every laid-out button
(one the clip cut would look pressable and not be); putting a whole leading block above the buttons
(a long titled block would push them into the clip); reordering a panel with no buttons, or taking a
marked step as the summary (it added a row into a list and split it); wrapping labels by measured width at layout time
(the layout has no font to measure with, and a cut marked with «…» keeps the button whole); the design's `›` inside the command
text (what is painted must be what is sent); new font glyphs for the marks such as `✓` (D9 measured
them out, and a glyph outside the bundled faces triggers discovery).

**Measurement.** The batches without the integrated chrome — the lettered ones and every other batch
that does not set `MEASURE_CHROME` — paint the historical panel: the claim as a row of text, the
identity as «id · phase · driver», the mark inside its line's text in one colour, the text header with
`▸`/`▾` and a run of `─`, frameless buttons whose reach spans the strip, and the buttons above the
whole body; their series are unaffected (D13). The integrated-chrome batches — `chrome`,
`chrome-zoom`, `projects`, `dock-v1-4-closed` and `dock-v1-4-open`, which set `MEASURE_CHROME` — now
paint the restyled panel: every bench panel declares `attention` and carries marks, so they paint the
32-point band in place of the identity row, the claim row and the blank under them, and the two-ink
marks. They paint no identity row at all — the bench panels declare no `phase` and no connection holds
them — so the body starts higher than in the historical panel and more of its rows show. No
integrated batch publishes actions, so the buttons' frames, their blank rows and the title block
above them are not measured; the chevron is painted only where a batch's panels publish a section. **Their series are not comparable across D97**:
a run from before this decision and one after measure different panels, the report does not say which
panel it painted, and nothing invalidates the older runs on its own — a comparison needs a new pair
measured after it, the same day.

## D98 — Settings and the floating surfaces are painted with the roles of the theme they show

**New signal, 17 November 2026 — #93.** With D94 every surface reads the theme's roles, and the
kit draws Settings, the menus and the viewers as one family of floating surfaces. Settings still
started from a neutral palette of its own (D67) on top of egui's stock dark look; the viewer, the
chrome's dialogs and the console's menu each started from egui's stock dark or light look and
painted their own mixes over it. That stock start is what let the desktop's light/dark switch reach
them, and what made Settings look like another application. This supersedes D67's «Integration with
D66» paragraph; the rest of D67 — the grouping, the footer, the typography preview — stands.

**Decision.** A floating surface is `raised` with a 1-point `line2` border and the kit's one shadow:
10 points of radius for a menu, 12 for a dialog. Its controls take the roles through the style
egui already installs (D94), written into the surface's own `Ui` from **the theme it shows** —
Settings its draft, the Board its derived theme, every other surface the live theme — so no
surface depends on what egui was told about the desktop. Settings' navigation sits on `panel`, its
selected row on `sel`, its primary action on `accent` with `on_accent` ink, its rules on `line`. The
theme picker is a floating menu: a search field, a segmented All / Dark / Light, and each theme with
a four-colour sample — background, accent, waiting, finished — and a check on the selected one. The
docked viewer keeps its header and footer on `panel` and lays the document on `base`. The console's
search box (a one-row line until #120, D61) is laid on `raised`, and the console header's actions
menu floats like the others.
Surfaces that show a theme of their own derive roles for it as well, so one frame may ask about
several themes; the memo of derived roles keeps the last four.

**Collapsed palettes.** Settings is where a broken configuration is undone, so its text, its error
ink and the ink on every control keep 4.5:1 over the fill they sit on; its navigation leaves a
`panel` its text cannot be read on for the dialog's surface (and the viewer's chrome leaves it for
`base`). Where a collapsed palette makes a fill, a control's border, the selection, the primary
button or the caret indistinguishable from what lies under it (under 1.03:1, below the kit's own
quietest step), that one colour is moved toward the ink — to the nearest step that stands apart, stopping short
of the ink itself; text on a moved fill is re-inked to what reads on it, which is what keeps it
legible; the caret, a line and not a fill, becomes the ink itself — so the control can still be
found; Settings
paints from the roles with those corrections written back, so its outline and every field egui
derives from them carry them too. The shared roles keep hover and selection apart from `base`, `raised`, `panel` and one another,
including a theme's own narrow mixes and a `panel` set to one of them. Settings inherits that floor;
its local ink, navigation and caret safeguards remain. Nothing else is corrected: an explicit focus is the person's colour verbatim (D94) wherever it can be seen, and the
old Settings palette's 3:1 floor on every border and on the accent is dropped with it.

**The surfaces step toward the readable text.** `raised`, `hover`, `line2` and a light theme's
`chrome` are mixed from the background toward `tx` rather than the raw foreground. For every theme
whose text reads, `tx` is the foreground and nothing changes, however narrow its contrast; a
configuration that collapsed the foreground now gets surfaces that show on everything built from
the roles — menus, the Board, the tab bar — not only in Settings. Only where the text had to be
replaced, and stepping toward it would take a text surface under 4.5:1 (a mid-grey background whose
text became black), does the step go the other way. The selection, a row text sits on, is moved to
a step of the text whenever the focus colour lands it on the background, a menu or a hovered row.
This amends D94's derivation table for those cases only.

**Measures kept.** Settings and the chrome's dialogs (Connect agents, What's new) keep the measures
Settings already had — the 15-point body and 40-point size fields of D67's native review, 36-point
controls — and the viewer keeps its two-line header; only colours, radii and the picker change. The
console title and tab label sizes take the font size's 40-point box. The
artboard's 40-point header and toolbar for the viewer, and its settings search field, were not
taken: the first would cut the path line, the second is a feature no section asked for yet.

| Discarded | Why |
|---|---|
| Keep D67's neutral Settings palette | The one window that did not look like Ivy, and the stock dark start under it followed the desktop. |
| Install the style once and let every surface inherit it | Settings previews a draft theme and the Board a derived one; the installed style is the live theme's. |
| Correct accent and borders to 3:1 as the old palette did | Rewrites an explicit focus colour wherever it is visible; D94 keeps it verbatim and corrects only text. |

**Shared collision floor, 17 November 2026 — #99.** A configured `panel` could equal selection
(`#1a372e` on Ivy Night) or hover (`#262b2d`), while black with foreground `#757575` left menu hover
at about 1.02:1. The floor belongs to `Roles::of`, so every consumer receives the same final fills.
An already distinct candidate is retained exactly. A collision searches the nearest background step
in increments of 0.01 below 0.95; corrected foregrounds keep their 4.5:1 text constraint and direction
handling. The bounded search retains its first-step fallback when no candidate can satisfy it.
Explorer's panel-only row roles retain the original translucent mixes when their actual panel
composites clear 1.03:1; otherwise they use corrected shared fills, selection also apart from the
final displayed hover. Configured background, panel and focus remain verbatim. Settings no longer
floors these fills locally, and still re-inks controls and selected text and protects the caret.

**The small dialogs take Settings' anatomy, 18 November 2026 — #119.** Rename tab and Rename console
had the kit's colours but stock egui's layout: two identical grey buttons in the order they were
added, a 16-point title and no close control. They now have a header with the title and a close
button over a rule, Settings' measures and inks through its palette, and a footer like Settings':
Save right-most on `accent` with `on_accent` ink, Cancel beside it without a frame, and the console's
Use terminal title frameless at the left edge, where Settings keeps Restore defaults. Save is
disabled while the name is invalid, as Settings disables its own primary. The close button cancels.
The header is one helper that Connect agents also uses. Its title takes the kit's 20-point floating
heading, not Settings' 25: these dialogs hold one field or one guide, and at 25 the title outweighed
them on a real desktop. The close cross is painted on the centre of a square as tall as the
controls, because laid out as button text it sat on the text baseline, visibly above the centre of
its box; that button is shared, so What's new gets the centred cross too. Connect agents gets no footer, because it
has no Save and Cancel pair, and its instructions keep full contrast instead of the muted note ink.

| Discarded | Why |
|---|---|
| Put the header and footer in `ui/`, the kit | `ui/` has no dependency on `settings`, and the readable muted and error inks the rename needs are the Settings palette's. |
| Make the rename an `egui::Modal` like Settings | Its dimming and input gating already run through the console's own rename state. A Modal would add a second backdrop and a second input block. |
| A footer with a Done button, or Save agent preferences moved there, in Connect agents | The owner chose no footer, because nothing in that dialog pairs Save with Cancel. |
| Cap the name field at 128 characters | It would cut a paste without saying so, and control characters would still need the disabled Save. |

## D99 — A console header splits its own console, groups its controls, and shows them where they are wanted

**18 November 2026 — #104.** The integrated header gains split right and split down, so a console is
split from its own header whether or not it has the keyboard; the titlebar's split buttons, which act
on the focused console, stay. This **amends D66**, whose approved per-pane set had no splits.

- **Groups.** After the title, right to left: maximize and close; review and information panel;
  split right and split down. A one-point rule sits on the boundary between groups and costs no
  width, so the cells and their thresholds stay where they were. It takes the ink of the title's
  own « / » separator, which reads at 4.5:1 on every header surface; the titlebar's border role was
  tried first and the owner found it barely visible on the desktop.
  A lone console has no maximize or close, and one rule.
- **Shown where wanted.** The focused console shows its controls all the time. Any other console
  shows them while the pointer is over its header. Hidden controls keep their place, so the title is
  laid out the same either way and never moves. With four consoles, six icons are in view instead of
  twenty-four (seven and twenty-eight since #120 added search, D61).
- **A control exists where it was painted.** A non-title target of a header is pressable only if
  that header's controls were painted in the previous pass, the rule D96 gave the tab close. A move
  and a press in one input batch — a fast click, the first click into a window — otherwise reaches a
  close nobody saw. The pass is recorded by frame number, not a flag, so a console that stopped
  being painted (a hidden sibling, a background tab) cannot come back pressable unseen. And a
  press within a double click of one that fell on hidden controls is the title's too: the first
  press hands the console the keyboard, which paints its controls at once, and the second click of
  a double click aimed at the title would otherwise land on whatever appeared under it.
- **What the strip's span measures.** The integrated strip's timed span now paints icons for the
  focused console and for any header under the pointer only, so its cost depends on focus and on
  where the pointer is; a batch that moves the pointer over headers paints more than one that does
  not, and the two are not the same work. And a header whose controls appear asks egui for one more
  pass, so the control under a still pointer gets its hover: a frame no report counts yet, one per
  header revealed, never one while a header stays shown.
- **Unavailable is absent.** A split the console cannot give both halves their minimum is not drawn
  and not pressable. For this header that replaces D62's dimmed control and F2.14's «visibly
  unavailable»; the historical strip keeps both. Painting and pressing read one offer, built with the
  same predicate the split itself checks.
- **Narrow.** The title keeps its 30 points: the review, panel, maximize and close cells come first,
  each split is added only if it still fits, and below the base the header is the title alone.
- **Maximized.** A maximized console offers no split: it is the focused console (D66), so the
  titlebar split already acts on it, and its view fills the area while the split judges the tree.

| Discarded | Why |
|---|---|
| Dim an unavailable split | In this header a dimmed control takes 25 points of the title exactly when the console is small, for an action that would be refused. |
| Every console's controls always visible | Four consoles paint twenty-four icons, most of them never aimed at. |
| Hover-reveal on every console, the focused one too | The controls disappear from the only header a newcomer looks at. |
| Hit by geometry alone | D96: a destructive control must not be pressable unseen. |
| A `\|` glyph or gap cells between groups | A glyph depends on the font; extra cells move every offset and threshold. |
| An overflow menu when narrow | One more gesture for every control, to save two cells. |
| Removing the titlebar's split buttons | Kept by the owner's decision. |


**Amended 18 November 2026 — #120.** The base group gains **search** to the left of review: after
the title, right to left, maximize and close; review, the panel and search; split right and split
down. It is the console's only search button now that the titlebar's is gone (D61), so it is part
of the base and not dropped before the title when narrow — the label-only threshold moves up by one
25-point cell, and the splits still go first.

**Amended 20 November 2026 — #157.** Controls nobody wants any more **linger, then fade**. The owner
found it abrupt that they vanished on the very pass the console lost focus or the pointer left its
header. Now they stay whole for 3 s and fade to nothing over 0.5 s; appearing is still immediate.

- **The countdown starts on the first pass that finds them unwanted.** Not at the last pass that
  wanted them: a window at rest paints nothing, so that stamp can be seconds old when a click on
  another console finally arrives, and the controls would vanish on the spot. A design challenger
  found this before any code existed.
- **What shows answers.** The controls are recorded as painted — and so pressable, by the rule
  above — through the wait and the fade until they drop below a quarter of their opacity; the
  last stretch of the fade is painted but answers no press, because an icon that faint is one
  nobody aims at, and a quick move-and-click could otherwise reach a close nobody saw. The review
  found that case. Focus or the pointer coming back makes them wanted again, whole on that pass.
  The double-click guard is untouched: it concerns controls that were not painted.
- **Only what was seen leaving lingers.** A header that went unpainted (its tab in the background,
  a sibling behind a maximized console) does not start a countdown when it comes back.
- **Rest.** The wait asks to be woken once, at its end; the fade asks for every frame for half a
  second; after it nothing is asked. The header span now also paints the headers that are
  lingering or fading.
- **Measured runs.** They paint integrated headers too, so the linger shares the focus green's gate
  (`focus_fades`, off in every measured run and every test window): there the controls still go on
  the pass that stops wanting them, and no published batch changes.

| Discarded | Why |
|---|---|
| egui's `animate_value_with_time` for the fade | It has no delay, and it eases back up when focus returns mid-fade instead of being whole at once; calling it also asks for a repaint every pass |
| A setting for the two durations | Not asked for; one more key to document for a feel the owner already chose |
| A fade when they appear | The owner chose immediate: a control reached with the pointer must be there to press |

## D100 — Configurable font sizes use typographic points (version 2)

Configurable font sizes are typographic points, converted once at the settings boundary using
96/72 logical units per point. Previously the UI and documentation said points while passing
values directly to egui, whose points are logical screen coordinates. This made equal numeric
sizes incomparable with desktop terminals such as Tilix.

Version 2 makes the new interpretation explicit. Legacy files retain their visual size; the
editor displays converted point values and upgrades on edited save. The default remains visually
unchanged (14 logical units = 10.5 pt; title 12 logical units = 9 pt). Renderer cell rounding and
benchmark geometry remain unchanged. Changing all old sizes in place without a format marker
was rejected because existing users would see their text grow by one third.


## D101 — Instructionless fonts get automatic fitting; text keeps its raw coverage

A native11pt comparison with Tilix exposed two effects: a prep-only TrueType font selected the
interpreter without glyph instructions to execute, and egui's coverage transfer made partial
edge pixels heavier. The owner accepted the combined automatic-fitting/raw-coverage preview.

Choose automatic fitting only for fonts that prefer the interpreter yet contain no glyph hint
instructions. Cache that decision once per loaded face and reuse it during hinter reconfiguration.
Native-instruction fonts and CFF/unknown outlines keep their previous selection.
Set raw coverage once for both UI themes at font installation, retaining it across theme switches.

The dependency patch is local, licensed and described in vendor/epaint/IVY-PATCH.md. Actual bitmap
regressions preserve the original native Geist masks and distinguish fitted from unfitted Ubuntu.
Forcing the automatic hinter for every font was used to diagnose but rejected for delivery: a
font with deliberate native instructions should not silently lose them. Configurable sizes use
D100's true points; this does not change placement, cell rounding or DPI/backend selection.


## D102 — Empty panels do not request setup, and startup guidance survives a late channel

The owner reported repeating setup after updates, including already running Claude sessions that
only learned of the MCP after restart. Saved registration and connectivity are distinct from panel
publication. The connection guide no longer colours its icon from a zero publication count; it
explains reconnection first and keeps first-time registration under manual diagnostics. Copy actions
remain clipboard-only. This supersedes D73's zero-publication highlight without claiming detected
connection health.

The bridge also took its publication-guidance snapshot before reading initialize, while Ivy starts
the child PTY before binding its control socket. An isolated same-process regression proved that
tools could work after late binding despite the missing guidance. D88's socket-presence gate is
replaced by existing-directory/whole-identity gates and explicitly conditional instructions. The
handshake claims no publisher and promises no live endpoint; actual tool calls keep their existing
identity, directory, peer and permission checks. Missing/gone directories and unsupported channels
still receive no guidance. No timeout, automatic client-config edits or injected reconnect commands
are introduced. Deferred tool schemas keep their existing client policy.

A native auxiliary test retained one Claude registration across replacement of the installed Ivy
payload with current main and verified publication/readback under a fresh console identity. This
proves that controlled replacement; it does not prove hot reload in a running Claude conversation
or identify the exact cause of the owner's earlier incident. Debian upgrade tests additionally
preserve Claude/Codex registration fixtures alongside Ivy's own data.

## D103 — The interface's icons are Phosphor glyphs, and Files shows each file by its type

**New signal, 18 November 2026 — #114.** The owner judged the hand-drawn icons amateurish next to any
modern editor, worst in Files, where the folder's stroke was heavy for its size and every file was
the same blank page. The ticket offered Phosphor or Lucide; the owner chose **Phosphor** in its
thread. Asked while the design was open, the owner chose to **tint each file type with a colour of
the theme**, as an editor's file-icon theme does.

**Decision — one set.** Every icon of the window's chrome — title bar, tabs, console headers, the
sidebar, Files, the Board and Settings — is one glyph of **Phosphor Regular**, from the
`egui-phosphor` 0.14.0 crate pinned exactly, and painted through `ui::icons`. The face is registered
in the bundled definitions (D95) as a named family holding **that face and nothing else**: Phosphor
maps the lowercase letters to blank glyphs and its codepoints sit in the private-use area the
terminal's icon family serves (F2.9), so it is never a member of a family that lays out text, and
`egui-phosphor`'s own installer, which puts it second in the proportional chain, is not used. An
icon is centred by its **em square**, the square every Phosphor glyph is drawn on: laid out at the
icon's size the galley is that square, to within a device pixel of row rounding, so an open folder
and a closed one, or a caret that turns, never move. Licence: MIT, notices in
`resources/fonts/Phosphor-MIT.txt` and `THIRD-PARTY.md`.

**Decision — Files.** A file's glyph and colour come from its name alone (`FileKind::of`; the tree
never opens a file to decide): Rust, Markdown, JSON, TOML/YAML/INI, images, SVG, the common code
types, PDF, archives, text, lock files and dotfiles, first match winning so `package-lock.json` is a
lock file. Folders show closed or open as they are expanded, with Phosphor's carets. A code, data
or image type's colour is a **terminal palette slot**, so every theme and every settings file
carries it (D94); lock files and dotfiles take the muted text role, and archives, text and plain
files the secondary one. Where a colour would keep less than **3:1** — WCAG's figure for non-text
marks, the same the focus border uses — against any surface its row can show under it, the file
falls back to the secondary text colour it had before. Those surfaces are the ones the list really
paints: Files' own, which is the panel or the content's base by D98, and its hovered and chosen
rows **as those translucent fills land on it**; the search results judge the same glyph and colour
on their floating surface and its hover and selection.

**The panel toggle keeps its state.** The console header's toggle shows the panel shown as a
square with its right half filled and the panel folded as an outline with its divider, the two
states the hand-drawn marks told apart; the integrated header's panel button keeps one glyph, as it
had one drawing. The close buttons of the floating surfaces (What's new, Connect agents) are
egui's own buttons holding the set's cross instead of a text one.

**Out, on purpose.** The status panel's line marks and the claim pill keep their own drawing: they
are D9's measured vocabulary and D97's, and the ticket named the chrome, not the panel. Agent and
brand logos are identities, not icons. Status dots are markers.

**Discarded:** Lucide (the owner's choice); rasterizing Phosphor's outlines into the colour-emoji
atlas (D25) — that atlas exists because epaint cannot paint colour glyphs, and Phosphor is monochrome
outlines epaint already rasterizes; vendoring the TTF with a hand-copied table of codepoints — the
crate's constants are generated from the face upstream, and its egui 0.36 unifies with eframe's;
centring each glyph's ink or its line box — the first moves paired glyphs and loses Phosphor's optical
centre, the second carries ascent padding; `Roles::stands_apart` as the colour check — its 1.03
separates surfaces, not ink, and passes a yellow at 1.2:1; shipping other weights — about 2 MB for
nothing the ticket asks.

**Measurement.** The chrome is painted in every batch of `scripts/measure-frame.sh`, so the series
after this compares like with like; a batch measured before it and one after paint different glyphs
and are not a delta. The face adds 489 KB to the binary.

---

## D104 — Linux console processes have optional, separate systemd scopes

**New signal, 18 November 2026 — #124.** systemd-oomd selected Ivy's application cgroup while
consoles were compiling; the GUI and unrelated agent sessions died together.

**Decision.** On Linux the Ivy executable explicitly enables a private re-exec launcher. The
launcher waits in the PTY before any user program executes; a worker places its PID in a distinct
`ivy-console-*.scope` in the session user's `app.slice`, then releases an in-place exec. The GUI
stays outside those scopes. Original argv, environment, working directory, controlling terminal
and process-group shutdown remain the launch contract. There is no extra installed executable.

**Compatibility.** Scope creation is optional and bounded by one second. A missing user manager,
denial or setup failure preserves ordinary startup. Unknown optional `OOMPolicy` support retries
a basic scope rather than throwing away isolation. A failed gate is killed before direct fallback,
so an uncertain late manager operation cannot move the requested program after it starts. After
exec release an unknown outcome never triggers a second execution. macOS and Windows keep their
direct launch and have no systemd dependencies.
Compatibility diagnostics use a bounded, non-panicking reporter: a closed or undrained standard
error stream must never prevent the requested program from starting.

**Responsiveness.** Linux opening, restoration, fallback and fresh-shell takeover reserve their
console identity and placement while a launch worker completes. Pending input and the latest grid
size reach the launched program in order. Closing drops the pending result; a late child is shut
down on its worker rather than installed. D16's published synchronous restoration timings describe
the previous launch path; they do not measure the completion latency of these asynchronous launches.
The worker transfers a finite prefix of already-queued output with its byte watermark. The normal
frame budget interprets that prefix at the original geometry before resizing or delivering input;
an empty bridge followed by an immediate resize loses this ordering when its reader starts late.
The frame report names console launch/status work and subtracts it separately from framework cost.
No new performance figure is claimed here.

**Truthful endings.** The process's cached wait status and the reader's ending remain distinct.
Retain the scope result through an observation window of 500 ms from confirmed root exit. Only
`Scope.Result=oom-kill` earns “This console was stopped because of memory pressure.” A confirmed
signal without OOM evidence earns a generic signal notice. Late or unavailable evidence stays
unknown; neither code 137 nor SIGKILL proves memory pressure. The existing dismissible notice keeps
the output copyable and prevents automatic window closure. Older managers may isolate without
providing reliable oomd attribution.
Before closing checks the child and sends signals, it retires observer reaping under the same
cache mutex. A stop flag alone cannot exclude an in-flight observer releasing the PID for reuse
between the last status check and a signal.

**Limits.** Separate leaf scopes improve victim selection; a policy targeting their common
ancestor or a global kernel OOM may still kill more than one console. Controlled kernel OOM and
systemd-oomd tests require a disposable environment; stressing the person's desktop is not a
substitute for that proof.

**Discarded.** Moving a root after user exec loses immediate descendants. A `systemd-run` wrapper
blurs exactly-once fallback and the owned PTY PID. OOM-score adjustments do not separate cgroup
victims. Kernel counters alone do not establish userspace systemd-oomd attribution. Changing global
OOM policy or introducing resource limits is outside this repair.

## D105 — A macOS console that inherited no locale gets the Mac's own, as `LANG`

**Decided.** On macOS only, when the application inherited none of `LC_ALL`, `LC_CTYPE` or `LANG`
with a value, every console is launched with `LANG=<language>_<REGION>.UTF-8` from the Mac's
`NSLocale`, checked with `newlocale`, and `en_US.UTF-8` when that name is not installed. A value the
user inherited — `C` included — is never replaced, and a caller's own `LANG` still wins.

**Why.** An app opened from the Dock or Finder inherits launchd's environment, which has no locale,
so every console ran in C. There zsh counts each byte of a UTF-8 glyph as a column: starship's `❯`
(three bytes, one cell) made the prompt two columns wider than painted, and typing `cd ` left a
`cdcd` that Backspace could not reach (#117). Setting `LANG=en_US.UTF-8` in the console removed it on
the reporter's Mac. Every reference terminal sets a locale on macOS from `NSLocale`: WezTerm, kitty
and Ghostty only when `LANG` is missing, with `en_US.UTF-8` behind it; iTerm2 and Terminal.app by a
default-on setting. Ivy was the outlier.

**Full `LANG`, decided by the owner on 19 November 2026.** It changes every category, not only the
encoding: numbers, sorting, dates and messages follow the region, so a script printing `1.5` with a
Spanish region may see `1,5`. That is what Terminal.app does, which is what a Mac user's scripts
already meet.

**Discarded.**
- **Only `LC_CTYPE=UTF-8`**, as Alacritty does when `setlocale` fails: enough for the width bug and
  leaves numbers in C, but a console that behaves differently from Terminal.app.
- **Always overriding**, as iTerm2's default does: replaces what a user set on purpose.
- **`setlocale` to validate**: it changes the process's own locale, which other threads read;
  `newlocale` does not.
- **`std::env::set_var` in the app**: undefined behaviour while consoles are launched in parallel.
  The value is set on each child's command instead.

**Open.** Measured only on Linux: the decision, and a decided `LANG` reaching the program, are tested
there. Unbound: the one call that hands `console_lang()` to the launch, which is `None` off macOS, and
the `NSLocale` read, which only type-checks for `aarch64-apple-darwin`. Two things only a Mac
settles: whether `cdcd` is gone in a
console of this build — the reporter's test also made zsh a login shell, so a co-factor may remain —
and whether `NSLocale.currentLocale` reports the system language or the app's own (English) one on
macOS 14 and later, which would turn a Spanish user's `es_ES.UTF-8` into `en_US.UTF-8`.

## D106 — A translocated Ivy offers no path for a client to save, and setup is confirmed in the client

**Decided.** When macOS runs Ivy from a Gatekeeper App Translocation copy — its executable's real
path has an `AppTranslocation` component — the connection guide offers no path a client could save:
not the MCP bridge, not the first-time setup prompt that carries it, not the session-recovery helper
or adapters. In their place it says that Ivy is running from a temporary copy, and that the remedy is
moving `Ivy.app` to Applications with Finder and opening it again from there. And the MCP bridge setup the
guide describes ends with a check the person makes in the client's own MCP list (`/mcp`), never with
the agent's word that it connected.

**Why.** A quarantined app opened from where it was downloaded runs from a randomized, read-only copy
under `/private/var/folders/…/AppTranslocation/`, and that copy disappears when Ivy quits. The guide
built the bridge path from its own executable, labelled it «found», and Codex saved it: on the next
launch the `ivy` server was `failed (0 tools)` (#145). Codex had reported the connection as working,
although the guide already asked it to prove that with `panel_get`/`panel_set` — Ivy cannot make a
client obey, so the last word belongs to something the person reads.

**Discarded.**
- **Removing the quarantine attribute** or suggesting `xattr`: ruled out as an installation step by
  `docs/10-installation.md`.
- **Offering the original bundle path** resolved through the Security framework's translocation
  query: it needs a native binding, and the original location is the Downloads folder or a mounted
  image, which is exactly the install the person has to fix.
- **Asking the Security framework whether the app is translocated** instead of reading the path: the
  same answer the path already gives, for a native binding.

**Open.** Measured only on Linux, with the decision tested as a pure function over a path. A real Mac
must confirm the notice on a freshly downloaded build. Not covered: an app opened straight from a
mounted image without quarantine, whose `/Volumes/…` path also disappears when the image is ejected.

## D107 — On Windows a console opens PowerShell, and the configuration lives in `%APPDATA%\ivy`

**Decided.** On Windows, a console's program is, in order: the shell variable when it names an
absolute Windows path to a file; `pwsh.exe` found in an absolute directory of the session's search
path; `powershell.exe` found the same way; `cmd.exe` by an absolute path (`%ComSpec%` when absolute,
else `%SystemRoot%\System32\cmd.exe`). That `cmd.exe` is also the program a failed launch falls back
to, and a launch whose program already is the fallback does not retry it. The choice is resolved once
per process. Settings, the layout and the recovery files live in `%APPDATA%\ivy`; `XDG_CONFIG_HOME`
still wins when it is an absolute path, so an isolated directory works the same on every platform.

**Why.** The first Windows installer closed on launch: Windows has no shell variable and no
`/bin/sh`, the fallback was `/bin/sh` too, and the configuration directory came only from
`XDG_CONFIG_HOME`/`HOME`, which Windows does not set (#161). PowerShell first is what Windows
Terminal does. The program still comes from the session's own environment, never from configuration
or a foreign file, which is what keeps launching it outside the sensitive surface.

**Discarded.**
- **`cmd.exe` first, from `%ComSpec%`**, as WezTerm does: the shell the agent CLIs document for
  Windows is PowerShell, and `cmd.exe` stays one keystroke away as the fallback.
- **Taking the shell variable as it comes**: a Git Bash or MSYS session exports `/usr/bin/bash`, a
  path Windows cannot launch, so every console would fail into `cmd.exe` with an error line.
- **A bare `cmd.exe`**: the launcher searches a bare name again, current directory and relative
  search-path entries included, so a repository holding a `cmd.exe` could run in place of the system's.
- **`%LOCALAPPDATA%` or `%USERPROFILE%\.config`**: the settings are a person's preferences, which
  Windows roams in `%APPDATA%`; a `.config` folder is a Unix convention Windows tools do not look in.

**Open.** Measured only on Linux, with the decisions tested as pure functions over given values. A
real spawn of the resolved default and of the fallback runs on the pull-request Windows job; the
installed app's launch is checked by hand until #162 adds a smoke test.

## D108 — On Windows a panel file is told apart by its file index, links are not followed at open, and project paths drop `\\?\`

**Decided.** Four Windows arms get what their Unix arms already had (#177):

- **A panel generation carries the file's identity on Windows too**: the volume serial number and the
  file index, read with `GetFileInformationByHandle` from the handle the snapshot already holds, in
  the fields Unix fills with `dev` and `ino`. A producer that renames identical bytes with the same
  modification time over `panel.json` after a retirement publishes a new generation, as on Unix. The
  cheap stamp the retired probe reads first stays without identity on Windows: a stamp that cannot
  tell two files apart costs at most one 500 ms probe before the whole generation is compared.
- **The viewer does not follow a link at the last component of an untracked file**: it opens with
  `FILE_FLAG_OPEN_REPARSE_POINT` and refuses what std's `is_symlink` calls a link on that handle — a
  symbolic link or a junction. Any other reparse point (deduplication, compression, a cloud
  placeholder) is opened again through its owner, so its content is what is read, and that second
  handle is kept only if it reached the same file.
- **A project's key, name and path, and a roadmap's watcher alias, come from `dunce::canonicalize`**,
  so on a local drive they read `C:\…` and not `\\?\C:\…`. The viewer's repository root keeps std's
  verbatim form.
- **`~/` and `~\` in a console link expand from `std::env::home_dir()`**, which std reads from
  `%USERPROFILE%` on Windows, not from `HOME`.

**Why.** Without identity, a re-publication of the same bytes after a retirement is invisible on
Windows. Without the flag, the Windows arm opens whatever a link swapped in after the walk points at.
A verbatim path is what people saw as a project's name and path, and `cmd.exe` given one as its
working directory warns and falls back to `C:\Windows`. And a `~/README.md` link did not open on
Windows, where std ignores `HOME`.

**Discarded.**
- **std's `MetadataExt::{volume_serial_number, file_index}`**: unstable on 1.97.1 (`windows_by_handle`).
- **Identity in the probe's stamp**: it would turn the stamp from a path metadata call into an open of
  our own, to save at most 500 ms.
- **Refusing any `FILE_ATTRIBUTE_REPARSE_POINT`**, or **keeping the handle opened on a reparse point
  that is not a link**: the first makes every untracked file under a redirected Documents folder
  unreadable; the second reads a deduplicated or compressed file's stub instead of its content.
- **Holding each intermediate directory open without share-delete**, or `NtCreateFile` relative to a
  directory handle: the race they close needs a live process of the same user, which can read the
  target itself; Git never writes junctions.
- **`dunce` for the viewer's root**: it is never shown, and a non-verbatim `root.join(name)` goes
  through Win32 normalisation — `aux` becomes a device and a trailing dot is stripped.
- **Stripping `\\?\` by hand**: `dunce` keeps it exactly where stripping changes the meaning (reserved
  names, paths over `MAX_PATH`, a trailing dot or space).
- **Code in the session file's exclusive create**: `create_new` already refuses any existing entry,
  dangling links included, and `%APPDATA%\ivy` inherits an ACL that grants only that user, SYSTEM and
  Administrators.

**Open.** Nothing here ran on a Windows machine before the pull-request job `windows-startup-checks`:
the author's Linux machine had no MSVC or LLVM tools, and `ring`'s build script needs them to
cross-check `app`. The reopen of a non-link reparse point is not covered by a test. A link swapped
into an intermediate directory between the walk and the open is still followed. On ReFS (a Dev Drive)
the 64-bit file index is not guaranteed unique; the 128-bit `FileIdInfo` would be. A project on a
network share keeps its `\\?\UNC\` form, which `dunce` does not strip. Codex route storage, which has
the same missing no-follow flag, is not supported on Windows at all yet and is #173's.
## D109 — Files opens any file: Markdown formatted, everything else as read-only, coloured code

**Decided.** One click on a file in Files opens it in the drawer's document tab, now called
**File**. A Markdown file keeps its formatted reader. Any other regular file opens in a **code view**
of the same tab:

- It is read-only.
- It has a line-number gutter.
- Its text is monospace and coloured by language family: Rust; the C-like languages (C, C++, Go,
  Java, Kotlin, Swift, C#, JavaScript, TypeScript, SCSS); CSS, which has block comments only; PHP; the
  `#`-comment family (Python, shells, Ruby, YAML, TOML, INI, `Makefile`, `Dockerfile`, a `#!` first
  line); JSON; SQL; plain text.
- It refreshes once a second, like the Markdown reader.
- A file it cannot show says so in one sentence: `Binary file` (a NUL in the first 8 KiB),
  `Not UTF-8 text`, or `Larger than 1 MiB`.

Colouring is a small scanner of our own that runs on the loader thread. The frame only paints its
runs.

- **What the scanner marks:** comments, strings, numbers, keywords, capitalised type names, and
  attributes, preprocessor lines and `$`/`@` names.
- **Where the colours come from:** keywords, strings, numbers, type names and attributes take their
  ANSI palette entry, then the bright one, keeping the first that reads at 4.5:1 on the document and
  on a selection over it and differs from the colours already taken; a theme that offers neither
  leaves that token in the text colour. Comments and line numbers take the muted text roles.
- **Long lines:** a line over 16 KiB is painted plain, cut, and marked; copying it gives the whole
  line.
- **Selecting and copying:** a selection is kept as file positions, so copying a selection longer
  than the screen, or `Ctrl+A`, copies the file's own text and never a line number.
- **Files:** directory rows lose their folder icon. The caret alone says a row is a folder, and the
  name starts right after it. The root caption keeps its icon.

This amends two earlier notes. #88's search said Ctrl+Enter opens only a Markdown file and that
Files «says it opens Markdown alone»: it now opens any file, and that sentence is gone. #89 said
maximize serves «Markdown alone». The whole drawer maximizes, which
the code already did, and code is where the width pays most.

**Why.** Reading what an agent just wrote meant spending a console on `less` or `vim`, and Files
selected every non-Markdown file without opening it (#88 left a general text viewer as separate
work). The owner asked for a read-only viewer with simple colouring (#159).

**Discarded.**
- **`syntect` (with `two-face`) or `tree-sitter`.**
  - `syntect` adds a few megabytes and a syntax dump.
  - `tree-sitter` compiles C per grammar, which hurts the Windows build.
  - Both bring colour themes of their own that would have to be mapped onto the 22-colour theme.
  - If grammar fidelity is ever wanted, only `viewer/highlight.rs` changes.
- **A pager in an auxiliary console:** it costs the console the feature saves, and a new console
  type was already rejected in #88.
- **A new sidebar tool for code:** the drawer's loader, refresh, painter and maximize already fit.
  A third tool would duplicate them.
- **Copying from egui's laid-out labels:** only the rows on screen are laid out in a frame, so a
  long selection would copy truncated without saying so.
- **Keeping the tab named «Markdown»:** it would say the wrong thing whenever it shows a `.php`.

**Open.**
- No measured batch loads or paints a document in the drawer. The code view's painting is inside the
  declared «document viewers» span, but it has no published figure.
- A selection is made by dragging within the visible rows. Dragging past the edge does not scroll.
- The header with the breadcrumb, Open and its menu came with #159's second pull request (D110).

## D110 — Open hands the desktop only documents, judged where the link leads, and never through a shell

**Decided.** The code view has a header row above the file:

- **The path**, from the Files folder the file was opened from down to the file, with the file's
  type icon. A file outside that folder shows its whole path instead. A click on a folder of the
  path shows it in Files:
  - inside the root Files shows, it is revealed, expanded and selected in that tree, and the root
    is kept;
  - outside that root, the folder opens as Files' new root.
  The folders are spelled from the root as Files took it, links unresolved, because Files compares
  a folder with that root.
- **Open**, which hands the file to the desktop's default application.
- **A menu beside Open**: Open in File Manager (the file shown selected), Copy absolute path, Copy
  relative path, and Wrap lines.

Opening a file runs whatever the desktop associates with its type, and the files come from a
cloned repository or an agent's output. So Open follows seven rules:

- **An allow-list, not a deny-list.** Open launches only these last extensions, in any case: `txt
  log md json yaml yml toml ini csv css rs c h cpp go java kt swift cs ts tsx pdf png jpg jpeg gif
  webp`. Anything else, a file without an extension included, is shown in the file manager
  instead, and once the path is resolved Open's tooltip says so before the click. A script is
  revealed, never run.
- **Only a regular file, read again at the launch.** A folder, a pipe or a device named like a
  document is revealed. Right before the launcher gets the path, it is read again without following
  links: if it is no longer a regular file, nothing is launched. That narrows the window in which a
  judged file can be swapped for a link; it does not close it.
- **Judged on the canonical path.** Every link is followed first, and the decision and the launch
  use that same path. A `notes.txt` link to `run.sh` is judged as `run.sh`.
- **A name must show as itself.** A name with a line break, a direction override or another
  character `panel_contract::is_shown_as_itself` refuses is revealed. On Windows a name ending in a
  dot or a space is revealed too, because Win32 drops that character and another file would open.
  Both Copy items refuse a path with a line break or a direction override anywhere in it, folders
  included, and say why. What they copy is the literal path, with no quoting.
- **No shell, one argument.**
  - Linux: `xdg-open` with the absolute path as its only argument.
  - macOS: `open`, and `open -R` to reveal.
  - Windows: `ShellExecuteExW` on the shell item of the path, and `SHOpenFolderAndSelectItems` to
    reveal, with COM started on the click's thread and the `\\?\` prefix of a canonical path
    removed.
- **A thread per click, and the child reaped.** The window never waits on a launcher. The thread
  waits for the process it started, so none is left as a zombie. A failure prints one line on
  standard error with the path escaped.
- **Linux reveals over D-Bus, with a deadline.** `FileManager1.ShowItems` is called with the file's
  `file://` address and given five seconds. If the call fails or times out, one line on standard
  error says why, and the parent folder opens instead, once it is read again as a folder and not a
  link. A reveal never falls back to opening the file.

Copy relative path counts from the Files folder of the source console **when the file was
opened**. Files follows the focused console, so reading that folder later would change the answer
under an open file. Outside that folder the item is off and says why.

**Why.** Associations differ per machine and installers add new ones: `.py`, `.jar` or `.js` on
Windows, `.desktop` on Linux. A list of what must not run would always lag behind what does. The
owner decided on 20 November 2026 that Open shows a script or an executable instead of running it.

**Discarded.**
- **A deny-list** (an exec bit, `PATHEXT`, known script extensions). It misses what an installer
  registers. And on a WSL `/mnt/c` drive every file has the exec bit.
- **`cmd /c start`.** `cmd` reads `& ^ % !` in names, and all four are legal on Windows.
- **`explorer /select,<path>`.** It splits its argument on commas, and a misparse may open the file
  instead of selecting it.
- **A `dbus-send` process.** It is one more process and one more argument to quote. `zbus` is
  already compiled for scope placement (D104).
- **A WSL gate.** It had no confirmed launch path, and it would have revealed `report (1).pdf` for
  its brackets.
- **A network-share check on Windows.** The loader and Files have already read that share before
  Open is offered.
- **`html`, `svg` and `xml` in the allow-list.** The default browser runs their scripts, and an
  XML file in the XHTML namespace is a page; the default handler for XML is often a browser.

**Open.**
- The macOS and Windows openers are compiled per platform, but no one has run them yet (F2.7).
- Under WSL with `wslu`, `xdg-open` may be `wslview`, which hands the path to PowerShell or `cmd`.
  Nothing mitigates that.
- No measured batch opens the header's menu or wraps a file.

## D111 — On Windows a console closes its pseudoconsole when its program ends and when it is closed, never on the interface pass

**Decided on 20 November 2026** ([#163](https://github.com/mashware/ivy/issues/163)). On Windows a
console's master lives in a shared slot, and **taking it out and dropping it is closing the
pseudoconsole** (`ClosePseudoConsole`):

- **When the program ends.** A thread started with the console waits on a handle of the program's
  process and, when it ends, closes the pseudoconsole. The output pipe breaks, the reader reads end of
  file, and the console reports an ordinary ending (`Ending::EndedWell`, D38): typing `exit` closes
  the pane, and the last pane closes the window, through the path Linux and macOS already take.
- **When the console is closed.** `Drop for Pty` hands the master to a thread that drops it and,
  on a second thread, gives the program 200 ms — the grace D55 gives on Unix — and ends it with `TerminateProcess` if it
  is still there, reading the result. Closing first is what tells the program: Windows sends the
  programs attached to the pseudoconsole a close event. The reader then ends with end of file instead
  of being abandoned at `READER_GIVE_UP` (D32), and a writer parked on a program that reads nothing
  fails with a broken pipe and ends (D29).
- **The reader drains to end of file.** After a close or after its host is gone, it keeps reading and
  discarding instead of returning. The reason is recorded and the output channel is let go first, so
  the host learns at once that the console is over.

**Why.** Three symptoms had one cause. `portable-pty` keeps the pseudoconsole in a value shared by the
master and the slave, and closes it only when the last of them goes. It also hands both pipe ends it
creates to the pseudoconsole's host and keeps neither. So nothing this crate held could produce an end
of file while the `Pty` lived: an exited program left a dead pane, the window never closed after its
last shell, and the «stopped receiving» notices never fired. The close itself ran last in `Drop`, on
the interface pass, after the reader had been told to stop. Microsoft documents that
`ClosePseudoConsole` can wait indefinitely for its last frame to be read, and before Windows 11 24H2
the call is synchronous.

**A refusal is seen now.** `portable-pty`'s `kill` reads `TerminateProcess`'s result the wrong way
round and then discards it. This crate calls `TerminateProcess` itself. A refusal writes D56's line,
and the pseudoconsole is closed anyway. **Whether the program ended is asked of its handle, never of
its exit code**: a program that exits with 259 reports the same code Windows uses for «still
running». The console's status reads it the same way, or such a pane would never close.

**On Windows, `exit` also ends programs still attached to that console.** `ClosePseudoConsole` sends
them Windows' close event and then ends them. On Linux a survivor holding the terminal keeps the pane
open (D32). Whether closing should end a console's background programs everywhere is #193's
question; this decision does not answer it beyond what the platform's own close does.

| Discarded | Why |
|---|---|
| **Overlapped writes and `CancelIoEx` for the writer**, the ticket's proposal | `portable-pty`'s pipes are anonymous pipes, which do not support overlapped I/O. It would mean replacing `openpty`. Closing the pseudoconsole already breaks the pipe the writer waits on |
| **A Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`** | It decides that closing a console ends its whole process tree, which is #193's open product question |
| **Terminating first and closing after**, the order the first version of this change had | The program is then killed with no close event and no grace, where Unix gives it 200 ms to save its state (D55). The review caught it |
| **A Job Object's own grace timer** | The close event plus this crate's 200 ms is the same grace without a Job Object |
| **Only the exit watcher, leaving `Drop` as it was** | Every close would still wait out the reader deadline, abandon the reader, and close on the interface pass with nobody draining |
| **A drain thread on a second reader on the close path** | It needed the watcher to check the stop flag. That opened a race: the watcher closing while `Drop` stopped the reader, with nobody draining. A reader that drains until end of file removes the race and the second reader |
| **`reaped_within` and `left-running` on Windows** | Windows has no zombies to reap, and the exit watcher already waits on the process |

**What it costs, declared.** Each console has one more thread, parked on its process handle, for as
long as its program lives, and a closed one has two short-lived threads, the close and the grace —
two because before 24H2 the close may never return, and the program must still be ended. A program
that survives both the close and `TerminateProcess` keeps the watcher and the reader for as long as
it lives, the same trade D56 makes. If the grace thread cannot be created, the program is ended at
once. If the closing thread cannot, the master is leaked rather than closed on the interface pass,
where a reader that is not draining could freeze the window for good; the reader and the writer then
stay for the life of the process, and standard error says so.

**Not verified.** The tests run only in the prerelease workflow's Windows job (prefix `windows_exit`,
D108). That image is Windows Server 2025, built on the 24H2 codebase, so it may not show the
synchronous close of Windows 10 and Windows 11 before 24H2. Nobody has watched a window close itself
on a Windows desktop yet (F2.7).

## D112 — On Windows an installed Ivy runs its consoles on OpenConsole, shipped beside `Ivy.exe`

**Decided.** The Windows installer carries `conpty.dll` and `OpenConsole.exe` from Microsoft's
`Microsoft.Windows.Console.ConPTY` package (microsoft/terminal), flat beside `Ivy.exe`. The
package version and the SHA-256 of each of the two files are pinned in
`packaging/windows/conpty.json`; `scripts/conpty.py` refuses any other file, and staging refuses a
pair that is not the pinned one. No code selects it: `portable-pty` opens kernel32's pseudoconsole
and then prefers a `conpty.dll` it finds by name, and that library is reported (a disassembly of
1.25) to start the `OpenConsole.exe` it finds beside itself. The pull-request Windows job runs one
test that writes a directory report (OSC 7) from Windows PowerShell and requires it to reach the
screen, with the `OpenConsole.exe` beside the test binary hosting the console, and runs D111's
close tests a second time on that host.

**Why.** The system's host (`conhost`) re-renders a console from its own buffer instead of passing
the program's output through. It is reported to swallow some OSC sequences (10/11), and nothing
showed it keeps a directory report, which is what the PowerShell integration of #169 rests on. The
owner chose on 20 November 2026 to ship Microsoft's newer host in that work rather than probe the
system's first or assume it. It is also what Windows Terminal and WezTerm run. And a `LoadLibrary`
by bare name with no file beside the executable goes on to the system directories, the current
directory and the search path, so until now a planted `conpty.dll` there could be what Ivy loaded.
Each file is pinned rather than the package, because nuget.org can re-sign a package, which changes
the archive and not the binaries.

**Measured.** On the pull-request Windows job (Windows Server 2025, 10.0.26100, 20 November 2026)
the OSC 7 test passed in 1.19 s with the bundled `OpenConsole.exe` hosting the console, and D111's
four close tests passed on that host as well as on the system's.

**Discarded.**
- **Probing the system's host first.** It would have told whether one build of Windows keeps OSC 7,
  not every build an installer reaches.
- **`PSEUDOCONSOLE_PASSTHROUGH_MODE` on the system's host.** `portable-pty` 0.9 defines it and never
  passes it, and what it does varies by Windows build.
- **Nesting `OpenConsole.exe` under `x64\`**, as the package's MSBuild targets do. The library is
  reported to look there too, after beside itself; flat is the one layout with nothing to get wrong.
- **The newest release.** It was eleven days old; the pin is the newest stable release older than a
  month.

**Open.**
- Development runs (`cargo run`) keep the system's host, and with it the planted-`conpty.dll`
  search above.
- An `OpenConsole.exe` that is missing is reported to make the library fall back to the system's
  host, which says nothing; one that is present and blocked (antivirus, application control) is reported to make
  every console fail to open instead. Neither is detected at run time.
- Which width rule the host measures text with is not passed: `portable-pty` hard-codes the
  pseudoconsole flags. Where it differs from the engine's one cell per code point — a `☁️` or a
  joined emoji — a line the host redraws could land a column off. Not measured.
- The installer's contents are checked by the release job's install test, not by any pull-request
  job.

## D113 — Send writes an annotated copy into the console's own folder and submits one line with Enter

**21 November 2026 — #178.** On an image open in the drawer (D72), a plain left drag or click draws
an arrow, an ellipse, a rectangle or a numbered marker; each marker takes a note, and the image a
general note. Marks are kept in the original's pixels, so zooming and panning move them with the
image. Undo (its button, or Ctrl+Z — Cmd+Z on macOS — with no other modifier, while the drawer
holds the keys and no text field has the keyboard), a delete per marker and Clear take them back;
deleting a marker renumbers the ones after it with their notes. A drag shorter than four screen
points leaves nothing: an image pixel means nothing on a 4 000-pixel screenshot fitted to 500
points. A marker's note field is named by its image's opening and a serial the marker gets when
placed, never given again on that image — not after an undo or a Clear — because egui keeps a
field's focus and undo history by its id: named by its place, deleting marker 1 handed marker 2
the first one's history, and a Ctrl+Z there brought the deleted note back (#197). Another image
starts its serials again, since the opening already tells its fields apart.
**«Send to <console> and press Enter»** then:

- **writes a copy into that console's `PANEL_DIR`**, the original's width × height with the marks
  drawn antialiased by tiny-skia and the markers' numbers from the bundled Geist outlines, named
  `annotated-<n>.png` with the first `n` whose name nothing in the folder takes, a dangling link
  included. It is created with `create_new` on a worker, and on Unix with mode 0600 (elsewhere the
  platform's default permissions); the original file is never written.
- **types one line into that console**: `Annotated screenshot: <copy> · 1: <note> · 2: <note> ·
  Note: <general>`, empty parts left out, every control character turned into a space. That text is
  painted under the image and its notes before the click, from the same function that composes what
  is sent — inside the paste markers when the console asks for them — together with «If something
  is typed at this console's prompt, Send asks first; Enter runs the line.» The paste is composed for
  the paste mode the console has when it is sent, after the copy is closed. The notes sit in a fixed
  column beside the image, so the image keeps its height however many markers there are, and go
  under it only where the drawer is too narrow for both; the line takes a band of its own under
  both, the full width, because a copy's path wraps into many lines in a column. Neither paints
  outside the drawer's body: on a drawer too short for the toolbar, a wrapped notice and the button,
  the image gives up its height first, then the notice is cut to one line, shown whole on hover,
  above a button that keeps its full height; where not even that line fits above the button, the
  line takes the column's head and the button is cut under it — not laid out at all where less than
  12 points of it would show, a strip nobody can read or aim at, the notice then adding «Enlarge the
  drawer to send» on hover — and never laid over the toolbar: a refusal clipped away says nothing at
  all, while a taller drawer gives the button back (#197).
- **presses Enter in a write of its own, on a later frame.** People driving Claude Code from tmux
  report that text and Enter sent together are read as part of the paste; a separate write a frame
  later is what a person's Enter is, for an agent and a shell alike. A frame egui runs twice still
  keeps the Enter for the next frame. The Git review's insertion keeps writing no Enter.

**A draft at the prompt is asked about before the line joins it (#196).** Each console keeps one
bit: whether the person sent bytes there since their last Enter. It comes from the app's own writes
on the person's behalf — keys, pastes, a held paste let through, a panel button, the Git review's
insertion, Send's line and its Enter — by one rule: after a write holding an Enter (`\r`) or a
Ctrl+C (`0x03`), only what follows the last of them stays; any other write adds to it. A fresh shell
started over a failed client clears it. The program's output, the terminal's replies and mouse
reports never touch it, and nothing is read from the screen (D6). With the bit set, Send writes
nothing and the drawer says «Something is typed at <console>'s prompt; it will be sent with this
line.» with three answers: **Send anyway**, which sends as a Send always has; **Type without
Enter**, which writes the copy and the line with no Enter, clears the marks and leaves the bit set;
and **Cancel**, which types nothing, keeps the marks and notes, and removes a copy already written.
The bit is checked at the click and, while the person has not answered, once more when the copy is
written, just before the line would be typed: what was typed while the copy was being written is
asked about too, with the copy kept for the answer. **The completed-copy check follows the current
frame's console input**, because checking first missed a draft typed in the very frame the copy
finished and the later Enter submitted both without asking. An Enter already owed for a line typed
in an earlier frame goes **before** new console input; moving all delivery after input would join
the next draft to that already-written line instead. The Git review's insertion retains its place
before console input. Once answered, a Send is not asked again — what
is typed after «Send anyway» goes with the line, as the person chose. Closing the image or its
console while the question waits drops the Send, a closed console said where the image is still
shown, and a copy it had written is removed with the folder of a closed console. A window that
closes over the question removes the copy after its consoles are gone, so their folders, which held
it, stay and say so — accepted for a window quit with a question unanswered. On a drawer too short for the Send button the answers are not laid out either,
and the notice says on hover to enlarge it. **The bit is a guess about a line editor, and its misses
are accepted:** Shift+Enter is a `\r` here and counts as an Enter, though Claude Code reads it as a
new line in the same draft; Ctrl+J submits in a shell and does not count; and a line cleared with
Ctrl+U or Esc, or keys a full-screen program took, still count as typed, so Send asks where nothing
is left. A wrong guess costs one click; a right one keeps a draft from being submitted glued to the
line.

**Only the copy's path goes in the line, never the path the program printed.** The copy's name is
the app's own (`[a-z0-9-]` and `.png` in a directory the app created); the original's is whatever a
program chose and may hold shell syntax. **Nothing is escaped for a shell**: the app cannot know
which shell or agent reads the line (D6), and no prefix keeps zsh from running a redirection. Into
a plain shell, Enter runs the notes as a command; the person sees the line and the console's name
before choosing to send, the D42 precedent.

**Every refusal that can be known before writing is checked on the frame after the click**, in the
window's input pass, before the copy is started — the image replaced, the console gone, no
`PANEL_DIR`, a paste waiting for an answer, a writer that gave up, a line longer than the console
keeps in one paste, a first free copy name that is no longer the one the line showed (taken since,
or an earlier one freed) — and a refused Send writes no file. **From the click to the typed line
the marks and notes are frozen**: the copy is drawn from them a frame after the click, and a delivered
Send clears them, so an edit made meanwhile would be wiped unsent; a Ctrl+Z in the very pass of the
click is refused too, and so is a second Send. **Until the line is typed, a Cancel beside «Sending…»
takes the Send back** (#197): the marks and notes stay and unfreeze, nothing is typed, and a worker
already writing is parked — a write cannot be interrupted — and polled from the window's input pass
until it ends, when its copy is removed, and with it the folder of a console that closed meanwhile.
While a worker is parked, Send is greyed out and reads «Still finishing the copy for <console>…» —
on the button, because any other notice can replace Cancel's — and a Send asked anyway is refused
with «Still finishing the previous copy» before any other check, so two workers never race for one
name; the decoder's abandoned read is the same pattern. What a parked worker leaves to say — a failure, a copy or folder it could not remove —
goes to standard error as well, and waits in its own console's slot until the drawer lays that
console's image out, or its Image tab with no image, because the image it was for may be closed by
then; there it joins the notice shown rather than replacing it,
which may be a refusal nobody has read yet. Once it ends cleanly the drawer says its copy is gone,
and that Send is ready only with marks to send and a folder to write into. **While the copy is
written, the line painted is the Send's own**: the worker's file takes the name the line showed, so
recomputing the first free name would paint `annotated-<n+1>.png` over a Send that types
`annotated-<n>.png`. When the copy is written the line goes to the image's own console in that
frame, whichever tab is shown and whether the drawer is open, **if that opening is still in its
console's slot**, or the copy is removed and nothing is typed anywhere. The drawer refuses to open
another image in that console while its Send is busy, and other consoles' slots are not affected
(#198); the Send still carries which opening of an image it belongs to, as a guard should that
refusal ever give. Each refusal for a console still open says why under its image and keeps the
marks; a delivered Send
clears them. A failure says everything it knows: a write that fails after its console closed names
both, the close said once whether the console closed before or after Cancel, and a half-written
copy that could not be removed is named with why. Since #198 a console
that closes takes its image, marks and notice with it, so what a Send for it still has to say — a
refusal, a failed write, a cancelled worker's last word — goes to standard error only, never under
another console's image.

**One Send runs at a time for the whole window, and a greyed Send says whose (#198).** While
another console's Send is between its click and its Enter, this console's button reads «Waiting for
the Send to <console>…»: the label and whether the button answers are read from one value, so no
button greyed by a Send reads «Send to … and press Enter»; one greyed for want of a folder keeps
that label and says why in the line under it. The Send's own image, once its line is typed and the
Enter is owed, reads «Sending…» with no Cancel beside it: nothing is left that Cancel could take
back, and it does not wait for itself. **A console that closes with its Send in flight
takes that Send back as Cancel would**, on the next input pass and whichever way it closed — the
console, its tab, a split or a retirement: its Cancel went with its image, and otherwise every
console's Send stayed greyed, with no Cancel anywhere, until the worker ended. Its worker is parked
as Cancel parks one, so the other consoles' Send reads «Still finishing the copy for <console>…»
until it ends: the rule of one parked worker is kept. Every line a Send tells standard error names
its console — «Send to <console>: …» — because that stream is shared by every console, and so do
«Waiting for…» and «Still finishing…»: by its label, followed by «(Console <n>)», the number the
app gives every console, unless the label already is that number, because two consoles routinely
share a title such as the agent they run. Discarded:
cancelling in each place a console is closed, which the next way of closing one would miss; and
letting the other consoles' Send start while a closed console's worker is still parked, which needs
more than one parked worker and is a decision of its own.

**The copies are the app's to remove.** A console records the copies delivered to it in a field
declared before its `PanelDir` (since #198 the console's image, `image: viewer::image::Console`), so when its `Pane` is dropped — the console, its tab or the window
closed — they are removed first, by the paths recorded and with `remove_file`; then `PanelDir`
finds its directory empty and removes it as it always has. A process that is killed drops nothing
and leaves both. A file the console's program left there is not touched and keeps the directory. A
copy written for a console that closed before it could be delivered is removed with the directory
it left, and so is that directory when the write failed; `PanelDir` had told standard error the
directory remains, so its removal is told there too. `tests/shape.rs` pins the field order. A
worker that never returns no longer locks the drawer: Cancel parks it, and only a new Send waits
for it.

Discarded: pins with normalised coordinates and no copy, because a model reads a circle drawn on
the pixels reliably and «pin 2 at 0.43, 0.71» poorly; a hand-written line rasteriser and digit
table, which gives aliased marks and crude numbers; the viewport's screenshot, which is at screen
scale and includes the interface; the paste and the Enter in one write, the case reported to fail;
driving the owner's Claude Code automatically to settle that before building, which would use their
account unattended for what the manual check of this decision answers; numbering by the copies
already delivered, which a stray `annotated-1.png` in the folder turned into a refusal of every
later Send to that console; renaming the copy when a name is taken after the line was shown, which
would send a line different from the one shown; teaching
`PanelDir::remove` the app's file names, which puts app knowledge into the contract crate (the
store's card «the contract removes what the contract puts there» argues for it, and is set aside
here because the copies are the app's, not the contract's); escaping the notes for a shell;
reading the prompt off the console's screen to know whether a draft is there, which is the app
deducing a program's state from its painting (D6); Send never pressing Enter, which gives up the
one-click Send #178 asked for and leaves an image unsent when the person walks away;
a deadline after which a stuck write is given up, and a Cancel offered only after some seconds,
because a slow disk is not a stuck one and no constant tells them apart — the person does;
re-keying or clearing egui's field state when a marker is deleted, which the serial makes needless
and which would not cover an undo or a Clear; carrying the serial on from one image to the next,
which protects nothing the opening in the id does not; and
refusing characters that are not shown as themselves in notes, since the person typing them is the
person sending them, and control characters become spaces so the line shown is the line sent.

**Amended by D120 (#199):** the copies field also holds screenshots pasted from the clipboard, and
they are removed with the console the same way.

## D114 — The window keeps its own standard error in a log file, and records a panic there itself

**Decided.** Before the window opens, Ivy replaces its own standard error with a pipe it owns. One
thread reads that pipe, writes every chunk to `ivy.log` and hands it to a second thread that writes
it to the standard error Ivy was started with; when that one falls 256 chunks behind, the terminal
loses chunks and the file does not, and the terminal is told how many bytes it missed and which file
has them as soon as it takes lines again. The file lives in the platform's log directory:
`~/Library/Logs/Ivy/` on macOS, `~/.local/state/ivy/` on Linux, `%LOCALAPPDATA%\Ivy\logs\` on
Windows. An absolute `$XDG_STATE_HOME` rules on every platform and the file goes in its `ivy/`, as
`$XDG_CONFIG_HOME` does for the configuration, so an isolated run writes its own log. Once it holds
4 MiB it becomes `ivy.log.1`, replacing the previous one; the size is checked before each write, so
the write that crosses the cap lands whole. Every window is a process of its own and they share the
name, so before each write a window compares the file it holds with the file at the name, and one
whose file was moved aside by another window, or deleted, follows the name. A file that cannot be
moved aside is emptied and says why on its first line, so the cap still holds. On Unix both files are
created readable by their owner only. A panic writes one record — time, thread, message, location
and backtrace — straight to the file, replacing Rust's default report, and hands it to the
terminal's thread without waiting for it. Settings shows the path and a button that shows it in the file manager. A measured run
captures nothing. When the file cannot be opened, standard error stays exactly as it was and one
line says why.

**Why.** Opened from the Dock, a desktop launcher or the Start menu, nobody reads Ivy's standard
error: launchd sends it to `/dev/null`, a launcher outside a systemd user unit drops it, and on
Windows it goes to a console window nobody keeps. Every failure line this project decided to write
there (D38, D41, D51, D56) was lost for the people who most need it, and a crash left no trace at
all. Taking the stream at the process level captures all of them, and anything a dependency writes,
without touching a single call site. The second copy keeps what a shell launch shows, and the cap
holds while the window runs: a console that keeps failing cannot fill the disk.

**How it ends.** `main`, not `run`, holds the capture: a window that cannot open is reported after
`run` returns, so `main` prints that line itself while the capture still stands. Closing the
capture puts the original standard error back first, which takes the pipe's writer away; the
threads then read its end and finish. The wait for them is bounded at 100 ms (D57): a program the
window started with an inherited standard error also holds the pipe, and is not waited for.

**Not measured.** The threads never run on the frame, and a measured run does not start them, so
there is no frame cost to declare (D13).

**Discarded.**
- **Pointing standard error at the file and nowhere else.** No running cap — rotation only at the
  next start — and a shell launch would lose what it shows today.
- **A logging macro at every call site.** About 120 sites in three crates, every new one able to
  forget it, and nothing a dependency writes would arrive.
- **Leaving it to the operating system.** No facility covers the three platforms, none records a
  panic that unwinds, and none tells the person where to look.
- **Chaining Rust's default panic report.** The file would hold every panic twice.
- **Writing the original from the copying thread.** A terminal that stops reading (Ctrl+S, a
  parent that never drains its pipe) would stop the file after one chunk and then the window.
- **One file per window.** A report would have to say which of several files to attach.
- **A new About dialog for the path.** Ivy has none; one row in Settings is enough to find the file.

**Open.**
- **What the runtime writes as it aborts is lost from both copies.** A stack overflow, a failed
  allocation, or the abort line after a panic inside a panic is written into the pipe and ends the
  process before the thread can copy it; the panic records themselves still reach the file. Before
  this decision a shell launch showed it.
- **Two windows crossing the cap in the same instant** can both move the file aside, and the second
  then replaces the first one's copy. It takes two processes within a few microseconds.
- **At closing, lines written after standard error is put back** can reach a terminal before lines
  still queued for it.
- **A panic record can come before the lines that led to it.** It is written straight to the file
  while earlier lines may still be in the pipe.
- **On Windows it is read, not run.** Rust's standard error asks for the process's handle on every
  write — `library/std/src/sys/stdio/windows.rs`: «Don't cache handles but get them fresh for every
  read/write» — which is what makes `SetStdHandle` take effect, and a process born with no handle
  discards the write as written instead of failing. A console is written as UTF-16, as Rust's own
  standard error does. No Windows machine has run the capture yet, and the Windows pull-request job
  runs only the tests it names.
- Helper programs (`panel-mcp`, `ivy-session`, `ivy-codex`) keep their standard error with whoever
  launched them.

## D115 — On Windows, Ctrl+Insert copies, Shift+Insert pastes, and Shift+Delete belongs to the program

**Decided.** The three chords that **D52** and **D53** left «for F2.7 to decide» are decided, for
Windows only; Linux and macOS keep exactly what D52 gives them.

| Chord | Before, on Windows | Now, on Windows |
|---|---|---|
| `Shift+Delete` (any other modifiers too) | a cut request with no modifiers → `0x18` to the program; `nano` exits | a key: `CSI 3;2~` (`CSI 3;m~` with others), as on Linux and macOS |
| `Ctrl+Insert`, exactly | a copy request → `0x03`, an interrupt | copies the marked text, through the same switch as `Ctrl+Shift+C` (`key.copy`); with copy switched off it reaches the program as `CSI 2;5~` |
| `Ctrl+Insert` with Shift or Alt too | a copy request (with Shift it copied, without it `0x03`) | a key in D52's form (`CSI 2;6~`, `CSI 2;7~`…) |
| `Shift+Insert`, exactly, text on the clipboard | the paste | the paste — unchanged |
| `Shift+Insert`, exactly, no text on the clipboard | `CSI 2;2~` to the program (D33's fall-through) | nothing at all, the rule `bytes` already applies to `Ctrl+Shift+V` |
| `Shift+Insert` with Ctrl or Alt too | a paste request (or a copy, with Ctrl, because the copy arm is tested first) | a key in D52's form |

**Why these.** They are the defaults of the terminals people use on Windows: Windows Terminal binds
`ctrl+insert` to copy and `shift+insert` to paste and leaves `shift+delete` to the program, and
WezTerm binds the same two. A Windows user presses them expecting copy and paste; a terminal there
that sent `CSI 2;5~` for Ctrl+Insert would look broken. Nobody turns Shift+Delete into a cut inside a
terminal, because there is nothing to cut.

**Where each half lives.** The vendored `egui-winit` (**D33**) gains its third change: its Windows
arms for Shift+Delete and Ctrl+Insert are removed, and its Shift+Insert arm requires Shift **without**
Ctrl or Alt. Removing an arm releases every chord it caught, so each one was followed to where it
lands now (the table). The copy for Ctrl+Insert is decided **in the keyboard pass**, from the key,
because a copy request carries no key: there Ctrl+Insert and Ctrl+C are the same event, and Ctrl+C
must stay `0x03`. That door sits **after** the keymap, so a `key.<action> = ctrl+insert` somebody
wrote still wins. Which platform the pass is on is a parameter, so the Linux suite tests the Windows
answer as well as its own.

**The rest of the window keeps the chords it had.** Every text field (Settings, renaming a pane,
Files' search) and the drawer's code viewer copy and cut only on a copy or cut request, so delivering
two keys instead would have taken Ctrl+Insert and Shift+Delete away from them — found in this
decision's review, and the owner chose to keep them. So before egui reads a batch, the window puts
the request **right behind** each exact Ctrl+Insert and Shift+Delete key, on Windows only
(`keyboard::add_requests_behind`). A text field ignores the key — egui leaves Shift+Delete alone on
Windows precisely because it expects the cut, and has nothing for Insert — and acts on the request.
A console acts on the key and drops the request behind it, armed from the key itself before the
keymap, so a chord somebody bound does not leave its request to reach the program as `0x03` or
`0x18`. The pair is this project's own translation, so unlike the desktop layer it is tested.
What the fields lose is every chord over the two keys **but the exact one**: upstream copied on any
Ctrl+Insert, cut on any Shift+Delete and pasted on any Shift+Insert, so Ctrl+Shift+Insert and
Ctrl+Alt+Insert no longer copy there, Ctrl+Shift+Delete and Alt+Shift+Delete no longer cut — egui's
field ignores Shift+Delete on Windows whatever else is held — and Alt+Shift+Insert no longer
pastes (Ctrl+Shift+Insert never pasted: upstream's copy arm caught it first). Each of those is a D52 key for a console, and a request for one of
them would be a second answer the console would have to drop as well.

**A shortcut bound to `shift+insert` is half a shortcut on Windows**: with text on the clipboard
the desktop layer still turns that chord into a paste before it is a key, so the binding fires only
over a clipboard with no text. Said in the configuration guide rather than refused, because the same
chord is ordinary on the other two platforms.

**AltGr needs no code, and that is a reading, not a run.** `winit` 0.30.13 clears Ctrl and Alt from
the modifiers while the right Alt is down on a layout that has AltGr
(`platform_impl/windows/keyboard_layout.rs`, `get_agnostic_mods`), and it fills the key's text with
the AltGr character; `egui-winit` drops text only while Ctrl is held, so `@` and `{` arrive as text.
The fake left Ctrl Windows sends with AltGr may produce a passing modifier change, which the right
Alt's own message clears before the character key. Confirmed by hand on Windows in D53's rows, or
named there as unverified.

### What was discarded, and why

| Discarded | Why |
|---|---|
| All three as plain keys on Windows, exactly as on Linux | The two Windows conventions above would be missing on the platform where they are expected |
| Shift+Delete kept as a cut, silenced | Silence kills a key D52 gives to programs (`kDC` with Shift) for a cut that does not exist |
| Telling Ctrl+Insert from Ctrl+C in the copy request | The request carries no key: impossible |
| A `Copy` action in the keymap with a Windows factory chord | Copy is a switch, not an action (**D26**): the desktop resolves Ctrl+Shift+C before it is a key, so such an action could only ever be bound to Insert chords, one platform's — a rebindable copy that is not rebindable |
| Delivering Shift+Insert as a key and pasting from the keyboard pass | The pass cannot read the clipboard; the paste branch of `egui-winit` is the only reader, and the paste door already tells Shift apart |
| Accepting that text fields lose Ctrl+Insert and Shift+Delete on Windows | Offered at the review; the owner chose to keep them |
| Producing the key and its request inside the vendored layer | A fourth change to somebody else's code, out of reach of every test (D53); the input hook is this project's own and the suite drives the same function the console uses to drop the request |
| The ticket's rule «on Windows, Ctrl+Alt with text means the text wins» | Not needed for the AltGr key (above), and wrong for left Ctrl + left Alt on a layout without AltGr: `winit` fills the text with the key without Ctrl, so `Ctrl+Alt+A` on a US layout would type `a`. Ctrl+Alt as an AltGr substitute is left out, recorded |
| The same chords as copy and paste on Linux and macOS | Changes two platforms nobody asked about, and widens D53's hand check to them |

## D116 — The Windows file viewer inspects directories through the handle it opened

**Decided.** On Windows, the viewer's bounded file loader opens with
`FILE_FLAG_BACKUP_SEMANTICS`, then uses the opened handle's metadata to reject a directory with
`This is a folder, not a file`. Regular-file rejection and the read ceilings remain on that same
handle. Unix keeps its nonblocking open, so a FIFO cannot hold the loader waiting for a writer.

**Why.** Windows refuses an ordinary file open of a directory before the existing metadata check
can identify it. The code and image viewers therefore showed an access error where their common
loader promises the folder explanation (#186). Allowing a directory handle makes the existing
check reachable; it does not allow reading the directory as a document.

**Discarded.** Inspecting the pathname before opening it, or inspecting it after an open failure,
would diagnose a separate pathname observation rather than the object the loader opened. Replacing
native paths with slash-normalized strings would not solve this open failure and would change
valid Unix filenames. A new path abstraction duplicates the platform path and canonicalization
boundaries already in place (D107, D110).

**Verification.** The code/image folder cases must run on Windows; a Linux pass does not exercise
this open flag. Native desktop interaction remains separate from those loader tests.

## D117 — Windows peer identity is verified on a fresh connection after its process is pinned

The first named-pipe connection is a bootstrap, not a process witness. Looking up its kernel PID
with `OpenProcess` can acquire a later process if the original connector has already exited and
its PID was reused. Reading creation time from that later handle, or comparing the PID twice,
does not bind it to the original connection.

**Decision.** An owned Windows producer opts into `peer_hello` v1. The server opens a candidate
process with `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION`, retaining its non-inheritable
handle before creating a fresh one-shot pipe. Only a connector whose kernel PID matches that
pinned, still-live process becomes a verified control stream. The new pipe has the existing
user/SYSTEM DACL, rejects remote clients and uses `FILE_FLAG_FIRST_PIPE_INSTANCE`; its random name
is never reused. Both streams' servers must match the independently inherited `PANEL_SERVER_PID`.

**Why the order matters.** The candidate process's handle prevents its PID from being recycled.
An earlier owner of that PID cannot have connected to a pipe that did not yet exist. Therefore a
matching connector on the new pipe is exactly the retained process object, including when the
bootstrap's numeric PID was already stale. The proof concerns the actual control stream, not the
historical owner of the bootstrap. Microsoft's [PID-lifetime explanation](https://devblogs.microsoft.com/oldnewthing/20110107-00/?p=11803/)
documents the retained-handle property.

The verified stream and its clones retain the same process object. Its birth identity is the
64-bit creation `FILETIME` from `GetProcessTimes`; liveness is a zero-timeout wait on that handle,
never a new lookup by PID. Diagnostic attribution remains independent of session authorization
(D91). A raw client or explicit unavailable-witness result has no verified peer, may publish with
Unknown provenance, and cannot acquire session or attested-hook authority from payload metadata.

Negotiation stays inside the existing counted connection workers, before authenticated requests
or diagnostic inspection. It has one absolute deadline and shares the channel's stop signal;
all pending overlapped operations are cancelled and drained before their buffers are dropped.
Bootstrap bytes never become authenticated control bytes. Silent legacy clients retain the
existing bounded-resource and shutdown behavior. The opt-in transport frames are specified in
[04-panel-contract.md](04-panel-contract.md); the ordinary JSON operations remain unchanged.

| Discarded | Why |
|---|---|
| `GetNamedPipeClientProcessId` followed by a retained handle on the same pipe | Protects identity after acquisition, but cannot establish whether it was already recycled. |
| Repeat the PID query or compare creation time read from that same handle | Both observations can consistently describe the successor. |
| Authenticate only the user's SID | The pipe's ACL already limits the user; a SID does not identify a console's process occurrence. |
| Replace the channel with local RPC | Adds another transport and client bindings when an ordered connection upgrade reuses the existing implementation. |
| Require a new preamble from every producer | Existing direct publishers must retain D91's Unknown publication behavior. |

This supplies a Windows process witness. It does not by itself implement Windows process ancestry,
native client recognition or complete recovery/hook authorization. Native Windows process/pipe
witnesses are required; cross-compilation is not runtime evidence.

## D118 — Windows graphics startup failures have a native explanation

**Decided.** Returned graphics initialization errors show one native Windows message box before
the process exits with failure. The original error still goes through D114's log. The notice says
that OpenGL 2.0 or later is required, includes the observed context version when available, and
says explicitly when it is unavailable. A configuration, shader or context failure is described
as a graphics initialization failure, never automatically as an insufficient OpenGL version.

**Why.** Installed-launch trials exposed machines whose system OpenGL cannot initialize the
painter. The root window is hidden until its first frame; the Start menu does not show stderr.
Keeping an error in a file helps support but leaves the person with no visible explanation.

The current egui_glow initializer reads GL_VERSION before its low-version rejection, but erases
it from PainterError. A Windows-only log observer accepts only that initializer's Debug version
record, retaining at most 256 bytes of the version and no renderer/vendor. It installs before
startup and stops retaining records when app creation begins successfully. It never replaces an
existing logger: missing evidence yields unavailable. This is a pinned dependency diagnostic
format, not a new provider-state contract. Measured runs install no observer.

**Discarded.** stderr alone is invisible from the Start menu. A second WGL probe adds a window
and context and can observe a different adapter. Vendoring the whole painter for one diagnostic
adds maintenance without changing the visible result. A renderer fallback or shipping Mesa is a
separate product decision. Native driver process termination after drawing begins is not a
returned startup error and is not covered.

**Evidence limit.** Routing and formatting tests do not establish visibility on a real Windows
desktop; that requires launching the installed app with insufficient and supported graphics.
## D119 — Windows process observations describe ancestry, never foreground authority

**Decided for #166.** Windows gains console-bound diagnostic ancestry and separate, handle-owned
process observations. A descendant is not called foreground: `inspect` and `inspect_descendant`
remain unsupported there. The explicit Ivy-owned agent launch approved for #185 remains the
separate prerequisite for Windows hook authority. No observation converts into the Unix
foreground evidence used for hooks or recovery.

**Why.** ConPTY has no Unix foreground process group. Giving every shell descendant that meaning
would let background programs claim the same authority. The user chose process identification
and provenance while preserving the existing authorization boundary. D91's labels remain
diagnostic: incomplete evidence is unknown and does not refuse a publication.

The Windows scope pins its original child and revokes on console close or root exit. A validated
peer chain reaching that exact root is inside; one reaching its verified, pinned Ivy parent first
is outside. A coherent parent-PID-zero boundary is outside too. Missing or dead history is unknown;
PID1 is ordinary. Each retained handle supplies a creation FILETIME for identity and a native
process sequence number for ordering. Parent sequence must be strictly earlier than child's;
missing, zero or malformed sequence evidence is unknown, with no wall-clock fallback. UTC clock
rollback can make a recycled parent's creation date appear earlier, so FILETIME ordering is
insufficient even with repeated reads. Sequences and handle revalidation provide a bounded
observation, without claiming an atomic process tree.

The sequence query uses dynamically resolved `NtQueryInformationProcess` class92, an existing
native implementation surface documented by [PHNT](https://github.com/winsiderss/phnt/blob/master/ntpsapi.h).
Microsoft documents the process sequence in its [telemetry header](https://learn.microsoft.com/en-us/windows/win32/devnotes/process_telemetry_id_information_type);
the strict ordering premise relies on the native implementation, not a universal public API
guarantee. A named native witness checks the exact8-byte result against telemetry, parent/child
order and retained-handle stability after exit. Unsupported queries conservatively yield unknown.

Executable and lifetime inspection use process handles. Directory and structural argument/script
inspection are separate, bounded, best-effort reads of mutable process parameters, initially for
native x64 only. Unsupported layouts are explicit. They grant no authority and expose no command
line or prompt payload. #165 must supply exact connected-peer evidence before publication labels
can be enabled. These are design decisions, not a claim of implemented or Windows-tested support.

The Windows channel receives that scope as a concrete capability, not a caller callback. The
Windows-only `panel-control` dependency on `term-core` reuses the same bounded native ancestry
reader; the channel brackets classification with its pinned peer and scope liveness. Reads and
wakeups remain concrete snapshots/native messages. A generic callback on a joined worker could
block shutdown; duplicating the ancestry reader would let its identity rules drift. Unix keeps its
existing inspector callback. No process-parameter read or UI lock enters classification.

**Discarded.** Fabricated Unix group/session fields misrepresent evidence; ancestry-as-foreground
changes the approved authority boundary; treating a missing parent as outside invents certainty;
UTC timestamps are not a monotonic exclusion proof; whole-tree scans and elevated helpers are
unnecessary for one diagnostic chain; raw command-line getters expose data the consumer did not
request. The next implementation must prove native supported-runtime argument parsing rather
than assume one parser represents every Windows program.


---

## D120 — A screenshot on the clipboard is pasted into a console's own folder and its own image

**22 November 2026 — #199.** The system's own screenshot tools already copy to the clipboard —
Print Screen on GNOME, Cmd+Ctrl+Shift+4 on macOS, Win+Shift+S on Windows — so that is where Ivy
takes a screenshot from, into the image a console holds since #198 (D72's amendment):

- **«Paste screenshot» in every console's header** (Phosphor's image glyph, between the review and
  the panel; a narrow header gives up the splits first, then this) reads the clipboard's image for
  that console, focuses it and shows the Image tab, as a Ctrl+click on an image path does. Found,
  it is written as `pasted-<n>.png` into that console's `PANEL_DIR` — the first `n` nothing there
  takes, a dangling link included, created with `create_new` on a worker, mode 0600 on Unix — and
  opened through the same entry as a clicked image path (D72, D113) in that console's slot. Not
  found, that console's empty Image tab says so after how to open an image, which now names the
  paste too: «… or copy a screenshot and press Paste screenshot or Ctrl+V here.» (Cmd+V on macOS).
  Every refusal is said there, in that console's own slot, in place of what the tab said — after
  it only over unsent marks, whose words, a failed Send among them, stay — so presses do not pile
  answers up: no folder, an image too large, a read that failed or ran out of time (a decoder that
  panics on another program's image included, which fails that read and not the worker), a
  clipboard busy with another read or unavailable, each with its cause.
- **The paste chord in the Image tab** does the same for the focused console, the one the tab
  shows — **only while the drawer holds the keys and no text field has the keyboard**, so a note
  being typed keeps its text paste, and not while a Send is on its way. A console that holds the
  keys leaves its events in egui's input, so the drawer cannot rely on being painted after it. The
  vendored egui-winit lets the chord through as a key when the clipboard holds no text — Ctrl+V or
  Ctrl+Shift+V (Cmd on macOS), the Paste key, and Shift+Insert on Windows, where Win+Shift+S leaves
  only an image — and turns every paste chord into `Event::Paste` when it does. Both are taken once
  per hold: egui-winit turns a held chord's repeats into more text pastes with no repeat flag, but
  lets the key's release through, and that release ends the hold — as does the window losing the
  keys, or a frame the tab did not look in.
- **Every answer is shown in its own console, and takes the keys**, however late it comes (decided
  with the owner on #199's first PR): that console is focused and the Image tab shown. Late means a
  stuck clipboard owner, at most the 10 s deadline. A paste asked over unsent marks is refused before
  the clipboard is read; a screenshot written while marks were drawn is kept in the folder, and the
  tab says where. A console that closed before its answer took its slot with it: standard error
  only, never another console's tab.
- **«A paste is already on its way» is painted, never written**: ahead of the word in the tab of
  the console it was asked again for, while that paste is on its way — or, asked for another
  console than the one being read, that its paste was not read and to press again once the first
  has opened, since one read runs at a time. The slot's notice is written
  by Send, the marks' refusal and every answer, so a status written into it and restored later went
  stale with each of them — lost, brought back after it stopped being true, or piled up — in three
  review rounds of #199's first PR.
- **The file is the console's**: it joins the annotated copies the console removes when it closes
  (D113's cleanup, the same field). A console that closes while its screenshot is written takes the
  file when the write ends, and its folder if nothing else is in it.
- **The same limits as an opened image** — a decode over 256 MiB, a PNG over 32 MiB — refuse it
  **before** a file is written, with `The clipboard image is too large to open (limit: … MiB).`
  They keep the file off the disk, not the memory: arboard has decoded the clipboard's image whole
  by the time it is measured, and offers no size to look at first.
- **An image read has a deadline of its own, 10 s**: arboard decodes the clipboard's PNG inside the
  read, and a debug build decoded a 2400×1600 PNG in 1.5–4 s under load (#178), so the 2 s text
  deadline would expire on an ordinary capture. The read stays on the clipboard's one worker, one
  read at a time, keyed by console, so a late reply cannot land as another console's paste. A worker
  that stops answers the read it held, unless that read was already said expired.

**Amends D113**: the copies field also holds pasted screenshots.

Discarded: watching the system's screenshots folder, whose path changes with the system and its
language and which means watching a folder of the person's all the time; «Open with Ivy» and an
`ivy open <file>` command, which need the running app to take orders from outside; a capture
button inside Ivy, which needs each platform's capture API — the three by the owner's choice in
#199. Opening the clipboard's pixels in memory with no file, because the ticket wants the file in
the console's folder. **A picker beside Send to send to another console**, which #199 asked for in
case the wrong header was pressed and its first PR built: with one image per console (#198) Send
goes to the console that holds the image, and the image stays on the clipboard, so pressing the
right console's button again is the remedy (the owner's choice when #198 landed). A second
clipboard thread for images, against one outstanding read keyed by console.

**Verification.** The clipboard formats on macOS and Windows are compiled but only checked by hand
there.

---

## D121 — An agent opens an image in its own console's slot, and the person is not moved to it

**22 November 2026 — #198.** `image_open(path)` on a console's control channel fills **that**
console's image slot (D72), the one whose endpoint the request arrived on. A name in the message
reaches nothing (D44). **The endpoint authenticates the user, not the console**: any process of the
same user that can read a console's `PANEL_DIR` can be that console (D44, D7), so «scoped by the
endpoint» is a same-user premise about which slot, never a statement about who asked.

**What moves on screen.** If the console is the focused one of the tab on show, and nothing else owns
the screen or the keys, the drawer opens on its Image tab (switching from Document or Changes) and
**the keyboard stays where it was**, as `split_open` leaves it. «Nothing else» means: the Board, the
Settings editor, a modal pane view, the title bar's menu, the connection guide, a maximized drawer,
or the drawer or Files holding the keys. A window too short for the drawer is asked before anything
is switched, so there too nothing moves. Where the drawer already shows the Image tab, which is
that console's, and the console is still in front — the focused one of the tab on show, under no
modal surface — the new image takes the place of the last and nothing has to move, the drawer's
keys, Files' keys and a maximized drawer included. Anywhere else nothing moves. The console's tab
card and its header then show an image glyph until the drawer paints that image or its refusal.
«Loading…» is not seeing it. The reply says «shown» where the drawer shows the image, whether it
opened on it or its Image tab was already on show, and «marked» everywhere else; both end «if the
drawer cannot show it, it says why there» (owner, 22 November 2026), because the reply comes before
the worker decodes the file and a truncated or corrupt PNG or JPEG passes every check made before
it — answering «shown» alone would tell the agent the person sees a picture the drawer cannot
paint. Reading the image's header in the bridge to refuse those too was discarded: it would be a
second decoder to keep in step with the window's, for a reply that already says where to look.
A window that runs
no pass answers nothing within the listener's five seconds and still applies the open on its next
pass, as `split_open` does.

**What is refused, and where.** The path must be absolute and local. On Windows only a drive
letter (`C:\…`, `\\?\C:\…`) or a volume GUID (`\\?\Volume{…}\…`) is local; every other prefix is
refused on the wire — a share, a device, and any other name in the NT object namespace
(`\\host\share\a.png`, `\\?\UNC\…`, `\\?\GLOBALROOT\Device\Mup\…`, `\\.\…`) — because the image
worker would otherwise authenticate to a remote host over SMB on an agent's word. It is an
allowlist because a refusal list let `\\?\GLOBALROOT\…` and a lowercase `\\?\unc\…` through. A
share (`\\host\…`, `\\?\UNC\…`, `\\.\UNC\…`) is answered «is not on this machine»; any other
refused prefix is answered «is not a drive path; give it as C:\…», because its text cannot say
whether it stays here — `\\.\C:\…` and `\\?\GLOBALROOT\Device\HarddiskVolume3\…` do,
`\\?\GLOBALROOT\Device\Mup\…` does not — and the agent can act on the form where it could not act
on a false «elsewhere». A mapped drive letter passes: it is the user's own mapping, not a host the
agent chose. **On Windows an agent's image never passes through a link** (owner, 22 November
2026, review round 3): a symbolic link under `C:\` — the file, or any folder on its way — can point at
`\\host\share`, and a cloned repository or an unpacked archive is where one comes from. So each
entry of the path is asked, from the root down and without following it (`symlink_metadata`),
whether it is a link, and the first one that is refuses the path with `<path> passes through a
link, and an agent's image is not opened through one`. A link is a name-surrogate reparse point,
which is the rule std's `is_symlink` applies: a symbolic link, a junction, or any other tag that
names a path. A reparse point that names nothing — a cloud placeholder, deduplication,
compression — passes, as D108 lets it pass: it holds the file's own data behind its filter, and
refusing it could refuse images a sync client such as OneDrive keeps as placeholders.

**One spelling for checking and opening** (owner, 23 November 2026: «Una sola ruta»).
On Windows the agent's path is made absolute by text with `std::path::absolute`
(`GetFullPathNameW`, without disk access), checked again for a drive or volume-GUID prefix, and
rebuilt under `\\?\`. Both the root-down walk and the read use that returned spelling: Win32
cannot rewrite the last segment of each partial path differently from the whole path. This
replaces the name denylist from review rounds 4–5. A `..` is resolved before the walk; a trailing
dot or space and a device name are interpreted once by Windows, and the resulting spelling is
what is checked and read. If the resulting prefix names a device or another machine it is
refused before disk access with `<path> is read by Windows as <whole>, which is not a file on a
drive` or `which is not on this machine`; an error making it absolute says `<path> cannot be read
as a Windows path: <error>`. Paths already under `\\?\` keep their literal names, with dot
components resolved while rebuilding that one spelling. The original path remains the tool's
address and the text in diagnostics; the window repeats the same spelling and walk before its
own read.

On Linux and macOS the path stays as given: `..`, `aux.png`, `con.png`, dots and spaces in a
file name are no longer rejected because Windows would interpret them differently. An entry
that is not there stops the walk and leaves the path to the open, which must pass through the
same entry and fails; **any other error refuses** (`cannot tell whether <path> passes through a
link: <error>`). The drawer follows the window's refusal over a link, or over an entry it could
not ask, with «Open it from Files to see it yourself.»: the person's own open is theirs to make.
A refusal while spelling the path is painted alone: it already tells the agent what Windows made
of the path. The one-spelling rule was checked against Rust's path and Windows namespace
implementation, but its Windows runtime witnesses still need a real Windows run.

Where a link leads is never read. The bridge walks before
its `stat`, and the image worker walks before it opens, **only for an agent's open**: the open
carries who named the path to the worker, and a person's own click or Files open is not walked. A
link swapped in between the walk and the open is not caught, the race D108 also leaves. Elsewhere
no link is refused: a link leads to what the user made reachable, as a mapped drive letter does on
Windows. That is accepted, not a claim that no path there names a host — macOS's `/net` and an
autofs `-hosts` map do.

The MCP bridge also `stat`s the file in its own process, which is not always the agent's, so a
missing file, a folder, something that is not a regular file or a file over 32 MiB is refused at
once. It then reads the file's first bytes and refuses anything that begins as
neither a PNG (`89 50 4E 47 0D 0A 1A 0A`) nor a JPEG (`FF D8 FF`), a file too short to say
included, with `<path> is not a PNG or JPEG image`, before the pane is reached (owner, 22 November
2026): told «opened» for a file that is not an image at all, the agent would wait for marks that
never come. **Those bridge checks are advisory**: the bridge and the window may see different
files (another mount namespace, a file replaced in between), and a hand-written line skips the
bridge. The window's interface pass never touches the file either: the image worker's thread reads
it, its checks stay the enforcement, and their refusal is painted in the drawer. The format is
decided by the file's bytes, never by its extension; a hand-written line that skips the bridge
still reaches the window, which paints «This file is not a PNG or JPEG image». Unsent marks on that
console's image refuse the call with a sentence the agent can act on; another console's marks never
do.

**What leaves the file.** The window reads the file with its own rights, which a sandboxed caller
that reaches the socket may not have. Nothing of the file goes back over the channel. Its pixels
leave only through the person's Send, a copy written into the console's own folder, so the person's
Send is the only gate for the file's content.

**Subagents.** The tool's description tells an agent working for another agent in the same console
not to call it (D88). Nothing on the wire tells them apart. A subagent that ignores it is bounded to
its own console's slot and refused over unsent marks.

**No measurement batch opens an image**, so no batch paints the glyph or the Image tab and no
published figure changes. The glyph's read sits inside the tab bar's and the header's declared spans
(D13).

| Discarded | Why |
|---|---|
| An `image` field in the panel document | A foreign `panel.json` (a cloned repository, a container) would make the app read local files, and the agent gets no error. |
| Resolving a relative path against the console's directory | Unknown on Windows without OSC 7, and the synchronous file checks would need a `stat` in the interface pass, where a dead mount hangs the window. |
| Resolving a relative path against the bridge's directory | The routed bridge runs in a daemon shared by every pane, so its directory is not the conversation's. |
| Replacing over unsent marks, or queueing behind them | The marks are the person's unfinished work; a queue adds a mechanism and leaves the agent believing the image is shown. |
| An extension gate on the wire | The bytes decide the format, and extension-less PNGs exist. |
| Replacing the Board, or switching a maximized drawer | Both take away what the person is reading. |
| Opening the drawer while the drawer or Files holds the keys | The person is typing there, and switching the drawer's tab or blurring Files would take the keys from them. |
| Following each link and checking where it leads (review round 2) | Review round 3 found three ways around it: an error looking at an entry ended the walk as a pass, a raw NT target such as `\GLOBAL??\UNC\…` came back rootless and was read as local, and a relative target was joined to the folder the link was spelled in rather than the one it really sits in. A screenshot folder reached through a junction to another local drive is refused now; the owner took that cost over a walk that has to model Windows' resolution. |
| Canonicalizing the path and testing the result | `canonicalize` opens the path, following each link, so it reaches the host it was meant to detect. |
| Refusing every reparse point, `FILE_ATTRIBUTE_REPARSE_POINT` | A cloud placeholder carries it, so every agent's image a sync client such as OneDrive keeps would be refused. Which reparse tags a real OneDrive folder carries has not been looked at on Windows. |
| A denylist of path shapes (review rounds 4–5) | It refused ordinary Linux/macOS names and still missed Windows rewrites (`CONIN$` was already missing). Three review rounds showed that checking partial paths and opening the whole path in different spellings cannot be repaired by enumerating names. One normalized verbatim spelling makes the two operations agree. |
| Asking each folder with a trailing separator | Avoided trailing-dot and space rewrites, but did not establish the same rule for device names. |
| Walking a person's own open too | The person chose the path; the rule is about an agent's word. |


## D122 — Windows Ctrl+Alt system characters take precedence over shortcuts

**Decided for #206.** On Windows, a pressed Ctrl+Alt gesture with nonempty printable
`text_with_all_modifiers()` becomes exactly one text event before clipboard and key shortcuts.
Missing text, control characters and releases retain the existing key path. Real AltGr retains
its existing path, as do Linux and macOS. The modifier supplement is the character Windows
actually emitted, including composition; `KeyEvent::text` can reconstruct a plain letter after
removing Ctrl and is not evidence that a shortcut typed a character.

**Why.** Left Ctrl+left Alt+2 on a Spanish layout retains Ctrl in winit, so the previous adapter
suppressed its `@`. Simply allowing Ctrl+Alt whenever ordinary text is present would type `a`
for Ctrl+Alt+A on a US layout. Sending a key event as well could invoke an application or
clipboard shortcut before the character is inserted.

**Discarded.** A table of keyboard layouts and key positions would duplicate Windows' input
translation. Comparing against `key_without_modifiers()` confuses Shift, Caps Lock and dead-key
composition with alternate-character input. Accepting ordinary nonempty `KeyEvent::text` admits
the US shortcut fallback. Altering only the terminal's text filter cannot recover text discarded
by the adapter and leaves other text widgets inconsistent.

**Evidence boundary.** Portable regressions exercise the actual dispatcher and terminal byte
translation. A disposable Windows witness supplies real input through winit and the production
adapter for Spanish, German, Polish and US layouts; it records actual character messages and emitted
bytes. It is not a hand check of the complete installed terminal and does not replace D53's
Windows rows. Neither native execution nor those manual rows are claimed passed by this change
before the corresponding run is observed. The existing vendor residual fingerprint is retained;
only the new module and narrowly pinned Windows call are excluded and checked separately.

## D123 — Windows GUI launch and background processes do not create a console

**Decided for #171.** The installed Windows GUI uses the graphical subsystem, embeds its icon and
version resources, and captures startup diagnostics without depending on an inherited console.
Background capability and Git commands are started without creating console windows.

**Why.** Starting a graphical application or its background helpers from Explorer must not flash an
extra console. The executable itself must identify Ivy to Windows, while failures still need a
persistent diagnostic destination when no standard error exists.

**Discarded.** Hiding a console after startup permits a visible flash; launching Ivy with a detached
or no-window flag in its test would hide a subsystem regression. Inspecting only the PE subsystem
cannot establish that the installed application starts and responds, and counting windows by process
ID cannot establish whether a process owns a console.

**Evidence boundary.** The disposable Windows installer trial starts installed Ivy with ordinary
creation flags from a detached host, checks console attachment, responsiveness and clean closure,
and compares the installed icon resources and version strings. A console child is the positive
control for the inspector. Named native tests and individual flag-removal mutations exercise both
background launch paths. The existing interactive launch trial remains independent. These witnesses
cover the inspected process and named paths, not every possible visible process; native results are
not claimed until the Windows CI run completes.


## D124 — PowerShell reports its directory at the prompt and calls Codex through a native alias

**Decision.** Recognise shell executable stems, including `.exe`. Start PowerShell with `-NoExit -EncodedCommand` containing UTF-16LE base64 startup text. Leave profile loading and execution policy unchanged. Wrap the profile-defined prompt, calling it first so its error-status inputs survive, then emit OSC 7 before returning its text. FileSystem paths use percent-encoded segments; other providers clear the report, and UNC reports retain their remote authority. The directory report works without the Codex helper.

When available, install a global PowerShell alias to the absolute sibling `ivy-codex-shell.exe`, a byte-identical installed copy of `ivy-codex.exe`. The executable name selects shell discovery: resolve native applications from the PATH at call time, searching absolute directories for `.exe` before `.cmd` and `.bat`, and pass all original arguments through the existing launch path. The explicit `ivy-codex.exe <client> [args]` contract stays unchanged. Preserve user functions and aliases with a diagnostic. Git Bash resolves `.exe` or `.cmd` instead of the extensionless npm shim. Remove inherited `IVY_CODEX_HELPER` when no helper is available.

**Why.** Directory consumers already speak OSC 7; the shell knows its location without interpreting its screen or reading another process. The desktop PATH can differ from the configured shell PATH. Native invocation preserves PowerShell success/failure, exit code and conditional chaining. Absolute PATH entries exclude a launcher planted only in the current repository.

**Revised after runtime evidence.** The first implementation used a shell function. In PowerShell7.7 a client exiting37 produced `LASTEXITCODE=37` but caller `$?=True`, allowing a following `&&` command to run. The user chose to redesign rather than accept that limitation. A distinct installed name separates native shell discovery from explicit helper invocation without an inherited mode variable or a per-console executable copy.

**Discarded.** An advanced function consumes native options such as `-V` as common parameters. Throwing on a nonzero exit changes script control flow. A per-console executable copy adds synchronous I/O and deletion while Windows may still hold the file. A PATH shim changes every child and still loses to aliases. A `.ps1` startup file is blocked by Windows PowerShell Restricted policy; ExecutionPolicy Bypass would change the profile policy too. A native PEB directory reader belongs to native process discovery. cmd integration and shared-conversation relay remain separate work.

**Limits.** Only a returned prompt reports a changed location; a framework redefining prompt afterward can replace the wrapper. Native arguments follow the PowerShell version itself (5.1 may drop empty native arguments); batch targets retain Rust command-line escaping restrictions and fail visibly where an argument is unsafe. Relative-only PATH installations are not resolved. A missing native entry leaves Codex unwrapped with a warning; removing an aliased executable later fails natively. Windows desktop badge, split, Review and link behavior requires native verification. No frame-performance claim is made.

## D125 — A shell that cannot start leaves a visible failed console

**Decided for #202.** After the existing startup and restoration attempts have failed to open a
shell, Ivy keeps its window open with one failed console. The application explains which program
was attempted and why it could not start. The notice belongs to the application, above the console,
and is never written as terminal output or producer-owned panel content. D40's retained failure and
D41's click dismissal apply: dismissing the explanation neither recovers the program nor makes the
window close automatically. Existing application close controls remain the way to close it.

The fixed failure headline must not disappear because a path or operating-system error is long.
Program and reason previews fit within the notice's upper-half allowance, with explicit Unicode-safe
elision retaining identifying context and the native error's cause. Complete diagnostics stay in
D114's existing log, accessible from Settings. The notice points there for full details.

**Why.** A person starting Ivy from the Windows Start menu cannot see standard error. Closing the
window over a failed first shell gives them no actionable explanation. Keeping a failed console
also lets the existing controls remain available without pretending that a program is running.

**Boundary.** Recovery belongs after the existing startup attempts. Strict launch errors still
belong to callers that retry a saved directory, fall back from a saved client, or retain the current
console when a replacement cannot start. This decision changes neither shell selection nor those
retry policies, and unrelated initialization errors are not reported as failed shell launches.

| Discarded | Why |
|---|---|
| Show a native message and exit | The owner chose to keep Ivy open with the explanation on its console. |
| Record only the log | A log preserves evidence but does not explain the disappearing window to the person opening it. |
| Wrap the full diagnostic in the existing all-or-nothing notice | Long executable paths can make the whole notice exceed its ceiling and hide even the failure headline. |
| Convert every launch error into a successful failed pane | Existing callers need the error to choose their directory and client fallbacks. |
| Start a helper program to print the error | No working shell is available, and the application already has a failed-console lifecycle. |

## D126 — Windows OpenCode metadata uses the bundled authenticated helper

**Decided for the activity/attention slice of #174.** The explicitly installed OpenCode TUI plugin
uses a persistent `ivy-session signals` child on Windows. It starts only Ivy's inherited absolute
local `.exe` helper path, with fixed arguments, no shell and no PATH fallback. The helper reuses
the existing private-directory and expected-server-PID checks on the connection that carries the
signals. The optional Windows peer upgrade retains its existing unverified fallback and numeric
server-PID limitation; activity metadata does not gain foreground, hook or recovery authority.
Linux and macOS keep their direct Unix socket transport.

**Why.** Node's supported named-pipe connection API alone does not supply the native directory
owner/DACL and server-PID checks required by D115. An independent native probe followed by a
different JavaScript connection would reopen the replacement race. The bundled helper already
owns the required transport boundary and its absolute location is supplied at console launch.

The child accepts only bounded OpenCode signal envelopes or explicit null, reconstructing a typed
signal request instead of proxying arbitrary channel tools. An authenticated-ready frame precedes
the first snapshot. Signal-specific reply decoding preserves `retryable`; the legacy producer
result deliberately ignores that extension and keeps its established behavior. A transient refusal
retries on the same connection with fresh state; a permanent refusal stops. Explicit withdrawal
pending during a refusal is retained during disposal instead of asking a stopped producer to refresh.
Disconnect still leaves a pending wait stale rather than declaring it answered (D80).

One absolute six-second deadline covers writing and reading a whole native exchange. Applying a
duration separately to every byte would let a trickled reply keep it alive indefinitely. Normal
plugin disposal shares one 4.75-second deadline across its existing drain, graceful exit and forced
child cleanup, inside OpenCode's five-second cleanup grace. The plugin never replaces a still-live
helper. The additional process belongs to this plugin's connection, not to a background service.

| Discarded | Why |
|---|---|
| Connect directly from Node by the Windows pipe name | Omits the established native owner, ACL and server-PID checks. |
| Authenticate on a separate native probe, then reconnect from Node | Authenticates a different connection from the one carrying metadata. |
| Add a new terminal escape protocol or modify OpenCode | No equivalent verified OpenCode native declaration contract exists in this integration. |
| Move Unix clients to the child too | Adds a process where the direct transport already has the required checks and live-client evidence. |
| Offer an arbitrary stdio channel proxy | Exposes unrelated commands when this producer needs only activity metadata. |
| Treat Windows process ancestry as agent authority | D119 explicitly reserves that boundary for separate work; the owner deferred recovery. |

**Evidence boundary.** Actual-asset fixtures and the native helper/channel witness are separate from
installed OpenCode desktop interoperability. Neither source review nor cross-compilation proves
Windows runtime behavior. Native CI, mutations and installed-client checks must report their own
results. This delivery leaves #174's conversation-recovery reporters deferred.


## D127 — Claude usage is opt-in local accounting

**Decision.** Ivy's application menu opens a dedicated AI usage screen. A
person selects local Claude roots and explicitly enables collection. Each
root is bound to its opened directory identity. Only normalized accounting
and minimized, store-local keyed evidence enter the private SQLite ledger.
The app retains accepted accounting across source removal, reports partial
coverage and prices only known token categories with bundled versioned USD
list rates. The original history is read-only. Stop revokes collection.
Clear removes the derived ledger, and removed originals can make rebuilding
impossible.

**Why.** Streaming Claude code records can repeat with different timestamps
and switch from a bare prompt count to an explicit cache split. A ledger
that keeps only the winner cannot reproduce conflicts after logs disappear.
A staged epoch publishes a consistent result after bounded collection and
reconciliation. A separate window-level modal preserves the drawer and the
producer's authority over task state (D6).

**Discarded.** The upstream public parser materializes whole files, and its
default pricing path can contact a server. Adapt the proven Claude extraction
rules at the pinned revision with its MIT notice. A ledger copy would double
the needed staging space. A standalone append journal would need separate
indexed replay and date queries. A per-console or Board view would imply a
usage association the local logs cannot prove.
