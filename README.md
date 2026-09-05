# agent-terminal

> **Code name.** Parked on purpose: the product has been redefined twice while this was being
> written, and a name for something that is still moving sounds wrong whatever it does. It gets
> picked up again with the prototype in front of us. Renaming is
> `mv ~/Mashware/agent-terminal ~/Mashware/<name>` plus a find and replace in `docs/`.
>
> The umbrella brand already exists: **Zabri** (`~/Mashware/zabri.io`). This would be one of its
> products, alongside `rule-engine`.

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
| **Also working** | `flow-panel` — the status panel and its contract, over WezTerm ([prototipos/](prototipos/)) |
| **Decided** | Stack, channel architecture, economic model, and twenty-eight decisions with their reasons ([docs/02-decisiones.md](docs/02-decisiones.md)) |
| **Next** | **F2.12** — the whole repository moves to English. It goes ahead of everything else, and the reason is written down in D27. Behind it: macOS and Windows, and the per-pane header ([docs/05-roadmap.md](docs/05-roadmap.md)) |

## Documentation

| Document | Contents |
|---|---|
| [01-vision.md](docs/01-vision.md) | What it is, who it is for, and what it is not |
| [02-decisiones.md](docs/02-decisiones.md) | Technical decisions with their reason and what was discarded |
| [03-arquitectura.md](docs/03-arquitectura.md) | The pieces and the three channels |
| [04-contrato-panel.md](docs/04-contrato-panel.md) | Specification of `panel.json` |
| [05-roadmap.md](docs/05-roadmap.md) | Phases, in order, with what is parked |
| [06-referencias.md](docs/06-referencias.md) | How each terminal is built, with data |
| [07-panorama.md](docs/07-panorama.md) | Who is in the space and what to steal from each |
| [09-glosario-de-prosa.md](docs/09-glosario-de-prosa.md) | The crib the English prose is written against |
