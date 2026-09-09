# Debt in this repository's own machinery

**What this file is for, and what it is not.** Everything here is a gap in the **tooling** — a guard
that does not bind, a floor with slack in it, a test that goes green by not running, prose that
drifted from the code. Real, worth writing down, and **nobody's next task**. It is deliberately not
the issue tracker: a tracker where these outnumber the product's defects makes its own number lie,
and «a console goes silent and says nothing» ends up buried under «a floor has no subject».

Read by whoever next works on the machinery. Triaged by nobody. Add a line, do not open an issue.

Format: `- <date> · <ticket> · <what> — <why it matters>`.

## Open

- 2026-09-09 · 210 · **`FLOW.md`'s validate note names scripts that no longer exist** — it says
  `scripts/medir-fotograma.sh`, `comparar` and `.INVALIDA`; since F2.12 renamed the repository to
  English the real names are `scripts/measure-frame.sh`, `compare` and `.INVALID`. Every
  `/flow:feat:validate` on this repo reads that note and is pointed at a path that is not there.
- 2026-09-09 · 210 · **`CursorShape::HollowBlock` is unreachable, and its painting branch is dead
  code** — DECSCUSR produces only `Block`, `Underline` and `Beam` (`vte-0.15.0/src/ansi.rs:1714`),
  and the other doors into `shape_from` are alacritty's own config and its vi mode, which nothing in
  this workspace enters. So `painter.rs`'s `HollowBlock` arm cannot be exercised by any input a
  program can send, and no test can reach it without production changes. **Left in place on
  purpose**: the arm keeps the `match` exhaustive over a public enum, and a `cursor.shape` setting
  would make the variant reachable again. What is owed is the note, not the deletion.
- 2026-09-09 · 210 · **Fourteen tests about the held paste skip silently on the platforms F2.7 is
  about to add** — counted (`grep -c 'if !hold_a_paste_on'`); the entry said twelve until a comment
  audit counted them — they open with `if !hold_a_paste_on(&mut pane) { return; }`, and that helper returns
  false when `keyboard::the_longest_line_a_reader_keeps()` is `None`. On Linux it is `Some(4_095)`,
  so they run; on macOS and Windows — exactly the platforms F2.7 exists to reach — the early return
  leaves every one of them green with nothing asserted. **A suite that goes green on a new platform
  by not running is the worst possible first signal there.** Whoever does F2.7 decides between a
  per-platform boundary, a skip that is reported, or a panic where no boundary is declared.
- 2026-09-09 · 210 · **The held paste's notice is pinned as a box and not as a question** — PR #1 of
  this ticket pinned that the box is drawn, filled, the right thickness, stroked inside, unclipped
  and in the right console. Three breaks an adversarial pass found are still green: painting the
  sentence in `theme.panel_background()` leaves it invisible inside a correct box; moving its origin
  to `area.left_top()` paints it outside its own border, which is what the `debug_assert` above it
  exists to prevent and which no test drives; and a corner radius of 120 turns the box into an oval
  whose corners cut the first line. `text_painted` reads `galley.text()` and never a colour or a
  position. The owner is whoever next works on that notice — #170 and #211 are both in that area.
- 2026-09-09 · 210 · **The third of `paint_cursor`'s three guards cannot be reached, and the clamp
  that feeds it is inert** — «no cell where the cursor stands» is named in the design beside the
  other two, and no `Screen` can produce it: the row is range-checked and the column clamped where
  the view is built, a grid of no columns is refused by the engine itself
  (`grid/row.rs:38`, `assert!(columns >= 1)`), and `View::new` is `pub(crate)` so nothing outside
  `term-core` can fabricate one. Deleting the column's clamp leaves the suite green. **Left in place
  rather than removed**, and with a test pinning the relationship that keeps it inert — the same
  answer this delivery gave the blink's own unreachable clamp. What is owed is the note: a guard
  nobody can trip reads, to the next person, as a case that happens.
- 2026-09-09 · 210 · **The size guard names two different `impl` blocks the same** —
  `crates/app/src/lib.rs` has two `impl Window` blocks — the big one and a second holding `describe`
  and `materialize`, the layout's two halves — and `shape.rs`'s `name_of` derives the same key for
  both. (`impl eframe::App for Window` is a third block and is **not** affected: it gets a key of its
  own. The entry said it was the second one until a comment audit read them.) The ratchet entry
  `src/lib.rs::impl Window` therefore covers whichever it matches and the other block is measured
  against that number instead of `IMPL_CEILING`'s 300. The second is about twenty lines today, so
  nothing is wrong now — what is wrong is that it could grow unnoticed, and that the ratchet's own
  number stops describing one block.
