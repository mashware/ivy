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

| Tool | What it does | Served |
|---|---|---|
| `panel_set` | Publish its pane's state | Yes, since F3's third PR |
| `split_open` | Open a new pane | Not yet — planned for F3's fourth PR |
| `split_read` | Read a pane's output | Not yet — planned for F3's fourth PR |
| `split_close` | Close it | Not yet — planned for F3's fourth PR |

The three that are not served are **not advertised either**: the server lists `panel_set` and nothing
else, and asking it for one of the others is refused by name. They are part of the same frozen wire
format and arrive with the scoping that makes them safe to answer; advertising one before then would
be a promise taken back later.

**The transport is a per-pane endpoint inside that pane's `0700` directory** — a Unix socket on
POSIX; on Windows, a named pipe that is **not bound until its security descriptor can be asserted**,
which is F2.7's. It carries newline-delimited JSON, and the MCP vocabulary is a binary that speaks it
rather than the channel itself.

**Security rule:** an agent reaches **its own pane** only. The rule is kept by a mechanism and not by
a check: **the pane a call acts on is the endpoint the connection opened**, and no message carries a
pane's identifier at all — so reaching somebody else's pane is not refused, it is unsayable. See the
Security table below for why a check would have been worth nothing.

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
| The panel's `actions` run commands in the shell | **The literal command is painted under its label and the click is the confirmation**, so what is on screen is what runs, character for character. `send` is plain text with no interpolation and no templates; every control character, line break, tab and zero-width scalar is **refused when the document is read, never cleaned** — cleaning would make the painted text stop being the sent text, which is the whole of this mitigation. A button that is not fully on screen is not pressable, and a press is refused on the frame the document changed |
| A `panel.json` of untrusted origin (cloned repo, container) | **The document's origin is unknowable and the application does not pretend otherwise.** `PANEL_DIR` is created by the application, `0700`, under the user's runtime directory and with a name drawn at random, so whoever writes there is a process of the user's own uid — the same trust level as the shell in the pane. There is nothing to test, and deducing an origin would be **D6**. What is enforceable is the premise: **the application never reads a panel document from a path it did not create** — no flag, no setting and no convenience that points it at a file somebody names. That one-line feature would be the whole of the breach |
| An agent writing into somebody else's pane | **The pane is the endpoint the connection opened, and the wire format carries no pane identifier for anybody to check.** The earlier wording here said the server «scopes by `PANEL_ID`», and that was not a mitigation: `PANEL_ID` is put in the child's environment, so every descendant of a pane inherits it and any process of the same uid reads it out of `/proc/<pid>/environ` — it is a name, not a credential, and it is eight hex digits drawn for collision avoidance rather than unguessability. What does the work is the directory's `0700` and the fact that there is nothing in a message to trust. **Through the file there is no scoping either**: whoever can write in the directory is the producer (D17) |
| Two producers publishing to one pane | **The first connection to publish holds the pane under a source it declares; while it does, the file is not read at all and a second source is refused by name.** The hold dies with the connection, so an agent that crashes does not freeze that panel for the life of the window. **D43** |
| Reading the buffer and sending it to a model | It exfiltrates whatever is on screen: tokens, keys, API responses. Choosing what gets sent must be explicit, never automatic |
| Setting `ANTHROPIC_API_KEY` when launching a panel | Forbidden: it would change the user's billing (see D4) |
| Secrets in the panel | The panel is persisted text; it is no place for credentials |
