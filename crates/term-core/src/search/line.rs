//! A logical line of the grid as text the regex engine can read, and the way back to its cells.
//!
//! **A logical line is what the program wrote between two line breaks**: the rows a wrap joined.
//! A match never crosses a hard break, as it never did in the terminal engine's own search.

use std::ops::{Range, RangeInclusive};

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;

/// The text of some rows, and where each of its characters sits on the grid.
pub(crate) struct Logical {
    pub(crate) text: String,
    /// For each character, in order: its first byte in `text`, its cell, and the last cell it
    /// takes — the spacer, for a wide character that has one.
    chars: Vec<(usize, Point, Point)>,
}

impl Logical {
    /// The cells a match over `bytes` of the text takes. `bytes` is never empty.
    ///
    /// **A wide character ends on its spacer**, which is where the terminal engine's walk toward
    /// newer lines left a match's end, so a selection made from a match is the one it always was.
    pub(crate) fn cells(&self, bytes: Range<usize>) -> RangeInclusive<Point> {
        let owner = |byte: usize| self.chars.partition_point(|&(at, ..)| at <= byte) - 1;
        self.chars[owner(bytes.start)].1..=self.chars[owner(bytes.end - 1)].2
    }
}

/// Whether the logical line `line` belongs to goes on in the row below.
pub(crate) fn wraps<T>(term: &Term<T>, line: Line) -> bool {
    let grid = term.grid();
    grid[line][grid.last_column()].flags.contains(Flags::WRAPLINE)
}

/// The first row of the logical line `line` belongs to.
pub(crate) fn first_row<T>(term: &Term<T>, mut line: Line) -> Line {
    let top = term.grid().topmost_line();
    while line > top && wraps(term, line - 1) {
        line -= 1;
    }
    line
}

/// The last row of the logical line `line` belongs to.
pub(crate) fn last_row<T>(term: &Term<T>, mut line: Line) -> Line {
    let bottom = term.grid().bottommost_line();
    while line < bottom && wraps(term, line) {
        line += 1;
    }
    line
}

/// The text of the rows `first` to `last`, joined with nothing between them.
///
/// **Each cell gives its own character and nothing else**, as the terminal engine's search read
/// it: spacers give nothing, characters drawn over a cell (combining marks, a variation selector)
/// are not part of the text, and a row's padding is — so `$` matches after it, as it always did.
pub(crate) fn extract<T>(term: &Term<T>, first: Line, last: Line) -> Logical {
    let grid = term.grid();
    let last_column = grid.last_column();
    let columns = grid.columns();
    let rows = (last.0 - first.0 + 1).max(0) as usize;
    let mut logical = Logical {
        text: String::with_capacity(rows * columns),
        chars: Vec::with_capacity(rows * columns),
    };
    for line in (first.0..=last.0).map(Line) {
        let row = &grid[line];
        for column in (0..columns).map(Column) {
            let cell = &row[column];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            let at = Point::new(line, column);
            // **A wide character in the last column can have no spacer**: a screen narrowed
            // without reflow truncates the row under it. Then the character ends where it is.
            let end = if cell.flags.contains(Flags::WIDE_CHAR)
                && column < last_column
                && row[column + 1].flags.contains(Flags::WIDE_CHAR_SPACER)
            {
                Point::new(line, column + 1)
            } else {
                at
            };
            logical.chars.push((logical.text.len(), at, end));
            logical.text.push(cell.c);
        }
    }
    logical
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Screen, Size};

    fn screen(cols: u16, bytes: &str) -> Screen {
        let mut screen = Screen::new(Size { rows: 3, cols });
        screen.advance(bytes.as_bytes());
        screen
    }

    fn cells_of(logical: &Logical, needle: &str) -> RangeInclusive<Point> {
        let at = logical.text.find(needle).expect("the needle is in the text");
        logical.cells(at..at + needle.len())
    }

    #[test]
    fn a_wrapped_line_is_one_text_and_a_hard_break_ends_it() {
        let screen = screen(5, "abcdefg\r\nxyz");
        let term = screen.term();
        assert!(wraps(term, Line(0)), "the premise: the first row wraps");
        assert_eq!(first_row(term, Line(1)), Line(0));
        assert_eq!(last_row(term, Line(0)), Line(1));
        assert_eq!(last_row(term, Line(2)), Line(2), "the hard break was crossed");

        let logical = extract(term, Line(0), Line(1));
        assert_eq!(logical.text, "abcdefg   ");
        assert_eq!(
            cells_of(&logical, "def"),
            Point::new(Line(0), Column(3))..=Point::new(Line(1), Column(0))
        );
    }

    #[test]
    fn a_wide_character_is_one_character_that_ends_on_its_spacer() {
        let screen = screen(10, "a日本b");
        let logical = extract(screen.term(), Line(0), Line(0));
        assert!(logical.text.starts_with("a日本b"), "{:?}", logical.text);
        assert_eq!(
            cells_of(&logical, "日本"),
            Point::new(Line(0), Column(1))..=Point::new(Line(0), Column(4))
        );
        assert_eq!(
            cells_of(&logical, "b"),
            Point::new(Line(0), Column(5))..=Point::new(Line(0), Column(5))
        );
    }

    #[test]
    fn the_spacer_a_wrap_leaves_before_a_wide_character_is_not_text() {
        // Five columns: «日» has no room after four letters and goes to the next row.
        let screen = screen(5, "abcd日");
        let logical = extract(screen.term(), Line(0), Line(1));
        assert!(logical.text.starts_with("abcd日"), "{:?}", logical.text);
        assert_eq!(
            cells_of(&logical, "d日"),
            Point::new(Line(0), Column(3))..=Point::new(Line(1), Column(1))
        );
    }

    #[test]
    fn a_wide_character_in_the_last_column_without_a_spacer_ends_where_it_is() {
        // Narrowing the alternate screen truncates rows without reflowing them, which leaves the
        // wide character in the last column and its spacer cut away.
        let mut screen = screen(4, "\x1b[?1049hab日");
        screen.resize(Size { rows: 3, cols: 3 });
        let term = screen.term();
        let last = term.grid().last_column();
        let cell = &term.grid()[Line(0)][last];
        assert!(
            cell.flags.contains(Flags::WIDE_CHAR) && cell.c == '日',
            "the premise: a wide character with no spacer in the last column"
        );

        let logical = extract(term, Line(0), Line(0));
        assert_eq!(
            cells_of(&logical, "日"),
            Point::new(Line(0), last)..=Point::new(Line(0), last)
        );
    }

    #[test]
    fn characters_drawn_over_a_cell_are_not_text() {
        let screen = screen(10, "e\u{301}x");
        let logical = extract(screen.term(), Line(0), Line(0));
        assert!(logical.text.starts_with("ex"), "{:?}", logical.text);
    }
}
