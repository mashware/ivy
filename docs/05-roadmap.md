# Roadmap

In order. **Every phase is worth having on its own even if the next one is abandoned** — that is the
rule that decides what goes into each of them.

Phases 1 and 2 are broken down into milestones because they are the ones that are going to be built.
From 3 onwards they are headlines on purpose: spelling out what has not been touched yet is making
things up.

---

# Part I — What you have at the end of each phase

Nothing technical here. This is the part to reread whenever it is not clear whether something
belongs in one phase or the next: **if it is not described here, it does not go in**.

## Phase 1 — «This exists»

**What you see:** a window with two consoles side by side. To the right of each one, a narrow strip
showing where that task stands: the phase, the current step, whatever is blocked.

**What you can do:** an agent on the left working on one task, another on the right on a different
one, and know at a glance which is making progress and which is waiting for you without reading what
they write.

**What is missing:** everything else. No tabs, no copy and paste, no scrollback, nothing that can be
moved around. Two fixed consoles.

**Why it is worth having anyway:** it is the only phase that can say no. If with this in front of
you the feeling is «it gives me nothing that two windows would not have given me», the project stops
here having spent little. If it is «I do not want to go back», the rest is work, not a bet.

## Phase 2 — «This is my terminal»

**What you see:** the whole app, the one in the mockup. Tabs along the top, and inside each of them
however many panes you want, arranged however you want.

**What you can do:** one tab per project or per context, and inside it as many tasks in parallel as
will fit. **Drag a pane and drop it somewhere else** when the work changes shape — which no current
terminal offers and which is what started this project. Copy, paste, search, go up through the
scrollback. Fold the panel away, or give it whatever width you want. Close it and find it as you
left it. **And when something claims you in a tab you are not looking at, that tab's card says so**
— if it is waiting for you, if it is stuck on something else, or if it finished and nobody looked.

**And what a terminal takes for granted, because terminals are what this competes against**
(**D19**): that your fonts show up —the icons of an `ls` and Japanese, not empty boxes—, that you
can change its font, its size, its colors and its shortcuts, and that the mouse reaches the program
inside so you can click inside `vim`.

**And that it can be read outside of here**: the terminal speaks **English** — its configuration
keys, its messages and its code.

**What is missing:** the panel is still read-only. It tells you things, you cannot do anything from
it.

**Why this is the real milestone:** this is where WezTerm gets uninstalled. And it is the first
version that can be shown to somebody on Mac or Windows.

## Phase 3 — «The panel answers back»

**What you see:** the same thing, but the panel has buttons. «Continue», «run the tests», «open the
MR» — whatever the task itself declares makes sense at that moment.

**What you can do:** answer the agent without typing. And in the other direction, the agent can ask
the app for things: open itself a pane to run the tests separately, without touching the one it has
in front of it. And when a harness can declare that `/clear` ended the current session through its
existing MCP connection, that pane loses the old panel and the auxiliary consoles the session still
controlled; a console you took over stays. A compacted session is still the same session and keeps
everything.

**What changes underneath:** up to here the app was a screen to look at; from here on it is a place
to work in. And the work stops being only the agents': **any tool —a deployment, a CI, a company
script— can publish its state and its buttons there.**

## Phase 4 — «What needs something from me?»

**What you see:** a new screen above the tabs. Not consoles: a list. At the top whatever is waiting
on a decision from you; below it whatever is blocked; at the end whatever is moving along by itself.

**What you can do:** start the day there. With eight tasks spread across four tabs the problem stops
being doing them and becomes knowing which one to go to.

> **Corrected in August 2026.** This used to say «no tool in the landscape has this: they all show
> the tasks, none of them order them by what claims you», and it **stopped being true**: Herdr's
> sidebar accumulates state from agent to panel, to tab and to workspace, and orders by who needs
> attention ([07-landscape.md](07-landscape.md) → E8). What still nobody has is not the **order**,
> it is the **body**: why it is stuck, which step it is on and how much is left. This phase's list
> is ordered with `attention` (D18) and read from whatever the producer publishes.

**Why it is parked:** it only works if the state that arrives is trustworthy, and that is known
after using Phase 3 for a while. Before that it is laying out a pretty list with data you do not
trust.

## Phase 5 — «Somebody watches for you»

**What you see:** Phase 4's list, but ordered and annotated by itself.

**What you can do:** delegate the watching. An agent that does not program — it only observes the
others and reports: «this one has been stopped for twenty minutes», «this one has finished and
nobody has reviewed it», «this one is running low on context».

**Why it is far off and it does not matter:** it needs no new piece, it is Phase 3 used a different
way. If it is never done, nothing extra will have been built to get there. That is on purpose.

## Where the cut is

| | |
|---|---|
| **Phase 1** | To find out whether it is worth it |
| **Phase 2** | For daily use — and the first thing that can be shown |
| **Phase 3** | It stops being a screen and becomes a place to work |
| **Phase 4** | What nobody else has |
| **Phase 5** | A gift, if it arrives |

---

# Part II — How it gets built

---

## Phase 0 — Done

- [x] Status panel working on top of WezTerm (`prototypes/flow-panel`)
- [x] `panel.json` contract defined and with a real producer (`flow`)
- [x] Two views: global (every task) and focus (one task, pinned)
- [x] Visual mockup of the app to judge proportions (`prototypes/mockup-terminal.html`)
- [x] Stack and architecture decisions ([02-decisions.md](02-decisions.md))

---

## Starting point

Checked on the development machine (2026-08-07):

| | |
|---|---|
| `rustc` / `cargo` | **1.97.1** — upgraded when F1.3 started (see below) |
| `git` | 2.53.0 |
| `inotify-tools` | not installed — and no longer needed (see F1.6) |

> **Corrected on 2026-08-07, when F1.3 started.** This table used to say «1.94.0 — ready, there is
> nothing to install», and it was incompatible with the `eframe` version pinned below: `eframe
> 0.36.1` demands `rustc 1.95`. It was checked with `cargo check` over a throwaway crate, and there
> was no way to write a single line of the `app` crate without resolving it. The toolchain was
> raised to 1.97.1 —what `rustup` was offering— instead of dropping `eframe` to 0.35: the framework
> version is the project's choice and the toolchain is the cheap thing to move. After the upgrade,
> `clippy` and the 8 tests of the workspace are still clean.

Crate versions resolved today against crates.io:

```toml
alacritty_terminal = "0.26"   # VT parser + terminal state
portable-pty       = "0.9"    # Unix PTY + Windows ConPTY
eframe             = "0.36"   # egui + native window
notify             = "8.2"    # file changes, cross-platform
serde / serde_json = "1"      # the contract
```

> **Amended on 2026-09-08: one of them is no longer resolved against crates.io.** `egui-winit`, which
> `eframe` pulls in, is served from a **copy in the tree** (`vendor/egui-winit`) with one `return`
> made conditional; everything else is the published 0.36.1 byte for byte. It is the first patched
> dependency in this repository, and it exists because the published version drops the `Ctrl+V`
> keypress whenever the clipboard holds no text — with an image on it, or empty — so the key reaches
> the program inside a console as nothing at all. Reported upstream as
> [emilk/egui#4065](https://github.com/emilk/egui/issues/4065), open since 2024, with the pull
> request that would settle it unmerged. **D33** carries the reason, what was discarded, and what has
> to happen for the directory to disappear. The sentence above — *the framework version is the
> project's choice* — still holds: what changed is that the choice now includes a version of our own
> when no published one will do.

### The prototype's UI is done with egui, not with GPUI

D3 left GPUI for the product and egui as an *option* for prototyping. For Phase 1 **egui** is
chosen, and it is worth writing down why, because it is a decision with an expiry date:

- What Phase 1 has to answer is **whether the whole thing holds up in your hands** — a real terminal
  with a panel beside it. That does not depend on the framework.
- egui is published on crates.io with versions and documentation. GPUI lives inside Zed's
  repository, with an unstable API and no documentation beyond its own code.
- Everything expensive in Phase 1 —the PTY, the grid, the contract— lives **outside** the painting
  layer (see the structure below). Changing framework afterwards means rewriting the `app` crate,
  not the project.

**When to jump to GPUI:** when egui stops giving the necessary performance or control, and not
before. The two concrete symptoms to watch for are in F1.3's risks.

> **Measured in F1.3, Aug 2026: neither of the two shows up.** The full repaint scales linearly —two
> consoles cost 1.96× one and the frame spends 36 % of its slot— and typing costs 36 µs. The jump is
> not due. What is left to check is the load these numbers do **not** cover: four panes and the
> panels painting at the same time, which is to say F1.4 and F1.6. The numbers, with what they prove
> and what they do not, are in the «Measured — 9 August 2026» annex of [D3](02-decisions.md).

### Structure

```
agent-terminal/
├── Cargo.toml              workspace
├── crates/
│   ├── panel-contract/     panel.json types, reading, validation   ← depends on nothing visual
│   ├── term-core/          one pane = PTY + grid                   ← depends on nothing visual
│   ├── panel-control/      the control channel and its MCP binary  ← depends on nothing visual
│   ├── host-load/          the load the measurement batches run    ← not shipped, only measured
│   └── app/                window, panes, painting                 ← the only piece tied to egui
├── vendor/egui-winit/      the patched copy every build uses (D33)
├── scripts/                measure-frame.sh, which runs the measurement batches
├── docs/
└── prototypes/
```

The first two boxes are the ones to protect. If `app` gets thrown away and rewritten in GPUI,
`panel-contract` and `term-core` keep working exactly as they are.

---

## Phase 1 — Minimal prototype

**Goal:** find out whether the whole thing holds up in your hands. No tabs, no buttons.

**Success criterion:** using it for **half an hour of real work** without missing WezTerm for
anything other than tabs.

Size: **S** = one sitting · **M** = several · **L** = this is going to hurt.

| Milestone | What gets built | How it is checked | Size |
|---|---|---|---|
| **F1.0** ✅ | `git init` and the workspace with the three empty boxes | a clean `cargo build` | S |
| **F1.1** ✅ | `term-core`: `portable-pty` launches the shell, reader thread, writing to the PTY | A test binary launches `zsh -l`, writes `ls\n`, prints the raw output | S |
| **F1.2** ✅ | `term-core`: the bytes go into `alacritty_terminal::Term`; dumping the grid to text | `vim` and `htop` produce a coherent grid; on resizing, it reflows | M |
| **F1.3** ✅ | `app`: painting the grid in egui — monospaced font, ANSI colors, cursor. Keyboard → PTY | `ls`, `vim` and **`claude`** can be used inside the window | L |
| **F1.4** ✅ | Two panes side by side, focus by click, draggable divider | Two live shells at once without treading on each other | M |
| **F1.5** ✅ | `PANEL_ID` / `PANEL_DIR` injected when each shell is launched; the directory is created and cleaned up on closing | `echo $PANEL_ID` gives different values in each pane | S |
| **F1.6** ✅ | `panel-contract` + `notify`: a panel to the right of each pane, repainted when the file changes, and a warning when what is shown is stale or could not be read | `flow` writes in one pane and only that one repaints | M |

### What Phase 1 leaves out on purpose

Long scrollback, mouse selection, copying, search, font ligatures, Kitty's graphics protocol, tabs,
buttons, MCP. All of that is Phase 2 or later. Putting it in here turns a one-week prototype into a
three-month one and does not answer the question any better.

### Concrete risks

| Risk | Where it shows up | What to do |
|---|---|---|
| **Glyph width** — CJK, emoji and combining characters knock the grid out of place | F1.3 | **There is nothing to port** (corrected, Aug 2026): the parser already resolves the width and marks the companion cell, and `term-core`'s reader exposes it as `CellWidth`. What is left for whoever paints is to **respect** the two columns, not to compute them |
| **Full repaint** — egui is immediate mode: it repaints everything every frame | F1.3 | **ANSWERED (Aug 2026): the symptom does not show up.** Two consoles cost 1.96× one —linear scaling— and the frame spends 36 % of its slot with the most naive painter possible. It only repaints when the PTY produces changes, as was foreseen. **Measured again in F1.4 with two real panes (Aug 2026), and the suspect changes:** with both consoles dumping at once the frame costs 5.92 ms and fits in the 120 Hz slot, but **only 0.74 ms of that is attributable to the framework** — 3.35 are spent by the parser swallowing its budget and the rest by an unoptimized painter of our own. The keyboard keeps responding while it lasts: 3.42 ms from the key to the byte, against the 36 µs with no dump. **It does not compare with the bench's 36 %**: that one measured full screens and continuous repainting, and this one a dump over nearly twice as many cells. **The worst case with two panes is still unmeasured**, and the p95 of the double dump already falls outside 120 Hz. Numbers and limits in the «Measured — 11 August 2026» annex of [D3](02-decisions.md) |
| **Key latency** — a terminal that feels slow to type in is useless | F1.3 | **ANSWERED (Aug 2026): 36 µs** from the key in the frame to the byte in the program. **There is no «key→pixel» number**: egui's events carry no stamp of their own and the clock of its input is «relative to whatever», so that span is not measurable from inside the process — publishing it under that name would have been making it up. What is measured, and broken down, is in [D3](02-decisions.md) |
| **Dead keys and IME** | F1.3 | Accept that it comes out mediocre in the prototype; it is Phase 2 |
| **Performance of the massive dump** (`cat` of a big file) | F1.2 | Cap how many bytes get processed per frame |
| **Windows** — ConPTY behaves differently | outside Phase 1 | Not touched until Phase 2. `portable-pty` abstracts it, but abstracting is not testing |

---

## Phase 2 — A real host

What turns the prototype into something used daily.

| Milestone | What |
|---|---|
| **F2.1** ✅ | Tabs — **closed on 16 August 2026**. Eight deliveries; the frame measured with one and two tabs in [D13](02-decisions.md)'s annex |
| **F2.2** ✅ | Arbitrary panes — **closed on 17 August 2026**. Seven deliveries; the n-ary tree with weights and the three chords in [D14](02-decisions.md), and the frame measured with two and with four panes in its annex |
| **F2.3** ✅ | **Dragging panes with the mouse** to rearrange them — **closed on 18 August 2026**. Three deliveries; the gesture, its atomicity and the identity of the drag in [D15](02-decisions.md), and the frame measured while dragging non-stop in its annex |
| **F2.4** ✅ | Folding the panel away per pane and **narrowing its width by dragging** — **closed on 19 August 2026**. Three deliveries; the per-pane width, the gesture's minimum and folding as a separate state in [D10](02-decisions.md), and what the gesture costs measured in its annex |
| **F2.5** ✅ | Saving and restoring the layout on opening — **closed on 24 August 2026**. Eight deliveries; the file of its own, its version and what is not understood in [D16](02-decisions.md), and in its annex what it costs to open with the consoles you left: **5.42 ms once only and before the first frame**, with batch `L` measuring it and `H` as the control |
| **F2.6** ✅ | Scrollback, selection and **copying**, and forwarding the mouse to the program inside — **closed on 25 August 2026**. Four deliveries; the scrollback's ceiling chosen with the measurements in front of us in [D21](02-decisions.md), how the left button is shared out in [D20](02-decisions.md), what «the text» that gets copied is in [D22](02-decisions.md), and who keeps each gesture —with the report's encoding, which the library does not bring— in [D23](02-decisions.md). Pasting had been brought forward out of phase ([#46](https://github.com/mashware/agent-terminal/issues/46)), see D11. In the annexes: the scrollback costs **+39 % more interpreting and +1.2 % of the full round trip**, having text marked **50–60 µs per frame and console** —declared **without isolating it**— and encoding a gesture for the program and writing it to it **5.1–5.3 µs**, that one isolated in a series of its own |
| **F2.7** | First compilation and real test on **macOS and Windows**. The keyboard is checked by hand against the list in [D53](02-decisions.md), because no test here can reach the stretch from the desktop's key to the framework's event; that decision already names the rows expected to differ on each platform. The control channel is exercised on each platform with its public endpoint removed before the channel: dropping it must return within its bound and leave no listener worker alive. **Two things that will read green on the new platforms without having run**, carried here when the debt file that held them was deleted (#150): every test that returns early on `hold_a_paste_on` or on `the_longest_line_a_reader_keeps()` being `None` — `grep -n "hold_a_paste_on\|the_longest_line_a_reader_keeps" crates/app/src` finds them, in more than one form — returns before asserting anything off Linux, where that boundary is `None` — decide between a per-platform boundary, a reported skip, or a panic where none is declared; and on Windows the pipe's write end belongs to the pseudoconsole, so `drop(pair.slave)` closes nothing and **no end of file arrives while the console lives** — the D32 survivor case as the platform's default |
| **F2.8** ✅ | **The panel says when it claims you, and the tab accumulates it** — **closed on 24 August 2026**. Three deliveries; the `attention` field and who declares it in [D18](02-decisions.md), the three measured symbols and the color table of its own in [D9](02-decisions.md)'s annex, and in D18's annex what the bar costs now that its card accumulates: **between 2.35 and 2.69 µs per frame**, measured with two tabs and the four producers claiming. The system notification was left out on purpose, until the field has producers |
| **F2.9** ✅ | **The system fonts** — **closed on 25 August 2026**. Four deliveries; CJK, the icons of an `ls` and **color emoji** stop coming out as boxes. That they be looked up **when they are needed** and not at startup was decided by a measurement ([D24](02-decisions.md): 23.5–31.0 ms at startup against 4.0–5.7 lazily), and color emoji goes its own way because **epaint cannot rasterize color** ([D25](02-decisions.md)). Two findings that reordered work: `Fonts::has_glyph` **lies** —it returns `false` for `'a'`—, and an icon font placed in front took **three letters out of every five** away from the terminal. In the annexes: drawing with emoji on screen costs **356 µs against the control's 269**, after the atlas and the pass of its own brought it down from 743. `COLR` —what Windows uses— and skin tones are left out, with their reason |
| **F2.10** ✅ | **Configuration** — font, size, colors and shortcuts — **closed on 6 September 2026**. It is the first thing anybody does on installing a terminal, and until **D19** it did not appear in this roadmap. Four deliveries: the file with its twenty-two colors, the font size with a veto of its own under the measurement gate, the twelve shortcuts —and `none`, which gives a chord back to the program inside— and the font **by its name**. The file is written by hand and **tolerates being broken**: a line that is not understood is said with its number and the rest is applied. Asking for a family is the one setting that costs anything on opening, so the machine is enumerated **only** when the file names one, which is what keeps **D24** standing; the report declares which of the two happened and the bench invalidates a run that paid it. Its own ceiling bit back: **D28**'s ratchet refused the last delivery's ten lines in `lib.rs`, and reading the settings file left it for `settings.rs` |
| **F2.12** ✅ | **The whole repository in English** — **closed on 5 September 2026**. — the project's own code, the comments, the tests and the documents have been **in Spanish since the first commit**, and that decision **is not written down anywhere**, which is what let it grow. It moves everything to English, configuration keys and messages included, **with no backwards compatibility**: nobody has this terminal installed, so there is no file out there whose anything has to be respected, and it is the only window in which it comes free. Commits, branches and PRs move to English **from now on**, without waiting for the milestone. **It is run first of what is left of the phase** — see below. [#122](https://github.com/mashware/agent-terminal/issues/122), **D27** |
| **F2.11** ✅ | **A header per pane** — Tilix style: the handle to drag it by, its close button, and at a glance which one has the focus — **closed on 18 September 2026**. Five deliveries; it comes out of **D20**, where it was discarded **as part of F2.6** for the price it puts on every published batch, and the milestone paid that price rather than argue it away: the strip in [D45](02-decisions.md), gated off every measured run and switched on for a batch of its own, and the close button — with no confirmation, absent on a pane with no sibling — in [D46](02-decisions.md). **The third of the original three things had already landed, outside the milestone** ([#209](https://github.com/mashware/agent-terminal/issues/209), **D36**): only the console being typed into fills its cursor, and a window that lost the desktop's focus outlines every one of them. In the annexes: two runs of batch `T` against its control `C` on merged `main` put the strip at **between 524 ns and 1.03 µs per frame** — under a thousandth of 1 % of the 60 Hz slot — with `T`'s grid one column narrower than `C`'s, declared rather than subtracted |
| **F2.13** ✅ | **Search in the scrollback** — **closed on 11 October 2026**. One delivery ([#177](https://github.com/mashware/agent-terminal/issues/177)); everything it decided is in [D61](02-decisions.md): `Ctrl+Shift+F` searches **the focused console** for literal text, `Enter` and `Shift+Enter` step through the history, **the match is the selection** — so copying it goes through **D22**'s rules with nothing of its own — and every match on screen is marked while the query is typed. The line it paints sits **over** the console, never cut out of it, so opening a search resizes nothing. A press that finds nothing walks the whole history once (11.8–13.6 ms at its median, accepted and written down); marking is per frame and bounded. Measured with a batch of its own, **`E`**, against `C`: the marking is **60–65 µs a painting** and the left console's paint span shows no difference from the control measurable from two runs. Closing the measurement also closed a declared hole: the keyboard's pass is now timed on every frame somebody types in |

**Where this list comes from, since D19.** The bar to clear before publishing is that of a WezTerm,
Ghostty or Kitty user — **not** that of an Orca user. That is what puts F2.9 and F2.10 into the
phase and what leaves out the built-in browser, the mobile app, the editor and the spread of agents:
the whole list of what is left out, with who has it, is in **D19**.

**F2.12 was done before F2.10, F2.7 and F2.11, and its number is not recycled either** — for the
same reason as F2.8 further down. It went in front because **the more code there is, the more
expensive it gets**: 43 860 lines of our own at the time, and every milestone adds to them. And
because during the migration any open branch conflicts on everything, so it demanded a clean tree —
which is why F2.10 was held at three deliveries out of four, with choosing the font by its name
waiting until the migration was finished. Both are closed now, a day apart.

**F2.13 appeared in September 2026 and its work is not new: what was missing was the milestone.**
It came out of reading Part I against this table — the phase promises «copy, paste, search, go up
through the scrollback» and three of the four had an owner. **Its place in the order is not decided
here**, only that it exists and that it is a milestone and not a corner of somebody else's.

**Opening a link is not a milestone, and it landed outside the table on purpose.** Every terminal
this phase measures itself against opens the addresses a program prints, and until
[#257](https://github.com/mashware/agent-terminal/issues/257) this one showed them in the console
and could do nothing else with them, and the panel did not even show its `line.link`: it had been
read and left unpainted since F1.6. It is the D19 bar, not an idea coming in through the front door: `Ctrl`+click on an
address or an OSC 8 hyperlink in a console, a click on a panel's link label, and only `http` and
`https` reaching the desktop's browser. What it decided — the gestures, the painted snapshot a click
resolves against, and the one boundary every address crosses — is **D51**, and batch `W` is what
declares its work to the bench.

**F2.8 is done before F2.6 and F2.7, and the number is not recycled.** Milestone identifiers live in
commits, decisions and in the knowledge store, so renumbering costs more than accepting that the
table is out of order. The reason for bringing it forward: it is the only thing in Phase 2 that
touches **the leg that differentiates**, and August's landscape left two competitors a single field
away ([07-landscape.md](07-landscape.md) → E7 and E8). Scrollback and portability are terminal, and
the terminal is no longer what has to be defended.

F2.3 is the whole reason the phase exists: it is the gap that led to considering this, and it is
what justifies not having taken an off-the-shelf multiplexer.

F2.7 cannot be delayed any further than here. The later it is tested on the other two platforms, the
more expensive every assumption that turned out to be a Linux one gets.

---

## Phase 3 — Bidirectional

- [x] `actions`: buttons declared in the contract that write to their pane's PTY (**D42**)
- [x] **MCP** server with `panel_set`, `panel_get`, `panel_patch`, `panel_retire`, `split_open`,
      `split_read`, `split_close` — the three `split_` verbs keep that word on purpose (**D44**)
- [ ] Scoped by the pane's endpoint and the live connection: each agent reaches its own pane and only
      the auxiliary consoles that connection opened

`panel_retire` is the intentional session boundary (**D50**), not another spelling of disconnect.
It clears the document, actions, attention and hold together, detaches untouched auxiliary consoles
that connection owns, preserves the ones the person took over, and answers before their
pseudoterminals are destroyed. A crash or accidental disconnect still preserves the last document
and every console; `/compact` does nothing. The observed file snapshot present at retirement is
suppressed in terminal-owned state rather than deleting `panel.json`, because the file cannot say who
produced it.

Automatic `/clear` retirement is only promised where a harness integration proves that its explicit
lifecycle event invokes this tool on the already-connected MCP server. This repository supplies the
operation but no harness adapter; unsupported integrations remain unsupported rather than opening a
fresh connection or inferring lifecycle from content. The operation is also absent on Windows with
the rest of the control channel until F2.7 proves the named pipe on a real host.

**Do not start without rereading the security table in [03-architecture.md](03-architecture.md).** A
file that declares commands and a button that runs them is code execution in the user's shell; the
command has to be seen before it is run.

---

## Phase 4 — Summary view

**Parked on purpose.** It depends on the panes and the contract being well worn in.

The screen that answers *«what requires something from me right now?»*: every task in every tab,
ordered by what is waiting on a decision, what is blocked, and what is moving along by itself. The
seed already exists — it is `flow-panel`'s global view — but the good version needs the activity
indicator and the state of every pane at once.

---

## Phase 5 — Supervisor

**Far off, and it is fine that it is.** An agent that observes the state of every pane and directs
the others.

The important part: **the infrastructure is the same as Phase 3's**. The buttons and the supervisor
use the same channel. If it is never done, nothing has been lost.

> **And now there is an external reason for it to stay that way.** The supervisor **cannot be a call
> of ours to a model**: since **4 April 2026** Claude subscriptions do not cover third-party tools,
> so an agent the app called over the API would be paid for by the user separately — exactly what
> **D4** exists to avoid. The supervisor has to be **another console agent**, in its own pane,
> speaking MCP like any other. See [02-decisions.md](02-decisions.md) → D4.

---

## Loose ends

- [ ] Decide the project's name — parked until the prototype is in front of us
- [ ] `flow-note "text"` — a short command to write into the panel of the current branch's task
- [ ] A hook that publishes the state automatically at the end of every step, instead of by hand
- [ ] Tie off two loose ends of the contract: `actions`, and making `header: false` not depend on
      `meta.json` (see [04-panel-contract.md](04-panel-contract.md))
- [ ] Review the terms of use before distributing to third parties (automating subscription sessions
      is a gray area; see [02-decisions.md](02-decisions.md) → D4). **Sharper since April 2026**:
      hosting the official binary is clearly inside —it uses its own session and we do not touch its
      token— but **automating it** from outside is not resolved in writing
- [ ] **A bridge between opencode and `panel.json`** — its backend exposes HTTP with events and its
      sidebar already computes what the panel wants to paint. It would be the contract's second
      producer without touching a line of its code (see [06-references.md](06-references.md))

~~Install `inotify-tools`~~ — it falls with F1.6: the `notify` crate does the same thing and on
macOS and Windows too. As long as the panel keeps running on top of WezTerm, polling by `mtime`
every second works and does not get in the way.

---

## Ideas with no phase

Things we like that **have no phase assigned**. They are neither debt nor commitment: they are
written down so as not to lose them and so that nobody sneaks them into a phase «while we are at
it». If one of them ever goes in, it will be through the front door — with its slot in Part I and
its decision in `02-decisions.md`.

### Tasks and board

**The idea.** That a task be a thing with a name of its own, not just «whatever is running in that
pane»: it is created before having a console, it is queued, it is linked to another one («when this
one finishes, start that one»), and it is seen on a board with columns. It is what Cline Kanban
does, and the **dependency chains** part is genuinely good — it solves chaining work without writing
a script.

**Why it does not go in now.** It clashes head-on with the shape of phases 1–3, and the clash is a
deep one, not one of scheduling:

- Here the unit is **the pane** — a live console, with its `PANEL_ID` and its `panel.json` (D7). A
  queued task does not have a console yet, so it does not have a pane, so it has nowhere to publish.
  Putting tasks in forces inventing a second unit above the pane, and that is a new data model, not
  a new screen.
- «When this one finishes, start that one» demands that the app **know** that something finished.
  That can only come from the contract, published by whoever does the work — never deduced (D6).
  Before Phase 3 is worn in there is no state to trust for triggering anything, and triggering
  processes off an unreliable signal is worse than not triggering.
- Phase 4 already covers 80% of the value without any of this: **ordering by what claims you** is
  more useful than a board, and it needs no task model — only the state that already arrives.

**What would make it go in.** That after using Phase 4 for a while something is still missing, and
that something is concrete: queuing work that does not have a console yet, or chaining without a
script. If what is missing can be solved by ordering Phase 4's list better, no board is needed.
