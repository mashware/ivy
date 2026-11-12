# Contract — the panel document

Current state: **implemented and in use in the application**, and also in `prototypes/flow-panel`
with `flow` as its first producer. What is here is the specification as it works today, plus what is
left pending at the end.

**This document is split in two on purpose** ([02-decisions.md](02-decisions.md) → D17):

| | |
|---|---|
| **Part I — The document** | What gets published. It is the contract: what gets versioned and what nobody may break |
| **Part II — The transports** | How it arrives. Today a file; tomorrow it may arrive by more than one |

The second is not the definition of the first. A `panel.json` is **one transport** of the document,
not the document. The source of truth is the state the application holds per pane, and the
transports feed it.

---

# Part I — The document

## Shape

```json
{
  "v": 1,
  "updated_at": "2026-08-06T18:12:00+02:00",
  "phase": "validate",
  "attention": "wait",
  "header": true,
  "stale_after_minutes": 30,
  "actions": [
    {"label": "Run the tests", "send": "cargo test --workspace"},
    {"label": "Continue", "send": "yes"}
  ],
  "lines": [
    "a loose line",
    {"text": "the two batch-read sources", "mark": "done", "ref": "#1",
     "link": "https://gitlab.com/…/9977"},
    {"text": "the mapping to the channel's shape", "mark": "current", "ref": "#3"},
    {"text": "35 392 tokens · 18% · 0.62 €", "mark": "info", "ref": "Context"},
    "",
    {"text": "mailsuite-mcp is waiting for the contract", "mark": "block"}
  ]
}
```

## Fields

| Field | Required | What it does |
|---|---|---|
| `v` | recommended | Contract version |
| `updated_at` | recommended | ISO 8601. **It is read and not painted yet** —see Pending— and **the stale notice does not come from here** (see rules) |
| `phase` | no | What is being run **right now**. Free text, and painted literally — **inside the identity line**, so `header: false` hides it too |
| `attention` | no | **Whether this pane claims something, and of what kind.** `wait` `block` `done`. Absent = it claims nothing. See below |
| `header` | no (`true` by default) | Whether the app adds its identity line at the top: the pane's identifier and its `phase` |
| `stale_after_minutes` | no (30 by default) | How much silence is legitimate before warning |
| `actions` | no | **The buttons this pane offers.** Each one is painted with its command underneath, and one click sends it to the program in that pane. See below |
| `lines` | **yes** | The body |

### `attention`

The only field the application **interprets**, and it is bounded to the only question it needs to
answer in order to navigate: *do I have to go there?* The complete decision, with what was discarded,
in [02-decisions.md](02-decisions.md) → D18.

| Value | What it says | Example |
|---|---|---|
| `wait` | **it is waiting for you**: a question, a permission, a decision | «do I approve the design?» |
| `block` | stuck on something else, not you | «waiting for another service's contract to exist» |
| `done` | it finished and nobody looked | the cycle ended and the diff is still unreviewed |

- **It accumulates upwards**: the tab shows whatever claims most among its panes, with `wait` beating
  `block` and `block` beating `done`. `wait` goes first because it is the only thing that gets
  unstuck with you in front of it right now.
- **Absent is absent.** Without this field the pane is not marked, and the app does **not** deduce it
  from the body, nor from `phase`, nor from whether the console writes. A line with `mark: block` is
  content of the summary, not a signal: an «I waited for approval on Tuesday» would mark the tab
  forever.
- **How it looks**, so that you know what you are asking for: in the pane's panel a row of the
  application appears —`? it is waiting for you`, `⊘ stuck on something else, not you`,
  `∎ it finished and nobody looked`—, **above the body and also with `header: false`**, because this
  signal is not one the producer hides. And on its tab's label **only the symbol** appears.
- **And if you stop publishing, the claim is seen dimmed**: it is still there, and it says that
  nobody is holding it. With one exception: `done` does **not** get dimmed by falling silent, because
  falling silent is what whoever finished does (D18).
- **It does not say what the program is doing.** For «working» or «idle» there is no field and there
  is not going to be one: that is either deduced by the app or declared by a harness that does not
  know the question ([02-decisions.md](02-decisions.md) → D6).

### `actions`

**The second field the application acts on**, after `attention`, and the only shape in this contract
that reaches a shell. Each entry is an object with two strings:

```json
{"label": "Run the tests", "send": "cargo test --workspace"}
```

- **`label`** is what the button is called.
- **`send`** is what the program receives. It is painted **under the label, literally**, and one
  click sends it — so what is on screen is what runs, character for character.

**The application appends the terminator itself.** A `send` cannot carry a line break (see below), so
a button always executes: there is no way to declare one that only writes the text without running
it, and no flag to get wrong. If that second kind is ever wanted it comes in as a new field, which
breaks no producer.

#### What is refused, and it is refused rather than repaired

A `send` or a `label` carrying any of these makes **that button vanish** — the rest of the document,
its other buttons included, is read as normal:

| | |
|---|---|
| any control character (`U+0000`–`U+001F`, `U+007F`–`U+009F`) | the kernel's line discipline acts on them **before the program sees them**: one interrupts the foreground process, one suspends the shell and leaves the pane looking dead, one closes the input |
| **a line break**, with the rest | a button is one command, not a document. One carrying a line break would run two commands while painting one |
| **a tab**, with the rest | in a line editor the tab is the completion key, so the same button would do something different depending on what is on disk |
| zero-width and direction-changing scalars (`U+200B`, `U+FEFF`, `U+00AD`, `U+202E`…) | they are painted as nothing, or paint the rest backwards, so the text a person reads is not the text that runs |

**Refused and never cleaned, and that is the load-bearing decision**: the painted command *is* the
confirmation this application requires before anything runs in a shell, so a filter that removed
characters on the way out would mean the bytes that ran are not the bytes that were read.

#### Ceilings

| | |
|---|---|
| **8** buttons | more are **truncated**, not rejected: a document with nine shows eight rather than losing its panel |
| **512** characters per `send` | a longer one makes that button vanish |

#### What a producer has to know about a `send`

- **It is painted on screen**, so a credential in one is in every screenshot of that window.
- **It is written into the shell**, so it lands in `~/.bash_history` or `~/.zsh_history` — outside
  anything this application controls, and surviving the pane. **It is no place for a credential**:
  pass one through the environment or a file the command reads.
- A button is **not pressable while it is not fully on screen.** In a short panel what does not fit
  is dropped from the bottom, and a label over a command nobody can read is the one state the rule
  about seeing a command before it runs forbids.
- A press is **refused on the frame the document changed**. The panel repaints on its own the moment
  somebody writes, so on that one frame what is under the pointer is not what the person read; the
  panel is drawn again and the press works on the next one.

### `lines`

Every element is a line. A loose string → normal style. An empty string → a gap. An object, with the
fields below. **An element that is neither a string nor an object —a number, a null— is discarded**,
it is not painted blank: an empty line where there was garbage would be inventing content.

| Field | What it does |
|---|---|
| `text` | The content. **It is cut to 512 characters when painted**: it is the only thing that bounds what laying out a single line costs, and one of 260 000 costs a whole frame all by itself |
| `mark` | `done` `current` `pending` `wait` `block` `info` — the app chooses the symbol, the color and the alignment |
| `ref` | Short label aligned in a column. **It does not have to be a number**: `#1`, `Now`, `Context`. **Short for real**: whatever goes past the width of its column is clipped, and the column cannot push the text out of the panel |
| `style` | `normal` `dim` `title` `accent` `ok` `warn` `error` — meant for lines without a `mark`, but **it can come alongside one**: then the mark sets the symbol and the style the color |
| `link` | **A web address this line points at.** It is painted as a compact label and one click on the label opens it in the desktop's browser. See below |
| `indent` | Manual indentation, in cells (inherited by the continuation lines). **It is bounded to 64** when read, and **when painted also against the panel's width**: an indentation of 40 is not honored whole in a panel of 36, because it would leave the text outside |
| `key` | The name **you** give this line, so it can be addressed later. **It is never painted**, it has no bound of its own — the document's ceiling holds it, as it holds every other string — and it is kept exactly as you wrote it, repeated keys included |

#### `link`

```json
{"text": "Review", "link": "https://example.test/panel"}
```

- **How it looks**: a short label beside the line —`!42 ↗` for a merge request, `#257 ↗` for a pull
  request or an issue, `pipeline 913 ↗`, otherwise the host and the address's last segment— or on a
  row of its own under the line when the line leaves no room for it. Hovering over the label shows
  **the whole address** before anybody clicks — in the form the browser will load, which can differ
  from how you spelled it: the host lowercased, an international host in punycode, `:443` dropped.
- **A plain click on the label opens it.** No modifier: the panel has no text selection competing
  for the button. The address goes to the desktop's own opener — **never to the pane's shell**, never
  fetched by this application. If the desktop's opener reports it could not start anything, one line
  on the terminal's standard error says why; nothing changes on screen. A browser that starts and
  then fails is past what the terminal can see.
- **Only complete `http` and `https` addresses with a host open.** A relative path, a missing host,
  whitespace, a control or invisible formatting character, `file:`, `mailto:` or any other scheme
  leaves the label painted and inert: no hand, no tooltip, and a click on it is an ordinary click.
- **The click opens what was painted.** If you publish a new document between the frame a person
  read and the frame their click lands in, the click opens the address under the pixels they saw,
  not the new one. A label the panel clipped, or one painted on the frame the document changed, is
  not clickable — the same rule as a button.
- The decision, with what was discarded, in [02-decisions.md](02-decisions.md) → D51.

**A line with no `key` is a line.** Nothing about publishing changes if you never write one, and
nothing about how a panel is painted changes if you do — the field exists for what a channel can say
about a document, not for what the document looks like. And **it is deliberately not `ref`**: that one
is painted, clipped to the width of its column, and legitimately repeated —`#1` twice in one panel is
ordinary— so an address built on it would change whenever the text did.

**Write sentences, not measured columns.** The app wraps to the width, aligns by blocks (separated by
empty lines) and clips to the height. Multiple spaces are collapsed: they are no good for aligning.

**And control characters never get painted**: a line break, a tab or an escape sequence inside `text`
or inside `ref` is replaced by a space. It is not censorship, it is that they knock the panel's grid
out of square — a `\n` would make a single line paint several rows on top of the ones below. Whoever
wants two lines, let them publish two elements in `lines`.

## Rules of the document

- **Whatever is not understood is ignored, field by field.** A badly written `stale_after_minutes`
  does not throw away the whole document: that field falls back to its default value and the rest is
  painted. Of the **content**, there are only two ways for a document not to be valid: that it is not
  JSON —which includes the half-written file and the one that is not UTF-8 text— or that it does not
  carry `lines`.
- **The body is free by design: there are no metric fields, and none get added.** Tokens, cost or
  remaining context are written as one more line —`{"text": "35 392 tokens · 18% · 0.62 €",
  "mark": "info", "ref": "Context"}`— never as fields of the format. The reason: the moment the
  contract names a metric, the app becomes the owner of what deserves to be seen, and the producer
  loses the only thing that makes it valuable here — being able to publish **whatever its task
  needs**, including whatever the user of that pane happens to ask its agent for, without waiting for
  a new version of the format. The app only understands marks, styles, alignment and `attention`; the
  vocabulary belongs to whoever does the work. The AI's metrics are a use case of the contract, not
  its reason for being ([07-landscape.md](07-landscape.md) → Conductor, which shows them and still
  does not have this panel).
- **`phase` is the text that gets shown**; whether the work is closed is decided by the real state,
  not by `phase`. A `phase` outside any known cycle (`watching`, `deploying`) is perfectly valid.
- **Publish *before* a long stretch, not after.** If the process dies halfway through, the document
  says what it was doing and the timestamp gives it away. A summary written only on finishing well
  lies by omission.
- **Stale notice**: if `stale_after_minutes` goes by without a refresh, the app flags it **in a line
  of its own inside the panel**, and that line does not depend on `header` — that flag hides the
  pane's identity, not the application's honesty. **No exceptions, and that is deliberate.** The
  prototype kept quiet when the work was closed, but to know that it had to read the phase of
  somebody else's `meta.json` and decide on its own what it means — exactly what D6 forbids. Whoever
  does not want the notice says so in their own document: a very high `stale_after_minutes` on
  publishing for the last time. **Where the time of the last publication comes from is defined by
  each transport**; for the file, see Part II.
- **And the app also says when it has not been able to read.** Publishing something that is not
  understood does not erase what came before, but neither does it go unnoticed: the panel shows the
  last good one with a line above it saying that this is not the latest and why. If there **never**
  was anything good, the panel is only that line. The one exception is that nothing has arrived yet,
  which is the normal state before the first publication: there there is neither panel nor notice,
  and the pane looks like any other console.

---

# Part II — The transports

Which one exists and why, with the five candidates compared and the three discarded, in
[02-decisions.md](02-decisions.md) → D17.

| Transport | State | Who it is for |
|---|---|---|
| **File** (`panel.json`) | **working** | Everybody. The bar to entry is zero: `cat >` from a script, a write from an agent |
| **MCP** (`panel_set`) | Phase 3 | The agent that also wants to pilot the app. It delivers the document **without writing the file** |
| **OSC over the PTY** | candidate, after F2.7 | Whoever is on the other side of an `ssh` or inside a container with no mounted volume |

**And every delivery brings a source and an order**, so that two producers on the same pane are not
decided by chance. **How they are expressed is undecided** — D17 leaves it open on purpose, because
with a single live transport it would be inventing the shape before having the second case in front
of us.

## Transport 1 — the file

**The default, and the one with no barrier to entry**: a shell script with `printf` publishes a
panel. [Transport 2](#transport-2--the-channel) is for the producer that already has a process
running inside the pane.

| Who reads | Where it looks |
|---|---|
| The application | **`$PANEL_DIR/panel.json`**, the directory it gives each console when it launches it |
| The prototype on WezTerm | `.claude/work/<trabajo>/panel.json`, by convention of `flow` |

The prototype discovers the file by convention; the app does it by pane identity (see
[02-decisions.md](02-decisions.md) → D7), since F1.5 for the directory and F1.6 for the reading.
**The document is the same**, only where it is looked for changes.

### Rules of this transport

- **It overwrites the whole file** on every write. It does not accumulate.
- **Whoever can, writes to a temporary file in the same directory and renames it on top.** That way
  the app never reads a half-written file. **It is a recommendation, not a requirement**: the first
  producer of this is an agent that overwrites the file and cannot do anything else, and a
  requirement its first producer cannot meet is a dead requirement. The app **does not break** on a
  partial read: it keeps the last good panel **and says in the panel itself that what is seen is not
  the latest**, so it does not flicker but neither does it pretend. The recommendation is still worth
  it for whoever has code of their own —a CI, a script, an MCP server—: **it spares their panel those
  notices**, which are correct but say nothing good about whoever publishes.
- **Publish in one go.** Since F1.6 the app finds out that you have published through the file
  system's notifications, and **every separate write is a notification**: a producer that builds its
  `panel.json` in pieces —the natural thing with `>>` in a shell loop— makes the app read it
  half-written over and over. **Measured**: about **42 reads and as many frames per publication**,
  against **one** if it is written in one go or with the temporary-and-rename above. And what is seen
  meanwhile is the «this is not the latest» notice appearing and disappearing with every piece. **The
  waste of frames is the invisible part**, and that is why it is written here: nobody is going to
  discover it by looking.
- **A file of more than 256 KiB is not read whole.** The ceiling bounds what a file from outside can
  make get reserved when it is interpreted, and along the way how many lines and of what length it
  makes sense to publish. **A warning for whoever publishes a lot, with the measured figures**: a
  file right at the ceiling costs around 2.8 ms and 1.8 MiB of peak if its lines look like those of a
  real panel, but **22 ms and 24 MiB** —a frame and a half— if they are thirty thousand tiny objects.
  The normal case does not come close: a `panel.json` like the ones written today is read in **10
  µs**. Publishing close to the ceiling shows; publishing many short lines shows twice as much.
- **Four ways for it not to get read**, apart from the two of the document: that it does not exist
  yet; that what is there **is not a regular file** —a directory, a named pipe: it is rejected
  without being opened, because opening it would freeze the window—; that it **cannot be opened or
  read**, which in practice is a matter of permissions; or that it goes over the size ceiling, and
  that one is read up to the ceiling before being rejected.
- **The stale notice is measured by the file's date**, not by `updated_at`. Since the producer
  overwrites the whole file, that date *is* the one of its last publication — and this way there is a
  notice even when the producer does not write its mark, which is exactly when it is most needed.
  `updated_at` is painted as what it is: what the producer says.

### What this transport does not give

- **It does not know who writes.** Anyone who can write in the directory *is* the producer.
- **It does not tell the producer that their document is broken.** The error is seen in the user's
  panel, not by whoever publishes — and that is what makes writing a new producer expensive, which is
  exactly what this project needs to be cheap.
- **It cannot be read back.** There is nothing to ask and nobody to answer: a file is written, not
  spoken to. A producer that wants to know what is on its panel — one that restarted, one that lost
  the document it published — needs the channel, whose `panel_get` exists for exactly that. This is
  deliberate rather than pending: the whole worth of this transport is that its barrier to entry is
  zero, and a question needs a connection that can answer.

---

## Transport 2 — the channel

**For the producer that already has a process inside the pane and would rather not own a file.** It
publishes the **same document**, through the same parser, with the same ceiling and the same
per-field tolerance: everything above this line is unchanged, and the one thing this section adds
is that **a channel has somebody to tell** what that tolerance dropped — see
[*What fell away is said*](#what-fell-away-is-said) below.

| Who reads | Where it listens |
|---|---|
| The application | **`$PANEL_DIR/control.sock`** on POSIX; on Windows, a named pipe that is **not bound yet** — see below |

**What travels is newline-delimited JSON, one request per line:**

```
→ {"tool":"panel_set","source":"flow","document":{ ...the panel document... }}
← {"ok":true}
← {"ok":true,"dropped":"4 lines carry an unknown mark \"doing\""}
→ {"tool":"panel_get"}
← {"ok":true,"document":{ ...the panel document... }}
→ {"tool":"panel_patch","ops":[ ...the edits... ]}
← {"ok":true}
← {"ok":true,"dropped":"1 line carries an unknown style \"bold\""}
→ {"tool":"panel_retire"}
← {"ok":true,"closed":2,"preserved":1}
→ {"tool":"split_open","side":"right","program":"cargo","args":["test"]}
← {"ok":true,"pane":"<opaque handle>"}
→ {"tool":"split_read","pane":"<opaque handle>","scrollback":false}
← {"ok":true,"text":"..."}
→ {"tool":"split_close","pane":"<opaque handle>"}
← {"ok":true}
← {"ok":false,"error":"..."}
```

### What fell away is said

**A publication or a patch that lost part of what it carried on the way in is still taken, and its
`ok` says what was lost.** The reading does not change — a value none of the three closed vocabularies
knows is dropped, the line stands, the panel paints — but over the channel there is somebody on the
other end, so the reply carries one more field, `dropped`, with a sentence naming it (**D58**):

```
← {"ok":true,"dropped":"the document carries an unknown attention \"waiting\"; 1 line carries an unknown mark \"doing\"; 2 lines carry unknown styles \"bold\" and \"red\""}
```

- **Three fields are reported, and only three**: `attention`, a line's `mark` and a line's `style`.
  Always in that order, joined by `; `. A field that lost nothing is not mentioned; a document that lost
  nothing gets the plain `{"ok":true}` it always got.
- **A value that is not a word counts as well** — `"mark": 42` is reported as `42`, without quotes, so
  a word and a number can be told apart. **`null` is not a loss**: it reads exactly like leaving the
  field out.
- **The count is of lines; the values are distinct and at most three**, in the order they arrived —
  `5 lines carry unknown marks "a", "b", "c" and others`. A value is shown up to 32 characters and cut
  with `…` after that; two values that agree up to the cut are named as one, and each of their
  lines still counts. Both bounds keep the
  reply under the client's own ceiling: every value is escaped twice on its way back, and a document
  of garbage at the ceiling would otherwise produce a reply the client has to refuse.
- **Only a taken request says it.** A refused publication or patch was not applied, so the refusal is
  the whole answer and carries no `dropped`. One refusal is not quite that: *the window did not answer
  in time* means the window may still take the document afterwards, and if it does, what that document
  lost is never said. Reading it back with `panel_get` shows what was understood.
- **The file says nothing**, as it never did: a `panel.json` on disk has nobody to answer, which is
  exactly the difference the two transports have always had (**D17**).

**Nothing else is reported.** Everything else the reader drops or clips goes on falling silently, over
this transport as over the file: an unknown field, any other known field whose value has the wrong
type (a `text` that is a number paints an empty line), an indentation past 64, a button it will not send, an
element of `lines` that is neither text nor an object. A bare `{"ok":true}` means none of the three
vocabularies lost a value — not that every byte was understood.

### `panel_get` — reading the document back

**It hands back the document this pane's panel is showing, and it takes nothing.** No arguments, no
source, no hold: the connection's own endpoint already decided which pane this is, and there is no
word here for asking about another.

It is for the producer that has **not** claimed the pane — one that restarted, one whose context was
compacted away, a second tool taking the console over — so gating it on a hold would take away the
only reason it exists. Three things follow, and they are the whole of it:

- **It answers with a document that arrived by file just as readily as with one published here.** The
  source of truth is what the application holds for that pane, not the file (**D17**), and that is
  where this reads from.
- **It carries nothing of the application's own.** The identity row, the claim indicator, the notices
  about a stale panel or a refused file are painted beside your document and are not part of it. What
  comes back is yours.
- **Reading changes nothing.** It takes no hold, so it does not stop the file being read, it does not
  refuse anybody else, and a panel driven by its `panel.json` goes on being driven by it.

**A read does not wait for the window to paint or run an interface pass.** The channel answers from
the last document the application committed for that pane, so `panel_get` remains available while
the window is minimised or occluded. Serialising the answer and writing it to the connection happen
outside the frame.

That committed boundary is deliberate. A `panel_set` or `panel_patch` merely waiting in the
window's bounded queue is not visible yet: until the interface thread accepts it, `panel_get`
returns the previous committed document. Once accepted, the complete new document replaces the old
one in a single step; a concurrent read gets one whole version or the other, never a mixture.

**Closing does not wait for a read already under way.** A read that upgraded this pane's weak
capability before closure may finish and answer successfully; one that starts after closure gets
`{"ok":false,"error":"this pane is gone"}`. The upgrade admits the read but does not choose its
version: the later pointer clone selects one complete committed document. Neither step keeps the pane
itself alive or delays its closing.

**What comes back is not byte-for-byte what you sent, and it is not meant to be.** The document is
read tolerantly — an unknown field is dropped, an indentation past 64 is clipped, a word none of the
vocabularies knows falls away — so what you get is the document **as the application understands it**.
Two consequences to know before you rely on it: a field the application did not understand is not
there, and a field you wrote at its default (`"header": true`) comes back absent, because once it is
read there is no difference between *not said* and *said the default*. What **is** guaranteed is that
publishing back what you received leaves the panel exactly as it is: read, edit, publish is stable.

A pane where nothing has been published yet answers
`{"ok":false,"error":"nothing has been published on this console yet"}`.

### `panel_retire` — intentionally ending this connection's session

**It removes what `/clear` ended, and only what this connection still controls.** It takes no
arguments: the endpoint already addresses the origin pane and the live connection is the authority.
The request is accepted only when that connection holds the panel. A fresh connection is refused,
including one with the same source or `PANEL_ID`.

One successful transition does all of this before it answers:

- removes the committed document, so its actions and attention disappear with it;
- releases the producer hold;
- detaches every auxiliary console still owned by this `(origin pane, connection)`;
- preserves every auxiliary console the person has taken over; and
- forgets every handle issued to the retired connection, including those for preserved consoles.

The reply carries the result it committed:

```json
{"ok":true,"closed":2,"preserved":1}
```

`closed` counts untouched auxiliary consoles detached for destruction. `preserved` counts
user-taken consoles kept open. The panel and tree have already changed when this answer is sent;
the detached pseudoterminals are destroyed afterwards through ordinary pane and PTY destruction, so their
shutdown delay cannot make a committed retirement look like a timeout.

**Retirement never deletes or rewrites `panel.json`.** The file cannot identify its producer and may
contain somebody else's publication. Instead the live pane keeps a bounded snapshot of the file's
metadata and bytes at retirement. That unchanged file stays suppressed while the pane lives; a
later valid file whose observed snapshot differs publishes normally even when its timestamp is
identical. Pane closure removes the private directory and restoration creates a fresh pane identity,
so this marker is not layout state.

A same-connection retry before any later MCP or file publication returns the original counts and
changes nothing again. Until `panel_set` begins the next generation, that retired connection cannot
open another auxiliary console; its former handles have already been forgotten. Any later
publication invalidates that retired generation. `panel_retire` is an ordering barrier: a later
request on that connection is not applied before the window has committed or refused it, even when
the caller already timed out waiting for the answer. `/compact` never
sends this request: it is the same session, and `panel_get` is how it recovers its document. A crash,
EOF or accidental disconnect sends no retirement either; ordinary connection close keeps the last
document and every auxiliary console as evidence (**D43**, **D44**, **D50**).

### `panel_patch` — changing part of it without republishing the rest

**It edits the document you published, one line or one field at a time**, and it needs the pane
claimed: publish once with `panel_set`, then patch it for as long as your connection lives.

```
→ {"tool":"panel_patch","ops":[
     {"op":"after","key":"mr-1","line":{"key":"mr-2","text":"per-event counters","mark":"current"}},
     {"op":"delete","key":"title"},
     {"op":"set","field":"attention","value":"done"}]}
← {"ok":true}
```

**The seven ops**, and `K` is a `key` some line of your document carries, `L` a line exactly as you
would write it inside `lines`:

| Op | What it does |
|---|---|
| `{"op":"replace","key":K,"line":L}` | The line carrying `K` becomes `L` |
| `{"op":"before","key":K,"line":L}` | `L` goes immediately before it |
| `{"op":"after","key":K,"line":L}` | Immediately after it |
| `{"op":"first","line":L}` | `L` goes at the start of `lines` |
| `{"op":"last","line":L}` | At the end of `lines` |
| `{"op":"delete","key":K}` | The line carrying `K` goes |
| `{"op":"set","field":F,"value":V}` | One scalar of the document |

`F` is one of `phase`, `attention`, `header` or `stale_after_minutes`. **`V` may be `null`, and for
all four it means *as if the document had never said it*** — absence for `phase` and `attention`, the
default for `header` and `stale_after_minutes`, because absence is what produces a default.

**Either the whole list applies or none of it does.** A key that names no line, or that names two,
refuses the **patch** — not the op — and the panel is left exactly as it was, down to the byte a
`panel_get` hands back. That is what makes a patch safe to send without reading first: the worst
outcome is that nothing happened and you were told why.

**The ops see each other, in the order you wrote them.** An `after` may name a key an earlier op
inserted, a `replace` may address a line the same request created, and deleting a key twice refuses
the second time. A list of edits is read the way a person writes one.

**What it refuses, each with the sentence you get back:**

```
← {"ok":false,"error":"no line of this panel carries the key \"tets\""}
← {"ok":false,"error":"the key \"mr-2\" names 2 lines of this panel"}
← {"ok":false,"error":"the key \"mr-2\" is already on a line of this panel, so this would give it two"}
← {"ok":false,"error":"this console's panel is its file's: publish with panel_set before patching"}
← {"ok":false,"error":"this panel is held by \"ci\" until that connection closes"}
← {"ok":false,"error":"\"actions\" is not patchable: a button reaches a shell, so it is only ever set with the whole document"}
← {"ok":false,"error":"over 256 KiB"}
← {"ok":false,"error":"a patch carries at most 64 ops and this one carries 65"}
← {"ok":false,"error":"a patch carries no ops"}
← {"ok":false,"error":"a patch's \"ops\" are over 256 KiB"}
← {"ok":false,"error":"\"updated_at\" is a field of a panel and not one a patch may set"}
```

**A repeated key is refused rather than guessed at.** Two lines may carry one key — the document is
yours and reading it is not the moment to rewrite it — so both keep it, and the ambiguity is reported
the moment you try to address it. Give the lines you mean to patch keys of their own.

**And a patch will not create one.** An insert whose line carries a key some line already has is
refused before it lands, rather than leaving you with an address that names two lines and refuses
everything until you republish the whole document. That matters for one case in particular: **a patch
whose answer timed out may still have been applied**, and the honest thing to do is send it again —
the resend is refused rather than doubling the line.

**Your ops are bounded on the wire too, at the same 256 KiB.** They are the one field of this channel
that can carry a whole document's worth of text, and a request line past the transport's bound ends
the connection instead of being answered — which would take your claim with it. Over the bound, the
patch comes back refused and the connection lives.

**`v`, `updated_at`, `lines` and `written_at` are fields of a panel that a patch may not set**, and
they say so rather than coming back as fields that do not exist: `lines` is what the ops are for, and
the other three are the document's own bookkeeping.

**Buttons are not patchable, and that is scope rather than security.** You already set any `actions`
you like with a `panel_set`, so a patch would add no reach; what it would add is a second path into
the one shape of this contract that reaches a shell, for a convenience nobody asked for.

**At most 64 ops in one request, and the 65th refuses the patch.** It is not a byte limit — the
transport's own bound already holds those — it is a bound on the work one request puts inside a frame.
A truncated instruction would be an edit that silently did not happen with `{"ok":true}` on the wire,
which is the exact failure this verb exists to remove.

**And one rule that will surprise you once, so it is written here rather than discovered**: *what an
op says* is not tolerated the way *what an op carries* is.

- A `line` an op carries **is a document line** and is read with the document's own tolerance: a
  `mark` nobody knows is dropped, an `indent` past 64 is clipped, and the line stands — and the patch's
  `ok` names the dropped mark or style exactly as a publication's does
  ([*What fell away is said*](#what-fell-away-is-said)).
- A field named by a `set` **is part of the instruction** and is not tolerated:
  `{"op":"set","field":"attention","value":"waiting"}` refuses the whole patch, where the same word
  inside a published document is dropped and the panel still paints.

The document did not change; what can be said *about* it diverges. A document is your summary and
losing it over a field nobody looks at would be the worst thing to do with it — an instruction nobody
can read has no sensible reading at all.

### The three `split_*` verbs

They let the connection **open a console beside its own, read it and close it**, and they are the
only verbs that name another pane:

- **`split_open` opens beside the pane that asked** — `side` is `left`, `right`, `top` or `bottom` —
  and the answer carries an **opaque handle**: sixteen random bytes the window scopes to this
  connection on this pane. **The person's keyboard does not move**: whoever asked was an agent, and
  splitting from the chord does move it because that gesture is a person's. Where the console cannot
  give both children their minimum, nothing is opened and the caller is told. Where the console is
  born with no addressable identity, the handle is `null` — the console exists and the person can use
  it, and this channel cannot name it.
- **There is no `env` on `split_open`, and there cannot be one.** The caller contributes **no
  variable at all**: the console's environment is the one a pane's launch always builds — the
  terminal's own fixed set, plus `PANEL_ID` and `PANEL_DIR` and nothing beside them. A field named
  `env` is not read, and an allow-list of forbidden names was discarded for that reason: the
  invariant is that the caller adds nothing, not that somebody maintains a list of what it may not add.
- **`split_read` returns what the console shows** — its visible screen, or the whole scrollback when
  `scrollback` is `true`, which is not the default because the small read is the one an agent usually
  wants. The answer's `text` is the console's own bytes: a wide character once, its zero-width scalars
  kept.
- **`split_close` closes a console this connection opened**, and refuses a handle it did not open, a
  console that is its tab's only one, and one the person has taken over.
- **A handle is dead the moment the person's input reaches that console** — a keystroke or a paste
  (the keyboard's single door), a wheel turn, or a mouse report: the three places the application
  writes what a person produced into a pseudoterminal. From then on `split_read` and `split_close`
  answer **with the reason**, and the console's own row says it to the person. **A press on a panel's
  button does not revoke it**: that is the application writing on the person's behalf, not the person
  taking the console back. This is an **application** boundary and not a confidentiality one: the
  program `split_open` launched keeps reading whatever the person types afterwards.

`panel-mcp`, a binary that ships with the terminal, speaks this channel over MCP for an agent that
would rather call a tool than open a socket. It is a client of this transport and not the transport
itself. When it starts with a whole identity — `PANEL_ID` set, non-empty and text, and `PANEL_DIR`
set and non-empty, the same rule its tool calls apply — its answer to `initialize` carries
`instructions` asking the agent to publish its panel and keep `attention` true. Without one, and on
Windows until the channel is bound there, it carries none: the bridge does not promise a panel it
cannot reach. **D88**.

### Rules of this transport

- **The pane is the endpoint you opened, and no message names a pane by its identifier.** `PANEL_ID`
  is inherited by every descendant of a pane and readable by any process of the same user, so it is a
  name and not a credential, and there is nothing in this transport for a producer to say it with.
  `panel_set` names nothing at all, and the three `split_*` verbs name **only consoles the same
  connection opened**, by an **opaque handle** the window handed back — never a pane id, never a
  name anybody could guess. What stops a producer reaching somebody else's pane is that the words do
  not exist.
- **The first publication claims the pane under its `source`, and the claim lasts as long as the
  connection does.** While it is held, that pane's `panel.json` **is not read at all**, and a second
  source is refused with an error naming the one holding it. The panel says who is driving it on its
  identity row, and confesses a file publication it refused. **D43**.
- **The hold ends when the connection closes** — including when the producer dies, which is the case
  it exists for. What was last published stays on screen; the file takes over from its next
  publication. An intentional `panel_retire` is the separate exception: it clears the session state
  and the untouched auxiliary consoles its own connection still controls (**D50**).
- **There is no sequence number and there is not going to be one.** One endpoint per pane plus the
  file unread means exactly one writer, and one connection's requests cannot reorder themselves.
- **A document delivered carries no date, so the application stamps its arrival.** That is what keeps
  a held panel able to go stale: an agent that hangs with its connection open must not show the same
  summary for ever.
- **The same 256 KiB ceiling, on the document and decided by the same parser as the file.** A
  document over it comes back as the file's own `too big` error and the connection survives, because
  the end of its line was seen. **A line that goes over the transport's bound — the document plus its
  envelope — ends the connection rather than being skipped**: the read stopped in the middle of
  somebody's request, so what comes next is its tail and there is no boundary left to resynchronise
  on.
- **A `source` is a name and takes at most 256 bytes.** A longer one comes back as a malformed
  request and the connection survives. The bound is what keeps the envelope bounded, so a document of
  exactly the ceiling is never pushed over the transport's bound by the name it carries.
- **Patching needs the hold; reading does not.** `panel_get` exists for the producer that has *not*
  claimed the pane, and `panel_patch` cannot exist without a claim. A pane with nothing on it answers
  the patch exactly what it answers a read — *nothing has been published on this console yet* — and a
  pane whose document is its file's says the move: *publish with panel_set before patching*.
- **A patch that applies stamps the panel's date, and a patch that is refused stamps nothing.** A
  producer that fails to patch for an hour still goes stale, which is exactly the failure the stale
  notice exists to expose.
- **The 256 KiB ceiling is on the result, not on the request.** A patch of a few hundred bytes whose
  result would be over it comes back as the same `over 256 KiB` the file gets, and the panel does not
  change. It is the only way a small request can be stopped from producing an oversized document.
- **A pane serves eight connections at a time.** Not a defence — reaching the endpoint already
  requires being this user — but a bound on what a client looping on `connect` can make the window
  spend.
- **Automatic retirement needs a connection-bound clear event.** This repository supplies the tool,
  not harness adapters. Support is claimed per harness only after a real `/clear` integration proves
  the existing MCP connection is used. A command hook, empty document, fresh connection or parsed
  terminal screen is not a substitute.

### What this transport does not give

- **It is not there on Windows yet.** A named pipe gets a default security descriptor that lets any
  process on the machine open it, and nothing in the milestone that built this channel can compile or
  run Windows code to prove otherwise; what cannot be proven is not bound. **F2.7** is where the pipe,
  its descriptor and the assertion over both arrive together. Until then a Windows pane publishes by
  file, which works exactly as it always did; `panel_retire` is therefore unavailable there too.
- **It does not survive the producer.** That is the point, not a gap: a hold that outlived its holder
  would freeze that panel for the life of the window.

### What it does give that the file does not

- **It knows who writes**, which is the whole of why the hold can exist at all.
- **It answers.** A document that is not a panel comes back as an error to whoever sent it, instead
  of being discovered by the person looking at the panel — which is the thing that made writing a new
  producer expensive. And a document that is taken but lost a mark, a style or an attention value says
  which ([*What fell away is said*](#what-fell-away-is-said)).
- **It writes nothing to disk**, so a producer that has no business owning a file does not have to.
- **It edits.** A document of six hundred lines whose third one changed is six hundred lines over the
  file transport and one op over this one — and the file has no word for saying *this line*, because
  it has no way of knowing whose document it is overwriting.

---

## Pending

- [x] **`actions` — declared buttons that write to their pane's program** — **done**: specified
      above. The risks are in [03-architecture.md](03-architecture.md) → Security, and what replaced
      the row that said the document's origin is checked is there too
- [ ] **`open` — a button that opens an address** (`{"label": "View PR", "open": "https://…"}`).
      Still out, and now for a narrower reason: `link` already opens an address from a document of
      outside origin through the desktop's URL handler (D51), so what an `open` would add is a second
      shape for the same act — a button rather than a line — and nobody has asked for it yet
- [x] **How the source and the order of every delivery are expressed** (D17) — **done, and the order
      turned out to be free**: the source is the **holder's name**, claimed on the first publication
      over the channel and released when the connection closes, and while it is held the file is not
      read at all. Herdr pairs its `--source` with a strictly increasing `--seq` to discard what
      arrives late ([07-landscape.md](07-landscape.md) → E8); **there is no `seq` here and there is
      not going to be one**, because one endpoint per pane plus an unread file leaves exactly one
      writer and a connection's requests cannot reorder themselves. **D43**
- [ ] **The system notification on going to `wait`.** The `attention` field already says when a pane
      claims (D18) and the tab's indicator already accumulates it; what is left is deciding whether
      the user is interrupted on top of that, and that is left for when the field has producers: a
      false interruption costs a great deal more than a false indicator
- [x] **The change of state as something more than paint** — **resolved by D18**: the producer
      declares it in `attention` instead of the app interpreting it from the body, which was the
      doubt this line left open
- [x] **Discovery through `PANEL_DIR`** instead of by convention of `flow` — **done in F1.6**. Each
      pane's identity and its directory have existed since F1.5; F1.6 added reading whatever gets
      published there, painting it, finding out on its own that it has changed and saying when it is
      old
- [x] **`link`** — **done**: specified above. It was left out of F1.6 on purpose because it is the
      only field that **opens** something coming from a document that may be of outside origin; the
      review it was waiting for is D51
- [ ] **`updated_at`, which today is read and not painted.** What is left is deciding whether it is
      worth showing what the producer *says* when the notice that comes out of the file's real date is
      already there
- [x] That `header: false` should not depend on a `meta.json` existing next to it — **done in F1.6**:
      the flag is read from the `panel.json` itself, its default value does not look at any
      neighboring file, and whoever paints honors it. **What it does not hide is the application's
      notice**, and that is deliberate
