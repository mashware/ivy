//! The terminal state: bytes are interpreted and settle into a grid.

mod metadata;

/// Native application publishing terminal metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramClient {
    Claude,
    Codex,
}
/// Lifecycle explicitly declared by the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramState {
    Idle,
    Working,
    Blocked,
    Done,
    Error,
}
/// Declared reason for requiring input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramKind {
    Permission,
    Question,
    Auth,
}
/// The latest root declaration; it is not an authenticated identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramSnapshot {
    pub client: ProgramClient,
    pub state: ProgramState,
    pub kind: Option<ProgramKind>,
}

use alacritty_terminal::Term;
use alacritty_terminal::grid::{Dimensions, Grid, Scroll as GridScroll};
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Config;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::RegexIter;
use alacritty_terminal::vte::ansi::{CursorShape as AlacrittyShape, CursorStyle, Processor};

use crate::Size;
use crate::search::{Search, Toward};
use crate::view::{self, CellWidth, Cursor, Osc8Span, View, ViewCell, ViewMetadata};

mod title;

/// Dimensions in the vocabulary of `alacritty_terminal`.
///
/// It exists so that this trait need not be implemented on [`Size`], which is public: this way the
/// dependency does not surface in this crate's API. The crate ships an equivalent type, but it lives in
/// a module it itself declares "test helpers", and building on that is leaning on something
/// its author has not committed to maintaining.
struct Dims {
    rows: usize,
    cols: usize,
}

impl Dimensions for Dims {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

impl From<Size> for Dims {
    fn from(size: Size) -> Self {
        Self {
            rows: size.rows as usize,
            cols: size.cols as usize,
        }
    }
}

/// The screen of a split: it interprets terminal bytes and maintains the resulting grid.
///
/// It is fed by hand with [`advance`](Screen::advance), in the chunks that whoever uses it decides. That
/// is the difference with letting a thread update it on its own: whoever paints controls how much work
/// it does per pass, which is what lets a huge dump not swallow it whole in one go.
pub struct Screen {
    term: Term<title::Listener>,
    parser: Processor,
    titles: title::Listener,
    terminal_title: Option<String>,
    metadata: metadata::Metadata,
}

/// A cell of the **visible** grid, and which half of it is being talked about.
///
/// **Its own vocabulary**, for the same reason as [`Scroll`]: whoever uses this crate does not have to know
/// the engine's types.
///
/// **The coordinates are those of what is seen**, with the scrollback offset already applied — that is,
/// the same ones with which [`View`] is indexed. Whoever paints counts rows and columns from the top
/// left of what is in front of them, and forcing them to subtract the offset would be asking them to know it
/// exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub row: u16,
    pub col: u16,
    /// Which half of the cell is being talked about.
    ///
    /// **It is not a detail of precision: without this you cannot select a single character.** Starting and
    /// ending in the same cell gives an empty selection or a one-character one depending on which side you entered
    /// and which you leave by, which is exactly what the mouse does in any terminal.
    pub half: Half,
}

/// Which half of a cell is being talked about, to start or end a selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    Left,
    Right,
}

impl Half {
    /// Which of the two halves that fraction of the cell's width falls in.
    ///
    /// **It lives here and not in whoever paints** so that the rule —the exact half— is written once. Whoever
    /// paints knows points and cells; where the boundary between the two halves falls belongs to this crate.
    pub fn from_fraction(within: f32) -> Self {
        if within < 0.5 {
            Self::Left
        } else {
            Self::Right
        }
    }
}

/// Translates the cell half into the engine's vocabulary.
fn engine_side(half: Half) -> Side {
    match half {
        Half::Left => Side::Left,
        Half::Right => Side::Right,
    }
}

/// What the inner program has asked the terminal about the mouse.
///
/// **Its own vocabulary and not the engine's flags**, for the same reason as [`Scroll`] and
/// [`Cell`]: whoever uses this crate does not have to know its types nor migrate with them.
///
/// **What is here is what the program ASKS for, not what the window decides to do.** The order of
/// priorities —when scrollback wins, when the program does— belongs to whoever hands out the gestures, not to
/// this crate: here only the question is answered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MouseReporting {
    /// It wants to be told about pressing and releasing the buttons.
    pub buttons: bool,
    /// It wants to be told also while dragging with a button held down.
    pub drag: bool,
    /// It wants to be told about movement **even when no button is held down**.
    pub motion: bool,
    /// It wants the report in the extended encoding, the one with no column ceiling.
    pub extended: bool,
}

impl MouseReporting {
    /// Whether the program wants the mouse for anything.
    ///
    /// **`extended` does not count**: it says *how* it wants the report, not *whether* it wants it. A program that
    /// only turned on that flag has asked for nothing, and sending it reports would be talking to it without its
    /// asking.
    pub fn wants_mouse(&self) -> bool {
        self.buttons || self.drag || self.motion
    }
}

/// How far one scrolls up or down through a console's scrollback.
///
/// **Its own vocabulary, not the engine's**, for the same reason as the rest of this crate: whoever uses it
/// does not have to know its dependencies' types nor migrate with them.
///
/// The numbers are **lines**, positive toward the old — the direction in which one scrolls up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
    /// That many lines toward the old; negative, toward the new.
    Lines(i32),
    /// That many screens toward the old; negative, toward the new. **The screen is decided by the
    /// grid**, not a fixed number: a narrow, tall console does not have the same page as a
    /// wide, short one.
    Pages(i32),
    /// All the way toward the old.
    ToTop,
    /// All the way toward the new, which is where the program writes.
    ToBottom,
}

impl Scroll {
    /// Translates it to what the engine understands. **Private to the crate**: the engine's type does not leave
    /// here.
    fn to_engine(self, rows: u16) -> GridScroll {
        match self {
            Self::Lines(count) => GridScroll::Delta(count),
            // A page is the whole screen minus one line, which is what any
            // pager does: it leaves one of context so as not to lose the thread between screen and screen.
            // **At least one line** —on a console one row tall that rule would give a page of zero and
            // the keys would do nothing— and **saturating**, because the count comes from the user and
            // `Pages(i32::MAX)` has to reach the top of the history instead of overflowing the product.
            Self::Pages(count) => {
                GridScroll::Delta(count.saturating_mul(rows.saturating_sub(1).max(1) as i32))
            }
            Self::ToTop => GridScroll::Top,
            Self::ToBottom => GridScroll::Bottom,
        }
    }
}

/// How many lines each console remembers of what scrolled off the top.
///
/// **The number is set by this project and not inherited from the engine**, which ships 10,000. It was chosen by measuring
/// —the bench is in `examples/scrollback.rs`— and the whole decision, with the table and what was discarded, is
/// **D21** in `docs/02-decisions.md`. What you have to know to touch it:
///
/// - **It is WezTerm's default**, which is the bar this project measures itself against.
/// - **Relaying out the text when the width changes walks the whole scrollback.** With these lines and
///   content that wraps it is ~984 µs per console; with the engine's 10,000, 4.07 ms — and one
///   boundary moves **two** consoles, so it would be more than eight milliseconds per frame
///   **while being dragged**.
/// - **Keeping scrollback makes interpreting a dump 85 % more expensive**, from 182 to 98 MiB/s. That price
///   is declared and accepted in D21; it is not an oversight that someone could "fix" by lowering this
///   without reading the decision.
/// - **Up to 14.4 MiB per console** with the scrollback full at 180 columns, and there is no ceiling on
///   splits (**D14**).
pub const SCROLLBACK: usize = 3_500;

/// How many lines beyond the screen marking a search's matches may walk, to finish a match a wrap
/// carries across the edge. A hundred lines of a wide console is a few times the screen, far past
/// any match a person types, and a bound on what a frame can pay. See [`Screen::mark_matches`].
pub const MARKING_REACH: usize = 100;

impl Screen {
    /// Creates an empty screen of the given size.
    pub fn new(size: Size) -> Self {
        let config = Config {
            scrolling_history: SCROLLBACK,
            // **A terminal nobody has spoken to blinks**, which is what every other terminal on the
            // machine does and what the engine's own default does not do. This is the only place the
            // choice is made: `Term::cursor_style` falls back to this field, and `CSI 0 SP q` sets the
            // style back to `None` and lands here again, so a program keeps both of its doors — a
            // steady shape or DEC mode 12 to switch the blink off, `CSI 0 SP q` to hand it back.
            // The reason and what was discarded: **D37**.
            default_cursor_style: CursorStyle {
                shape: AlacrittyShape::Block,
                blinking: true,
            },
            ..Config::default()
        };

        let titles = title::Listener::default();
        Self {
            term: Term::new(config, &Dims::from(size), titles.clone()),
            parser: Processor::new(),
            titles,
            terminal_title: None,
            metadata: metadata::Metadata::default(),
        }
    }

    /// Whether the program inside is asking for a blinking cursor.
    ///
    /// **Apart from [`view`](Screen::view) because it costs a call and the view costs a snapshot of
    /// the whole grid.** Whoever paints needs this once per pane per frame to decide the ink, long
    /// before there is any reason to build a view.
    ///
    /// **It says what the program wants, never what is on screen.** The bit is independent of
    /// whether the cursor is shown at all, so a `true` here is not a promise that anything is drawn.
    pub fn cursor_blinks(&self) -> bool {
        self.term.cursor_style().blinking
    }

    /// Interprets a chunk of the terminal's output.
    ///
    /// It accepts any cut: the parser keeps its state between calls, so an escape
    /// sequence split across two deliveries produces the same result as a whole one. It is what makes
    /// it correct for the PTY to deliver partial reads.
    pub fn advance(&mut self, bytes: &[u8]) {
        self.metadata.advance(bytes);
        self.parser.advance(&mut self.term, bytes);
        if let Some(update) = self.titles.take() {
            self.terminal_title = update;
        }
    }

    /// The latest usable title explicitly published by this screen's program.
    ///
    /// This is transient terminal metadata, not an authenticated session identity. The parser's
    /// title events update it; a silent process exit does not clear it.
    /// Whether a file URI authority names this machine.
    pub fn is_local_file_host(host: &str) -> bool {
        metadata::is_local_host(host.as_bytes())
    }

    /// Whether this screen has received an OSC 7 directory report, including an unusable one.
    pub fn working_directory_reported(&self) -> bool {
        self.metadata.reported()
    }

    pub fn working_directory(&self) -> Option<&std::path::Path> {
        self.metadata.path()
    }

    /// Completed directory reports and resets advance this revision, even when the path repeats.
    pub fn working_directory_generation(&self) -> u64 {
        self.metadata.generation()
    }

    pub fn terminal_title(&self) -> Option<&str> {
        self.terminal_title.as_deref()
    }

    /// Enable native clients before feeding output; disabling the active client withdraws it.
    pub fn set_program_clients(&mut self, claude: bool, codex: bool) {
        self.metadata.enabled_claude = claude;
        self.metadata.enabled_codex = codex;
        if self.metadata.program.is_some_and(|s| match s.client {
            ProgramClient::Claude => !claude,
            ProgramClient::Codex => !codex,
        }) {
            self.metadata.program = None;
            self.metadata.program_generation = self.metadata.program_generation.wrapping_add(1);
        }
    }

    /// Latest valid native declaration.
    pub fn program_snapshot(&self) -> Option<ProgramSnapshot> {
        self.metadata.program
    }
    /// Revision of completed native declarations, clears and resets.
    pub fn program_generation(&self) -> u64 {
        self.metadata.program_generation
    }
    /// Take coalesced capability probes since the previous call.
    pub fn take_program_query(&mut self) -> bool {
        std::mem::take(&mut self.metadata.query)
    }

    /// Changes the grid's size and reflows its content.
    ///
    /// **It only changes the grid.** The program running inside the pseudoterminal does not find out by
    /// this route: for that there is [`Pty::resize`](crate::Pty::resize), and both must be called —
    /// **this one first**. As soon as the program knows the new size it starts sending bytes for it,
    /// and if the grid were still the old one they would be interpreted against the wrong dimensions.
    /// They are separated because they are two pieces that do not know each other, and hiding one inside
    /// the other would tie the grid to always having a pseudoterminal behind it — which is not the case in
    /// the tests, nor does it have to be later.
    pub fn resize(&mut self, size: Size) {
        self.term.resize(Dims::from(size));
    }

    /// The grid's current size.
    ///
    /// It is read from the grid itself instead of being stored separately: with two copies of the same datum,
    /// resizing and updating only one lets them diverge without anything failing.
    pub fn size(&self) -> Size {
        let grid = self.term.grid();
        Size {
            rows: grid.screen_lines() as u16,
            cols: grid.columns() as u16,
        }
    }

    /// Scrolls up or down through what scrolled off the top.
    ///
    /// **Nothing reaches the inner program**: looking at the scrollback is the terminal's business, and the
    /// program neither finds out nor has any reason to. It is what separates this from sending it the arrows.
    ///
    /// **And the offset stays where it is even if output keeps coming in.** The engine itself
    /// sustains it: while there is an offset, what arrives accumulates below without moving what is being
    /// looked at. It is what one expects when stopping to read something while a program keeps writing.
    pub fn scroll_by(&mut self, amount: Scroll) {
        self.term.scroll_display(amount.to_engine(self.size().rows));
    }

    /// Whether the inner program has asked that what is pasted reach it **marked as pasted**.
    ///
    /// A shell that asks for it does not execute what it receives that way even if it carries line breaks: it leaves it
    /// written so it can be reviewed before hitting Enter. Without the question, whoever pastes several lines
    /// runs them one by one without having seen them, which is what this accessor exists to prevent.
    ///
    /// **It returns `bool` and not the engine's type on purpose.** This crate exists to survive the
    /// interface being rewritten, and it achieves that by not letting its dependencies' types surface through its
    /// public API: whoever uses it does not have to know them nor migrate with them.
    pub fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Whether the inner program has asked for **application cursor keys** (DECCKM, `CSI ?1h`).
    ///
    /// It is what a curses program sends on starting, through the `smkx` of the `TERM` this crate
    /// advertises, and from then on terminfo tells it that Home is `ESC O H` rather than `ESC [ H`.
    /// A `bool` and not the engine's flags, for the same reason as
    /// [`bracketed_paste`](Screen::bracketed_paste).
    pub fn application_cursor_keys(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// What the inner program has asked the terminal about the mouse.
    ///
    /// **It returns its own type and not the engine's flags**, for the same reason as
    /// [`bracketed_paste`](Screen::bracketed_paste): this crate exists to survive the interface
    /// being rewritten, and it achieves that by not letting its dependencies' types surface through its API.
    pub fn mouse_reporting(&self) -> MouseReporting {
        let mode = self.term.mode();
        MouseReporting {
            buttons: mode.contains(TermMode::MOUSE_REPORT_CLICK),
            drag: mode.contains(TermMode::MOUSE_DRAG),
            motion: mode.contains(TermMode::MOUSE_MOTION),
            extended: mode.contains(TermMode::SGR_MOUSE),
        }
    }

    /// Whether turning the wheel has to reach the program **as arrows**.
    ///
    /// It is what makes `less` and `man` scroll with the wheel without asking for the mouse: they ask for the
    /// alternate screen and turn on alternate scrolling, and the terminal translates the turn into
    /// the keys they already know how to attend to.
    ///
    /// **Alternate scrolling comes turned on out of the box** —it is in the engine's default
    /// mode, read and not assumed, just as `xterm` does—, so in practice what decides is the
    /// alternate screen. The flag is still queried because a program **can turn it off**: `vim` does
    /// so as soon as it wants the mouse for itself, and giving it arrows then would move its insertion point
    /// without anyone touching a key.
    pub fn wheel_acts_as_arrows(&self) -> bool {
        let mode = self.term.mode();
        mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL)
    }

    /// A snapshot of the visible grid: all the cells and the cursor.
    ///
    /// It is the reader that whoever paints uses, and it **does not touch the screen**: unlike querying the
    /// damage, it can be called as many times as needed and always returns the same while no
    /// new bytes come in. That is what lets the same frame be painted twice without the
    /// second coming out blank.
    ///
    /// It coexists with [`to_text`](Screen::to_text) instead of replacing it: they are two distinct contracts
    /// —one gives legible text, the other cells with attributes— and merging them is asked for by no one.
    pub fn view(&self) -> View {
        let size = self.size();
        let cols = size.cols as usize;
        let mut cells = vec![ViewCell::default(); size.rows as usize * cols];
        // Empty in the common case: it only grows with the cells that carry something attached.
        let mut combining = Vec::new();
        let mut row_wraps = vec![false; size.rows as usize];
        let mut osc8 = Vec::<Osc8Span>::new();

        // The visible iterator cannot tell whether its first row continues one just outside it.
        // Read that one boundary while the engine coordinates are still available, then carry only
        // our boolean into the snapshot.
        let grid = self.term.grid();
        let display_offset = grid.display_offset() as i32;
        let history = grid.history_size() as i32;
        let top_line = -display_offset;
        let previous_line = top_line - 1;
        let continued_from_above = previous_line >= -history
            && grid[Line(previous_line)][Column(cols.saturating_sub(1))]
                .flags
                .contains(Flags::WRAPLINE);
        let hyperlink_above = (previous_line >= -history)
            .then(|| grid[Line(previous_line)][Column(cols.saturating_sub(1))].hyperlink())
            .flatten();
        let next_line = size.rows as i32 - display_offset;
        let hyperlink_below = (next_line < grid.screen_lines() as i32)
            .then(|| grid[Line(next_line)][Column(0)].hyperlink())
            .flatten();

        let contents = self.term.renderable_content();
        // **The selection range is pulled out ONCE and queried per cell**, and whoever answers is the
        // engine: its two fine rules —a wide character's companion goes with its head, and the block
        // cursor is not highlighted at the ends— are not redone here.
        let selection = contents.selection;
        let cursor_shape = contents.cursor.shape;
        // The iterator's coordinates and the cursor's go in the same system, which counts from
        // the start of the scrollback. Adding the offset brings them to visible coordinates, which
        // are the ones whoever paints can index with. For the **cells** the sum is enough —the
        // iterator only walks the visible—; for the **cursor** it is not, because it can end up outside:
        // see below.
        let scroll = contents.display_offset as i32;

        // **Whether the previous cell was the head of a wide character AND was highlighted.** See below why
        // it is necessary to keep track.
        let mut wide_head_was_highlighted = false;
        for indexed in contents.display_iter {
            let row = indexed.point.line.0 + scroll;
            let col = indexed.point.column.0;
            // The iterator does not leave the visible —checked against its source—, so this should
            // never fire. It stays as an assertion and not as a silent skip: if a
            // future version changed the range, it would trip in the tests instead of leaving blank
            // rows that no one could explain.
            debug_assert!(
                row >= 0 && row < size.rows as i32 && col < cols,
                "the iterator left the visible: row {row}, column {col}"
            );
            if row < 0 || row >= size.rows as i32 || col >= cols {
                continue;
            }
            let selected = selection.is_some_and(|range| {
                range.contains_cell(&indexed, contents.cursor.point, cursor_shape)
            });
            let mut cell = view::cell_from(indexed.cell, selected, &mut combining);
            if indexed.cell.flags.contains(Flags::WRAPLINE) {
                row_wraps[row as usize] = true;
            }
            if let Some(hyperlink) = indexed.cell.hyperlink() {
                let flat = row as usize * cols + col;
                let continues_last = osc8
                    .last()
                    .is_some_and(|span| span.end == flat && span.target == hyperlink.uri());
                if continues_last {
                    let span = osc8
                        .last_mut()
                        .expect("the span was present one line above");
                    span.end += 1;
                } else {
                    osc8.push(Osc8Span {
                        start: flat,
                        end: flat + 1,
                        target: hyperlink.uri().to_owned(),
                    });
                }
            }
            // **A wide character's companion is highlighted along with its head, and THIS crate sets that.**
            // The engine only does the opposite path —it marks the head when the companion falls inside,
            // so that its text does not come out cut off— and that is enough for it because its painter draws the background of a
            // wide character two cells wide. Ours paints **the background of each cell
            // separately**, so without this a wide character at the edge of the selection comes out with half a
            // letter highlighted and half not, right where there is CJK or an emoji.
            //
            // It is carried with a flag and not with a second pass because the walk is by rows and the
            // companion comes **right behind** its head: one more pass over the whole grid is
            // paid on every frame to fix two cells.
            if cell.width == CellWidth::WideTail && wide_head_was_highlighted {
                cell.selected = true;
            }
            wide_head_was_highlighted = cell.selected && cell.width == CellWidth::WideHead;
            cells[row as usize * cols + col] = cell;
        }

        // **The cursor may not be visible, and now that is said instead of clamped.** Scrolling up through the
        // scrollback the cursor stays in the active area, which sits below what is being looked at:
        // clamping it to the last row would draw it in a place where it is not, and whoever looked would believe
        // that the program writes there. This is exactly what this spot's comment
        // announced ever since F1.2, when the scrollback did not exist.
        let cursor_row = contents.cursor.point.line.0 + scroll;
        let cursor = (0..size.rows as i32).contains(&cursor_row).then(|| Cursor {
            row: cursor_row as u16,
            col: (contents.cursor.point.column.0 as u16).min(size.cols.saturating_sub(1)),
            shape: view::shape_from(contents.cursor.shape),
        });

        // A semantic label cut by the top or bottom of the retained snapshot is not enough to
        // activate its hidden target. Refuse only the boundary span; complete links elsewhere stay
        // usable even when the logical terminal line itself is longer than the viewport.
        osc8.retain(|span| {
            let clipped_above = span.start == 0
                && hyperlink_above
                    .as_ref()
                    .is_some_and(|hyperlink| hyperlink.uri() == span.target);
            let clipped_below = span.end == cells.len()
                && hyperlink_below
                    .as_ref()
                    .is_some_and(|hyperlink| hyperlink.uri() == span.target);
            !clipped_above && !clipped_below
        });

        View::new(
            size,
            cells,
            combining,
            ViewMetadata {
                row_wraps,
                continued_from_above,
                osc8,
                working_directory: self.working_directory().map(std::path::Path::to_path_buf),
            },
            cursor,
            contents.display_offset,
        )
    }

    /// Starts a selection at that cell, **throwing away any that existed**.
    ///
    /// **The point is from the visible grid** and here it is converted to the engine's system, which counts from
    /// the start of the scrollback: subtracting the offset is what makes selecting something that
    /// scrolled off the top mark **that** and not what occupies its place on screen today.
    ///
    /// **It is not checked that the cell exists.** The engine clamps the point on its own when converting it to a
    /// range, and a cell outside the grid is not a caller's error: it is the mouse leaving the
    /// console while dragging, which is the normal case.
    pub fn start_selection(&mut self, at: Cell) {
        self.term.selection = Some(Selection::new(
            SelectionType::Simple,
            self.engine_point(at),
            engine_side(at.half),
        ));
    }

    /// Extends the started selection up to that cell, if there is one.
    ///
    /// **With no started selection it does nothing, and that is not a just-in-case guard**: the drag is
    /// attended on every frame while the button is held down, and the very first is the one that
    /// starts it. The rest arrive here without anyone having released anything.
    pub fn extend_selection(&mut self, to: Cell) {
        let point = self.engine_point(to);
        let half = engine_side(to.half);
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, half);
        }
    }

    /// Throws away any selection that existed. With no selection, it does nothing.
    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// Whether there is something selected right now.
    ///
    /// **It is not the same as "a selection has been started"**: dragging and returning to the starting point leaves
    /// a started and **empty** selection, and nothing is copied from that. Whoever asks to decide whether to
    /// copy has to ask this.
    pub fn has_selection(&self) -> bool {
        self.term
            .selection
            .as_ref()
            .is_some_and(|selection| !selection.is_empty())
    }

    /// The text of what is selected, or nothing if there is no selection.
    ///
    /// **The rules of what "the text" is belong to the engine and are written in D22 of the decision log**:
    /// the lines that wrap
    /// are joined **with no break**, the padding at the end of each line is **trimmed**, a wide
    /// character's companion **is not duplicated** and a tab's gap is skipped. Inheriting the code is
    /// correct; inheriting the decision without saying it, not.
    ///
    /// **It does not filter control characters**, unlike pasting: what comes out of here are characters from
    /// cells and the engine does not keep escapes. Pasting filters because it protects the inner program from what
    /// comes in; here the direction is the opposite.
    pub fn selected_text(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    /// Marks **every match of `search` on screen** in a snapshot [`view`](Screen::view) took, and
    /// says how many cells it marked.
    ///
    /// **A step after `view` and not a flag on it**, so a console with no search open pays nothing,
    /// and so whoever paints can time this walk apart from taking the snapshot (D13, D61).
    ///
    /// **The walk starts at the beginning of the logical line the top row belongs to and ends at the
    /// end of the one the bottom row belongs to**, so a match that a wrap carries across the top or
    /// bottom edge is marked on the part that shows — **but never more than [`MARKING_REACH`] lines
    /// beyond the screen.** A single line wrapped across the whole history (minified JSON, a dump
    /// with no newline) would otherwise make every frame walk all of it, which is the per-frame cost
    /// D61 says the search does not have.
    pub fn mark_matches(&self, view: &mut View, search: &mut Search) -> usize {
        let grid = self.term.grid();
        let rows = grid.screen_lines() as i32;
        let offset = grid.display_offset() as i32;
        debug_assert!(
            view.size() == self.size() && view.scroll() == offset as usize,
            "the snapshot is not this screen's as it is now, so its cells are not where the walk marks"
        );
        let (start, end) = self.marking_bounds();
        let mut marked = 0;
        for found in RegexIter::new(start, end, Direction::Right, &self.term, &mut search.regex) {
            let mut point = *found.start();
            loop {
                let row = point.line.0 + offset;
                if row >= rows {
                    break;
                }
                if row >= 0 {
                    view.mark_matched(row as usize, point.column.0);
                    marked += 1;
                }
                if point >= *found.end() {
                    break;
                }
                point = point.add(&self.term, Boundary::None, 1);
            }
        }
        marked
    }

    /// Where marking walks: from the start of the logical line the top row belongs to, to the end
    /// of the one the bottom row belongs to — **following the wraps itself, and stopping
    /// [`MARKING_REACH`] lines beyond the screen.** The engine's own search for a line's ends
    /// follows a wrap to its end whatever its length, so clamping its answer would still pay its
    /// walk; this one never takes the step past the reach.
    fn marking_bounds(&self) -> (Point, Point) {
        let grid = self.term.grid();
        let rows = grid.screen_lines() as i32;
        let offset = grid.display_offset() as i32;
        let oldest = Line(-(grid.history_size() as i32));
        let newest = Line(rows - 1);
        let last = grid.last_column();
        let wraps = |line: Line| grid[line][last].flags.contains(Flags::WRAPLINE);

        let top = Line(-offset);
        let mut start = top;
        while start > oldest && start > top - MARKING_REACH && wraps(start - 1) {
            start -= 1;
        }
        let bottom = Line(rows - 1 - offset);
        let mut end = bottom;
        while end < newest && end < bottom + MARKING_REACH && wraps(end) {
            end += 1;
        }
        (Point::new(start, Column(0)), Point::new(end, last))
    }

    /// Goes to the next match of `search` that way, scrolls to it and **makes it the selection**.
    ///
    /// **The match is the selection on purpose**: copying it then goes through the same four rules
    /// as anything else copied (D22), with nothing of its own to get wrong. It also answers where
    /// the next step starts — from the selection, which the engine rotates as output arrives and
    /// drops when its lines leave the history — so no position is kept anywhere else to go stale.
    ///
    /// **The step starts one cell past the current match.** The engine counts a match that sits
    /// exactly on the point it is handed, so starting on the match itself would find it again, on
    /// every press, for ever. With nothing selected it starts at the edge of what is being looked
    /// at, so the first press finds what is nearest to the person's eyes.
    ///
    /// **When the view is not already at the bottom, the match is kept off the last row**, which is
    /// where the search's own line is painted over the console. At the bottom there is nothing
    /// newer to scroll in, and the painter moves its line instead.
    ///
    /// Returns whether there was a match. **When there was none nothing moves**: neither the
    /// selection somebody had nor where they were looking.
    pub fn find(&mut self, search: &mut Search, toward: Toward) -> bool {
        let rows = self.term.grid().screen_lines() as i32;
        let offset = self.term.grid().display_offset() as i32;
        let current = self
            .term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term));
        let (direction, side, origin) = match (toward, current) {
            (Toward::Older, Some(range)) => (
                Direction::Left,
                Side::Left,
                range.start.sub(&self.term, Boundary::None, 1),
            ),
            (Toward::Newer, Some(range)) => (
                Direction::Right,
                Side::Right,
                range.end.add(&self.term, Boundary::None, 1),
            ),
            (Toward::Older, None) => (
                Direction::Left,
                Side::Left,
                Point::new(Line(rows - 1 - offset), self.term.grid().last_column()),
            ),
            (Toward::Newer, None) => (
                Direction::Right,
                Side::Right,
                Point::new(Line(-offset), Column(0)),
            ),
        };

        let Some(found) = self
            .term
            .search_next(&mut search.regex, origin, direction, side, None)
        else {
            return false;
        };
        let (start, end) = (*found.start(), *found.end());
        let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
        selection.update(end, Side::Right);
        self.term.selection = Some(selection);

        self.term.scroll_to_point(end);
        self.term.scroll_to_point(start);
        let offset = self.term.grid().display_offset() as i32;
        if end.line.0 == rows - 1 - offset && offset > 0 {
            self.term.scroll_display(GridScroll::Delta(-1));
        }
        true
    }

    /// Brings a visible cell into the engine's coordinate system.
    fn engine_point(&self, cell: Cell) -> Point {
        let scroll = self.term.grid().display_offset() as i32;
        Point::new(
            Line(i32::from(cell.row) - scroll),
            Column(usize::from(cell.col)),
        )
    }

    /// Dumps **the active area** to text — where the program writes, not necessarily what is
    /// being looked at.
    ///
    /// **The difference matters ever since there is scrollback, and that is why it is said this way.** Scrolling up through what
    /// scrolled off the top, what is seen is **not** the active area; this still returns the active
    /// area, which is what whoever checks what a program wrote wants. Whoever needs what
    /// is seen has [`view`](Screen::view), which does carry the offset applied.
    ///
    /// It trims the trailing spaces of each line, discards the empty lines at the end and joins with
    /// line breaks, with no break at the end. A screen with no content returns the empty string.
    ///
    /// The trailing empty lines are discarded because a screen almost always has its bottom half
    /// blank, and dragging those breaks along turns any comparison into a count of
    /// invisible lines. How many rows the grid has is said by [`size`](Screen::size), which is
    /// where that information does not get in the way.
    pub fn to_text(&self) -> String {
        let grid = self.term.grid();
        let mut lines: Vec<String> = Vec::with_capacity(grid.screen_lines());

        for row in 0..grid.screen_lines() {
            lines.push(a_text_line(grid, row as i32));
        }

        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }

        lines.join("\n")
    }

    /// Dumps **everything the console still holds, the history included**, oldest line first.
    ///
    /// **The scrollback is where the answer to «what did it say before» lives**, and until now it was
    /// reachable only by eye: [`to_text`](Screen::to_text) returns the active area, which is what the
    /// program wrote, and an agent that reads a console it opened needs what can still be read. The
    /// history is the engine's, up to the ceiling **D21** chose, and it comes out with the same rules
    /// [`to_text`](Screen::to_text) uses — a wide character once, its zero-width scalars kept,
    /// trailing spaces trimmed.
    ///
    /// **The blank rows of a console that has not scrolled are dropped**, at both ends and for the
    /// same reason `to_text` drops the trailing ones: dragging invisible lines along turns any
    /// comparison into a count of them.
    pub fn scrollback_text(&self) -> String {
        let grid = self.term.grid();
        let history = grid.history_size() as i32;
        let mut lines: Vec<String> = Vec::with_capacity(history as usize + grid.screen_lines());

        for row in -history..grid.screen_lines() as i32 {
            lines.push(a_text_line(grid, row));
        }

        let first = lines
            .iter()
            .position(|line| !line.is_empty())
            .unwrap_or(lines.len());
        lines.drain(..first);
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }

        lines.join("\n")
    }
}

/// One row of the grid as text, with the two rules the two dumps share.
///
/// **A wide character is emitted once** —its companion cell carries the flag and no content, and
/// copying it would duplicate the glyph— and **the zero-width scalars attached behind a cell are
/// kept**, because the program wrote them: without that, `☁️` comes out as `☁`, which is a different
/// character from the one on screen. It is the rule **D22** sets for copying.
fn a_text_line(grid: &Grid<alacritty_terminal::term::cell::Cell>, row: i32) -> String {
    let row = &grid[Line(row)];
    let mut line = String::with_capacity(grid.columns());
    for col in 0..grid.columns() {
        let cell = &row[Column(col)];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        line.push(cell.c);
        line.extend(cell.zerowidth().into_iter().flatten());
    }
    line.trim_end().to_string()
}

#[cfg(test)]
mod tests {

    #[test]
    fn marking_never_walks_further_than_its_reach_beyond_the_screen() {
        // One line wrapped across far more rows than the reach, with the screen in its middle: the
        // walk would otherwise follow it to both ends on every frame.
        let mut screen = Screen::new(Size { rows: 4, cols: 10 });
        screen.advance("x".repeat(10 * 400).as_bytes());
        screen.scroll_by(Scroll::Lines(200));
        let top = -(screen.view().scroll() as i32);

        let (start, end) = screen.marking_bounds();

        assert_eq!(
            start.line.0,
            top - super::MARKING_REACH as i32,
            "the walk up passed its reach"
        );
        assert_eq!(
            end.line.0,
            top + 3 + super::MARKING_REACH as i32,
            "the walk down passed its reach"
        );
    }
    use super::*;
    use crate::{CellWidth, CursorShape, TermColor, Underline};

    /// A cell to start or end a selection by.
    fn cell(row: u16, col: u16, half: Half) -> Cell {
        Cell { row, col, half }
    }

    /// Selects from the first cell to the last **included**, which is what a drag does.
    fn select(screen: &mut Screen, from: (u16, u16), to: (u16, u16)) {
        screen.start_selection(cell(from.0, from.1, Half::Left));
        screen.extend_selection(cell(to.0, to.1, Half::Right));
    }

    #[test]
    fn a_line_that_wraps_is_copied_without_the_break_that_no_one_wrote() {
        // **The first of the rules of what "the text" is, and the most noticeable.** A line
        // longer than the grid is split by the terminal, not the program: copying it with a break inside
        // returns something that **no one wrote**, and pasting it into a shell runs two commands instead of one.
        let mut screen = Screen::new(Size { rows: 4, cols: 10 });
        screen.advance(b"0123456789abcde");

        select(&mut screen, (0, 0), (1, 4));

        assert_eq!(
            screen.selected_text().as_deref(),
            Some("0123456789abcde"),
            "the wrapped line was copied split"
        );
    }

    #[test]
    fn two_real_lines_are_copied_with_their_break() {
        // **The other side of the rule, and without this test the one above is satisfied by removing all the
        // breaks.** What distinguishes a wrapped line from two lines is who put the ending: the
        // terminal or the program.
        let mut screen = Screen::new(Size { rows: 4, cols: 10 });
        screen.advance(b"one\r\ntwo");

        select(&mut screen, (0, 0), (1, 2));

        assert_eq!(
            screen.selected_text().as_deref(),
            Some("one\ntwo"),
            "two lines written by the program were glued into one"
        );
    }

    #[test]
    fn the_padding_at_the_end_of_the_line_is_not_copied() {
        // **Selecting up to the right edge does not drag along the spaces no one wrote.** A
        // grid is full of empty cells to the right of what was written; copying them turns any
        // paste into a line with a tail of spaces.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"good");

        select(&mut screen, (0, 0), (0, 19));

        assert_eq!(
            screen.selected_text().as_deref(),
            Some("good"),
            "the padding at the end of the line was copied"
        );
    }

    #[test]
    fn a_tabs_gap_is_not_copied_as_spaces() {
        // **The fourth rule of what "the text" is.** A tab leaves empty cells up to the next
        // stop; copying them as spaces turns an aligned table into a row of loose spaces, and
        // pasting it anywhere that tabs again knocks it out of place.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"a\tb");

        select(&mut screen, (0, 0), (0, 8));

        assert_eq!(
            screen.selected_text().as_deref(),
            Some("a\tb"),
            "the tab's gap was copied as spaces"
        );
    }

    #[test]
    fn a_wide_characters_companion_is_not_duplicated_when_copying() {
        // **A wide character takes up two cells and is ONE.** The second is its companion: copying both
        // would return the character repeated, which is what you see when pasting CJK or an emoji.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance("日本".as_bytes());

        select(&mut screen, (0, 0), (0, 3));

        assert_eq!(
            screen.selected_text().as_deref(),
            Some("日本"),
            "the wide character's companion was copied as a character"
        );
    }

    #[test]
    fn the_highlight_of_a_wide_character_covers_its_two_cells() {
        // **A wide character takes up two cells and is seen as one thing**, so highlighting only the first
        // splits it down the middle: half a letter over the selection's background and half over the
        // console's. Who decides this is the engine —the companion goes with its head—, and this test is what
        // prevents redoing the rule here with a coordinate comparison that does not know it.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance("日x".as_bytes());

        // Only the HEAD of the wide character: column 1 is its companion and is not touched.
        screen.start_selection(cell(0, 0, Half::Left));
        screen.extend_selection(cell(0, 0, Half::Right));

        let view = screen.view();
        let highlighted: Vec<usize> = (0..3)
            .filter(|col| {
                view.cell(0, *col as u16)
                    .expect("the cell is in the grid")
                    .selected
            })
            .collect();

        assert_eq!(
            highlighted,
            vec![0, 1],
            "the highlight did not cover the wide character's two cells: it looks split down the middle"
        );
    }

    #[test]
    fn marking_a_wide_characters_companion_also_highlights_its_head() {
        // **The opposite path from the one above, and the engine sets it.** Dragging from the right you
        // enter through the companion before the head: without this rule, the gesture's first frame
        // would highlight the character's empty half and not the one that carries the letter. Redoing the highlight here
        // with a coordinate comparison takes this rule out without anything complaining.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance("日x".as_bytes());

        // Only the COMPANION: column 1.
        screen.start_selection(cell(0, 1, Half::Left));
        screen.extend_selection(cell(0, 1, Half::Right));

        let view = screen.view();
        assert!(
            view.cell(0, 0).expect("the cell is in the grid").selected,
            "the companion was marked and its head —the one carrying the letter— was left unhighlighted"
        );
    }

    #[test]
    fn dragging_and_returning_to_the_starting_point_leaves_nothing_to_copy() {
        // **"There is a started selection" and "there is something selected" are not the same**, and whoever decides whether to
        // copy has to ask the second: a bare click leaves the first set, and copying then
        // would return the cell under the pointer without anyone having selected it.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"good night");

        screen.start_selection(cell(0, 2, Half::Left));
        screen.extend_selection(cell(0, 2, Half::Left));

        assert!(
            !screen.has_selection(),
            "a click without dragging left something selected"
        );
    }

    #[test]
    fn changing_columns_clears_the_selection_and_keeping_them_keeps_it() {
        // **The two halves of the same rule inherited from the engine, and both are needed.** When the
        // number of columns changes the text relays out: what was marked stops being where it was, and
        // keeping the range would leave highlighted a stretch no one chose. With the same columns nothing
        // relays out, so the selection **survives** — and without this second half, "always clear"
        // would pass the test.
        let mut screen = Screen::new(Size { rows: 6, cols: 20 });
        screen.advance(b"good night");
        select(&mut screen, (0, 0), (0, 3));
        assert!(screen.has_selection(), "nothing ended up being selected");

        screen.resize(Size { rows: 3, cols: 20 });
        assert!(
            screen.has_selection(),
            "changing only the rows cleared the selection: nothing relays out, so there is no reason"
        );

        screen.resize(Size { rows: 3, cols: 10 });
        assert!(
            !screen.has_selection(),
            "changing columns left the selection set, highlighting a stretch no one chose"
        );
    }

    #[test]
    fn jumping_to_the_alternate_screen_clears_the_selection() {
        // **What was marked is not on the new screen.** A full-screen program —`vim`,
        // `less`, `htop`— switches to the alternate one on startup, and a selection that survived would end up
        // highlighting cells of content that no longer exists.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"good night");
        select(&mut screen, (0, 0), (0, 3));
        assert!(screen.has_selection(), "nothing ended up being selected");

        screen.advance(b"\x1b[?1049h");

        assert!(
            !screen.has_selection(),
            "the selection survived the jump to the alternate screen"
        );
    }

    #[test]
    fn clearing_a_region_that_crosses_the_selection_clears_it() {
        // **The program cleared what was marked.** If the selection survived, it would keep highlighting
        // empty cells and copying them would return nothing — or worse, what the program writes there afterward.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"good night");
        select(&mut screen, (0, 0), (0, 3));
        assert!(screen.has_selection(), "nothing ended up being selected");

        // Clear the whole screen, which is what `clear` does.
        screen.advance(b"\x1b[2J");

        assert!(
            !screen.has_selection(),
            "what was marked was cleared and the selection stayed there"
        );
    }

    #[test]
    fn what_is_seen_is_selected_also_when_scrolled_up_through_the_history() {
        // **The selection's coordinates are those of what is LOOKED at**, not those of the active area. Scrolled
        // up through the scrollback, marking the first row has to give the old line being looked at; with
        // the coordinates untranslated it would give the one occupying that place in the active area today.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }
        screen.scroll_by(Scroll::ToTop);

        select(&mut screen, (0, 0), (0, 7));

        assert_eq!(
            screen.selected_text().as_deref(),
            Some("entry 0"),
            "selection went by active-area coordinates instead of by what is seen"
        );
    }

    #[test]
    fn the_program_can_ask_for_and_withdraw_that_pasted_text_reach_it_marked() {
        // On this mode depends whether pasting several lines is left written instead of being run line by
        // line, so it has to be possible to ask it — and for the answer to follow the program, which
        // sets it on starting an interactive shell and removes it on handing the terminal over to something else.
        let mut screen = Screen::new(Size::default());
        assert!(
            !screen.bracketed_paste(),
            "no one has asked for it and it is already set"
        );

        screen.advance(b"\x1b[?2004h");
        assert!(
            screen.bracketed_paste(),
            "it was asked for and it did not find out"
        );

        screen.advance(b"\x1b[?2004l");
        assert!(
            !screen.bracketed_paste(),
            "it was withdrawn and it stayed set"
        );
    }

    #[test]
    fn the_program_can_ask_for_and_withdraw_application_cursor_keys() {
        // Home and End change form while this is on, so the answer has to follow the program: a
        // curses program turns it on when it starts and off when it hands the terminal back.
        let mut screen = Screen::new(Size::default());
        assert!(
            !screen.application_cursor_keys(),
            "no one has asked for it and it is already set"
        );

        screen.advance(b"\x1b[?1h");
        assert!(
            screen.application_cursor_keys(),
            "it was asked for and it did not find out"
        );

        screen.advance(b"\x1b[?1l");
        assert!(
            !screen.application_cursor_keys(),
            "it was withdrawn and it stayed set"
        );
    }

    #[test]
    fn a_freshly_created_screen_has_no_content() {
        let screen = Screen::new(Size::default());

        assert_eq!(screen.to_text(), "");
        // The size is not lost by dumping empty: it is said by `size`, not the text.
        assert_eq!(screen.size(), Size::default());
    }

    #[test]
    fn the_trailing_empty_lines_are_not_dragged_along() {
        let mut screen = Screen::new(Size::default());

        screen.advance(b"primera\r\nsegunda");

        // Two lines with content, not 24 with twenty-two blank behind.
        assert_eq!(screen.to_text(), "primera\nsegunda");
    }

    #[test]
    fn the_plain_text_appears_without_escape_sequences() {
        let mut screen = Screen::new(Size::default());

        screen.advance(b"\x1b[31mgood\x1b[0m");

        let text = screen.to_text();
        assert!(text.starts_with("good"), "unexpected text: {text:?}");
        assert!(
            !text.contains('\x1b'),
            "the dump must not carry raw escapes: {text:?}"
        );
    }

    #[test]
    fn resizing_leaves_the_requested_size_and_keeps_the_content() {
        let mut screen = Screen::new(Size::default());
        screen.advance(b"good");

        screen.resize(Size {
            rows: 30,
            cols: 100,
        });

        assert_eq!(
            screen.size(),
            Size {
                rows: 30,
                cols: 100
            }
        );
        assert!(
            screen.to_text().starts_with("good"),
            "the content did not survive the resize: {:?}",
            screen.to_text()
        );
    }

    #[test]
    fn the_view_is_dense_and_the_size_of_the_grid() {
        let screen = Screen::new(Size { rows: 3, cols: 10 });

        let view = screen.view();

        assert_eq!(view.size(), Size { rows: 3, cols: 10 });
        assert_eq!(view.cells().len(), 30);
        assert_eq!(view.row(2).expect("the last row is missing").len(), 10);
        assert!(view.row(3).is_none(), "there should not be a fourth row");
        assert!(
            view.cell(0, 10).is_none(),
            "there should not be a column 10"
        );
    }

    #[test]
    fn a_wide_character_takes_up_two_columns_and_the_second_keeps_its_gap() {
        // It is the reason the view is dense: if the companion were omitted, whoever paints would have
        // to keep track of the offset, and that is where a row with CJK gets knocked out of place.
        let mut screen = Screen::new(Size::default());
        screen.advance("漢a".as_bytes());

        let view = screen.view();

        let first = view.cell(0, 0).expect("cell 0 is missing");
        assert_eq!(first.c, '漢');
        assert_eq!(first.width, CellWidth::WideHead);

        let companion = view.cell(0, 1).expect("the companion is missing");
        assert_eq!(
            companion.width,
            CellWidth::WideTail,
            "the wide character's second column has to be in the view"
        );

        // And the next letter falls in column 2, not column 1: the width did not knock the row out of place.
        let next = view.cell(0, 2).expect("cell 2 is missing");
        assert_eq!(next.c, 'a');
        assert_eq!(next.width, CellWidth::Single);
    }

    #[test]
    fn the_gap_left_by_a_wide_one_that_does_not_fit_is_distinct_from_a_space() {
        // Four columns: "abc" fills three and the wide character needs two. It does not fit, so the
        // terminal leaves the last one blank and moves it down whole. That blank was not written by the
        // program, and it is the only sign that the row does not end where it seems.
        let mut screen = Screen::new(Size { rows: 2, cols: 4 });
        screen.advance("abc漢".as_bytes());

        let view = screen.view();

        assert_eq!(
            view.cell(0, 3).expect("the last column is missing").width,
            CellWidth::WrapFiller,
            "the wrap gap has to be distinct from a normal space"
        );
        assert_eq!(view.cell(1, 0).expect("row 1 is missing").c, '漢');
        assert_eq!(
            view.cell(1, 0).expect("row 1 is missing").width,
            CellWidth::WideHead
        );
    }

    #[test]
    fn inverse_video_arrives_already_resolved() {
        let mut normal = Screen::new(Size::default());
        normal.advance(b"X");
        let normal = *normal.view().cell(0, 0).expect("the cell is missing");

        let mut inverse = Screen::new(Size::default());
        inverse.advance(b"\x1b[7mX");
        let inverse = *inverse.view().cell(0, 0).expect("the cell is missing");

        assert_eq!(
            (inverse.fg, inverse.bg),
            (normal.bg, normal.fg),
            "inverse video has to arrive with the colors already swapped"
        );
    }

    #[test]
    fn the_attributes_and_the_color_arrive_unresolved() {
        let mut screen = Screen::new(Size::default());
        // Named red, bold, dim, italic, underlined and struck through all at once.
        screen.advance(b"\x1b[31;1;2;3;4;9mX");

        let cell = *screen.view().cell(0, 0).expect("the cell is missing");

        assert_eq!(
            cell.fg,
            TermColor::Indexed(1),
            "named red is entry 1 of the palette"
        );
        assert!(cell.bold && cell.dim && cell.italic && cell.strikeout);
        assert_eq!(cell.underline, Underline::Single);
    }

    #[test]
    fn the_five_underlines_are_distinguished() {
        // With a boolean, an LSP diagnostic in `nvim` —which is curly— reached the view with
        // no mark and there was no way to recover it.
        let cases = [
            (&b"\x1b[4m"[..], Underline::Single),
            // The double one is `4:2`, not the `21` you read about around: for this parser `21` is cancelling
            // the bold. Checked in its attribute table, not assumed.
            (&b"\x1b[4:2m"[..], Underline::Double),
            (&b"\x1b[4:3m"[..], Underline::Curl),
            (&b"\x1b[4:4m"[..], Underline::Dotted),
            (&b"\x1b[4:5m"[..], Underline::Dashed),
            (&b"\x1b[24m"[..], Underline::None),
        ];

        for (sequence, expected) in cases {
            let mut screen = Screen::new(Size::default());
            screen.advance(sequence);
            screen.advance(b"X");

            let cell = *screen.view().cell(0, 0).expect("the cell is missing");
            assert_eq!(
                cell.underline,
                expected,
                "with the sequence {:?}",
                String::from_utf8_lossy(sequence)
            );
        }
    }

    #[test]
    fn with_inverse_video_the_intensity_refers_to_the_color_that_becomes_background() {
        // It pins the edge inherited by whoever resolves the theme: the intensity describes the **declared**
        // foreground, and with inverse video that color ends up in `bg`. Here what has to be dimmed is
        // the red, not the theme's background. If this changes, let it change with this test in front.
        let mut screen = Screen::new(Size::default());
        screen.advance(b"\x1b[2;31;7mX");

        let cell = *screen.view().cell(0, 0).expect("the cell is missing");

        assert_eq!(cell.fg, TermColor::Background);
        assert_eq!(
            cell.bg,
            TermColor::Indexed(1),
            "the red becomes the background"
        );
        assert!(cell.dim, "and it is the red that is dimmed, not the fg");
        assert!(
            cell.inverse,
            "and it has to be said: without this mark, whoever paints cannot know which of the two \
             colors the intensity refers to, and applies it to the one that is no longer seen"
        );
    }

    #[test]
    fn the_cursor_comes_out_in_visible_coordinates() {
        let mut screen = Screen::new(Size::default());
        screen.advance(b"good");

        let cursor = screen
            .view()
            .cursor()
            .expect("with the screen up to date the cursor is visible");

        assert_eq!((cursor.row, cursor.col), (0, 4));
        assert_eq!(cursor.shape, CursorShape::Block);
    }

    #[test]
    fn a_turned_off_cursor_is_reported_as_such() {
        let mut screen = Screen::new(Size::default());
        screen.advance(b"\x1b[?25l");

        assert_eq!(
            screen
                .view()
                .cursor()
                .expect("hidden is not the same as off-screen")
                .shape,
            CursorShape::Hidden
        );
    }

    /// Whether the cursor of a screen that has been sent `bytes` asks to blink.
    ///
    /// **It asks the same way the application does**, through [`Screen::cursor_blinks`]. An earlier
    /// version read the bit off the view instead, on the reasoning that the view is the only thing
    /// that leaves this crate — which is false, `cursor_blinks` is public and is exactly what the
    /// painter calls. Two auditors found it independently: every one of these tests passed with
    /// `cursor_blinks` hard-coded to `false`, because none of them went anywhere near it.
    fn blinking_after(bytes: &[u8]) -> bool {
        let mut screen = Screen::new(Size::default());
        screen.advance(bytes);
        screen.cursor_blinks()
    }

    #[test]
    fn a_terminal_nobody_has_spoken_to_blinks() {
        // The case a shell prompt is in, and the reason this is not the engine's own default: it
        // ships `{ Block, false }`, so leaving it alone means a blink nobody ever sees. **D37**.
        assert!(blinking_after(b""));
        assert!(
            blinking_after(b"good"),
            "and writing is not asking for anything"
        );
    }

    #[test]
    fn an_even_shape_is_steady_and_an_odd_one_blinks() {
        // DECSCUSR: 1/3/5 blink, 2/4/6 do not. The three steady ones matter more than the three
        // blinking ones here, because they are the only way a program can undo the new default.
        assert!(!blinking_after(b"\x1b[2 q"), "steady block");
        assert!(!blinking_after(b"\x1b[4 q"), "steady underline");
        assert!(!blinking_after(b"\x1b[6 q"), "steady beam");

        assert!(blinking_after(b"\x1b[1 q"), "blinking block");
        assert!(blinking_after(b"\x1b[3 q"), "blinking underline");
        assert!(blinking_after(b"\x1b[5 q"), "blinking beam");
    }

    #[test]
    fn dec_mode_12_is_the_other_door() {
        assert!(!blinking_after(b"\x1b[?12l"));
        assert!(blinking_after(b"\x1b[?12h"));
        assert!(
            blinking_after(b"\x1b[2 q\x1b[?12h"),
            "and it wins over a steady shape sent before it, being the later word"
        );
    }

    #[test]
    fn zero_hands_the_default_back() {
        // `CSI 0 SP q` clears the style rather than setting one, so what comes back is whatever this
        // crate configured. It is the sequence that makes the new default reversible in both
        // directions, and the reason no rule of our own was written to implement it.
        assert!(blinking_after(b"\x1b[2 q\x1b[0 q"));
    }

    #[test]
    fn a_shape_picked_through_the_other_door_inherits_the_default() {
        // **OSC 50 `CursorShape=` sets the shape and nothing else** (`vte-0.15.0/src/ansi.rs:1465`),
        // reaching `set_cursor_shape` (`alacritty_terminal/src/term/mod.rs:2213`), which does
        // `get_or_insert(default_cursor_style)` and then writes only the shape. So a program that picks
        // its shape through that door and never speaks about blinking gets **this terminal's
        // default**, which since **D37** is «blink» — before it was «steady».
        //
        // **Pinned as the decision it is.** It follows from the rule the whole delivery rests on —
        // the terminal blinks unless the program says otherwise, and picking a shape says nothing
        // about blinking — but it is reached by a path DECSCUSR does not use, and a reader who finds
        // a blink they did not ask for deserves to find this test rather than guess.
        assert!(
            blinking_after(b"\x1b]50;CursorShape=1\x07"),
            "beam through OSC 50"
        );
        assert!(
            !blinking_after(b"\x1b[2 q\x1b]50;CursorShape=1\x07"),
            "and a program that DID say steady first keeps it: the shape door does not undo the \
             blink door"
        );
    }

    #[test]
    fn hiding_the_cursor_leaves_the_blink_asked_for() {
        // **The trap the whole repaint design rests on.** The two bits are independent: a full-screen
        // program sends `CSI ?25l` and keeps reporting that it wants a blink. Arming a repaint from
        // this bit would wake the window to paint a cursor nobody draws, which is why the alarm reads
        // what the painting drew instead. Pinned here so the day the engine couples them, it is a
        // failing test and not a mystery in the frame's cost.
        let mut screen = Screen::new(Size::default());
        screen.advance(b"\x1b[?25l");
        let cursor = screen.view().cursor().expect("hidden is not off-screen");

        assert_eq!(cursor.shape, CursorShape::Hidden);
        assert!(screen.cursor_blinks());
    }

    #[test]
    fn the_alternate_screen_does_not_give_the_cursor_style_back() {
        // **The blink crosses `CSI ?1049h` and back exactly as the shape does, and neither is
        // restored on the way out.** Both live in the one `Term::cursor_style`, and `swap_alt`
        // (`term/mod.rs:714`) swaps the grid, the keyboard modes and the selection — never the
        // style — while what `?1049` saves and restores is the DEC cursor, which is position and
        // attributes. So a full-screen editor that asks for a steady beam leaves the shell it came
        // back to with a steady beam.
        //
        // **Pinned because it decides whether the window keeps waking up.** The alarm is armed from
        // a blink, so if the engine ever started scoping the style to the screen it is on, a shell
        // would silently get its blink back — or lose it — on every editor that exits, and the
        // symptom would be a frame count nobody can explain rather than a failing test. That it
        // matches xterm and kitty is why it is pinned rather than worked around.
        let mut screen = Screen::new(Size::default());
        screen.advance(b"\x1b[?1049h\x1b[6 q");
        assert!(
            !screen.cursor_blinks(),
            "a program inside the alternate screen could not switch its blink off"
        );

        screen.advance(b"\x1b[?1049l");
        assert!(
            !screen.cursor_blinks(),
            "leaving the alternate screen handed the blink back, so a shell starts blinking again \
             behind an editor that asked for a steady cursor"
        );
        assert_eq!(
            screen
                .view()
                .cursor()
                .expect("the cursor is on the primary screen")
                .shape,
            CursorShape::Beam,
            "the shape did not survive the swap either, so the two are not the one field"
        );

        // And the other direction, which is the one a shell is in: nothing was said before the
        // swap, so the terminal's own default has to come back out of it untouched.
        let mut untouched = Screen::new(Size::default());
        untouched.advance(b"\x1b[?1049h\x1b[?1049l");
        assert!(
            untouched.cursor_blinks(),
            "a trip through the alternate screen left the terminal's default behind"
        );
    }

    #[test]
    fn reading_the_view_does_not_consume_the_screen() {
        // Unlike querying the damage, this reader is not destructive. If it were, the
        // second frame of a repaint would come out blank.
        let mut screen = Screen::new(Size::default());
        screen.advance(b"good");

        let first = screen.view();
        let second = screen.view();

        // Checking only that the two are equal would not be enough: if the reader returned an empty view
        // both times, the test would pass just the same. You have to see first that there is something to lose.
        assert_eq!(first.cell(0, 0).expect("the cell is missing").c, 'g');
        assert_eq!(first, second);
    }

    #[test]
    fn a_sequence_split_across_two_calls_gives_the_same_result() {
        // It is the contract that holds up the whole design: the PTY delivers partial reads, so
        // an escape can arrive cut down the middle. If this did not hold, one would have to
        // accumulate bytes before interpreting them and the design would be different.
        let mut whole = Screen::new(Size::default());
        whole.advance(b"\x1b[31mrojo");

        let mut split = Screen::new(Size::default());
        split.advance(b"\x1b[");
        split.advance(b"31mrojo");

        assert_eq!(split.to_text(), whole.to_text());
        assert!(split.to_text().starts_with("rojo"));
    }

    /// **What scrolls off the top is remembered, and that is the half that did not exist.** With the scrollback
    /// off —as it was— scrolling up led nowhere: the first line of a dump was
    /// lost as soon as the screen filled up.
    #[test]
    fn what_scrolls_off_the_top_can_be_seen_again() {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }

        // Without scrolling up, the first thing is gone: the screen is four rows.
        assert!(!text_of(&screen.view()).contains("entry 0"));

        screen.scroll_by(Scroll::ToTop);

        assert!(
            text_of(&screen.view()).contains("entry 0"),
            "scrolling all the way up, the first line of the dump had to appear"
        );
    }

    /// **And the ceiling is the ceiling**: past the lines that are remembered, the oldest falls off. Without this
    /// test, a badly set ceiling —or the engine's, which is three times larger— would go unnoticed
    /// until you looked at the memory.
    #[test]
    fn no_more_of_the_history_is_remembered_than_the_ceiling_says() {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        // Quite a bit more than fits, so the beginning must have fallen off.
        for n in 0..(SCROLLBACK + 500) {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }

        screen.scroll_by(Scroll::ToTop);

        // **Scrolling all the way up leaves the offset at what is remembered**, so this number *is*
        // the ceiling. Asserting on it —and not on which line ended up first— is what makes the
        // test fall if someone changes the constant or inherits the engine's, without depending on how
        // the engine counts the screen's rows.
        assert_eq!(
            screen.view().scroll(),
            SCROLLBACK,
            "the scrollback kept something other than what its ceiling says"
        );
        let top = text_of(&screen.view());
        assert!(
            !top.starts_with("entry 0"),
            "more than fits was written, so the oldest had to have fallen off: {}",
            top.lines().next().unwrap_or("")
        );
    }

    /// **Scrolling up through the scrollback, the cursor stops being visible — and the snapshot says so instead of clamping it.**
    /// Clamping it drew it in the last row, that is, in a place where it is not: whoever looked would believe
    /// that the program writes there. It is the case that `view`'s comment announced ever since F1.2.
    #[test]
    fn scrolling_up_through_the_history_the_cursor_stops_being_visible() {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }
        assert!(
            screen.view().cursor().is_some(),
            "without scrolling up, the cursor has to be visible"
        );

        screen.scroll_by(Scroll::ToTop);

        assert_eq!(
            screen.view().cursor(),
            None,
            "all the way up the cursor is off-screen, and saying so is the only honest way"
        );
    }

    /// **The snapshot says how far it has been scrolled up**, which is what separates a still screen from one that
    /// stayed behind while the program kept writing.
    #[test]
    fn the_snapshot_says_how_far_it_has_been_scrolled_up() {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }
        assert_eq!(screen.view().scroll(), 0);

        screen.scroll_by(Scroll::Lines(3));

        assert_eq!(screen.view().scroll(), 3);
    }

    /// **What keeps coming in does not send you back to the end.** It is what one expects when stopping to read something
    /// while a program keeps writing: the new accumulates below and what is being looked at does not
    /// move. Without this, scrolling up while something dumps would be useless — which is exactly the case in which
    /// it is needed.
    #[test]
    fn what_keeps_coming_in_does_not_send_the_view_back_to_the_end() {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }
        screen.scroll_by(Scroll::ToTop);
        let looking = text_of(&screen.view());

        for n in 20..40 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }

        assert_eq!(
            text_of(&screen.view()),
            looking,
            "output kept coming in and the view moved: reading something while a program writes would be \
             impossible"
        );
    }

    /// And **scrolling all the way down returns to the latest**, which is where the program writes.
    #[test]
    fn scrolling_all_the_way_down_returns_to_where_the_program_writes() {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }
        let up_to_date = text_of(&screen.view());
        screen.scroll_by(Scroll::ToTop);

        screen.scroll_by(Scroll::ToBottom);

        assert_eq!(text_of(&screen.view()), up_to_date);
        assert!(
            screen.view().cursor().is_some(),
            "and the cursor is visible again"
        );
    }

    /// **The scrollback dump keeps what the active-area dump drops.** The lines that scrolled off the
    /// top are exactly what an agent reading a console it opened needs, and `to_text` deliberately
    /// does not return them — that difference is the whole reason the two dumps exist apart.
    #[test]
    fn the_scrollback_dump_keeps_the_lines_that_scrolled_off_the_top() {
        let mut screen = Screen::new(Size { rows: 3, cols: 20 });
        for n in 0..8 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }

        let active = screen.to_text();
        let everything = screen.scrollback_text();

        assert!(
            !active.contains("entry 0"),
            "the premise failed: nothing scrolled off"
        );
        assert!(active.contains("entry 7"), "the last line is not on screen");
        assert!(
            everything.contains("entry 0"),
            "the scrollback lost what scrolled off the top"
        );
        assert!(
            everything.contains("entry 7"),
            "the scrollback lost the active area"
        );
        assert!(
            everything.starts_with("entry 0"),
            "the scrollback does not begin at the oldest line: {everything:?}"
        );
    }

    /// **A page is the screen minus one line**, as in any pager: it leaves one of
    /// context so as not to lose the thread between screen and screen. With the whole screen, the joining
    /// line is lost on every jump and no one knows where they were.
    #[test]
    fn a_page_leaves_one_line_of_context() {
        let mut screen = Screen::new(Size { rows: 10, cols: 20 });
        for n in 0..50 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }

        screen.scroll_by(Scroll::Pages(1));

        assert_eq!(
            screen.view().scroll(),
            9,
            "a page of ten rows has to scroll up nine lines, not ten"
        );
    }

    /// On a console a single row tall the screen-minus-one rule floors at one line: without the floor
    /// a page would be zero lines and page-up/page-down would do nothing even with history behind them.
    #[test]
    fn a_page_on_a_one_row_console_moves_one_line() {
        let mut screen = Screen::new(Size { rows: 1, cols: 20 });
        for n in 0..50 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }

        screen.scroll_by(Scroll::Pages(1));

        assert_eq!(
            screen.view().scroll(),
            1,
            "a page on a one-row console has to move one line, not zero"
        );
    }

    /// The page count is whatever the user sent, so the product saturates: `Pages(i32::MAX)` used to
    /// overflow the multiplication and panic in debug builds. It now reaches the top, like `ToTop`.
    #[test]
    fn a_huge_page_count_saturates_at_the_top() {
        let mut screen = Screen::new(Size { rows: 10, cols: 20 });
        let mut top = Screen::new(Size { rows: 10, cols: 20 });
        for n in 0..50 {
            let line = format!("entry {n}\r\n");
            screen.advance(line.as_bytes());
            top.advance(line.as_bytes());
        }

        screen.scroll_by(Scroll::Pages(i32::MAX));
        top.scroll_by(Scroll::ToTop);

        assert_eq!(
            screen.view().scroll(),
            top.view().scroll(),
            "a huge page count has to end at the top of the history, not overflow"
        );
    }

    /// The text dump still gives **the active area** even while the scrollback is being looked at, and its
    /// documentation says so. It is what whoever checks what a program wrote wants; whoever
    /// wants what is seen has the snapshot.
    #[test]
    fn the_text_dump_gives_the_active_area_even_while_looking_at_the_history() {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            screen.advance(format!("entry {n}\r\n").as_bytes());
        }
        let active = screen.to_text();

        screen.scroll_by(Scroll::ToTop);

        assert_eq!(screen.to_text(), active);
        assert!(
            !text_of(&screen.view()).contains("entry 19"),
            "the test is not looking at the scrollback, so it compares nothing"
        );
    }

    #[test]
    fn a_freshly_opened_console_does_not_claim_the_mouse() {
        // The everyday case: a shell does not ask for the mouse, so the terminal keeps it whole
        // for selecting. If this were the other way around, dragging in a `bash` would stop marking text.
        let screen = Screen::new(Size { rows: 4, cols: 20 });
        assert!(
            !screen.mouse_reporting().wants_mouse(),
            "a shell that asked for nothing took the mouse"
        );
    }

    #[test]
    fn each_mouse_flag_is_read_separately() {
        // **The three are asked for with distinct sequences and mean distinct things**: a program that
        // only wants clicks does not want to be told about every pixel the pointer travels. Reading them
        // as a block would send motion reports to `vim`, which asks for `1002` and not `1003`.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"\x1b[?1000h");
        assert_eq!(
            screen.mouse_reporting(),
            MouseReporting {
                buttons: true,
                ..MouseReporting::default()
            },
            "asking for clicks turned on something more"
        );

        screen.advance(b"\x1b[?1002h");
        assert!(
            screen.mouse_reporting().drag,
            "asking for the drag was not read"
        );
        assert!(
            !screen.mouse_reporting().motion,
            "asking for the drag also turned on motion"
        );
        // **And the three REPLACE, they do not accumulate** —read in the engine, not assumed—: asking for the
        // drag turns off the clicks-only one. It matters because whoever reads these flags decides with
        // `wants_mouse()`, which looks at them all: if they accumulated, turning off the drag would leave the clicks
        // on and the console would not recover the selection.
        assert!(
            !screen.mouse_reporting().buttons,
            "asking for the drag left the clicks-only mode on: the three replace each other"
        );

        screen.advance(b"\x1b[?1003h");
        assert!(
            screen.mouse_reporting().motion,
            "asking for motion was not read"
        );
    }

    #[test]
    fn asking_only_for_the_extended_encoding_is_not_asking_for_the_mouse() {
        // **`extended` says HOW the report is wanted, not WHETHER it is wanted.** Counting it as wanting the
        // mouse would make a program that turns on the encoding up front —and many turn it on
        // along with other things at startup— take the left button without having asked for it, and with
        // it the selection.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"\x1b[?1006h");

        assert!(
            screen.mouse_reporting().extended,
            "the extended encoding was not read"
        );
        assert!(
            !screen.mouse_reporting().wants_mouse(),
            "turning on only the encoding took the mouse"
        );
    }

    #[test]
    fn turning_off_the_mouse_returns_it() {
        // A program that finishes turns off what it turned on. Without this, exiting `htop` would leave the console
        // unable to select until it is closed.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"\x1b[?1000h");
        assert!(screen.mouse_reporting().wants_mouse());

        screen.advance(b"\x1b[?1000l");

        assert!(
            !screen.mouse_reporting().wants_mouse(),
            "the program turned off the mouse and the terminal kept believing it wanted it"
        );
    }

    #[test]
    fn the_wheel_acts_as_arrows_only_on_the_alternate_screen() {
        // **Alternate scrolling comes turned ON out of the box** —read in the engine, not assumed:
        // its default mode already brings it, just like `xterm`—. So the only thing needed for
        // the turn to be translated is the alternate screen, and whoever does not ask for it does not receive arrows: sending them
        // to a shell would change the command it has written on each turn.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        assert!(
            !screen.wheel_acts_as_arrows(),
            "a shell without the alternate screen received arrows when turning the wheel"
        );

        screen.advance(b"\x1b[?1049h");
        assert!(
            screen.wheel_acts_as_arrows(),
            "on the alternate screen the turn was not translated, and `less` would not scroll"
        );
    }

    #[test]
    fn a_program_can_turn_off_alternate_scrolling() {
        // The other half of the rule, and the one needed because the flag comes on: an alternate-screen
        // program that turns it off —`vim` does so as soon as it wants the mouse for itself— stops
        // receiving arrows on turning. Without this half, turning the wheel inside an editor would move the insertion
        // point without anyone touching a key.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(b"\x1b[?1049h");
        assert!(screen.wheel_acts_as_arrows());

        screen.advance(b"\x1b[?1007l");

        assert!(
            !screen.wheel_acts_as_arrows(),
            "the program turned off alternate scrolling and kept receiving arrows"
        );
    }

    /// What the snapshot shows, in text: it is what is **seen**, not what the program has written.
    fn text_of(view: &View) -> String {
        (0..view.size().rows)
            .map(|row| {
                view.row(row)
                    .expect("the row is inside")
                    .iter()
                    .map(|cell| cell.c)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    // ── What goes attached to a cell ──────────────────────────────────────────────────────────

    /// What is attached to the view's first cell, written with the same words whoever paints uses.
    fn attached_at(screen: &Screen, col: u16) -> Vec<char> {
        let view = screen.view();
        let cell = *view.cell(0, col).expect("the cell exists");
        view.combining_of(&cell).to_vec()
    }

    #[test]
    fn the_selector_that_turns_a_symbol_into_an_emoji_reaches_the_view() {
        // **It is the reason this datum exists.** `☁` and `☁️` are the same character with distinct
        // presentation, and what separates them —the U+FE0F selector— takes up no column: it stayed in the engine,
        // so the two forms reached whoever paints **identical** and there was no way to distinguish them.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance("\u{2601}\u{FE0F}".as_bytes());

        assert_eq!(
            attached_at(&screen, 0),
            vec!['\u{FE0F}'],
            "the emoji selector did not reach the view"
        );
    }

    #[test]
    fn the_same_symbol_without_a_selector_brings_nothing_attached() {
        // **The other half, and without it the one above is satisfied by always returning the selector.** What
        // is asserted is that the view distinguishes the two cases, not that it knows how to produce one.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance("\u{2601}".as_bytes());

        assert!(
            attached_at(&screen, 0).is_empty(),
            "a cell with nothing attached brought something"
        );
    }

    #[test]
    fn each_cell_keeps_its_own_and_not_the_neighbors() {
        // **Each cell's span points into a shared vector**, so two cells with things attached is
        // the case where a badly computed offset gives one what belongs to the other. The two selectors
        // are distinct on purpose: with the same one, crossing them would give the same result.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance("\u{2601}\u{FE0F}\u{2601}\u{FE0E}".as_bytes());

        assert_eq!(attached_at(&screen, 0), vec!['\u{FE0F}']);
        assert_eq!(attached_at(&screen, 1), vec!['\u{FE0E}']);
    }

    #[test]
    fn what_is_attached_has_a_ceiling_and_the_engine_does_not() {
        // **The dependency's `Vec<char>` grows without a cap**, so a program that spews combining
        // marks onto the same cell makes it grow without limit: an unbounded input from outside,
        // the pattern D8 punished with the output queue. The ceiling is set by this crate when copying.
        //
        // **The FIRST mark is distinct from all the rest, and that is half the test.** With
        // two hundred equal marks, keeping the **last** sixteen instead of the first gives
        // the same result and this test passes just the same — the second disguise the project's rules
        // name: choosing numbers with which the defect and the fix coincide. And alternating two marks
        // is not enough either: 200 − 16 = 184 is even, so the final stretch starts with the same one as
        // the beginning and **the test passes broken again**. Both corrections were needed.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        let mut bytes = String::from("a");
        let alternating: Vec<char> = (0..200)
            .map(|i| match i {
                0 => '\u{0300}',
                i if i % 2 == 0 => '\u{0301}',
                _ => '\u{0308}',
            })
            .collect();
        bytes.extend(alternating.iter());
        screen.advance(bytes.as_bytes());

        let attached = attached_at(&screen, 0);
        assert_eq!(
            attached.len(),
            16,
            "what is attached to a cell went past this crate's ceiling"
        );
        assert_eq!(
            attached,
            alternating[..16].to_vec(),
            "the trim kept the wrong marks: what decides the presentation is what goes \
             RIGHT behind the character, so the ceiling has to cut off at the end"
        );
    }
    #[test]
    fn the_cell_does_not_fatten_without_anyone_finding_out() {
        // **The grid is allocated WHOLE every frame** (`vec![ViewCell::default(); rows × cols]`),
        // so each byte that fattens the cell is paid multiplied by the cells and by the frames.
        // This number is not sacred —what is attached to a cell already raised it from 24 to 28— but raising it has to
        // be a decision someone makes looking at it, not an effect of having added a field. And it has already
        // done its job: the span's first version went in two nested fields and cost **32**;
        // packing it into an integer gave back four bytes per cell, which are four per frame too.
        assert_eq!(
            std::mem::size_of::<ViewCell>(),
            28,
            "the view cell changed size: look at which field did it and whether it is worth it"
        );
    }
    #[test]
    fn the_text_dump_does_not_leave_behind_what_is_attached() {
        // **Returning `☁` where the program wrote `☁️` is returning ANOTHER character.** The emoji
        // selector takes up no column but is text all the same, and whoever reads this dump —a test today, an
        // agent reading its console tomorrow— takes away something no one wrote. It is the same rule that
        // **D22** sets for copying with the mouse, which the engine already meets on its own.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance("\u{2601}\u{FE0F}".as_bytes());

        assert_eq!(
            screen.to_text(),
            "\u{2601}\u{FE0F}",
            "the text dump left the selector behind along the way"
        );
    }

    #[test]
    fn the_text_dump_of_the_ordinary_does_not_change() {
        // **The other half, and without it the one above is satisfied by putting in extra characters.** What the
        // program did not write cannot appear here.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance(b"good");

        assert_eq!(screen.to_text(), "good");
    }

    #[test]
    fn what_is_attached_goes_with_the_wide_characters_head_and_not_with_its_companion() {
        // **It is the normal case of this milestone, not a corner**: CJK and emojis are WIDE, so
        // anything attached to them reaches a two-column cell. Whoever paints walks cell by
        // cell, and if the companion carried a copy of what is attached, the right half of an emoji would
        // be painted with a different presentation than the left.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance("\u{1F600}\u{FE0F}".as_bytes());

        let view = screen.view();
        let head = *view.cell(0, 0).expect("the head exists");
        let companion = *view.cell(0, 1).expect("the companion exists");

        assert_eq!(head.width, CellWidth::WideHead);
        assert_eq!(companion.width, CellWidth::WideTail);
        assert_eq!(
            view.combining_of(&head),
            &['\u{FE0F}'],
            "the wide character's head lost what it carried attached"
        );
        assert!(
            view.combining_of(&companion).is_empty(),
            "the companion kept a copy of what is attached, and it is not its own"
        );
    }

    #[test]
    fn what_is_attached_is_still_there_when_scrolling_up_through_the_history() {
        // **Scrolling up through the scrollback rebuilds the view from another spot in the grid**, and that is where
        // a new datum falls off without anyone noticing: what is seen up there no longer comes from the active area.
        // If the selector were lost on scrolling up, the same line would be painted one way while it is
        // written and another when looked at again.
        //
        // **It is first checked that the line is NOT visible without scrolling up**, and that is not ceremony: without that step
        // the test passes just the same without scrolling even one line, and then it does not talk about the scrollback. It was
        // uncovered by mutating it.
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance("\u{2601}\u{FE0F}\r\n".as_bytes());
        for _ in 0..5 {
            screen.advance(b"relleno\r\n");
        }

        let before_scrolling = screen.view();
        assert_ne!(
            before_scrolling.cell(0, 0).expect("row 0 exists").c,
            '\u{2601}',
            "the selector's line is visible without scrolling up: this test would not be looking at the scrollback"
        );

        screen.scroll_by(Scroll::ToTop);
        let view = screen.view();
        assert!(
            view.scroll() > 0,
            "it did not scroll up through the scrollback: the offset is still at zero"
        );
        let cell = *view
            .cell(0, 0)
            .expect("the first line of the scrollback is visible");
        assert_eq!(cell.c, '\u{2601}');
        assert_eq!(
            view.combining_of(&cell),
            &['\u{FE0F}'],
            "what is attached was lost when scrolling up through the scrollback"
        );
    }

    fn link_targets(view: &View) -> Vec<(u16, u16, view::Link)> {
        let mut links = Vec::new();
        for row in 0..view.size().rows {
            for col in 0..view.size().cols {
                if let Some(link) = view.link_at(row, col) {
                    links.push((row, col, link));
                }
            }
        }
        links
    }

    #[test]
    fn a_plain_url_crosses_engine_wraps_and_style_changes() {
        let mut screen = Screen::new(Size { rows: 4, cols: 12 });
        screen.advance(b"See https://exa\x1b[31mmple.test/a\x1b[0m now");

        let links = link_targets(&screen.view());
        assert!(
            links.iter().any(|(row, _, link)| {
                *row > 0
                    && link.source == view::LinkSource::Plain
                    && link.target == "https://example.test/a"
            }),
            "the engine wrap or the style boundary split the visible URL: {links:?}"
        );
    }

    #[test]
    fn a_program_written_line_break_does_not_create_a_url() {
        let mut screen = Screen::new(Size { rows: 3, cols: 20 });
        screen.advance(b"https://example.\r\ntest/path");

        let links = link_targets(&screen.view());
        assert!(
            links
                .iter()
                .all(|(_, _, link)| link.target != "https://example.test/path"),
            "two physical lines were joined even though the program wrote the break: {links:?}"
        );
    }

    #[test]
    fn plain_url_punctuation_keeps_balanced_parentheses_and_drops_the_sentence() {
        let mut screen = Screen::new(Size { rows: 3, cols: 48 });
        screen.advance(b"See (https://example.test/a_(b)). next");

        let links = link_targets(&screen.view());
        assert!(
            links
                .iter()
                .any(|(_, _, link)| link.target == "https://example.test/a_(b)"),
            "the balanced URL did not survive its surrounding punctuation: {links:?}"
        );
        assert!(
            links.iter().all(|(_, _, link)| !link.target.ends_with('.')),
            "sentence punctuation became part of the target: {links:?}"
        );
    }

    #[test]
    fn trailing_sentence_marks_and_unmatched_brackets_are_not_targets() {
        for suffix in [".", ",", ";", ":", "!", "?", ")", "]", "}"] {
            let mut screen = Screen::new(Size { rows: 2, cols: 40 });
            screen.advance(format!("https://example.test/x{suffix} next").as_bytes());
            let links = link_targets(&screen.view());
            assert!(
                links
                    .iter()
                    .any(|(_, _, link)| link.target == "https://example.test/x"),
                "the trailing {suffix:?} was not removed according to the literal grammar: {links:?}"
            );
            assert!(
                links
                    .iter()
                    .all(|(_, _, link)| !link.target.ends_with(suffix)),
                "the trailing {suffix:?} became part of the target: {links:?}"
            );
        }
    }

    #[test]
    fn every_declared_plain_url_boundary_is_honored() {
        for prefix in [" ", "(", "[", "{", "<", "\"", "'"] {
            for suffix in [" ", "<", ">", "\"", "'", "`", "\t"] {
                let mut screen = Screen::new(Size { rows: 2, cols: 40 });
                let text = format!("{prefix}https://example.test/x{suffix}");
                screen.advance(text.as_bytes());
                assert!(
                    link_targets(&screen.view())
                        .iter()
                        .any(|(_, _, link)| link.target == "https://example.test/x"),
                    "the declared boundary pair {prefix:?} / {suffix:?} was not recognized"
                );
            }
        }
    }

    #[test]
    fn a_wide_character_before_and_inside_a_plain_url_keeps_cell_hit_testing_exact() {
        let mut screen = Screen::new(Size { rows: 3, cols: 30 });
        screen.advance("日 https://例.test/x".as_bytes());
        let view = screen.view();

        let wide_head = link_targets(&view)
            .into_iter()
            .find(|(_, _, link)| link.target == "https://例.test/x")
            .expect("the URL with a wide character was not recognized");
        assert_eq!(wide_head.2.source, view::LinkSource::Plain);
        let wide_col = (0..view.size().cols)
            .find(|col| view.cell(0, *col).is_some_and(|cell| cell.c == '例'))
            .expect("the wide URL character is visible");
        assert_eq!(
            view.link_at(0, wide_col + 1)
                .expect("the wide tail should belong to its head's target")
                .target,
            "https://例.test/x"
        );
    }

    #[test]
    fn an_osc_8_target_wins_over_its_url_shaped_label() {
        let mut screen = Screen::new(Size { rows: 3, cols: 48 });
        screen.advance(
            b"\x1b]8;;https://destination.test/osc\x1b\\https://label.test/plain\x1b]8;;\x1b\\",
        );

        let link = screen
            .view()
            .link_at(0, 10)
            .expect("the OSC 8 label did not retain its semantic target");
        assert_eq!(link.source, view::LinkSource::Osc8);
        assert_eq!(link.target, "https://destination.test/osc");
    }

    #[test]
    fn an_osc_8_wide_labels_two_cells_resolve_to_one_target() {
        let mut screen = Screen::new(Size { rows: 2, cols: 10 });
        screen.advance(b"\x1b]8;;https://example.test/wide\x1b\\");
        screen.advance("日".as_bytes());
        screen.advance(b"\x1b]8;;\x1b\\");
        let view = screen.view();

        assert_eq!(
            view.link_at(0, 0),
            view.link_at(0, 1),
            "the wide label's companion did not retain the sparse OSC 8 span"
        );
        assert_eq!(
            view.link_at(0, 1).expect("the companion is linked").source,
            view::LinkSource::Osc8
        );
    }

    #[test]
    fn an_osc_8_label_clipped_by_scrollback_is_not_resolved() {
        let mut screen = Screen::new(Size { rows: 2, cols: 8 });
        screen.advance(b"\x1b]8;;https://example.test/hidden\x1b\\");
        screen.advance(b"a-semantic-label-longer-than-the-view");
        screen.advance(b"\x1b]8;;\x1b\\");

        assert!(
            link_targets(&screen.view()).is_empty(),
            "a visible suffix of an OSC 8 label activated its hidden target"
        );
    }

    #[test]
    fn an_osc_8_label_that_continues_below_the_view_is_not_resolved() {
        // The label starts on the view's rows and goes on past its bottom edge: what is visible is
        // a prefix of it, and a prefix is not the text that named the target.
        let mut screen = Screen::new(Size { rows: 2, cols: 8 });
        screen.advance(b"\x1b]8;;https://example.test/below\x1b\\");
        screen.advance(b"abcdefghijklmnopqrstuvwx");
        screen.advance(b"\x1b]8;;\x1b\\\r\nz1\r\nz2\r\nz3\r\nz4");
        screen.scroll_by(Scroll::ToTop);
        let view = screen.view();
        assert_eq!(
            view.cell(0, 0).expect("the top row is visible").c,
            'a',
            "the view does not start at the label, so it would not be clipped at the bottom"
        );

        assert!(
            link_targets(&view).is_empty(),
            "a visible prefix of an OSC 8 label activated a target whose label runs off the view"
        );
    }

    #[test]
    fn a_full_stop_hidden_by_a_closing_bracket_is_still_the_sentences() {
        let mut screen = Screen::new(Size { rows: 2, cols: 48 });
        screen.advance(b"(see https://example.test/a.) next");

        let links = link_targets(&screen.view());
        assert!(
            links
                .iter()
                .any(|(_, _, link)| link.target == "https://example.test/a"),
            "the bracket kept the full stop inside the address: {links:?}"
        );
    }

    #[test]
    fn a_scheme_embedded_in_a_word_and_an_uppercase_scheme_are_inert() {
        for text in ["xhttps://example.test", "HTTPS://example.test"] {
            let mut screen = Screen::new(Size { rows: 2, cols: 32 });
            screen.advance(text.as_bytes());
            assert!(
                link_targets(&screen.view()).is_empty(),
                "the literal plain-link grammar accepted {text:?}"
            );
        }
    }

    #[test]
    fn a_plain_url_in_scrollback_resolves_from_the_painted_offset() {
        let mut screen = Screen::new(Size { rows: 3, cols: 40 });
        screen.advance(b"https://example.test/history\r\n");
        for line in 0..6 {
            screen.advance(format!("filler {line}\r\n").as_bytes());
        }
        assert!(
            link_targets(&screen.view()).is_empty(),
            "the old URL unexpectedly remained on the active screen"
        );

        screen.scroll_by(Scroll::ToTop);
        let view = screen.view();
        assert!(view.scroll() > 0);
        assert!(
            link_targets(&view)
                .iter()
                .any(|(_, _, link)| link.target == "https://example.test/history"),
            "the retained snapshot lost link resolution when its rows came from scrollback"
        );
    }
}
