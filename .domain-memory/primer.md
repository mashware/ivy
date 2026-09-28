# Project primer

## What this is

An **agent-hosting terminal**: window → tabs → panes, where each pane is an independent console
**plus a status panel**. The panel is not written by the app: it is written by *whoever does the
work* (an agent, a CI run, a deployment, a script) through a file with a defined format
(`panel.json`). The app only paints it.

It solves the problem of working with several terminal agents in parallel without knowing, at a
glance, which one is working and which is waiting, what step each task is on, which one is running
low on context, how much it has spent, and what is blocked.

**What it is NOT:** it is not an agent (it does not reason and it does not call models), it is not a
multiplexer of remote sessions (that is tmux/Zellij), and it does not rewrite the terminal emulator
(that is consumed as a library).

## Users

- **A developer orchestrating several agents at once** (Claude Code, opencode, Codex, Gemini CLI) —
  needs the global view of the state of every task in flight.
- **A state producer, agent or not** — any process running inside a pane that wants to publish how
  it is going: a CI run, a deployment, a script of one's own.

## Main modules / domains

- **Terminal** — the emulator per pane. It exists because without a correct and fast console nothing
  else matters; the VT engine and the PTY are third-party (D2), and here they are only hosted.
- **Panel** — paints the state published by the producer. Its reason for being is that the
  information that **cannot** be deduced from outside (remaining context, cost, blockers) is worth
  more than the information that can, and only whoever does the work knows it.
- **The `panel.json` contract** — the open format that separates the app from any particular
  domain. It is what allows four panes with four different producers and one global summary of them
  all.
- **Channels (D5)** — three, deliberately: MCP (agent → app: open panes, publish state, read
  output), `panel.json` (producer → app) and the PTY (app → agent: buttons and everything that does
  not speak MCP). The PTY is not the main mechanism, it is the universal fallback.
- **Identity per pane (D7)** — `PANEL_ID` / `PANEL_DIR` injected when each shell is launched. It is
  what unties the contract from any particular tool and, at the same time, MCP's security boundary.

## Invariants / constraints

- **The app never sets `ANTHROPIC_API_KEY`** or another provider's equivalent when launching a
  panel: it would take precedence over the subscription session and would change the user's billing
  without them noticing (D4).
- **The app does not deduce state.** It does not parse a TUI's painting or third-party files to
  infer how a task is going. Only whoever does the work knows that semantics — deducing it already
  produced three mistakes in a row in the prototype (D6).
- **Agent control goes through the console, not through an API.** Subscriptions are only worth
  anything inside each provider's official client; making somebody pay for the API on top of what
  they already pay kills adoption (D4).
- The panel's `actions` are **code execution** in the user's shell: the command is visible before it
  runs and the text sent is plain, with no interpolation and no templates.
- A `panel.json` of **foreign origin** (a cloned repo, a container) is untrusted: never run anything
  from it without visible confirmation.
- Reading the buffer and sending it to a model **exfiltrates** whatever is on screen (tokens, keys,
  API responses): what gets sent is chosen explicitly, never automatically.
- **MCP is scoped by `PANEL_ID`**: an agent reaches its own pane and no other.
- **Genuinely cross-platform** (Linux, macOS, Windows). This rules out at the root any Linux-only
  solution, however convenient (D1).
- **It works without the panel:** if nobody publishes anything, it is still a correct terminal.
- Every phase of the roadmap stands on its own even if the next one is abandoned; what a phase
  leaves out is out on purpose.

## External stakeholders / integrations

- **MCP (Model Context Protocol)** — the agent→app channel. Chosen because Claude Code, opencode,
  Gemini CLI and Codex all speak it: the server is written once and serves them all, and it **does
  not consume a separate API budget** (it is added to the session the agent already has through its
  subscription).
- **`alacritty_terminal`** — VT parsing and terminal state, as a library.
- **`portable-pty`** — the PTY, abstracting over Unix and Windows's ConPTY.

## Glossary

- **Pane** — an independent console inside a tab: terminal + panel. It is the unit of identity
  (`PANEL_ID`) and MCP's security boundary. **`split` is the gesture that creates one**, never the
  thing itself.
- **Panel** — a pane's state view. Never written by the app.
- **Producer** — whoever publishes the state in `panel.json`: an agent, a CI run, a deployment, a
  script. The app does not know which and does not care.
- **Open contract** — that the panel's format belongs to no particular product, unlike opencode's
  panel, which only knows about opencode.
- **Stale** — a summary whose `updated_at` has aged. A summary written only when a step finishes
  lies if the process dies halfway; the timestamp gives it away (D6).

## Stack

Rust 1.97.1, pinned exactly rather than as a range because the numbers this project publishes are
only comparable with each other if the compiler is the same. **GPUI** for the product and **egui**
for prototyping (D3); Electron/Tauri + xterm.js discarded for its ceiling with many panes. The live
decisions are in `docs/02-decisions.md` (D1–D28) and the phased plan in `docs/05-roadmap.md`: Part
I defines what belongs to each phase and Part II how it is built.
