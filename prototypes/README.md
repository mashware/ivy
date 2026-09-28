# Prototypes

## `flow-panel`

The status panel working, in Python, over WezTerm. **It is the contract's reference
implementation** ([../docs/04-panel-contract.md](../docs/04-panel-contract.md)).

The working copy is installed at `~/.local/bin/flow-panel`. This is the repository's — if you
touch one, sync the other.

```
flow-panel [path]                 global view: every work in the repo
flow-panel --focus [path]         only the work of the current branch
flow-panel --work MT-1234 [path]  pinned to one work
flow-panel --json [path]          list of open works (for the picker)
```

Shortcuts in WezTerm: `Ctrl+Alt+F` global view · `Ctrl+Alt+G` pick a task and pin it.

**What is worth taking from here for the app:** the adaptive rendering (width and height), the way
the phase bar degrades, the short links with OSC 8, the alignment by blocks and the stale-summary
warning. All of it has to be reimplemented, but the decisions are already made and tried.

## `mockup-terminal.html`

A visual mock-up of the app: tabs, panes, a panel per pane, context and cost, an activity indicator
and folding. Fake data, real palette.

It is for judging proportions before writing Rust. Open it in a browser and play with the controls
at the top.
