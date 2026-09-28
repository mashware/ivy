---
id: asp_the-background-tab-lives-in-another-window_329695
slug: the-background-tab-lives-in-another-window
name: >-
  A tab that is never painted kept the startup grid and the program inside
  formatted for a window that does not exist — fixed in F2.1
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:33.386Z'
updated_at: '2026-08-16T16:56:18.193Z'
last_verified: '2026-08-16T16:56:18.193Z'
file_paths:
  - crates/app/src/lib.rs
  - scripts/measure-frame.sh
symbols:
  - 'Window::measure_the_ones_out_of_sight'
  - measured
  - resize
  - paint_pane
  - INITIAL_GRID
content_hashes:
  crates/app/src/lib.rs: 'sha256:ca5c117b121ef422e01d80872e0ebf3a1c92fb2c22b23317c8778bbb2fa86650'
  scripts/measure-frame.sh: 'sha256:c2df1b6a447aabec1f099c5994634febb7aa0d0c0db959270f484726d7d22e46'
tags:
  - measurement
  - tabs
  - F2.1
  - verified-in-the-real-app
  - fixed
feature_id: feat_term-core_nx0g1r
---

## What it does

> **Status (16 August 2026): FIXED** in PR #64 (`9702da5`). It is kept written down because it
> explains why `Window::measure_the_ones_out_of_sight` exists, and because the trap the fix brought with
> it turned out to be **worse than the defect**: see [[the-consoles-width-depends-on-an-outside-file]].

**The defect was this: the size announced to the program inside was only worked out when painting.**
`medido` was written in `paint_pane`, which only runs for the active tab, and `resize()`
reads it in the logical half of the next frame. A background tab **is never painted**, so it kept
`INITIAL_GRID` —**24×80**— while the visible one had 59×57.

Verified **against the release binary**, not read: with `MEASURE_TABS=3` the report published
`tab 0: was visible → 59×57` and the two background ones at `24×80`.

**Two consequences, and neither of them was cosmetic:**

1. **Of product.** An agent in the background formatted its output to **80 columns** when the window
   was giving it 57. The milestone's promise is that the background one «keeps running»; it ran, but
   **believing it lived in another window**, and on coming back to that tab what had been written
   was fitted to a width that was not its own until the program redrew.
2. **Of measurement.** The batch of two tabs with the background one dumping would have measured
   that console parsing **1 920 cells instead of 3 363**, almost half the work, and the annex would
   have published an **optimistic cost without saying so**.

> **And the bias could not be estimated with what was already known.** The D3 annex showed that the
> wrapping **width** does not move the cost of parsing (4.35 / 4.51 / 4.35 ms at 57 and at 21
> columns), but **the height has never been measured**. With 24 rows instead of 59 there are some
> 2.5 times more screen scrolls per line dumped: **the direction of the bias was unknown**, which is
> worse than knowing it and having it be large. That is the reason it was decided to fix it before
> measuring, instead of measuring on the 24×80 and declaring the bias.

It is the same family as [[measuring-something-that-changes-the-grid-is-not-a-subtraction]] —a piece
that keeps part of the room belonging to the rest— but by the opposite route: here it was not that
the piece stole space, it is that **the measured part was never told how much it had**.

**The good part, while it lasted:** the report publishes the grid of every console of every tab, so
whoever read it saw the 24×80 and did not swallow it.

## How it was fixed

`Window::measure_the_ones_out_of_sight` walks, at the end of `ui`, the panes of the tabs that are not
painted, tiles the window's real rectangle and writes `medido`. The path downstream already existed:
`pump_them_all` walks every pane of every tab and calls `resize()`, which applies `medido` to
the grid and to the pseudoterminal.

**Confirmed in the field, not only in tests:** the header of batch G of the annex «Measured — the
frame with tabs» of **D13** publishes the background tab at **59×21**, where before the milestone it
would have said 24×80. And criterion AC14 was tested down to its furthest clause —that the size
reaches the program inside— by asking it with `stty size`.

## Where it lives

`Window::measure_the_ones_out_of_sight` (the fix), `Window::paint_pane` and
`Pane::resize` in `crates/app/src/lib.rs`; the symptom showed in the header of the
`on_exit` report and affected the batches of `scripts/measure-frame.sh`. The annex that confirms it,
in `docs/02-decisions.md`, under **D13**.
