---
id: asp_the-panel-yields-and-its-width-is-in-cells_k5w9y5
slug: the-panel-yields-and-its-width-is-in-cells
name: >-
  The panel's width is set in cells and not in points, and in a narrow window
  the one that yields is the panel — checked with four panes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T15:18:46.860Z'
updated_at: '2026-08-17T16:10:27.076Z'
last_verified: '2026-08-17T16:10:27.076Z'
file_paths:
  - crates/app/src/panel.rs
  - docs/02-decisiones.md
symbols:
  - DEFAULT_WIDTH
  - MINIMUM_CONSOLE
  - set_aside
  - PanelTiling
content_hashes:
  crates/app/src/panel.rs: 'sha256:6e305e69f27f7fd80801f34a64fa332f571bc0b1924c7d85017fc37faedcd0a1'
  docs/02-decisiones.md: 'sha256:e931de7f9b29b21429b9a259db1cd9648470337c6e11d13deba391055a972ea5'
tags:
  - product
  - tiling
  - egui
  - measured
  - F2.2
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Two product decisions about the tiling of the width, both with their reason.**

**1. In cells, not in points.** The user's decision was «fixed width in points, no folding», and it was honored in what it meant —it does not depend on the window's width— but **counted in cells of the font and not in points**.

> **Update (F2.4, August 2026): the «it cannot be dragged» part is NO longer true.** The width became **each pane's own** and its border can be dragged; what remains intact is everything else on this card, which is what mattered: it is counted in **cells**, the one that yields is still the panel, and the deduction still goes before measuring the grid. The constant of 36 stopped being the width and became **how many cells** a pane nobody has touched **is born with**. See [[the-layout-cache-invalidates-by-cells-not-by-points]] for the gesture's cost.

Counting it in cells and not in points is what still rules: A width in points narrows in columns when the desktop's zoom goes up, and what the panel has to preserve is **how many characters fit**, which is what decides whether its text can be read. A panel that on one screen shows the whole lines and on another cuts them is not the same panel.

> Watch out when testing it: with a cell of 8 points, «36 cells» and «288 points» give the same number. A test written with that width does not tell the two implementations apart — the cell size has to be varied for the difference to appear.

**2. The panel yields, the console does not.** When it does not all fit, the panel narrows until it disappears and the console keeps a floor of columns. **The panel is margin and the console is the work**: in a narrow window what is spare is the status, not the program. With the rule the other way round, dragging the divider would leave an interactive program with no room to write while its panel keeps its thirty-six columns.

**And the deduction goes before measuring the grid**, not after. Measuring the whole rectangle first, the program inside would believe it has columns the panel has taken away from it and would write underneath it. It is the central criterion of the PR that made the panel visible.

**The tiling's predicate is «there is something published», not «there is a directory».** Every console in the application is born with its directory and almost none has a file, so tiling by having a directory would take the width away from all of them from the first frame — and it is a failure that a test set up with no directory does not see.

### Measured with four panes (17 August 2026) — and it closes issue #33

The rule «the panel yields, the console does not» **holds the worst case without touching anything**, and the open question of [#33](https://github.com/mashware/agent-terminal/issues/33) —«with four panes the panel would eat the whole console»— is answered with numbers. Window of 1280×800 points, panel published in all four:

| Layout | Grid of each console | What happens to the panel |
|---|---|---|
| Two columns | 49×39 | it fits whole (36 columns) |
| **Four in a row** | **49×20** | each pane gets some 37 columns, the panel does not fit and **yields down to 17** to leave the console its floor of **20** — exactly `MINIMUM_CONSOLE` |
| **Four in a square** | **24×39** | **it yields nothing**: what is split in half is the HEIGHT, not the width |

**So the layout somebody would actually use —two columns of two— does not take a single column away from the console**, and the four-in-a-row one leaves it at its floor instead of eating the room. Nothing to bring forward from F2.4.

And along the way, what the same annex proves about the frame: **the interface framework's cost does not get worse when the panes are doubled** —402 to 530 µs of «rest of the frame» across six runs, with the variation within one batch of the same order as the variation between batches—, and what does grow is the **wait in the queue** (46 ms with two consoles dumping, 88 and 91 with four), because the budget of 128 KiB is **per pane**. The whole numbers, in the «Measured» annex of **D14**.

## Where it lives

`crates/app/src/panel.rs`: the two constants and the function that tiles, with its type `PanelTiling`. The geometry lives in the panel's module and not in the window's, which is the one that calls it. The check with four panes, in the «Measured» annex of D14 (`docs/02-decisiones.md`) and in the `H` and `I` batches of `scripts/measure-frame.sh`.
