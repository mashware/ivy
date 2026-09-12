# Contract — the panel document

Current state: **implemented and in use in the application**, and also in `prototipos/flow-panel`
with `flow` as its first producer. What is here is the specification as it works today, plus what is
left pending at the end.

**This document is split in two on purpose** ([02-decisiones.md](02-decisiones.md) → D17):

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
| `updated_at` | recommended | ISO 8601. **It is read and not painted yet** —like `link`, see Pending— and **the stale notice does not come from here** (see rules) |
| `phase` | no | What is being run **right now**. Free text, and painted literally — **inside the identity line**, so `header: false` hides it too |
| `attention` | no | **Whether this pane claims something, and of what kind.** `wait` `block` `done`. Absent = it claims nothing. See below |
| `header` | no (`true` by default) | Whether the app adds its identity line at the top: the pane's identifier and its `phase` |
| `stale_after_minutes` | no (30 by default) | How much silence is legitimate before warning |
| `actions` | no | **The buttons this pane offers.** Each one is painted with its command underneath, and one click sends it to the program in that pane. See below |
| `lines` | **yes** | The body |

### `attention`

The only field the application **interprets**, and it is bounded to the only question it needs to
answer in order to navigate: *do I have to go there?* The complete decision, with what was discarded,
in [02-decisiones.md](02-decisiones.md) → D18.

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
  know the question ([02-decisiones.md](02-decisiones.md) → D6).

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
| `link` | A URL. **It is read and not painted yet** — see Pending |
| `indent` | Manual indentation, in cells (inherited by the continuation lines). **It is bounded to 64** when read, and **when painted also against the panel's width**: an indentation of 40 is not honored whole in a panel of 36, because it would leave the text outside |

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
  its reason for being ([07-panorama.md](07-panorama.md) → Conductor, which shows them and still does
  not have this panel).
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
[02-decisiones.md](02-decisiones.md) → D17.

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

| Who reads | Where it looks |
|---|---|
| The application | **`$PANEL_DIR/panel.json`**, the directory it gives each console when it launches it |
| The prototype on WezTerm | `.claude/work/<trabajo>/panel.json`, by convention of `flow` |

The prototype discovers the file by convention; the app does it by pane identity (see
[02-decisiones.md](02-decisiones.md) → D7), since F1.5 for the directory and F1.6 for the reading.
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

---

## Pending

- [x] **`actions` — declared buttons that write to their pane's program** — **done**: specified
      above. The risks are in [03-arquitectura.md](03-arquitectura.md) → Security, and what replaced
      the row that said the document's origin is checked is there too
- [ ] **`open` — a button that opens an address** (`{"label": "View PR", "open": "https://…"}`).
      Left out of the milestone that delivered `send`, and for the same reason `link` was left out of
      the whole of F1.6: it is the only field that would **open** something coming from a document
      that may be of outside origin, so painting it and sanitizing the address that goes with it
      deserve a review of their own. A `send` reaches a shell this application already owns; an
      `open` reaches the desktop's URL handler, which it does not
- [ ] **How the source and the order of every delivery are expressed** (D17). Herdr solves it with a
      `--source` that is stable per integration, which grants authority and is released on leaving,
      and a strictly increasing `--seq` with which it discards whatever arrives late
      ([07-panorama.md](07-panorama.md) → E8). Here there is neither one nor the other, and it will
      be needed as soon as MCP delivers documents alongside the file
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
- [ ] **`link` and `updated_at`, which today are read and not painted.** For `updated_at` what is
      left is deciding whether it is worth showing what the producer *says* when the notice that
      comes out of the file's real date is already there. `link` was left out of the whole F1.6
      milestone on purpose: it is the only field that would **open** something coming from a document
      that may be of outside origin —a cloned repo, a container—, so painting it and sanitizing the
      address that goes with it deserve a review of their own
- [x] That `header: false` should not depend on a `meta.json` existing next to it — **done in F1.6**:
      the flag is read from the `panel.json` itself, its default value does not look at any
      neighboring file, and whoever paints honors it. **What it does not hide is the application's
      notice**, and that is deliberate
