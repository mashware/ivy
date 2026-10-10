//! How the grid is read from outside: a view of cells and the cursor.
//!
//! Everything here is **ours**. No type from `alacritty_terminal` shows through, and not by
//! whim: the painting layer is going to be rewritten —egui now, GPUI when needed— and whoever paints
//! should not have to know the library underneath nor migrate with it.
//!
//! The view is a **snapshot**, not a window: it is built whole at the moment it is requested and
//! keeps no reference to the screen. That allows reading it while the screen keeps
//! advancing, and it is what lets whoever paints hold on to their own during a frame.

use crate::Size;

/// A color exactly as the program declared it, **unresolved**.
///
/// Unresolved on purpose: translating to pixels is choosing a theme, and the theme is not this
/// crate's business. Here only *what the program said* is kept, so that whoever paints decides what it
/// means.
///
/// The sixteen named colors are, for the terminal, the first sixteen entries of the
/// palette, so they are handed out as [`TermColor::Indexed`] instead of duplicating them in named
/// variants. What does need its own variant is what is **not** a fixed entry: the theme's foreground
/// and background, the cursor color and the dimmed forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermColor {
    /// An entry of the 256-color palette. The first sixteen are the named ANSI colors.
    Indexed(u8),
    /// An exact color requested by the program.
    Rgb { r: u8, g: u8, b: u8 },
    /// The theme's default foreground.
    Foreground,
    /// The theme's default background.
    Background,
    /// The cursor's own color.
    Cursor,
    /// Dimmed variant of one of the first eight entries of the palette.
    ///
    /// The terminal distinguishes "dim red" from "red", and whoever paints decides whether they are
    /// two colors or the same one with less alpha. Collapsing them here would lose the distinction
    /// without being able to recover it.
    Dim(u8),
    /// The theme's bright foreground.
    BrightForeground,
    /// The theme's dim foreground.
    DimForeground,
}

/// How many columns a cell occupies, and which of them it is.
///
/// **Single source of the width.** The library underneath marks it with two distinct flags; having it
/// here too in a separate boolean would be the same fact in two places that can disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellWidth {
    /// One column.
    Single,
    /// First of the two columns of a wide character. **It is the one that carries the character.**
    WideHead,
    /// Second column of a wide character: it occupies its slot and has no character of its own.
    ///
    /// It goes in the view instead of being omitted because the view is **dense**: column `n` of a row
    /// is always at position `n`. Skipping it would force whoever paints to keep track of the
    /// offset, which is exactly the error that misaligns a row with CJK or emoji.
    WideTail,
    /// Gap at the end of a row where a wide character **did not fit**: the character went whole
    /// to the next row and this column is left empty on purpose.
    ///
    /// It is distinguished from [`CellWidth::Single`] because it is not a space the program wrote,
    /// but padding from the terminal. Today the two would paint the same —the content is a space— but
    /// confusing them when measuring columns or selecting text would give a result nobody could
    /// explain.
    WrapFiller,
}

/// Underline style the program requested.
///
/// There are five and not one, and the difference shows every day: the wavy underline is what an
/// LSP diagnostic uses in `nvim`. With a boolean, that compile error reached the view
/// with no mark at all and there was no way to recover it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    /// Wavy. The one for an LSP's errors.
    Curl,
    Dotted,
    Dashed,
}

/// How many zero-width characters are kept attached to a cell.
///
/// **The engine has none**, and that is the reason this one exists: the `Vec<char>` of
/// `alacritty_terminal 0.26` grows without a cap, so a program that spews combining marks onto the
/// same cell makes it grow without limit. It is an unbounded input from outside — the same pattern **D8**
/// punished with the output queue.
///
/// **Sixteen, and the reason is to bound, not to preserve a concrete sequence.** The first version of this
/// comment said six were needed so as not to split a family of emoji joined by joiners
/// (`👨‍👩‍👧‍👦`), and **that is false with this engine**: it was checked by running it against
/// `alacritty_terminal 0.26`, which only attaches characters of **zero width**, and the four members of
/// that family are **wide**. They arrive as **four cells**, each with **one** joiner attached — not
/// as one cell with six. What can truly stack many is a repeated combining mark, which has
/// no canonical number at all.
///
/// So this number **preserves nothing concrete**: it puts a ceiling where the engine has none, and
/// sixteen is generous for everything that has been seen to arrive. If some day something real is cut off
/// here, it is raised — with the sequence in hand, which is what was missing the first time.
const COMBINING_PER_CELL: usize = 16;

// The span is packed into a single 32-bit integer (see `CombiningSpan`), so the ceiling has to fit
// in the low five bits. Without this, raising it to 32 would make `count` wrap around **silently** and
// every cell would declare it carries nothing.
const _: () = assert!(COMBINING_PER_CELL <= MAX_COUNT as usize);

/// The zero-width characters attached to a cell: **where they are, not which they are**.
///
/// It is opaque on purpose. The characters live in a flat vector of the [`View`], and this is the span that
/// points to them; they are read with [`View::combining_of`].
///
/// **Why they do not go inside the cell.** [`ViewCell`] is `Copy` and the grid is rebuilt **whole every
/// frame**, so a `Vec` per cell would be a memory allocation per cell and per frame on the
/// hottest path of the project — and it would strip `Copy` off a type that has it for exactly that reason.
/// Asking about a cell that carries nothing costs comparing an integer with zero.
///
/// **And what that deal costs, said here so nobody discovers it the hard way: comparing two
/// [`ViewCell`] only means something within the SAME view.** This is where the characters are stored,
/// not which they are, so two cells that show exactly the same thing in two different views
/// can come out **different**, and within one same view two cells with nothing attached come out
/// equal only because their empty span starts at the same place. Today nobody compares them; the day
/// someone needs to, what they want to compare is the character and what [`View::combining_of`] returns.
/// **It goes in ONE integer and not two fields, and it is four bytes per cell.** Measured: with `offset: u32` and
/// `count: u8` as fields of a nested type, the cell goes from 24 to **32** bytes, because the type brings
/// its own padding and the `u8` cannot slip into the hole the cell already had. Packed it is **28**.
/// Four bytes per cell, allocated and zeroed **every frame**, on the path this very
/// module watches with a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CombiningSpan(u32);

/// How many fit in the low five bits of the span.
const MAX_COUNT: u32 = 31;
/// And how many characters there can be ahead in the vector, in the upper twenty-seven.
const MAX_OFFSET: u32 = (1 << 27) - 1;

impl CombiningSpan {
    /// The span that starts at `offset` and lasts `count`, **or nothing if either does not fit**.
    ///
    /// **Nothing, and not a clipped span**: an `offset` that did not fit and got saturated would point at
    /// **real characters of another cell**, which is worse than having none. The first version saturated, and a
    /// review caught it. It is unreachable —the vector is bounded by cells × sixteen, and here fit
    /// a hundred and thirty-four million— so this defends against nothing: **it chooses the failure mode**.
    fn new(offset: usize, count: usize) -> Self {
        let (Ok(offset), Ok(count)) = (u32::try_from(offset), u32::try_from(count)) else {
            return Self::default();
        };
        if offset > MAX_OFFSET || count > MAX_COUNT {
            return Self::default();
        }
        Self(offset << 5 | count)
    }

    fn offset(self) -> usize {
        (self.0 >> 5) as usize
    }

    fn count(self) -> usize {
        (self.0 & MAX_COUNT) as usize
    }
}

/// A cell of the grid, already resolved for whoever paints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewCell {
    pub c: char,
    /// What goes **attached** to this character and occupies no column: presentation selectors, combining
    /// marks, the joiners of an emoji sequence.
    ///
    /// **Without this, `☁` and `☁️` are the same cell.** They are the same character with different
    /// presentation —the second carries the emoji selector behind it— and whoever paints had no way to
    /// distinguish them: the character that separates them occupies no column, so it stayed in the engine.
    ///
    /// The characters are read with [`View::combining_of`]; this only says where they are.
    pub combining: CombiningSpan,
    /// Color of the character. **Inverse video is already applied**: if the program requested it, this is
    /// the declared background. Resolving it here avoids each painting layer repeating the same rule, and
    /// one of them forgetting it.
    pub fg: TermColor,
    /// Color of the background, with inverse video already applied just like in `fg`.
    pub bg: TermColor,
    pub width: CellWidth,
    /// The two colors **already come swapped**; this says that they are.
    ///
    /// It is not redundant information nor an invitation to redo the swap. It is needed because
    /// the intensity —bold and dim— describes the color the program declared as **foreground**,
    /// and with inverse video that color is the one now traveling in `bg`. Without this datum, whoever
    /// paints applies the intensity to the wrong field: with `\x1b[2;31;7m` it dims a color no longer
    /// visible and leaves the red at full intensity. A review of the painter proved it, not theory.
    pub inverse: bool,
    /// Bold. **Not resolved here**: "bold = bright" is a theme decision, not a reading one.
    ///
    /// **Mind inverse video.** The intensity describes the color the program declared as
    /// foreground; if it requested inverse video, that color is now the `bg` field. Which is to say that with
    /// `\x1b[2;31;7m` what has to be dimmed is the red, which is in `bg`, not the `fg`. Resolving it
    /// fully here would require applying the theme, which belongs to the layer above.
    pub bold: bool,
    /// Dim. Same warning about inverse video as in `bold`.
    pub dim: bool,
    pub italic: bool,
    pub underline: Underline,
    pub strikeout: bool,
    /// The program requested not to show the content. The character is still there: whoever paints decides.
    pub hidden: bool,
    /// This cell is within what is selected right now.
    ///
    /// **A boolean per cell and not the range of the selection**, which is the founding criterion of this
    /// crate: the range is a type of the engine and exposing it out here would leak it. And whoever paints does not need the
    /// range —it already traverses the grid cell by cell—, it needs to know of each one whether it is highlighted.
    ///
    /// **The engine decides it and not a comparison of coordinates here**, and the difference is not cosmetic:
    /// the companion of a wide character counts as selected when its head is, and the
    /// block cursor is **not** highlighted at the ends of the selection. Redoing those two rules here
    /// would be rewriting what the dependency already upholds, and with them wrong the selection looks broken
    /// exactly where there is emoji.
    pub selected: bool,
    /// This cell is inside a match of the search open on this console (D61).
    ///
    /// A snapshot from [`Screen::view`](crate::Screen::view) leaves it `false`, and in this crate
    /// only [`Screen::mark_matches`](crate::Screen::mark_matches) sets it — so a console with no
    /// search open pays nothing for it.
    pub matched: bool,
}

impl Default for ViewCell {
    fn default() -> Self {
        Self {
            c: ' ',
            combining: CombiningSpan::default(),
            fg: TermColor::Foreground,
            bg: TermColor::Background,
            width: CellWidth::Single,
            inverse: false,
            bold: false,
            dim: false,
            italic: false,
            underline: Underline::None,
            strikeout: false,
            hidden: false,
            selected: false,
            matched: false,
        }
    }
}

/// Shape of the cursor, exactly as the program requested it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Underline,
    Beam,
    HollowBlock,
    /// The program turned it off. Whoever paints must not draw it.
    Hidden,
}

/// Where the cursor is and what shape it has.
///
/// The row and the column are **visible coordinates**: row 0 is the top of what is seen,
/// not that of the history. Whoever paints can index the view directly with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub shape: CursorShape,
}

/// Where a link painted in a console came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSource {
    /// Characters written directly into the grid spell the destination.
    Plain,
    /// The producer attached a semantic destination with the OSC 8 terminal sequence.
    Osc8,
}

/// A link resolved from one cell of this exact painted snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The exact candidate carried by the painted snapshot, before the app's web policy validates it.
    pub target: String,
    /// Whether the destination was visible text or semantic terminal metadata.
    pub source: LinkSource,
}

/// One run of visible cells carrying the same OSC 8 destination.
///
/// It is sparse on purpose: putting a pointer in every [`ViewCell`] would enlarge the dense type
/// rebuilt for every visible cell and every frame, while most screens contain no semantic link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Osc8Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) target: String,
}

/// Side metadata that keeps link resolution out of the dense cell type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewMetadata {
    pub(crate) row_wraps: Vec<bool>,
    pub(crate) continued_from_above: bool,
    pub(crate) osc8: Vec<Osc8Span>,
}

/// A snapshot of the visible grid.
///
/// **Dense**: there are exactly `size.rows * size.cols` cells, and the one at row `f` column `c` is
/// at position `f * size.cols + c`. Without gaps and without jumps, including the companions of the
/// wide characters.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// Private along with the rest: the density is an invariant between the size and the cells, and with the
    /// size public anyone could write `view.size.cols = 5` over a view of 80. From
    /// there `cell(1, 0)` would return a real but wrong cell instead of `None`, and
    /// nothing would fail — the constructor's check only runs in debug.
    size: Size,
    cells: Vec<ViewCell>,
    /// Everything attached to the cells, **flat and in a single place**.
    ///
    /// Each cell that carries something points to its span here; those that carry nothing —almost all— take no space.
    /// See [`CombiningSpan`] for why they do not go inside the cell.
    combining: Vec<char>,
    /// Whether each visible row continues into the next one without a program-written break.
    row_wraps: Vec<bool>,
    /// Whether row zero is itself the continuation of a row above the visible snapshot.
    continued_from_above: bool,
    /// Semantic links present in the snapshot, stored once per contiguous run rather than per cell.
    osc8: Vec<Osc8Span>,
    /// Where the cursor is, **if it is visible at all**.
    ///
    /// **It is optional since there is history, and it is not prudence**: scrolling up through the history the
    /// cursor is still where it was —in the active area— and that lands **off the screen**.
    /// Clamping it to the last row would draw it in a place where it is not, which is worse than not
    /// drawing it: whoever looks would believe the program is writing there.
    cursor: Option<Cursor>,
    /// How many lines have been scrolled up through the history: **zero is "the latest is being looked at"**.
    ///
    /// **Nobody paints it today, and saying so matters**: it is what separates a still screen from one that has
    /// fallen behind while the program keeps writing, and showing it —a mark, a bar— belongs to
    /// its own milestone. What it upholds in the meantime is that the effect of scrolling up can be **checked**
    /// from outside this module, which is the only way a history test can assert about what
    /// is seen and not about the field where the value is noted down.
    scroll: usize,
}

impl View {
    pub(crate) fn new(
        size: Size,
        cells: Vec<ViewCell>,
        combining: Vec<char>,
        metadata: ViewMetadata,
        cursor: Option<Cursor>,
        scroll: usize,
    ) -> Self {
        debug_assert_eq!(cells.len(), size.rows as usize * size.cols as usize);
        debug_assert_eq!(metadata.row_wraps.len(), size.rows as usize);
        Self {
            size,
            cells,
            combining,
            row_wraps: metadata.row_wraps,
            continued_from_above: metadata.continued_from_above,
            osc8: metadata.osc8,
            cursor,
            scroll,
        }
    }

    /// The zero-width characters attached to a cell, in the order in which they arrived.
    ///
    /// Empty is the ordinary case. **The whole cell is passed and not its span**: `view.combining_of(cell.combining)`
    /// said the same word twice, and that repetition was the sign that the seam was placed
    /// where it is not used.
    pub fn combining_of(&self, cell: &ViewCell) -> &[char] {
        let span = cell.combining;
        let (offset, end) = (span.offset(), span.offset() + span.count());
        // **A span that ran off the end is a badly built view, not a datum of the program**, so it fires
        // in the tests instead of disguising itself as "this cell carried nothing". In production it degrades to
        // empty, which is the only safe thing that can be done with an index no longer trusted.
        //
        // The first version said this same thing in a comment and did the opposite: it returned empty **without
        // firing**, that is, exactly the disguise it said it avoided. A review caught it.
        debug_assert!(
            end <= self.combining.len(),
            "a span of the view points outside its own vector: {offset}..{end} of {}",
            self.combining.len()
        );
        self.combining.get(offset..end).unwrap_or_default()
    }

    /// The size of the grid this view portrays.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Where the cursor is and what shape it has, **or nothing if it is not visible**.
    ///
    /// It is not visible when the history has been scrolled up enough to leave it off the
    /// screen. See the field.
    pub fn cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    /// How many lines have been scrolled up through the history. **Zero is "the latest is being looked at"**.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// A cell by row and column, or `None` if it falls outside.
    pub fn cell(&self, row: u16, col: u16) -> Option<&ViewCell> {
        if row >= self.size.rows || col >= self.size.cols {
            return None;
        }
        self.cells
            .get(row as usize * self.size.cols as usize + col as usize)
    }

    /// A whole row, or `None` if it falls outside. It always has `size.cols` cells.
    pub fn row(&self, row: u16) -> Option<&[ViewCell]> {
        if row >= self.size.rows {
            return None;
        }
        let width = self.size.cols as usize;
        let offset = row as usize * width;
        self.cells.get(offset..offset + width)
    }

    /// All the cells, in reading order.
    pub fn cells(&self) -> &[ViewCell] {
        &self.cells
    }

    /// Marks one visible cell as inside a search's match. Out of the grid, nothing.
    pub(crate) fn mark_matched(&mut self, row: usize, col: usize) {
        let cols = self.size.cols as usize;
        if row < self.size.rows as usize
            && col < cols
            && let Some(cell) = self.cells.get_mut(row * cols + col)
        {
            cell.matched = true;
        }
    }

    /// Resolves the link painted under one visible cell.
    ///
    /// OSC 8 wins over a URL-shaped label. Plain URLs are reconstructed only for the clicked
    /// logical line, joining rows only where the terminal marked a wrap. A candidate that touches a
    /// clipped edge is refused because the snapshot cannot prove that its whole target was painted.
    pub fn link_at(&self, row: u16, col: u16) -> Option<Link> {
        if row >= self.size.rows || col >= self.size.cols {
            return None;
        }
        let clicked = row as usize * self.size.cols as usize + col as usize;
        if let Some(span) = self
            .osc8
            .iter()
            .find(|span| span.start <= clicked && clicked < span.end)
        {
            return Some(Link {
                target: span.target.clone(),
                source: LinkSource::Osc8,
            });
        }

        self.plain_link_at(row as usize, clicked)
    }

    fn plain_link_at(&self, clicked_row: usize, clicked: usize) -> Option<Link> {
        let rows = self.size.rows as usize;
        let cols = self.size.cols as usize;
        let mut first_row = clicked_row;
        while first_row > 0 && self.row_wraps[first_row - 1] {
            first_row -= 1;
        }
        let clipped_start = first_row == 0 && self.continued_from_above;

        let mut last_row = clicked_row;
        while last_row + 1 < rows && self.row_wraps[last_row] {
            last_row += 1;
        }
        let clipped_end = last_row + 1 == rows && self.row_wraps[last_row];

        let mut glyphs = Vec::with_capacity((last_row - first_row + 1) * cols);
        for row in first_row..=last_row {
            let start = row * cols;
            let mut col = 0;
            while col < cols {
                let flat = start + col;
                let cell = &self.cells[flat];
                match cell.width {
                    CellWidth::WideTail | CellWidth::WrapFiller => {
                        col += 1;
                        continue;
                    }
                    CellWidth::WideHead => {
                        glyphs.push(Glyph {
                            c: cell.c,
                            start: flat,
                            end: if col + 1 < cols { flat + 2 } else { flat + 1 },
                        });
                    }
                    CellWidth::Single => glyphs.push(Glyph {
                        c: cell.c,
                        start: flat,
                        end: flat + 1,
                    }),
                }
                col += 1;
            }
        }

        for start in 0..glyphs.len() {
            let scheme_len = if chars_at(&glyphs, start, "https://") {
                8
            } else if chars_at(&glyphs, start, "http://") {
                7
            } else {
                continue;
            };
            if start == 0 {
                if clipped_start {
                    continue;
                }
            } else if !plain_start_boundary(glyphs[start - 1].c) {
                continue;
            }

            let mut end = start + scheme_len;
            while end < glyphs.len() && !plain_end_boundary(glyphs[end].c) {
                end += 1;
            }
            if end == glyphs.len() && clipped_end {
                continue;
            }
            end = trim_plain_end(&glyphs, start, end);
            if end <= start + scheme_len {
                continue;
            }
            let cell_start = glyphs[start].start;
            let cell_end = glyphs[end - 1].end;
            if !(cell_start <= clicked && clicked < cell_end) {
                continue;
            }

            let mut target = String::new();
            for glyph in &glyphs[start..end] {
                target.push(glyph.c);
                target.extend(self.combining_of(&self.cells[glyph.start]).iter().copied());
            }
            return Some(Link {
                target,
                source: LinkSource::Plain,
            });
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct Glyph {
    c: char,
    start: usize,
    end: usize,
}

fn chars_at(glyphs: &[Glyph], start: usize, expected: &str) -> bool {
    expected
        .chars()
        .enumerate()
        .all(|(offset, c)| glyphs.get(start + offset).is_some_and(|glyph| glyph.c == c))
}

fn plain_start_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | '[' | '{' | '<' | '"' | '\'')
}

fn plain_end_boundary(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\'' | '`')
}

/// Where a plain address ends once the sentence around it is taken off.
///
/// **The two trims repeat until neither moves the end**, because prose nests them: in
/// «(see https://x.test/a.)» the bracket hides the full stop, and one pass of each left the stop
/// inside the address.
fn trim_plain_end(glyphs: &[Glyph], start: usize, mut end: usize) -> usize {
    loop {
        let before = end;
        while end > start && matches!(glyphs[end - 1].c, '.' | ',' | ';' | ':' | '!' | '?') {
            end -= 1;
        }

        if let Some(close) = end
            .checked_sub(1)
            .filter(|_| end > start)
            .map(|index| glyphs[index].c)
        {
            let open = match close {
                ')' => Some('('),
                ']' => Some('['),
                '}' => Some('{'),
                _ => None,
            };
            if let Some(open) = open {
                let opens = glyphs[start..end]
                    .iter()
                    .filter(|glyph| glyph.c == open)
                    .count();
                let closes = glyphs[start..end]
                    .iter()
                    .filter(|glyph| glyph.c == close)
                    .count();
                if closes > opens {
                    end -= 1;
                }
            }
        }

        if end == before {
            return end;
        }
    }
}

// ── Translation from the library's vocabulary ───────────────────────────────────────────
//
// It lives here and not in `screen.rs` so that it stays in a single place what is converted into what: if the
// dependency changes shape, this is the file that gets touched.
//
// **Functions and not `impl From`, and this is the milestone's criterion, not style.** A trait `impl`
// carries no visibility: `impl From<Color> for TermColor` is **public** API even though `Color` is from
// the dependency, and it comes out published in the documentation. With it in place, bumping `alacritty_terminal`
// would break this crate's API without anyone touching a line of ours — which is exactly what
// the criterion exists to prevent. It was checked by generating the doc and searching for `From<Color>`.

// `Cell` with an alias, like `CursorShape` right below: this crate **exports its own `Cell`**
// from the sibling module, and with the foreign one imported bare the same name would mean two
// different things depending on the file — a rustdoc link `[`Cell`]` written here would point
// at the dependency's and would resolve without warning. The alias goes on the foreign one, which is the rule.
use alacritty_terminal::term::cell::{Cell as AlacrittyCell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape as AlacrittyShape, NamedColor};

pub(crate) fn color_from(color: Color) -> TermColor {
    match color {
        Color::Spec(rgb) => TermColor::Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        },
        Color::Indexed(i) => TermColor::Indexed(i),
        Color::Named(n) => named_color(n),
    }
}

fn named_color(named: NamedColor) -> TermColor {
    {
        use NamedColor::*;
        match named {
            // The sixteen named ones are the first sixteen entries of the palette.
            Black => TermColor::Indexed(0),
            Red => TermColor::Indexed(1),
            Green => TermColor::Indexed(2),
            Yellow => TermColor::Indexed(3),
            Blue => TermColor::Indexed(4),
            Magenta => TermColor::Indexed(5),
            Cyan => TermColor::Indexed(6),
            White => TermColor::Indexed(7),
            BrightBlack => TermColor::Indexed(8),
            BrightRed => TermColor::Indexed(9),
            BrightGreen => TermColor::Indexed(10),
            BrightYellow => TermColor::Indexed(11),
            BrightBlue => TermColor::Indexed(12),
            BrightMagenta => TermColor::Indexed(13),
            BrightCyan => TermColor::Indexed(14),
            BrightWhite => TermColor::Indexed(15),

            DimBlack => TermColor::Dim(0),
            DimRed => TermColor::Dim(1),
            DimGreen => TermColor::Dim(2),
            DimYellow => TermColor::Dim(3),
            DimBlue => TermColor::Dim(4),
            DimMagenta => TermColor::Dim(5),
            DimCyan => TermColor::Dim(6),
            DimWhite => TermColor::Dim(7),

            Foreground => TermColor::Foreground,
            Background => TermColor::Background,
            Cursor => TermColor::Cursor,
            BrightForeground => TermColor::BrightForeground,
            DimForeground => TermColor::DimForeground,
        }
    }
}

pub(crate) fn shape_from(shape: AlacrittyShape) -> CursorShape {
    match shape {
        AlacrittyShape::Block => CursorShape::Block,
        AlacrittyShape::Underline => CursorShape::Underline,
        AlacrittyShape::Beam => CursorShape::Beam,
        AlacrittyShape::HollowBlock => CursorShape::HollowBlock,
        AlacrittyShape::Hidden => CursorShape::Hidden,
    }
}

/// The underline style, which is five mutually exclusive flags.
///
/// The library removes them all before setting the right one, so there is at most one set.
fn underline_from(flags: Flags) -> Underline {
    if flags.contains(Flags::DOUBLE_UNDERLINE) {
        Underline::Double
    } else if flags.contains(Flags::UNDERCURL) {
        Underline::Curl
    } else if flags.contains(Flags::DOTTED_UNDERLINE) {
        Underline::Dotted
    } else if flags.contains(Flags::DASHED_UNDERLINE) {
        Underline::Dashed
    } else if flags.contains(Flags::UNDERLINE) {
        Underline::Single
    } else {
        Underline::None
    }
}

/// Translates a cell of the engine to ours, saying separately whether it goes inside the selection.
///
/// **The selection arrives as a parameter and is not read from the cell**, because it is not the cell's: the engine keeps it
/// as a range of the grid, not as a flag per cell. Whoever traverses the grid already knows of each
/// cell whether the range includes it, and asking here would force passing it the whole range —that is, to
/// leak a type of the engine all the way to this module.
///
/// **`combining` is the vector of the whole view, and this function writes into it.** What goes attached to the cell
/// does not fit inside it —see [`CombiningSpan`]—, so it is copied to the end of the common vector and the cell is
/// left with the span.
pub(crate) fn cell_from(
    cell: &AlacrittyCell,
    selected: bool,
    combining: &mut Vec<char>,
) -> ViewCell {
    {
        let flags = cell.flags;

        let attached = cell.zerowidth().unwrap_or_default();
        let count = attached.len().min(COMBINING_PER_CELL);
        // **The span is taken before copying**, which is when `offset` equals the current end of the vector.
        let span = CombiningSpan::new(combining.len(), count);
        // **The FIRST ones, not the last.** What decides the presentation is the character right
        // behind the cell's, so clipping from the other end would take away exactly the
        // one that matters.
        combining.extend_from_slice(&attached[..count]);

        // There are three flags, not two: besides the wide character and its companion, the terminal marks
        // the gap it leaves at the end of a row when the character does not fit and drops it whole to the
        // next. Treating it as a normal cell would lose the only sign that that space was not
        // written by the program.
        let width = if flags.contains(Flags::WIDE_CHAR) {
            CellWidth::WideHead
        } else if flags.contains(Flags::WIDE_CHAR_SPACER) {
            CellWidth::WideTail
        } else if flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
            CellWidth::WrapFiller
        } else {
            CellWidth::Single
        };

        // Inverse video is resolved here, swapping the two colors. It is a rule of the
        // terminal, not the theme: leaving it to whoever paints would make it repeat itself in each painting
        // layer, and it is enough for one to skip it for `vim`'s cursor to turn invisible.
        let (fg, bg) = if flags.contains(Flags::INVERSE) {
            (color_from(cell.bg), color_from(cell.fg))
        } else {
            (color_from(cell.fg), color_from(cell.bg))
        };

        ViewCell {
            c: cell.c,
            combining: span,
            fg,
            bg,
            width,
            inverse: flags.contains(Flags::INVERSE),
            bold: flags.contains(Flags::BOLD),
            dim: flags.contains(Flags::DIM),
            italic: flags.contains(Flags::ITALIC),
            underline: underline_from(flags),
            strikeout: flags.contains(Flags::STRIKEOUT),
            hidden: flags.contains(Flags::HIDDEN),
            selected,
            matched: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A view of a single cell with that vector of characters attached behind it.
    fn view_with(combining: Vec<char>) -> View {
        View::new(
            Size { rows: 1, cols: 1 },
            vec![ViewCell::default()],
            combining,
            ViewMetadata {
                row_wraps: vec![false],
                continued_from_above: false,
                osc8: Vec::new(),
            },
            None,
            0,
        )
    }

    /// A cell that points to that span.
    fn cell_pointing_to(span: CombiningSpan) -> ViewCell {
        ViewCell {
            combining: span,
            ..ViewCell::default()
        }
    }

    fn text_view(rows: &[&str], row_wraps: Vec<bool>, continued_from_above: bool) -> View {
        let cols = rows.first().expect("at least one row").chars().count();
        assert!(rows.iter().all(|row| row.chars().count() == cols));
        let cells = rows
            .iter()
            .flat_map(|row| {
                row.chars().map(|c| ViewCell {
                    c,
                    ..ViewCell::default()
                })
            })
            .collect();
        View::new(
            Size {
                rows: rows.len() as u16,
                cols: cols as u16,
            },
            cells,
            Vec::new(),
            ViewMetadata {
                row_wraps,
                continued_from_above,
                osc8: Vec::new(),
            },
            None,
            0,
        )
    }

    #[test]
    fn a_span_that_does_not_fit_in_the_integer_leaves_the_cell_with_nothing_instead_of_being_clipped()
     {
        // **Clipping to the maximum that fits would be showing the neighboring cell's as if it were
        // this one's.** The characters of all the cells live in a common vector, so a `count`
        // saturated to the thirty-one that fit in the integer takes away those of whoever comes
        // behind — and the one who looks has no way to know that what they see was written there by no one.
        // Giving up leaves the cell with nothing attached, which is exactly how it looked before this
        // delivery.
        //
        // The forty characters of the vector are those a saturated `count` would return: if it were
        // clipped, this cell would keep thirty-one of them.
        let view = view_with(vec!['\u{0301}'; 40]);
        let cell = cell_pointing_to(CombiningSpan::new(0, 40));

        assert!(
            view.combining_of(&cell).is_empty(),
            "a span that does not fit was clipped instead of giving up, and what was clipped are characters of another cell"
        );
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "points outside its own vector")]
    fn a_span_that_runs_off_the_vector_fires_instead_of_passing_as_a_cell_with_nothing() {
        // **Returning empty just like that turns a badly built view into "this cell carried
        // nothing".** They are two different things and look the same, so the failure would travel to the painting
        // and manifest as an emoji that lost its selector — a thousand lines from where it broke.
        // In debug it fires here; in production it degrades to empty, which is the only safe thing that
        // can be done with an index you no longer trust.
        let view = view_with(vec!['\u{FE0F}']);
        let cell = cell_pointing_to(CombiningSpan::new(0, 2));

        let _ = view.combining_of(&cell);
    }

    #[test]
    fn a_plain_candidate_starting_at_a_clipped_top_edge_is_inert() {
        let view = text_view(&["https://x "], vec![false], true);

        assert_eq!(
            view.link_at(0, 2),
            None,
            "the snapshot treated its clipped edge as proof of a URL boundary"
        );
    }

    #[test]
    fn a_plain_candidate_continuing_below_the_snapshot_is_inert() {
        let view = text_view(&["https://ex", "ample.test"], vec![true, true], false);

        assert_eq!(
            view.link_at(0, 2),
            None,
            "the snapshot activated a target whose ending had not been painted"
        );
    }
}
