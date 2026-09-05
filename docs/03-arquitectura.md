# Architecture

## The pieces

```
┌─ Window ───────────────────────────────────────────────────┐
│  Tabs                                                       │
│  ┌─ Pane ────────────────────┬─ Pane ──────────────────────┐│
│  │  Terminal    │  Panel     │  Terminal   │  Panel       │ │
│  │  (PTY)       │            │  (PTY)      │              │ │
│  │  PANEL_ID=a3f2           │  PANEL_ID=b7c1              │ │
│  └───────────────────────────┴────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
         ▲                ▲                    ▲
         │ PTY            │ reads panel.json   │ MCP
         │                │                    │
   buttons/keyboard  $PANEL_DIR/panel.json    MCP server
                            ▲
                            │ writes
                     the agent, flow, a CI run, a script…
```

## The three channels

### 1. State — the panel document (producer → app)

Published by whoever does the work. **The source of truth is the state the app holds per pane**,
and the transports feed it ([02-decisiones.md](02-decisiones.md) → D17):

| Transport | State | Who it is for |
|---|---|---|
| **File** — `$PANEL_DIR/panel.json`, overwritten whole, watched by the app | working | everybody: the barrier to entry is zero |
| **MCP** — `panel_set` hands the document over without writing the file | Phase 3 | the agent that also drives the app |
| **OSC over the PTY** | candidate, after F2.7 | whoever is behind an `ssh` or in a container with no volume |

Full specification: [04-contrato-panel.md](04-contrato-panel.md).

**Why the file is the default one:** it is the only one where the dumbest possible producer publishes
without writing code — `cat >` from a script — and the only one where the state survives the producer
dying without anybody programming anything. It is also read with `cat` and crosses a mounted volume
with no protocol of its own. **What it does not give:** it does not know who writes, and it does not
tell whoever published that their document was broken.

### 2. Control — MCP (agent → app)

An MCP server exposing tools to the agent running in the pane:

| Tool | What it does |
|---|---|
| `panel_set` | Publish its pane's state |
| `split_open` | Open a new pane |
| `split_read` | Read a pane's output |
| `split_close` | Close it |

**Security rule:** by default, an agent reaches **its own pane** only (the one of its `PANEL_ID`).
Widening that reach is an explicit decision by the user, not the default behaviour.

### 3. Input — PTY (app → agent)

The panel's buttons write into their pane's PTY. It is the universal fallback: it works with any
program, whether it speaks MCP or not.

**Accepted risk:** a file that declares commands and a button that runs them is code execution in the
user's shell. See Security, below.

---

## Reading the output

For whatever does not speak MCP, the screen has to be read. The piece that makes that viable is
**shell marks (OSC 133)**: they delimit where the prompt starts, where the command does, and where
its output does. Without them the app sees only a river of text; with them it can bound «the last
command's output».

They are switched on by loading the shell's integration script (WezTerm already ships one; in the
app's own it will be equivalent).

**What must not be done:** parsing the painting of a text interface to deduce state. It breaks with
every version of the agent. That is what channel 1 is for.

---

## Security

| Risk | Mitigation |
|---|---|
| The panel's `actions` run commands in the shell | The command must be visible before it runs; `send` is plain text with no interpolation and no templates |
| A `panel.json` of untrusted origin (cloned repo, container) | Treat the file as untrusted; never run anything from it without visible confirmation |
| An agent writing into somebody else's pane | The MCP server scopes by `PANEL_ID`. **Through the file there is no scoping**: whoever can write in the directory is the producer (D17) |
| Reading the buffer and sending it to a model | It exfiltrates whatever is on screen: tokens, keys, API responses. Choosing what gets sent must be explicit, never automatic |
| Setting `ANTHROPIC_API_KEY` when launching a panel | Forbidden: it would change the user's billing (see D4) |
| Secrets in the panel | The panel is persisted text; it is no place for credentials |
