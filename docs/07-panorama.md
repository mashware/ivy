# Landscape — who is here and what to steal from them

August 2026. Sorted by category, with what is worth taking from each one.
The details of *how* each terminal *is built* are in [06-referencias.md](06-referencias.md);
this is the map of the territory.

**If you only came for one thing, go to [«How we differ»](#how-we-differ--august-2026):**
the summary of this month's eight analyses, with what no longer sets us apart and the three things
that remain.

---

## A. Terminals with their own engine and UI

Maximum control and performance; you get to draw every button yourself.

| | Stack | Worth taking |
|---|---|---|
| **Alacritty** | Rust, GPU | Its engine (`alacritty_terminal`) **is our dependency** |
| **WezTerm** | Rust, GPU | Its own multiplexer with detachable sessions; config in executable Lua; `wezterm cli` as a control surface |
| **Kitty** | C + Python | Its own graphics protocol (images in the terminal), scripting API |
| **Ghostty** | Zig (`libghostty`) + native UI per platform | **The separation model**: shared core, native interface on every system |
| **Warp** | Rust + its own UI on `wgpu` | **Command blocks**: each command and its output are an object you select, fold and share. **Since 2026 it is also an agent orchestrator and open source — see E5, it is the most serious competitor on the list** |
| **Rio**, **Contour**, **Foot** | Rust / C++ / C | Niche; little to copy for this case |

---

## B. Terminals on top of a toolkit

They inherit buttons, menus and drag-and-drop. In exchange, they are tied to one platform.

| | Stack | Worth taking |
|---|---|---|
| **Tilix** | VTE + GTK3 | **The pane model we want**: a tree of panes, dragging terminals to rearrange them, saveable sessions |
| **Ptyxis** | VTE (GTK4, GPU) + libadwaita | Container integration (Podman/Toolbx/Distrobox) as a concept of "one environment per tab" |
| **Konsole** | Qt | Mature profiles and panes |

---

## C. Workspace terminals

Here the terminal stops being a grid of characters and becomes a canvas. **The category
closest to what we want.**

| | Stack | Worth taking |
|---|---|---|
| **Wave Terminal** | Electron | **Blocks**: the window is composed of blocks (terminal, editor, Markdown/JSON/image preview) and the tabs are grouped into *workspaces* from a sidebar. It is our structure under another name |
| **Extraterm** | Electron | **The most interesting thing on the whole list** (see below) |
| **Tabby** | Electron | Built-in SSH manager, plugin ecosystem, good support on Windows |
| **Hyper** | Electron | Extensions that are trivial to write; slow and heavy — the warning about where not to go |
| **VS Code** | Electron + xterm.js | Built-in terminal with panes and tabs at a scale of millions of users |

### Extraterm deserves a paragraph of its own

It wraps **each command in a frame** with its output inside, colored according to the result: blue
if it worked, red if it failed. And on top of those frames it builds things no other terminal has:

- The output of a command is **editable in place**
- The `from` command **reuses the output of an earlier command as the input** of another
- A cursor mode to navigate the scrollback like a text editor
- `show` to view files, images and highlighted code inside the terminal
- File upload and download that works **even through SSH**

That is OSC 133 taken all the way. And it is directly applicable: if the app knows where
each command starts and ends, the panel can offer «send this output to the agent» or
«retry this command» without anyone copying and pasting anything.

---

## D. Multiplexers

| | Worth taking |
|---|---|
| **tmux** | The substrate on which half of the orchestration space (category E) is built. Persistent sessions, `send-keys`, `capture-pane` |
| **Zellij** | Rust; **its own interface is made of WASM plugins**, a clickable bar, floating panes you can move with the mouse, dragging borders to resize |

---

## E. Agent orchestrators — the new space

Born in 2025-2026. **They all share the same pattern: one git worktree per agent, a panel
to see what each one is doing, and diff review before merging.**

### E1 — The ones that hide the terminal

They present a board or dashboard; the console sits underneath.

| | What it is | Note |
|---|---|---|
| **Conductor** | Desktop app: Claude Code / Codex / Cursor agents in parallel, each in its own worktree, with a visual panel and diff review | **macOS only and proprietary.** See below |
| **Crystal → Nimbalyst** | Electron; several Claude Code sessions in worktrees, conversation tracking, built-in git operations | Crystal is the old name |
| **Vibe Kanban** | CLI + web, kanban board; each task in its own worktree; supports Claude Code, Codex, Amp, Cursor CLI, Gemini | **The company behind it shut down in April 2026**; it continues as community open source |
| **Cline Kanban** | Local server + board in the browser; each card is a task with its own ephemeral worktree and its own terminal. Agent-agnostic (Claude Code, Codex, opencode, Cline CLI). Dependency chains, cron, approval hooks, headless in CI | **The closest one on the whole list.** See below |

### Conductor deserves a paragraph of its own, because it is the one that forces the thesis to be sharpened

Three panels: a workspace sidebar (each one named after a city), **chat in the center**, and on
the right the live diff plus the built-in terminal. The chat is not the agent's real TUI: it is
their own interface, so they consume the harness **stream** instead of hosting its painting. Hence it
sits in E1 — the terminal is a secondary panel.

**What it shows, and why it matters.** According to third-party reviews it shows **a token count and
cost under each answer, and a warning when the agent approaches the context limit**. *(Not
verified: it does not appear in their official documentation, it comes from Grokipedia and reviews,
and it is macOS, so it could not be checked by running it either.)* If it is true, it is the only one
on the list that shows those specific data.

**And even so it is not the panel we want, and here is the distinction that has to be kept clear.**
The axis is not **how much** data is shown, it is **who decides what is seen**. Conductor picks three
metrics and paints them; even if it picked forty, it would still be the one picking. Our panel is
free text written by the producer: what step the task is at, what has it blocked, and **whatever the
user of that pane feels like putting there** — by asking their own agent for it, without anyone
having had to design that field beforehand. Tokens and cost are an **extra** of the contract, not its
reason to exist.

The three consequences of having built it by reading the stream:

1. **It only works for Claude Code, Codex and Cursor.** A CI, a deployment or a script of your own do
   not appear in any stream.
2. **There is no way back.** Their scripting reference documents **no** mechanism at all for
   a process of the workspace to report to the interface: no file, no command, no endpoint. It is the
   only one of the three just analyzed that does not even have the equivalent of `superset-notify`.
3. **It exposes no MCP of its own:** *«Conductor does not define a separate MCP config format. It uses the
   MCP configuration that Claude Code and Codex load for the session.»* It only passes on the agent's
   configuration, so the agent → app channel of **D5** does not exist.

**Their hooks are worktree hooks, not agent hooks** — `pre-setup`, `post-setup`, `pre-run`, `post-run`,
`pre-archive`, `post-archive`, in `.conductor/settings.toml`. Prepare, start and clean up; nobody
publishes state there. And they inject `CONDUCTOR_WORKSPACE_PATH`, `CONDUCTOR_ROOT_PATH`,
`CONDUCTOR_WORKSPACE_NAME` and `CONDUCTOR_PORT`: there is identity per pane, but for naming resources
and handing out ports, not for publishing anything. **D7** confirmed in its shape, not in its purpose.

### E5 — Warp: the one that meets all five conditions

It was in category A for its command blocks, and in 2026 it changed category with two changes:

- **14 April 2026** — universal agent support: Claude Code, Codex, Gemini CLI and opencode
  integrated **as first-class citizens**, not as just any shell processes.
- **30 April 2026** — **open source**. 64 132 stars and daily movement.

And it has an **Agent Management Panel**: the state of every agent of every session, with five
values (`Working`, `Blocked`, `Canceled`, `Failed/Errored`, `Success`), cancelling one mid-flight,
seeing which ones are waiting for input or have failed, and **jumping to the associated pane**.

**What point 3 of the conclusions called «the free square» was: open + cross-platform +
the terminal as a first-class citizen. Warp meets all three, and it is the only one on the list that
does:**

| | |
|---|---|
| Its own native terminal, GPU, not a browser | ✅ |
| macOS, Linux and Windows — our **D1** | ✅ |
| Hosts Claude Code, Codex, Gemini CLI, opencode | ✅ |
| A panel with everyone's state and a jump to the pane | ✅ |
| Open source | ✅ |

**What they are still missing, verified in their code and not on their website:**

1. **The list of producers is an `enum` compiled into the app.** There is a `trait ThirdPartyHarness`
   with one implementation per agent —`harness/claude_code.rs`, `harness/codex.rs`,
   `harness/gemini.rs`— and a closed `enum Harness`. The same pattern as Superset, but inside the
   binary: a CI, a deployment or a script of your own do not get in without a *pull request* to Warp.
2. **Five states and no way in.** Their documentation describes no file, command or
   endpoint through which an arbitrary process could publish into that panel.
3. **It is a management panel, not a panel per pane.** It lives in the top right and it is a list of
   sessions with their traffic light, to decide which one you go to. It is not a canvas per pane
   whose content is written by whoever does the work.

**Warp Agent CLI is another product, and it does not compete with us.** It is Warp's own agent pulled
out into a binary that runs **in any terminal**, even over SSH without installing anything on the
remote machine, with its own «tmux-like» PTY multiplexer, automatic model routing and subagents both
local and in the cloud. **Inference is billed at API cost, with no margin.** It competes with Claude
Code — and it is a **potential producer of `panel.json`**, just like Cline IDE.

**The opportunity it brings, and it touches D3.** Not everything released is the same; verbatim from
their README:

> Warp's UI framework (the `warpui_core` and `warpui` crates) are licensed under the **MIT license**.
> The rest of the code in this repository is licensed under the **AGPL v3**.

Verified in the `Cargo.toml` of both crates: `license = "MIT"`. So there exists **a UI framework in
Rust, with GPU, a permissive license and proven in a real cross-platform terminal**. It is a
third candidate for [02-decisiones.md](02-decisiones.md) → D3, noted there with its two cautions:
it is not verified that `warpui` is usable independently —it may drag in dependencies on
AGPL crates from the same repository—, and nothing from the rest of the code gets copied into a
project that is not AGPL.

**The risk, and it is the highest in this whole landscape.** With Cline and with Xirp the sentence
was «if they publish an open format, they take the differentiator». With Warp it is worse for two
reasons: **they already have everything else**, and **they are already open source with 64k stars**,
so the idea does not even have to occur to them — anyone can propose it in a *pull request*. What we
have left is one single thing, and it is worth being crystal clear about it: **that the content of
each pane's panel is decided by whoever does the work, and that whoever does the work can be
anyone.**

### E6 — cmux: the only one with an open channel, and the one that shows where the limit is

A native macOS terminal in **Swift + AppKit**, using **`libghostty` as a library** — not a fork.
It is our **D2** applied and the Ghostty separation model that category A already praised.
**GPL-3.0-or-later**, free, macOS only (iOS on TestFlight). Vertical tabs, splitting in both
directions, a built-in browser, and a sidebar that shows per tab the git branch, the
linked PR, the directory, **the listening ports** and the text of the last notification.

**Two things separate it from everything else in this category:**

1. **It has no adapters.** *«cmux is a terminal, so any agent that runs in a terminal
   works»* — Claude Code, Codex, opencode, Gemini CLI, Kiro, Aider, Goose, Amp, Cline, Cursor
   Agent. Neither Warp's `enum Harness` nor Superset's ten wrappers. And when an agent launches
   subagents, it turns them into **native panes, split in the window**, instead of hidden processes.
2. **It has a genuinely open channel.** Notifications come in through **OSC 777** (title and body),
   **OSC 99** (Kitty's: with a subtitle and identifiers as well), the `cmux notify` command or agent
   hooks. **Any process can emit them**: a CI, a script, a cron job. It is the closest anyone
   has come to this contract.

And its **socket API** does what our MCP channel does: creating workspaces, opening panes, sending
input, **reading the contents of the screen**, screenshots and driving the browser.

**Why it is still not this panel — and the limit is structural, not a matter of maturity:**

- **The payload is one line of text, and it is ephemeral.** Title, subtitle, body. No progress, no
  steps, no lists, no blockers. And the life cycle is *received → unread → read → deleted*:
  **it disappears when you read it**. The sidebar keeps «the last notification», one line. That
  answers «something happened», not «where does this stand»; two hours later it answers nothing.
- **OSC travels inside the terminal's stream.** Only what is inside that PTY emits it, mixed
  in with the program's output. `panel.json` is **out of band**: it is written by a process that does
  not live in that pane —a CI on another machine, a cron job, a service— without dirtying the
  scrollback or competing with a TUI's painting.
- **A notification cannot say «I am still alive».** The `updated_at` with `stale_after_minutes` gives
  away the producer that died halfway through; OSC has nowhere to put that.

**What has to be stolen from them, and it is one of the best ideas in the landscape: understanding
OSC 777 and 99 as well as our own contract.** They are standards that Kitty, rxvt and half a dozen
tools already emit, so supporting them gives free compatibility with everything that notifies today,
without asking anyone to adopt `panel.json`. The contract for whoever wants a panel; the OSC
sequences so that the rest works without changing anything.

### The seven analyzed, side by side

| | Xirp (E3) | Superset (E4) | Conductor | **Warp (E5)** | **cmux (E6)** | **Orca (E7)** | **Herdr (E8)** |
|---|---|---|---|---|---|---|---|
| The terminal is | first-class | inside the browser | a secondary panel | **first-class** | **first-class** | first-class, in Electron | **the substrate: it is the multiplexer** |
| Platforms | macOS | macOS + AppImage | macOS | **all three** | macOS | **all three + mobile** | **all three, one binary** |
| Open | no | ELv2 | no | **AGPL + UI under MIT** | **GPL-3.0+** | **MIT** | **Apache-2.0** |
| Agents it accepts | 3, with an adapter | 10, with an adapter | 3, with an adapter | 5, compiled `enum` | **any, with no adapter** | **any, with no adapter** | **any**; 17 with an integration |
| Who decides what is seen | the app (3 constants) | the app (4 events) | the app (its metrics) | the app (5 states) | **the producer (free text)** | the app (6 indicators) | the app (4 states) **+ the producer's tokens** |
| Way for a third party to publish | harness hooks | hooks + `superset-notify` | **none** | **none** | **OSC 777/99, `cmux notify`** | **the OSC title, de facto** | **`report-agent --source custom:*`** |
| Channel for driving the app | MCP | MCP | **no** | not verified | socket API | CLI + MCP | **NDJSON socket, 70+ methods** |
| What is published is | a state | an event | metrics | a state | **a one-line notification** | a state | **a state + key-value pairs** |
| Persists when you come back | as state | no | yes | as state | **no: it is deleted when read** | as state | **yes, and it survives a restart** |
| **Structured state chosen by whoever does the work** | **no** | **no** | **no** | **no** | **no** | **no** | **almost: no lists, no marks, no free-text phase** |

The last row is the project, and the two above it explain why. Six of the seven have state
but with a vocabulary the app picks; **cmux has a genuinely open channel and that is why it shows
where the limit is**: what fits through there is a one-line notice that is deleted when read. Nobody
has the thing in between — a structured, persistent body written in the producer's vocabulary
(**D6**).

**The last column is the one that forces everything else to be sharpened.** Herdr does have a
documented channel, identity per pane and a producer that can be anyone; what it does not have is a
**body**: four states of its own and key-value pairs «for display only». The empty square has
narrowed from «nobody opens the channel» to **«nobody lets anything with shape travel down the
channel»**.

**And Orca forces the third-party row to be qualified.** They did not design their channel for this:
they read the terminal title, and it turns out anyone can write a title. So a CI **can** indeed
tint a colored dot for it with no adapter at all, just as in cmux — but it can only say one of the
words Orca already knew. **An open channel with a closed vocabulary is still a traffic light**,
and that is exactly the limit this project exists to get past.

### Cline deserves a paragraph of its own

They are two products and only one of them concerns us. **Cline IDE** is an agent inside the editor:
it does not compete with us, it competes with Claude Code. It is a potential *producer* of
`panel.json` — and the usual example of a closed panel, because only Cline knows its state.

**Cline Kanban** is competition, and it reached two of our conclusions on its own:

- **It diagnoses the same thing**: that the bottleneck is no longer the agent but the human who does
  not know which of their eight terminals each thing is in.
- **It confirms D4 from outside**: they built the orchestration by launching each agent's official
  binaries in a process, not by calling models through the API. It is the only route that preserves
  the subscription, and it was taken by a company with money behind it. **Verified in August 2026,
  and it was true:** since **4 April 2026** the Pro, Max and Team subscriptions **do not cover** use
  through third-party tools —opencode, OpenClaw and anything that routes through OAuth—, and anyone
  integrating Claude needs an API key with pay-per-use billing. The whole timeline and what it
  means for this project, in [02-decisiones.md](02-decisiones.md) → D4.

And they put the gap in writing: *«We're not trying to replace your terminal.»* They are a layer on
top. We are the layer underneath. The three differences that remain, and that are the project's
reason to exist:

1. **Their terminal is inside the browser.** The board is a local server you open in a
   tab. Whatever emulator they embed, it is not a console with panes, mouse and
   measured performance: it is a viewer inside a card.
2. **The state is defined by Cline, not by whoever does the work.** They show «card state + the
   agent's last output»: a closed vocabulary, one per supported runtime. A CI, a deployment or
   a script of yours has nowhere to publish «I have 12k of context left» or «I am blocked
   waiting for approval» — precisely what cannot be deduced from outside (D6).
3. **It is task-centric, not session-centric.** The model is «drop the card, hit play». A long
   exploration session does not fit in there, nor does a normal shell next to an agent.

**The risk, said plainly:** if Cline publishes an open format so that any process can write
into its card, it takes our contract differentiator in one release. What they cannot copy
cheaply is the native terminal underneath: the browser is a structural decision of theirs.

### E2 — The ones that keep the terminal at the center

A text interface on top of tmux. **Closer to our idea**, but they inherit tmux's limits.

| | What it is |
|---|---|
| **Claude Squad** | A TUI for several agent sessions in parallel, on top of tmux + worktrees. Claude Code, Codex, opencode, Amp |
| **amux** (two different projects with the same name) | One is an agent multiplexer on top of tmux; the other, a control plane with a web and mobile dashboard for dozens of sessions |
| **dmux** | Agents in parallel with tmux + worktrees, with hooks across the whole worktree life cycle |

### E3 — Xirp (Spotify): the square next door

Public beta on **10 August 2026**, out of **36 000+ internal sessions** and thousands of
engineers. A desktop app with **persistent terminal sessions** —you switch from one to another
without losing state—, a grid view, one worktree per task, and Claude Code, Codex and Gemini CLI
authenticated **through their native CLI**. It is not E1, because the terminal is not hidden; it is
not E2, because it is not a TUI on top of tmux. **It is the closest anyone has come to our square**,
and that is worth saying without embellishment.

It confirms three decisions at once:

| | |
|---|---|
| **D4** | It authenticates through each agent's official CLI, not through the API — with a company invoice behind it, which is exactly where paying for the API separately would have been easiest to justify |
| **D5** | MCP as the channel for feeding context into the session: *«A session launched from a catalog entity or Portal Workspace can receive relevant context through MCP»* |
| **D6** | The state is declared to them through harness hooks; they do not deduce it from the painting |

And what it does **not** do, which is where we are still alone:

1. **Three states and that is it.** `Working`, `Idle`, `Waiting for input`, plus the branch name.
   Not the step of the task, not the blocker, not anything the producer decides to put there on its
   own. It is level 2 of D6's annex, and it cannot grow to level 3 with more hooks: the harness does
   not know that semantics.
2. **There is no contract.** The state producer has to be a harness that Xirp already supports. A CI,
   a deployment or a script of your own have nowhere to write.
3. **macOS only, closed and with a mandatory Spotify account.**
4. **Its axis is another one.** The big value is the *organizational context* of Portal/Backstage
   —service catalog, ownership, decisions, autogenerated documentation that feeds back into future
   sessions—. It is an enterprise platform play, and much of it switches off without Portal.

**The risk, and it is the most serious on this whole list.** Spotify already made this exact play:
they opened Backstage in 2020 and it became the de facto standard for developer portals. If they
repeat it —opening Xirp and publishing a format so that any process can publish state— they take the
contract differentiator **and** on top of that they have the native terminal, which is what Cline
cannot copy cheaply. What leaves us room today is not technology: it is that their product is tied to
Portal and to macOS, and that their state model is three constants.

**Reliability of this entry.** The three states and the hooks are in their docs
([getting-started](https://backstage.spotify.com/docs/xirp/getting-started)); the interface detail
comes from secondary press coverage and from sparse beta documentation. **Nobody on the project has
run the app** — it is macOS and it asks for an account. If anyone can install it, the fact to bring
back is whether there is any state field beyond those three.

### E4 — Superset: the most complete product, on the ground we discarded

A desktop app, **Electron + React + `@xterm/xterm`**, source-available under **Elastic License
2.0** and free. It is the most advanced in product terms on this whole list: a worktree per task, diff
review with comments and inline editing, persistent terminals that survive a restart, panes and
tabs, a built-in browser with port detection **per workspace**, agents launched **on cron**, an MCP
server for driving it from another agent, and remote workspaces. It supports ten harnesses: Claude
Code, Codex, opencode, Cursor Agent, Copilot, Grok, Amp, Gemini, Pi and Mistral Vibe.

**It confirms four decisions, and one of them to the letter.** Everything through the agent's CLI
without touching the API (**D4**); an MCP server as the agent → app channel (**D5**); a mechanism for
the agent to declare state instead of the app guessing it (**D6**); and **it injects
`SUPERSET_TERMINAL_ID` into the terminal's environment**, with the hooks bailing out early if it is
not there — which is **D7** under another name and with the same job.

**Where it does not reach:**

1. **Four events, and they are in its code:** `eventType: "Start" | "Stop" | "PermissionRequest" |
   "PendingQuestion"`. The payload that travels is `{terminalId, eventType, agent:{agentId,
   sessionId}}`. There is no room for the step of the task, for the blocker, or for anything the
   producer wants to add.
2. **There is no panel, there is a traffic light with a little bell.** An orange indicator in the
   sidebar, a badge on the dock, a sound when it finishes and a system notification. It solves «which
   pane do I go to», not «how is it going». The closest thing to a contract is `superset-notify
   review "…"`: one state out of four plus a free sentence, which travels as notification text and
   not as the pane's content.
3. **One adapter per harness, not a contract.** They install a different wrapper per agent —
   `~/.cursor/hooks.json`, a plugin in `~/.config/opencode/plugin/`, a
   `.github/hooks/superset-notify.json` inside the worktree that they also add to
   `.git/info/exclude`. Ten agents, ten integrations to maintain; and a CI run or a script of your own
   does not get in through that door. That maintenance is exactly what an open contract saves.
4. **The ground is the one we already discarded.** Electron + xterm.js is Orca's stack, with a
   measurement of our own in the «Measured — 8 August 2026» annex of
   [02-decisiones.md](02-decisiones.md) → D3: negative scaling, 7.1 MiB/s aggregate with four panes
   against 8.6 with one.

**And here is somebody else's empirical evidence for D6**, which is the most valuable thing on this
card. Two findings, both in their own repository:

- **Bug [#5259](https://github.com/superset-sh/superset/issues/5259), open.** The «Task complete»
  notification fires **on every turn of the model**: a task with 63 turns produced 63 alerts. The
  cause is literal — Cursor Agent's `stop` hook means *«I finished this turn»* and Superset read it as
  *«I finished the task»*. It had already happened to them (#1785) and the *debounce* fix (#1786) was
  closed without merging. **A declared event is not enough if the app puts on top of it a semantics
  the harness never had**: it fails just the same as guessing. And it is not fixed with a *debounce*,
  because the missing datum is nowhere in the system.
- **Their own `HOOKS_INVESTIGATION.md`** documents that the v1 compatibility path skips the scoping by
  identity: it is enough for the `session_id` to come from the agent's own payload for the guard to
  pass, and then a foreign session `curl`s the local notification server with the pane and terminal
  identifiers empty. It is the failure **D7** names as its reason for existing, documented by them.

**The risk, compared with Xirp: different and smaller.** Xirp is worrying because of who is behind it
and because it has the native terminal; Superset, because of how far it has got in product terms. But
the ELv2 closes the door on them becoming a standard the way Backstage was, and their emulator lives
in the browser — the same structural decision we already pointed out in Cline, and one that does not
change without redoing the product.

### E7 — Orca: the one that already won adoption, and where it falls short

A desktop app, **Electron + TypeScript**, **MIT** licence, from Stably AI (YC W22). The repository was
created on **17 March 2026**; as of 24 August it has **52 261 stars and 3 612 forks**, with daily
pushes. On adoption there is no contest with anyone else on this list, and that weighs more than any
technical card: **it is the one defining what people expect from the category.**

**The product.** One worktree per task; **25-30 agents** preconfigured and «anything that runs in a
terminal» pointed at its binary; «Ghostty-class» terminals with WebGL, infinite panes and scrollback
that survives a restart; *Design Mode*, which opens a real Chromium window per worktree so you can
point at an interface element and send it to the agent; diffs with annotations, GitHub and Linear;
worktrees over SSH with reconnection; a companion mobile app; and a CLI for scripting the app from any
shell.

**What it shows of each pane, which is what matters here:**

| What you see | What it means |
|---|---|
| spinning | working |
| amber question mark | **it is waiting on you**: permission or an answer |
| emerald dot | finished, or active and quiet |
| red dot | blocked, interrupted or failed |
| grey dot | idle |
| nothing | a shell with no recognized agent |

Six indicators, the same ones on the agent's tab and on its worktree's card, with a timestamp. It is a
better-painted traffic light than Superset's, and with more states; it still answers **«which pane do
I go to»** and not **«how is it going»**.

**Where that state comes from, and here is the part to read carefully:** *«from the terminal's OSC
title and from the agent's hooks»*. It has two opposite consequences and both matter.

- **In its favour, and it has to be acknowledged: the channel is de facto open.** A terminal's title is
  emitted by **any** process — a CI run, a deployment, a `printf` in a script. You do not have to be a
  supported agent or install an adapter. That puts Orca **alongside cmux, not alongside Conductor**: it
  is the second on the list through which a third party can say something. But what fits through that
  door is **one word of the vocabulary Orca chose** (`working`, `idle`), and it is not even documented
  as a contract: it is a consequence of looking at the title.
- **Against it: for the agent they really care about, they do not trust that.** For Claude Code they
  install a **status line hook** that emits the OSC sequences, and on top of that they **read
  `~/.claude`** to paint the consumption and the closeness to the rate limit. Going to fetch the state
  from the harness's **private disk** is D6 in reverse: it is not declared by whoever works, the app
  goes and spies on it. It works today, it is one adapter per agent, and it breaks the day Anthropic
  changes that format — which it owes nobody.

**Their CLI goes in the opposite direction from ours.** `orca worktree create`, `terminal send`,
`wait`, `split`, `goto`, `click`, `fill`, `artifacts share`: it is for a script to **command** Orca,
not for a process inside to **tell** its own story. There is no command for publishing state. They do
have a skill registry and **MCP**, so the agent → app channel of **D5** exists.

**Three things that stop setting us apart because of it, and they are noted above:** genuinely
cross-platform (all three, plus mobile), hosting any CLI agent without an adapter, and **showing cost
and closeness to the limit** — which until now we only attributed to Conductor, and unverified. Orca
does it and it can be checked.

**What still stands against it, and it is the same three as always:** the panel's body is a
six-state traffic light and the app picks it, there is no way for the producer to write a list of
steps with their marks, and what a third party publishes is one word with no persistence of its own. A
CI run that wants to say «blocked waiting for approval for 40 minutes» can only put `working`.

**The ground is the one we already measured and discarded.** Electron + `xterm.js` with WebGL, with a
measurement of our own in the «Measured — 8 August 2026» annex of
[02-decisiones.md](02-decisiones.md) → D3: **negative scaling**, 7.1 MiB/s aggregate with four panes
against 8.6 with one. On top of that, according to third-party reviews, **400-800 MiB of RAM at rest**
with a handful of agents, ~250 MB of installer, and Linux **only as an AppImage**. Their product
dodges that ceiling because it fans out agents that think; ours cannot, and that is why D3 stays
decided.

**The risk, and it is the biggest on the whole list.** It is the only one that puts together **mass
adoption + MIT + a channel any process already gets in through**. What it lacks to take legs 1 and 2
is not technology: it is **deciding that the panel's body is written by the producer**. That fits in a
*pull request* from anyone, and with 3 612 forks the probability of somebody thinking of it is not
low. What they cannot copy cheaply is the native ground: Electron is a structural decision of theirs,
just as the browser is in Cline and in Superset.

**A note on names, for whoever searches later:** there is another project called `orca` on GitHub
(`orca-cli/orca`, a Go binary with a unified review queue). It is not this one. The one here is
[stablyai/orca](https://github.com/stablyai/orca), `onorca.dev`.

### E8 — Herdr: the closest of the whole list, and it has to be said without embellishment

**It is not an orchestrator with a board: it is the substrate.** A multiplexer written in **Rust**, a
TUI inside your own terminal, **Apache-2.0**, a **client-server** architecture —the server owns all
the state and survives you disconnecting, so the agents keep running—, named sessions, remote attach
over SSH, and worktrees wrapping `git worktree`. Its tagline is literal: *«the runtime your coding
agents live on»*. It is tmux with agent awareness, and that is why it is here and not in category D.

The repository was created on **27 March 2026** — ten days after Orca — and as of 24 August it has
**31 888 stars and 2 281 forks**, with daily pushes. **A single binary for the three platforms**,
Windows included with its own bundled ConPTY.

**Its states, and where they come from.** `idle`, `working`, `blocked` —plus a `done` that appears in
the API and in the sidebar and that their own documentation marks as rare—, with **accumulation
upwards**: agent → pane → tab → workspace, with priority `blocked > working > idle > done`. One
blocked agent in a workspace of ten paints the whole workspace as blocked. The detection goes in
layers:

1. **Processes**: it recognizes the foreground binary (`claude`, `codex`, `pi`…) and marks that an
   agent lives there.
2. **Screen heuristics**: with no integration installed, it contrasts **the live crop of the buffer at
   the bottom** against rules in TOML, also looking at the terminal's title and the OSC sequences.
3. **Integrations**: hooks installed in the agent's configuration that report semantic state over the
   socket, and that **override** what the heuristics said.

**Layer 2 is exactly what D6 forbids, and they publish the price themselves.** Their documentation
warns that *«unusual new agent prompts may initially show as idle instead of blocked»* and that the
blocking detection is «deliberately strict» for the agents that are only read off the screen. It is
**the second piece of outside evidence for D6**, after Superset's bug of the 63 alerts, and this time
written by the maker itself in its own manual. And it has a measurable cost: the detection polls each
pane **every 300-500 ms** in its own task, and reviews report high CPU with many panes scrolling fast,
with «install the integrations» as the recommended remedy.

**And now the uncomfortable part: their channel is the most similar to ours that exists.** This is not
a nuance, it is the finding of the card.

```
herdr pane report-agent "$HERDR_PANE_ID" \
  --source custom:my-agent \
  --agent my-agent \
  --state working \
  [--seq N] [--message "why it is blocked"] \
  [--agent-session-id …] [--agent-session-path …]
```

- **They inject identity per pane into the environment** — `HERDR_PANE_ID`, `HERDR_BIN_PATH`. That is
  **D7**, under another name and with the same purpose, and it is the third on the list to do it
  (Superset, Conductor, Herdr).
- **`--source custom:*` is documented and open to any process**, not just to a supported agent. A CI
  run, a deployment or a script of your own get in through the front door, with no adapter and without
  asking anybody's permission. **This is the project's leg 2, done by somebody else and published.**
- **They solve two problems we still have not solved:** `--source` gives **authority per source** —each
  integration commands its own and releases it on leaving, so two producers on the same pane do not
  tread on each other— and `--seq` is a strictly increasing number with which they **discard reports
  that arrive late**. Our `panel.json` is today one file per pane where the last writer wins, with no
  authority and no order. See «Concrete things to steal».
- Their socket is **NDJSON with more than 70 methods**, a negotiated protocol version and a published
  schema (`herdr api schema --json`), with subscription events such as `pane.agent_status_changed`.
  The agent → app channel of **D5** exists, and it is the most complete on the whole list.

**Where it stops, which is where we go on alone. Three things, and only three:**

1. **The vocabulary of the state is still theirs.** The producer **declares** —that they do meet— but
   it can only say one of four words that Herdr chose. Our free-text `phase` and the list with six
   marks (`done`, `current`, `pending`, `wait`, `block`, `info`) do not fit in that `--state`. The
   `--message` is one sentence, and only for explaining a block.
2. **The body is key-value pairs and they are «display only».** `report-metadata` adds *tokens* to the
   panel, declaredly without authority over the state; the documented one is `summary`, which is
   painted as `$summary` in the sidebar's row. **Scalars and maps: there are no lists of steps, no
   references, no links, no cost, no remaining context as fields.** *(A limit of 32 keys of 80
   characters appears in a third-party guide and not in their official documentation: unverified.)*
3. **There is no panel, there is a row.** The sidebar aggregates every agent of every workspace with a
   coloured dot and a line. It is the best answer on the list to **«which pane do I go to»** — better
   than ours, because it adds up across workspaces — and it does not try to answer **«how is it
   going»**. And it cannot: it lives inside a TUI, where the space available is a row of text, not the
   right-hand half of the console.

**What it also stops setting us apart on:** genuinely cross-platform (one binary, Windows included),
open source, hosting any agent without an adapter, and **the producer declaring instead of the app
deducing**. That last one had been in the pitch from the beginning and today it has an owner.

**And what is still ours against it, precisely:** their channel is **a local socket**
(`/tmp/herdr-<session>.sock`). A file in a directory is not: a container with the directory mounted, a
CI run on another machine writing over a shared volume, or an `scp` publish into `panel.json` without
being able to talk to any process of ours. That is the real difference between «out of band» and
«outside the process».

**A security risk worth noting, and it reinforces D7.** Their documentation **does not describe
authentication or access control** over the socket: access appears to be implicit for any local
process that reaches it, and the `pane_id` travels as a parameter. If that is so, any process on the
machine can report on **any** pane, which is the same hole Superset documented in its
`HOOKS_INVESTIGATION.md`. *(Not verified by running it, and on Windows they use named pipes, which
have a different permission model.)*

**The product risk, and it is the highest of the whole list along with Orca's.** The distance between
`report-metadata` and our contract **is one field, not an architecture**. They already have identity
per pane, a socket with a schema, authority per source and a sequence number; what they lack is
accepting a structured body instead of pairs of strings. That fits in a release. What they cannot do
cheaply is the panel next to the console: they live inside somebody else's terminal, and that is a
structural decision of theirs just as Electron is in Orca and the browser is in Superset and in Cline.

**A note on names:** the repository moved from `ogulcancelik/herdr` to
[herdrdev/herdr](https://github.com/herdrdev/herdr) (the first one redirects); the documentation is at
`herdr.dev`. Not to be confused with `wmux`, another agent multiplexer.

---

### E9 — Gentle: the agent that grew a face, and the marker that has no wall to hang on

Two repositories of the same author (Alan Buscaglia, *Gentleman Programming*), announced together in
September 2026 and worth separating, because **only one of them is on our square** and it is not the
one being announced.

| | What it is | Numbers, 6 September 2026 |
|---|---|---|
| **gentle-pi** («Gentle Shell») | An MIT package installed **inside Pi**, an agent CLI by Earendil (MIT, 15+ providers) that runs in **your** terminal. It is not a terminal and it is not a multiplexer: it is **the face of an agent**. It ships a status bar with context gauges, model and **subscription usage**, a git diff overlay on `alt+g`, and subagents as `pi --mode rpc` child processes | 432 ★ · 81 forks · pushed the same day |
| **gentle-agent-state** | An MIT shell layer that paints an agent's state **in tmux's tab bar**, in Zellij, or in Ghostty's title. Three states, `working` / `blocked` / `idle`, declared by the agent through hooks and plugins — **not deduced**. The tab shows the **worst state of its panes**: `blocked > working > idle` | 54 ★ · 6 forks · **last push 1 July 2026** |

**The one that touches us is the small one, and it touches the pane we thought was ours.** The
rollup by worst state onto the tab is, line for line, what **F2.8** does with `attention` — and this
repository has had it since June, two months before that milestone closed. It also belongs on the
list of what no longer sets us apart, in the row that fell in August: **the state is declared by
whoever works, not deduced by the app** (`~/.claude/settings.json` hooks, an opencode plugin, a Pi
extension).

**And it has a de facto third-party channel, undocumented as such.** Every adapter ends in a single
neutral dispatcher, `~/.config/agent-state/scripts/agent-report.sh`. Nothing stops a CI run, a
deployment or a script of your own from calling it — exactly the way Orca's OSC title ended up open
without meaning to. What it does **not** have is a body: the vocabulary is *«deliberately small»*,
three words and a coloured dot, less than Herdr's four states plus its key-value pairs.

**What it does not have at all is a wall to hang the marker on.** It needs tmux, Zellij or Ghostty
to paint, and what it can paint there is a character in a tab title. That is the same limit this
landscape already attributes to Herdr —*«the place to paint it»*— and the reason none of them can
show, next to the console, what the producer wanted to say.

> **The detail that says the most about the map**, and it is one line of its README:
> `HERDR_ENV=1` **disables every adapter so Herdr can own the integration**. The two of them already
> know about each other and one steps aside for the other. This square is no longer empty of people:
> it is empty of the contract.

**No expiry, and it shows in the workaround.** Nothing declares when a state stops being valid, so
the project self-heals: a heartbeat in tmux's `status-right` and a script that **clears stale
`blocked` states when a pane becomes visible**. That is repairing a deduction after the fact, which
is the opposite of `updated_at` + `stale_after_minutes`.

**What to steal:** the sound on the transition to `blocked` and back to `idle`, which this project
does not have and costs nothing; and the honesty of the three-state table, which reads in five
seconds.

---

## What this landscape says about the project

**1. The «one worktree per agent» pattern is already standardized.** Everybody does it, and `flow`
already does it. It does not have to be invented: it has to be leaned on. Pane identity (D7) fits
exactly there — one worktree, one pane, one `PANEL_ID`.

**2. Our square has neighbours, and one of them is Spotify.** The E1 ones hide the terminal; the E2
ones keep it but build it *on top of tmux*, inheriting what tmux does not give (a decent mouse,
buttons, dragging panes). That left the square empty until two appeared, from different sides:
**Xirp** (E3) is a native app with a first-class terminal and persistent sessions, and **Superset**
(E4) goes much further in product terms —panes, tabs, diffs, ports, cron— but with the emulator
inside the browser. And **Warp** (E5), which is no longer a neighbour: it meets the five conditions
—a native terminal, the three platforms, five hosted harnesses, a status panel with a jump to the
pane, and open source with 64k stars—. You can no longer say «nobody hosts agents as a first-class
terminal», nor «nobody does it open and cross-platform». And **cmux** (E6) knocks down half another
sentence: **there is one with an open channel**, because any process can emit OSC 777/99 at it. What
is left standing has to be said precisely, because the short version is no longer true: **nobody
defines an open contract for publishing structured, persistent state.** The first four have state with
a vocabulary the app picks —three constants, four events, a few metrics, five states—; cmux lets you
pick the text but only a title and a body fit, and they are erased on being read. None of them has a
panel per pane whose body is written by whoever does the work.

**3. Conductor's gap no longer exists: Warp closed it.** It was the most polished of the group and it
is **macOS and proprietary**, so cross-platform and open was free space. Cline Kanban covered it from
one side —open, cross-platform, agent-agnostic—, Xirp from the other —a first-class terminal, with
Spotify behind it, though macOS and closed— and **Warp closed it entirely**: open, the three platforms
and the terminal as a first-class citizen, all three at once. What is left unoccupied **is no longer a
combination of platform and licence**, because that combination exists. It is **the open contract**,
and nothing else. It is worth not fooling ourselves about this: any selling point that is not the
contract, Warp today meets better than we do.

**4. A serious warning: the space is full and it has churn.** There are comparisons of twelve tools,
Crystal has had to rename itself, and the company behind Vibe Kanban shut down. That there is demand
does not mean it is easy to monetize. Go in knowing it: for one's own use this is a safe investment;
as a product, the differentiation has to be obvious in thirty seconds.

**5. The differentiation in one sentence.** «The host where anybody publishes state» is no longer
distinguishable in thirty seconds from «agent-agnostic board» —Cline Kanban says something similar—,
nor by hosting terminals, which Xirp does, nor by being a native, open, cross-platform terminal that
hosts five agents, which is Warp. It is distinguished by the *anybody*, and by **what** gets published.
The distinction that holds:

**They tell you whether the agent is working or waiting. We let whoever does the work say how it is
going — and whoever does the work does not have to be an agent.**

Underneath, the usual legs still hold: an open contract for any producer, a first-class terminal,
cross-platform, and not paying for the API on top of what you already pay by subscription. What has
changed is the weight: **the first has gone from being one of three to being the one that holds up the
rest.**

**And a precision Conductor forces us to make, because it is easy to get this wrong.** The axis is
**not how much data gets shown**. Conductor already shows tokens, cost and a context-limit warning, and
that does not give it our panel: it chose them. The axis is **who decides what is seen**. Our panel is
free text from the producer —the step of the task, what is blocking it, and whatever the user of that
pane cares to put there by asking their own agent for it— without anybody having had to design that
field beforehand. The AI's metrics are an **extra** of the contract; confusing them with the goal would
take us to compete on ground where there are already four products ahead of us.

**6. And the real race, said without embellishment — with two rivals of different classes.** Superset
(E4) is the most complete of the orchestrators and it sits on Electron + xterm.js, the stack D3
measured and discarded: **it has to change its ground to catch us, and that means redoing the app.**
With **Warp (E5) that advantage does not exist**: its ground is Rust with its own GPU work, just as
good as ours, and on top of that it already runs on the three platforms. Against Superset the race is
«we build product while they redo the foundations». Against Warp there is no race on foundations:
**the only thing separating us is having decided that the app is not the owner of the semantics.** It
is a design decision, not a technical advantage, and a decision is copied in a release — all the more
so with them being open source, where the idea can arrive in a *pull request* from anyone. The clock
runs faster than it looked.

---

## How we differ — August 2026

The summary of this month's eight analyses. It is written here because the answer changed: a month ago
it was three legs and today it is one, and whoever comes back to this question in three months needs
today's version, not the first week's.

### What no longer sets us apart

This is the important thing to accept, because it was in the pitch and today it has an owner:

| | Who already has it |
|---|---|
| A native terminal, not a browser | Warp, cmux, Xirp, **Herdr** (it is the multiplexer itself) |
| Genuinely cross-platform | **Warp**, **Orca** (mobile too), **Herdr** (one binary, with Windows) |
| Open source | Warp, cmux, Superset, **Orca** (MIT), **Herdr** (Apache-2.0) |
| One worktree per agent | **all of them** |
| Hosting any CLI agent | **cmux**, **Orca** and **Herdr** (with no adapters), Warp (five) |
| Warning when an agent is waiting | **all of them** |
| Cost, tokens, remaining context | Conductor (unverified), **Orca** (verified: it reads `~/.claude`), **gentle-pi** (its status bar publishes the subscription's usage) |
| A channel for driving the app from the agent | Superset and Xirp (MCP), cmux (socket), Orca (CLI + MCP), **Herdr** (socket, 70+ methods) |
| Any process being able to publish something | **cmux** (OSC 777/99), Orca (the OSC title, de facto), **Herdr** (`--source custom:*`, documented) |
| **It being declared by whoever works instead of deduced by the app** | **Herdr** (its hook layer overrides the heuristics) and **gentle-agent-state** (hooks and plugins, four agents, since June 2026) |

Ten things. None of them serves any longer to explain the project in thirty seconds, and the last one
—which had been in the pitch from the beginning— fell in August. **Of the ten, Orca has six and Herdr
seven.** Neither of the two has the one that is missing: **what fits through that channel.**

> **September 2026 adds an eleventh, and it was ours until F2.8 closed:** rolling the state of the panes
> up onto the tab by the worst of them. `gentle-agent-state` has done it in tmux since June (E9), and it
> paints the same three words this project paints — with a colour and a letter, and with no body behind
> them.

### What does remain, and it is three things

**1. The panel's body is decided by whoever does the work, and it is structured.** The others have
rich state with a vocabulary the app picks. cmux opens the channel but only carries one line, Orca
opens it without meaning to so as to carry a word of its own, and **Herdr, which is the one that gets
furthest, carries four states of its own plus «display only» key-value pairs**.
Nobody has the thing in between: **a list with marks** (`done`, `current`, `pending`, `wait`, `block`,
`info`), references, links and a free-text `phase`, written by the producer in its own vocabulary,
without anybody having had to design that field beforehand.

**2. The producer can be anybody, and through an out-of-band channel.** Not a compiled `enum` (Warp),
not a wrapper per agent (Superset), not a supported harness (Xirp, Conductor), not «whatever lives
inside that PTY» (cmux, over OSC). A file that anybody writes from anywhere: an agent, a CI run on
another machine, a deployment, a cron job, a script of your own.

> **Half a leg, since August 2026.** Herdr documents `--source custom:*` and explicitly admits a
> producer that is not an agent. What is left ours on this point is **the medium**: their channel is a
> local socket, and a file is not. A container with the directory mounted, a CI run on another machine
> writing over a shared volume, or an `scp` publish without being able to talk to any process of ours.

**3. It is persistent state, not an event.** It is still there when you come back, and it **knows how
to expire** (`updated_at` + `stale_after_minutes`). Herdr persists and survives a restart, but **nobody
declares when its state stops being valid**. The practical difference: the other seven tell you *which
pane to go to now*; this one tells you *how each one is going* when you arrive.

### Said in one sentence

**They tell you whether the agent is working or waiting. We let whoever does the work say how it is
going — and whoever does the work does not have to be an agent.**

### The real position on the map

The serious rivals have different halves of this square, and none of them both:

| | Has | Lacks |
|---|---|---|
| **Warp** (E5) | native + cross-platform + open | **the contract** |
| **cmux** (E6) | native + open + an open one-line channel | **cross-platform** and the structured contract |
| **Orca** (E7) | cross-platform + MIT + a de facto open channel + **the adoption** | **the native ground** and the structured contract |
| **Herdr** (E8) | native + cross-platform + open + **a documented channel with identity per pane** | **the contract's body** and the place to paint it |

The complete combination —native, cross-platform, open **and** a structured contract— is still empty.
But it is a gap **one design decision wide, not one technology wide**, and the four surrounding it are
open source, so the idea can reach them in a *pull request* from anyone. **With Orca and Herdr that
stops being hypothetical:** almost 6 000 forks between the two and daily pushes is a lot of people
looking at the same problem, one with the channel open by accident and the other with the channel open
on purpose, with identity per pane and a published schema. **Herdr is missing one field; we are
missing the product.** That is the race, said in one line.

**And who the real competition is, since D19:** **terminal emulators**, not the eight in this
landscape. This document is still the map of the league next door —you have to know what they do and
what to steal from them— but **the bar that decides whether this ships is set by a WezTerm user, not
by an Orca one**. The list of what is left out, with who has it, is in
[02-decisiones.md](02-decisiones.md) → D19.

**The consequence for prioritizing, and it is the reason this summary is written:** the contract is not
the last leg to implement once the terminal is polished. It is **the only one that holds the project
up**, and everything done before it is done knowing it does not differentiate.

---

## Concrete things to steal

| From | What |
|---|---|
| **Extraterm** | Frames per command, coloured by result; reusing a command's output as input |
| **Warp** | Foldable, shareable command blocks. And from the global agent panel: **jumping from the list to the specific pane** — the global summary does not only inform, it is navigation (see E5) |
| **Wave** | Composition out of heterogeneous blocks (terminal, editor, preview) and workspaces in a sidebar |
| **Tilix** | A tree of panes and dragging terminals to rearrange them |
| **Zellij** | An interface made of plugins; floating panels movable with the mouse |
| **WezTerm** | `wezterm cli` as an external control surface; detachable sessions |
| **Ghostty** | Separating core from interface so as to go to Mac and Windows without redoing the engine |
| **cmux** | **Understanding OSC 777 and 99 as well as our own contract** — they are standards that Kitty, rxvt and half a dozen tools already emit, so supporting them gives free compatibility with everything that notifies today, without asking anybody to adopt `panel.json`. And turning subagents into **native panes** instead of leaving them as hidden processes |
| **opencode** | The context and cost block; an indicator for a session busy somewhere else |
| **Conductor / Vibe Kanban** | Diff review before merging as part of the flow, not as a separate step |
| **Conductor** | **Reserving a range of ports per pane** (`CONDUCTOR_PORT` to `+9`) instead of detecting which port something came up on. It fits with `PANEL_ID`/`PANEL_DIR` from D7 and takes a deduction job away from the app, which is exactly what D6 forbids |
| **Cline Kanban** | Chains of dependencies between tasks: when one finishes the next starts on its own, with no script to write (see «Ideas with no phase» in [05-roadmap.md](05-roadmap.md)) |
| **Xirp** | **Notifying** on going to «waiting for input», not just painting it: it solves «which pane do I go to now» without having all four in view (see «Pending» in [04-contrato-panel.md](04-contrato-panel.md)) |
| **Orca** | A **status line hook** as a way for an agent that knows nothing about you to emit state without touching its code: you install it in its configuration and it paints the title. It works for any harness with a configurable status line, and it does not force you to maintain a wrapper per agent |
| **Herdr** | **Authority per source and a sequence number.** `--source custom:<name>` identifies each producer, gives it command over its own and releases it on leaving; `--seq` is a strictly increasing counter with which they **discard reports that arrive late**. Our `panel.json` is one file per pane where the last writer wins: with two producers on the same pane —an agent and the CI run it launched— or with out-of-order writes, there is no way to know which one holds. **It is the most concrete gap this landscape finds in the contract.** And the second one: **accumulating the state upwards** (agent → pane → tab → workspace, with `blocked` beating everything), which is what makes their sidebar answer «which pane do I go to» better than ours |
| **Superset** | A built-in browser with **port detection per pane** — and the producer already knows which port it brought its server up on, so it can publish it. Agents launched on cron: a `panel.json` with a scheduled producer asks nothing new of the contract. And **telling two waits apart**: a permission granted in a second (`PermissionRequest`) is not a question that demands thinking (`PendingQuestion`); our contract has a single `wait` |
