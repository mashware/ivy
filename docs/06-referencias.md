# References

What has been researched, with what each case contributes. Data as of August 2026.

---

## How each terminal is built

| Project | Engine | Interface | Lesson |
|---|---|---|---|
| **Warp** | Their own, Rust | **Their own** UI framework over `wgpu` | They wrote even their own button system. They published [«Why is building a UI in Rust so hard?»](https://www.warp.dev/blog/why-is-building-a-ui-in-rust-so-hard). They could afford it as a funded company |
| **Zed** | **`alacritty_terminal`** (crate) | Their own GPUI | **The pattern we copy**: the engine as a library, the painting our own |
| **Ghostty** | Their own `libghostty`, Zig | **GTK4** on Linux, Swift/AppKit on Mac | The most celebrated project chose a native toolkit per platform, not its own UI |
| **PhpStorm / IntelliJ** | Their own **[JediTerm](https://github.com/JetBrains/jediterm)**, Java | Swing | JetBrains, with hundreds of engineers, also wrote their own: there was no cross-platform one worth having |
| **VS Code** | xterm.js | Electron | The web model works at scale… with one or two active terminals |
| **Tilix** | VTE | GTK3 | One person managed buttons and draggable panes **because they wrote neither of the two pieces** |
| **Ptyxis** | VTE (GTK4, GPU) | GTK4 + libadwaita | GNOME Terminal's successor; Christian Hergert doubled VTE's performance before building it |

**Conclusion:** what is scarce is not the cross-platform UI framework — there are plenty. It is the
**cross-platform terminal widget**. That is why everybody ends up writing their own… except Zed,
which consumes it as a crate. See [02-decisiones.md](02-decisiones.md) → D2.

---

## opencode — the closest reference

**Architecture:** client-server. A TypeScript/Bun backend exposing **HTTP + Server-Sent Events**;
on top of it, several clients: a TUI (OpenTUI, previously Go/Bubble Tea), a Tauri desktop app, IDE
extensions.

**Its right-hand sidebar** shows, top to bottom:

- The task's title
- **Context**: `35 392 tokens · 18% used · $0.62 spent`
- **MCP**: connected servers
- **LSP**: active analysers
- **Todo**: the plan with `[✓]` / `[ ]` and the current step highlighted
- The working directory

Plus a **SESSIONS** panel: pinned (curated by the user) and ephemeral, including any that is **busy
or retrying somewhere else**.

**What we copy:** the context and cost block (the only thing only the agent knows), the activity
indicator, the pinned/ephemeral distinction, and that the panel be collapsible from the start.

**What we do not:** it duplicates the plan in the message flow *and* in the sidebar. The panel exists
precisely so that state is not repeated in the conversation.

**Which model it uses, because it changes who pays:** it is **the most agnostic of them all** — more
than 75 providers: Anthropic, OpenAI, Copilot, Gemini, Bedrock, Groq, DeepSeek, Ollama and local
models — and that is why it is by far the most popular open-source agent. But **with Claude it goes
through the API and pays tokens since 4 April 2026**, when subscriptions stopped covering
third-party tools ([02-decisiones.md](02-decisiones.md) → D4). Hosted here as an agent, its user's
Claude pays per use; with Copilot or Ollama, it does not.

**And it is not competition: it is the best candidate for a second producer.** Its sidebar already
computes exactly what our contract wants to paint — tokens, percentage, cost, the plan with its
steps, MCP, LSP, directory — only locked inside its own interface. And its architecture puts it
within reach: **the backend exposes HTTP with events**, so a small bridge listening to those events
and writing `panel.json` would publish without touching a line of its code. Today the contract has
**one** producer (`flow`); this is the cheapest way there is for it to have two, and having
producers is «the only thing that can turn into a moat» ([01-vision.md](01-vision.md)).

**Where it differs — and why that is our gap:** its sidebar only knows about opencode. And its
most-voted open requests are [a collapsible panel with multiple windows](https://github.com/anomalyco/opencode/issues/19388)
and [pinned session tabs](https://github.com/anomalyco/opencode/issues/24451) — people are asking an
agent to become a window manager, because the host that should be doing it does not exist.

---

## xterm.js's limits

From the issue [«Support dozens of terminals on a single page»](https://github.com/xtermjs/xterm.js/issues/4379):

- Browsers cap the number of live WebGL contexts; **multiplexers need dozens of panes and the
  browser does not support it**
- Terminals compete for the main thread: **2 instances → 30 fps, 4 → 15 fps**

It applies only with several terminals **producing output at the same time** — which is exactly our
case. The basis of [02-decisiones.md](02-decisiones.md) → D3.

---

## Subscriptions and billing

- [Using Claude Code with Pro or Max](https://support.claude.com/en/articles/11145838-use-claude-code-with-your-pro-or-max-plan) — if `ANTHROPIC_API_KEY` is set, it takes precedence and bills that account
- [claude-code#43333](https://github.com/anthropics/claude-code/issues/43333) — `claude -p` with OAuth billing as API
- [claude-code#37686](https://github.com/anthropics/claude-code/issues/37686) — a Max subscriber with $1 800 in two days

The basis of [02-decisiones.md](02-decisiones.md) → D4.

---

## On WezTerm (the prototype's base)

- Latest **stable**: February 2024. Development is still alive, but only through daily nightlies
- **It does not draw decorations on Wayland** and GNOME does not do it for it either
  ([wezterm#4962](https://github.com/wezterm/wezterm/issues/4962)) → it runs under XWayland
- There are no buttons or menus of its own: [wezterm#1485](https://github.com/wezterm/wezterm/issues/1485)
  has been asked for since 2022
- Dragging the pane divider has [known defects](https://github.com/wezterm/wezterm/discussions/4399)

It serves perfectly as the base for the panel's prototype. It does not serve as a product.

---

## Tilix (the starting point)

In **declared maintenance mode**: no new features and looking for maintainers. The underlying
problem is not the abandonment, it is two decisions from 2016: **GTK3** (also in maintenance) and
**D** with `gtkd`, which is the real reason nobody picks up the baton.
