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
| `panel_get` | Read its pane's last committed document | Yes |
| `panel_patch` | Change addressed parts of the document it holds | Yes |
| `panel_retire` | Intentionally end the session state this connection owns | Yes, D50 |
| `split_open` | Open a new pane | Yes, since F3's fourth PR |
| `split_read` | Read a pane's output | Yes, since F3's fourth PR |
| `split_close` | Close it | Yes, since F3's fourth PR |

All seven are advertised because all seven are answered. The three `split_*` act on **the panes this
connection opened**, named by an **opaque handle** the window hands back — sixteen random bytes,
scoped to (the pane whose channel asked, the connection) — and **the person's first byte into one of
them takes it back**: from then on the handle answers with the reason and never with the buffer.
**The revocation is an application boundary and not a confidentiality one** (F10): the process
`split_open` launched keeps reading what the person types after that, and the promise an agent author
is given is «the app stops handing you that buffer; the program you opened does not».

**The transport is a per-pane endpoint inside that pane's `0700` directory** — a Unix socket on
POSIX; on Windows, a named pipe that is **not bound until its security descriptor can be asserted**,
which is F2.7's. It carries newline-delimited JSON, and the MCP vocabulary is a binary that speaks it
rather than the channel itself.

**Security rule:** an agent reaches **its own pane**, and the consoles it opened itself, and no other.
For `panel_set` the rule is kept by a mechanism and not by a check: **the pane a call acts on is the
endpoint the connection opened**, and no message carries a pane's identifier at all. For the three
`split_*` verbs there *is* something to say — a console other than the caller's — and what is said is
an **opaque handle scoped to the connection that opened it** (D44), so reaching a console somebody
else opened is not refused, it is unsayable. See the Security table below for why a check on
`PANEL_ID` would have been worth nothing.

`panel_retire` is the explicit end of that connection's session (**D50**). It is accepted only from
the connection currently holding the panel. In one logical window transition it removes the
document —and therefore its actions and attention—, releases the hold, detaches auxiliary consoles
that connection still controls and forgets all its handles. A console the person took over stays.
The answer is sent after the panel and tree commit and before the detached pseudoterminals are
destroyed through ordinary pane and PTY destruction.

Retirement does **not** delete `panel.json`: the file cannot prove who produced it. The live pane
keeps a bounded snapshot of the file's metadata and bytes at retirement, so that unchanged file
cannot resurrect; a later valid file whose observed snapshot differs publishes normally. Closing a
pane already removes its private directory, and restoring creates a fresh identity, so the marker
does not belong in the saved layout. Connection loss is deliberately different: a crash, EOF or
accidental disconnect releases the hold but preserves the last document and every console.
Compaction is not retirement.

This repository supplies no lifecycle adapter. A harness may claim automatic clear-session support
only after a real integration run proves its explicit lifecycle event reaches `panel_retire` through
the already-connected server. Unsupported harnesses remain unsupported rather than being emulated
with terminal parsing or a fresh connection. The control endpoint, including retirement, remains
unavailable on Windows until F2.7 supplies and tests the named pipe security boundary.

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
| A link in a panel or in a console's output opens an address | **Nothing opens on its own, and nothing reaches a shell.** A panel's `line.link` opens on a click on its label; a console's plain address or OSC 8 hyperlink opens on `Ctrl`+click; hovering shows the exact address before the click — over a panel label, and over a console link while `Ctrl` is held — because an OSC 8 label may say anything and that is where its destination becomes visible. **One boundary accepts only complete `http` and `https` addresses with a host**; every other scheme, a relative path, whitespace, a control or an invisible formatting character is inert. What is kept, shown and opened is the parser's form of the address — the form the browser will load — with the characters a command interpreter reads percent-encoded, and it goes to the desktop's own opener: never to the pane's PTY, never built into a command by this application, never fetched by it. **The opener itself does build one under WSL** (`cmd.exe /c start`, escaping only `^` and `&`), which is what that encoding is for. When the opener reports it could not start anything, one line says so on standard error. And the click opens what was **painted**, not what the producer or the program says by the time the click is routed. **D51** |
| A `panel.json` of untrusted origin (cloned repo, container) | **The document's origin is unknowable and the application does not pretend otherwise.** `PANEL_DIR` is created by the application, `0700`, under the user's runtime directory and with a name drawn at random, so whoever writes there is a process of the user's own uid — the same trust level as the shell in the pane. There is nothing to test, and deducing an origin would be **D6**. What is enforceable is the premise: **the application never reads a panel document from a path it did not create** — no flag, no setting and no convenience that points it at a file somebody names. That one-line feature would be the whole of the breach |
| An agent writing into somebody else's pane | **The pane is the endpoint the connection opened, and `panel_set` carries no pane identifier for anybody to check.** The earlier wording here said the server «scopes by `PANEL_ID`», and that was not a mitigation: `PANEL_ID` is put in the child's environment, so every descendant of a pane inherits it and any process of the same uid reads it out of `/proc/<pid>/environ` — it is a name, not a credential, and it is eight hex digits drawn for collision avoidance rather than unguessability. What does the work is the directory's `0700` and the fact that there is nothing in a message to trust. **Through the file there is no scoping either**: whoever can write in the directory is the producer (D17). The three `split_*` verbs do name a console, and what they name it with is an **opaque per-connection handle** — sixteen random bytes, checked against the pair (origin pane, connection) — so a console the connection did not open cannot be said. **D44** |
| An agent reading a console the person then types into | **The handle is revoked by the person's own input, and the revocation is an application boundary rather than a confidentiality one** (F10). A keystroke or a paste, a wheel turn, a mouse report, and a held paste sent from its notice's button end it (a panel button's press does not: that is the application acting on the person's behalf). From that byte on, `split_read` and `split_close` answer with the reason, and the console's own row says it. **The program the agent opened keeps reading what the person types afterwards** — the promise is «the app stops handing you that buffer», and it is written that way wherever it is promised |
| Two producers publishing to one pane | **The first connection to publish holds the pane under a source it declares; while it does, the file is not read at all and a second source is refused by name.** The hold dies with the connection, so an agent that crashes does not freeze that panel for the life of the window. **D43** |
| A hook clearing another session's panel or consoles | **Only the connection currently holding the panel can send `panel_retire`.** A source name, `PANEL_ID`, `PANEL_DIR`, a command-hook process or an opaque console handle grants no retirement authority. The observed file snapshot is suppressed in application-owned state rather than deleted, because its producer is unknowable. **D50** |
| Reading the buffer and sending it to a model | It exfiltrates whatever is on screen: tokens, keys, API responses. Choosing what gets sent must be explicit, never automatic |
| Setting `ANTHROPIC_API_KEY` when launching a panel | Forbidden: it would change the user's billing (see D4) |
| Secrets in the panel | The panel is persisted text; it is no place for credentials |
