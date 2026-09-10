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
- 2026-09-10 · 192 · **The shape guard stops counting at `mod tests`, so a type declared after it is
  free** — `crates/app/tests/shape.rs`'s counter breaks at the first line reading `mod tests`, and
  `DeafConsole` sits **after** that line in `pty_output.rs`: about fifty-six production lines the
  600-line ceiling never sees, in a module the guard measures at 107 against a real 814. A previous
  delivery already made a file four lines *worse* by wrapping test helpers in a module of another
  name to get them counted. This delivery put its new code **before** that line on purpose — being
  counted is the point of a ratchet — but the hole is still there for whoever does not know.
- 2026-09-10 · 192 · **`Eq` on the public ending value is held by nothing, deliberately** — dropping
  it from the derive leaves all eighty-five tests green, because nothing in the workspace behaves
  differently with or without it. Not filled: a test would assert the declaration rather than an
  effect, which is this project's first named disguise, and the derives **are** checked character for
  character by the blinded contract verifier every review runs. Written down so the next reader does
  not rediscover it as a gap and fill it with the wrong kind of test.
- 2026-09-10 · 192 · **A console whose program left a survivor never ends, and the obvious fix is
  worse than the silence** — when something the shell launched holds the slave end, the read never
  reaches end of file: the console never counts as finished, the window never closes itself over it,
  and nothing is said. **D32** documents the mechanism and declines to kill the survivor. What is
  owed is the note, not a ticket, and a skeptic established why: `Child::try_wait` answers «the child
  is gone», not «this console is over» — the survivor can still be writing, so a console declared
  finished on it would keep painting **after** saying it had ended, and with `all_finished` on the
  other end it would auto-close a window over somebody's development server. Its owner is whatever
  milestone answers «what does closing mean»; **F2.7 meets the same fact as a platform default**,
  because on Windows the pipe's write end is owned by the pseudoconsole and shared with the master
  through one `Arc`, so dropping the slave closes nothing and no end of file arrives while the
  console lives at all.
- 2026-09-10 · 192 · **Which stream a developer line goes to, and how many times it comes out, is
  held by no test** — the decision is testable and tested (the sentence is a returned value), but
  from inside the process nothing can observe standard error, so replacing `say` with a `println!`
  that prints twice leaves the suite green. Measured. What would close it is a child process that
  runs the path and a parent that reads its streams — `crates/app/examples/leak.rs` is the precedent
  — and it cannot be written yet for the failing case, because a pseudoterminal whose reading failed
  cannot be built from outside `term-core`.
- 2026-09-10 · 192 · **The input half of what the frame hands the pump is read by no test, and the
  line it feeds has none at all** — `Silences::of` reads the backlog and the ending off one
  pseudoterminal; hardcoding the **ending** to `None` fails a test, hardcoding the **backlog** to
  `None` does not. A console nobody typed into owes nothing, so both sides answer `None` and the
  assertion agrees with the defect — D30's second disguise, caught by mutating rather than by
  reading. And the sentence that half feeds, `DeafConsole`'s (**D29**), has no test in the workspace
  at all: its two literal strings are asserted nowhere. Binding either needs a program that stops
  reading with enough bytes owed to it.
- 2026-09-10 · 192 · **The runtime's panic line names no console** — the decision not to say
  anything of our own about a reader that died rests on the default hook already printing the panic
  with its file and its line. It does, and it prints `thread '<unnamed>' panicked at …`: the reading
  threads are spawned with `thread::spawn` and no `Builder::name`, so nothing ties the panic to a
  console, and with several panes open the line says which file failed but not whose. Naming those
  threads belongs to the crate that spawns them.
- 2026-09-10 · 192 · **A gap that was recorded here and then closed, kept because the reasoning was
  the interesting part** — «reverting the pane's question leaves the whole suite green» was true, and
  the reason given for not fixing it was that the state needs a reading that failed, which cannot be
  built from outside `term-core`. **Wrong**: the question is «did it end well», so any other ending
  serves, and a bridge built over an empty channel leaves the reading with no listener while the
  program is alive. Two reviewers refuted it independently and the test exists
  (`a_console_whose_reading_ended_badly_does_not_close_the_window_on_its_own`). The transferable half:
  **«this state is unreachable» is a claim about the whole state space, and it was made from one
  corner of it.**
- 2026-09-11 · 192 · **A console that is quiet AND deaf is reported by a decision nothing wires up**
  — the sentence over the glass withholds «what you type still reaches it» when the writer has given
  up, and both sentences are pinned by tests. What is **not** pinned is the reading of `gave_up`
  itself: a write failure cannot be produced from outside `term-core` any more than a failed read
  can, so hardcoding that half to `false` leaves the suite green. Same shape, and same missing seam,
  as the entry above about the input half of `Silences::of`.
- 2026-09-11 · 192 · **Three tests of the notices' precedence no-op silently off Linux** — they open
  with `if !hold_a_paste_on(&mut pane) { return; }`, and the boundary a held paste needs is
  `const Some(4_095)` under `cfg(linux)` and `None` elsewhere. On macOS and Windows — **F2.7**, the
  next milestone — those three report green while asserting nothing. The helper's own docblock says
  the `false` return is not a pass; what is missing is a `#[cfg]` that makes the whole file's
  paste-dependent tests *absent* rather than empty, which is a change over all of them.
- 2026-09-11 · 192 · **The mouse pass hit-tests a notice against the previous frame's tiling** —
  inherited from the held paste's notice, whose comment records it and accepts it with its reason
  (the press is answered where the person aimed it, which is what they could see). With the second
  notice it doubles: on the one frame a producer first publishes a panel, a press over the panel's
  new columns can dismiss a sentence that was drawn under them.

