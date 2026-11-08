# Ivy

<img src="resources/branding/ivy.svg" alt="Ivy logo" width="128" />

**Ivy** is the public product name of this terminal ([#339](https://github.com/mashware/agent-terminal/issues/339)).
The repository and configuration directory retain `agent-terminal` so existing settings and saved
layouts keep working. The [identity assets](resources/branding/) contain the approved logo.

An **agent-hosting terminal**: tabs, panes, and in each pane a console with its own status panel
fed by an open contract.

## The problem

Working with several agents at once (Claude Code, opencode, Codex, Gemini CLI) means having N
consoles open today and no way to tell, without looking at them one by one, which is making
progress, which is waiting for you, and which has run out of context.

Every agent has started solving this on its own inside its own text interface — opencode already
has a sidebar with plan, context and cost, and open requests for tabs and multiple windows. That
is: **every agent is reimplementing a window manager, worse**, because the host that should be
doing that job does not exist.

## The proposal

The host provides the structure — tabs, panes, panels — and defines **an open contract** so that
anyone can publish state into their pane's panel: an agent, a CI run, a deployment, a script.
Every agent goes back to being just an agent.

## State

| | |
|---|---|
| **Working** | `cargo run -p app --release` opens a terminal that gets used: tabs, panes placed where you want them, a shell inside, colours, `vim`, keyboard, resizing and large dumps without freezing the window. Each pane paints the status panel its producer publishes, is dragged and dropped with the mouse, remembers where you left it, says when the work there is claiming you, and lets you scroll back, select and copy. **Fifteen milestones closed**: F1.0 to F1.6, F2.1 to F2.6, F2.8 and F2.9 |
| **Also working** | `flow-panel` — the status panel and its contract, over WezTerm ([prototypes/](prototypes/)) |
| **Decided** | Stack, channel architecture, economic model, and twenty-eight decisions with their reasons ([docs/02-decisions.md](docs/02-decisions.md)) |
| **Next** | **F2.12** — the whole repository moves to English. It goes ahead of everything else, and the reason is written down in D27. Behind it: macOS and Windows, and the per-pane header ([docs/05-roadmap.md](docs/05-roadmap.md)) |

## Documentation

| Document | Contents |
|---|---|
| [01-vision.md](docs/01-vision.md) | What it is, who it is for, and what it is not |
| [02-decisions.md](docs/02-decisions.md) | Technical decisions with their reason and what was discarded |
| [03-architecture.md](docs/03-architecture.md) | The pieces and the three channels |
| [04-panel-contract.md](docs/04-panel-contract.md) | Specification of `panel.json` |
| [05-roadmap.md](docs/05-roadmap.md) | Phases, in order, with what is parked |
| [06-references.md](docs/06-references.md) | How each terminal is built, with data |
| [07-landscape.md](docs/07-landscape.md) | Who is in the space and what to steal from each |
| [08-configuration.md](docs/08-configuration.md) | The settings file: its keys, and what happens when one is wrong |
| [09-prose-glossary.md](docs/09-prose-glossary.md) | The crib the English prose is written against |
