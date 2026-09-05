# Vision

## What it is

A terminal that hosts agents. Three levels:

```
Window
└── Tabs
    └── Panes               ← each one is an independent console
        ├── Terminal        ← where the agent runs (or whatever else)
        └── Panel           ← state of that task, fed from outside
```

The panel is not written by the app. It is written by **whoever is doing the work**, through an open
contract. The app only paints it. Today that contract arrives through a file, but **the transport is
not the contract** ([02-decisiones.md](02-decisiones.md) → D17): the document is what gets
versioned, and it can also arrive over MCP or from the terminal itself.

## Who it is for

People who work with several terminal agents in parallel and need to know, at a glance:

- which one is working and which one is waiting on you
- what step each task is on
- which one is running low on context
- how much each one has spent so far
- what is blocked and why

## Who we compete against

**Against terminal emulators** —WezTerm, Ghostty, Kitty, iTerm2, Windows Terminal—, not against
agent orchestrators ([02-decisiones.md](02-decisiones.md) → **D19**). The user to be won over is the
one who already has one of those open with agents inside.

That decides the bar: they have to be given what they take for granted —scrollback, copying,
searching, **their fonts**, **being able to configure it**, the mouse inside `vim`, and that it runs
on their machine— and **nothing from the league next door**: no built-in browser, no mobile app, no
editor, no board. Against terminals the panel has no competition: none of them has state per pane.
Against orchestrators you would be the same product with fewer things.

Development-environment features are **the evolution, not the price of entry**, and they arrive if
this has users.

## What sets it apart

Contrasted against eight competitors in August 2026 ([07-panorama.md](07-panorama.md) → «How we
differ»). **One** thing is left, not several:

**The body of the panel is decided by whoever does the work, and it is structured, persistent
state.** The complete orchestrators —Xirp, Superset, Conductor, Warp, Orca— have rich state, but
with a vocabulary the app chooses: three constants, four events, five states, six indicators, a few
metrics. cmux does open the channel —any process can emit OSC 777/99 to it— but all that fits
through there is a title and a body that **are erased when read**. **And Herdr, which is the one
that goes furthest, does let anyone publish —`--source custom:*`, documented— but what fits is four
states of its own and key-value pairs «for display only».** Nobody has the middle ground: a list
with marks, references and links, written in the producer's vocabulary, that is still there when you
come back and that knows how to expire. That is what allows four panes with four different producers
and a global view that summarizes them.

**What no longer sets it apart, and was written here: preserving subscriptions.** All eight do it
—they all launch the agent's official binary and none of them charges for the API separately—. It is
still the right decision ([02-decisiones.md](02-decisiones.md) → D4) and a condition for this to be
adopted, but it is not an argument that distinguishes anything.

## What it is NOT

- **It is not an agent.** It does not reason, it does not call models, it has no opinion about the
  work.
- **It is not a remote session multiplexer.** That is tmux/Zellij and it does not compete with them.
- **It does not rewrite the terminal emulator.** That engine is consumed as a library.

## Principles

1. **The app knows nothing about domains.** It does not know `flow`, nor git, nor tickets. Only the
   contract.
2. **Whoever does the work publishes the state.** The app never deduces or parses in order to guess.
3. **It has to work without the panel.** If nobody publishes anything, it is still a correct
   terminal.
4. **Information that cannot be deduced from outside is worth more than information that can.** What
   step the task is on, what has it blocked, and whatever the user of that pane wants to publish
   while playing with their agent: only whoever does the work knows that. The AI metrics —remaining
   context, cost— are a **use case** of this, not its definition; see the free-body rule in
   [04-contrato-panel.md](04-contrato-panel.md).

---

## Is it worth it? — assessment of August 2026

Written after analyzing eight competitors. It is kept because the question is going to come back,
and because it has two halves with different answers that are worth not confusing.

### How long they would take to copy it: days

Reading a JSON from a known directory and painting a list with marks has no technical difficulty
whatsoever. Any of the eight has it working in an afternoon and polished in a week. **There is no
technical moat, and believing otherwise would be the worst mistake of this project.**

### Why none of them has done it already

All eight have had the chance. Warp has been going for years and its list of agents is **compiled
into the binary** as an `enum`. Conductor does not have a single way back. Orca reads the terminal
title, and anyone can already get in through there, but only to say a word of its own. It is not
inability: it is that their product pushes the other way.

1. **An open contract degrades their proposition.** The ones that sell a product —Warp, Xirp,
   Superset, Conductor, Orca— sell the same thing: *«we understand your agent»*. A format where
   anyone publishes whatever they want turns them into **a viewer of other people's files**. The
   first sells; the second is infrastructure, and infrastructure is given away by whoever does not
   have to monetize it.
2. **A contract is a commitment, not a feature.** It has to be versioned and never broken. A team
   that iterates every week runs away from that — that is why Warp chose an `enum` in the binary
   rather than a stable surface, and it is not an oversight: it is a choice.
3. **It forces you to give up owning the semantics.** If the producer writes badly, the app looks
   bad. No product team accepts that the quality of its interface depends on third parties, unless
   that is precisely its thesis.

**The advantage is not that they cannot: it is that they do not want to.** That holds up longer than
a technical advantage, and much less than a patent.

> **And there is a counterexample that has to be looked at head-on: Herdr.** It does not sell «we
> understand your agent», it sells being the substrate where they run; it gives the infrastructure
> away because that is its thesis, not its cost; and **it already opened the channel to producers
> that are not agents, documented**. The three brakes above do not apply to it. What holds it back
> is something else, and it is structural: **it lives inside a TUI, where the place to paint a body
> is a row of text**. That is why its sidebar answers «which pane do I go to» better than ours and
> does not try to answer «how far along it is». Moving that costs it the whole product, just as
> getting out of Electron would cost Orca.

### The real risk, and it is not the copying

That the need **gets solved by another route**: that the harnesses publish structured progress
natively. If Claude Code, Codex and company start emitting current step, plan and blockage through
their own hooks, the rest will get 80 % of the content for free from the stream, and this contract
is left with the remaining 20 % — the CI, the deployment, the script of one's own.

That scenario is **more likely** than «Warp reads `panel.json`». And the defense is not technical:
it is having producers writing before it happens.

### The two questions

**For personal use: yes, with margin.** The eight complete products do not do this —verified product
by product, not guessed—. The argument that «three do not even run on Linux» **no longer holds**:
Orca and Herdr run on all three platforms, they are free and they are very polished. What is left is
the body of the panel, not the availability. F1.0 to F1.3 are closed, the terminal is in use and the
panel prototype works: there is less ahead than has already been done.

**As a product: fragile, and it has to be said.** Competing with Warp —cross-platform, open source,
64k stars—, with Orca —MIT, 52k, daily pushes— and with Herdr —Apache-2.0, 32k in five months—
leaning on a design decision that can be copied in an afternoon is not impossible, but it is not a
business plan. That the differentiation has to be obvious in thirty seconds was already written in
the landscape before this round of analysis; it still holds, and now with less margin.

### The only thing that can turn into a moat

**That the format has producers.** Without adoption it is a JSON file; with adoption it is a
standard, and then the value stops being in the app — it is the play of the LSP and of Backstage:
whoever defines the protocol that others implement wins even if their client is not the best.

There is already a real one here: **`flow` publishes today**, in the prototype. Warp has five agents
integrated by hand; this has one that publishes of its own accord. It is little, but it is of a
different nature.

### The test that is still pending, and it is the one that decides

No more competitor analysis — with eight it is answered. **Use the panel with real content for a
week of work and see whether it changes the day.** Not a demonstration: personal use, with `flow`
publishing.

- If it changes it, the priority moves to getting more producers writing, not to polishing the
  terminal.
- If it does not change it, that has been found out with two milestones of work instead of ten, and
  what is left is a terminal that works.

What is **not** valid is to keep polishing the terminal hoping the differentiator will clear itself
up. It is not going to get any clearer than it is today, and every month that passes is a month in
which the idea may occur to someone else — which is exactly what has happened **ten** times in
August 2026 with other pieces of this proposal, the last of them —that the state be declared by
whoever does the work instead of deduced by the app— with Herdr.
