//! Walking the grid a logical line at a time: the step to the next match, and marking.

use std::ops::{Range, RangeInclusive};

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Line, Point};

use super::line::{Logical, extract, first_row, last_row, wraps};
use super::{Search, Toward, next_char};

/// A match a step found, and whether it was reached by carrying on from the other end.
pub(crate) struct Hit {
    pub(crate) cells: RangeInclusive<Point>,
    pub(crate) wrapped: bool,
}

/// The matches inside `span` of `logical` that count, left to right.
///
/// **An empty match never counts**: it has no cell to select or mark. **A candidate that whole word
/// rejects gives its characters back**: the search goes on past where it started, so
/// a whole word overlapping a rejected candidate — `1.1` at the end of `v1.1.1`, `b` in `xa b` for
/// `a b|b` — is still found. Matches that count never overlap.
fn matches(search: &Search, logical: &Logical, span: Range<usize>) -> Vec<Range<usize>> {
    let text = &logical.text;
    let mut found = Vec::new();
    let mut at = span.start;
    while let Some(range) = search.pattern.find_in(text, at..span.end) {
        if !search.whole_word || is_whole_word(text, range.clone()) {
            at = range.end;
            found.push(range);
        } else {
            match past_rejected(text, range.start) {
                Some(next) => at = next,
                None => break,
            }
        }
    }
    found
}

/// Where the search goes on after a whole-word candidate starting at `start` was rejected: one
/// character on, and then **past every position inside a run of word characters**, where a match
/// would start with a word character touching another and be rejected the same way. Going on one
/// character at a time instead made a long run quadratic: `\d+` over `x` and a scrollback of
/// digits finds and rejects one candidate per digit, each read to the end of the run.
fn past_rejected(text: &str, start: usize) -> Option<usize> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut at = next_char(text, start)?;
    while text[at..].chars().next().is_some_and(word)
        && text[..at].chars().next_back().is_some_and(word)
    {
        at = next_char(text, at)?;
    }
    Some(at)
}

/// Whether `range` of `text` is a whole word: each of its ends that is itself a word character
/// does not touch another word character.
///
/// **A word character is a letter, a digit or `_`, in any script**, and **only an end that is
/// itself one has to stand apart** — the boundary rule of `\b` and of the reference editors: «foo(»
/// is a whole word in «foo(bar)», its own «(» being the boundary. Judged here rather than with a
/// `\b` in the pattern so that a literal and a pattern follow the same rule.
fn is_whole_word(text: &str, range: Range<usize>) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let inside = &text[range.clone()];
    let stands_apart = |own: Option<char>, outside: Option<char>| {
        !own.is_some_and(word) || !outside.is_some_and(word)
    };
    stands_apart(
        inside.chars().next(),
        text[..range.start].chars().next_back(),
    ) && stands_apart(inside.chars().next_back(), text[range.end..].chars().next())
}

/// The matches of `search` from row `first` to row `last`, one logical line at a time, each
/// handed to `each` with the cells it takes.
///
/// **The rows are cut at every hard break**, so a match never joins two lines the program ended,
/// and **a logical line that runs past `first` or `last` is matched only between them**: marking
/// hands its bounded rows here, and reading the rest of a line wrapped across the whole history on
/// every frame is exactly the cost D61 says marking does not have. **One row past each cut is read
/// as context and never matched**: the character on the other side of a cut is what tells whole
/// word, `^`, `$` and `\b` that the cut is not the line's start or end.
pub(crate) fn mark<T>(
    term: &Term<T>,
    search: &Search,
    first: Line,
    last: Line,
    mut each: impl FnMut(RangeInclusive<Point>),
) {
    let grid = term.grid();
    let (top, bottom) = (grid.topmost_line(), grid.bottommost_line());
    let mut from = first;
    while from <= last {
        let mut to = from;
        while to < last && wraps(term, to) {
            to += 1;
        }
        let before = if from > top && wraps(term, from - 1) {
            from - 1
        } else {
            from
        };
        let after = if to < bottom && wraps(term, to) {
            to + 1
        } else {
            to
        };
        let logical = extract(term, before, after);
        for range in matches(search, &logical, logical.span(from, to)) {
            each(logical.cells(range));
        }
        from = to + 1;
    }
}

/// The next match of `search` from `origin` that way, through the whole of what the console holds.
///
/// **Walking older, a match counts when it starts at or before `origin`; walking newer, when it ends
/// at or after it.** The caller puts `origin` one cell past the current match, since a match sitting
/// on it would otherwise be found again on every press. Past the history's edge the walk carries on
/// from the other end, and what it finds there is marked as wrapped — as is everything, when
/// `origin_wrapped` says the origin itself already stepped off an edge.
pub(crate) fn step<T>(
    term: &Term<T>,
    search: &Search,
    origin: Point,
    toward: Toward,
    origin_wrapped: bool,
) -> Option<Hit> {
    let grid = term.grid();
    let (top, bottom) = (grid.topmost_line(), grid.bottommost_line());
    let home = (first_row(term, origin.line), last_row(term, origin.line));
    let home_line = extract(term, home.0, home.1);
    let found = |cells, wrapped| Hit {
        cells,
        wrapped: wrapped || origin_wrapped,
    };

    match toward {
        Toward::Older => {
            let before = |cells: &RangeInclusive<Point>| *cells.start() <= origin;
            if let Some(cells) = pick(search, &home_line, toward, before) {
                return Some(found(cells, false));
            }
            if let Some(cells) = upward(term, search, home.0 - 1, top) {
                return Some(found(cells, false));
            }
            if let Some(cells) = upward(term, search, bottom, home.1 + 1) {
                return Some(found(cells, true));
            }
            pick(search, &home_line, toward, |cells| !before(cells)).map(|c| found(c, true))
        }
        Toward::Newer => {
            let after = |cells: &RangeInclusive<Point>| *cells.end() >= origin;
            if let Some(cells) = pick(search, &home_line, toward, after) {
                return Some(found(cells, false));
            }
            if let Some(cells) = downward(term, search, home.1 + 1, bottom) {
                return Some(found(cells, false));
            }
            if let Some(cells) = downward(term, search, top, home.0 - 1) {
                return Some(found(cells, true));
            }
            pick(search, &home_line, toward, |cells| !after(cells)).map(|c| found(c, true))
        }
    }
}

/// The match in `logical` nearest the way the step walks — the last one walking older, the first
/// walking newer — among those `counts` accepts.
fn pick(
    search: &Search,
    logical: &Logical,
    toward: Toward,
    counts: impl Fn(&RangeInclusive<Point>) -> bool,
) -> Option<RangeInclusive<Point>> {
    let mut cells = matches(search, logical, 0..logical.text.len())
        .into_iter()
        .map(|range| logical.cells(range))
        .filter(|cells| counts(cells));
    match toward {
        Toward::Older => cells.last(),
        Toward::Newer => cells.next(),
    }
}

/// The last match in the logical lines that end at row `from` or above it, down to row `until`,
/// walking up a whole logical line at a time. **`from` must be the last row of its logical line**:
/// the rows below it are not read.
fn upward<T>(
    term: &Term<T>,
    search: &Search,
    from: Line,
    until: Line,
) -> Option<RangeInclusive<Point>> {
    let mut line = from;
    while line >= until {
        let first = first_row(term, line);
        let logical = extract(term, first, line);
        if let Some(cells) = pick(search, &logical, Toward::Older, |_| true) {
            return Some(cells);
        }
        line = first - 1;
    }
    None
}

/// The first match in the logical lines that start at row `from` or below it, up to row `until`.
/// **`from` must be the first row of its logical line**: the rows above it are not read.
fn downward<T>(
    term: &Term<T>,
    search: &Search,
    from: Line,
    until: Line,
) -> Option<RangeInclusive<Point>> {
    let mut line = from;
    while line <= until {
        let last = last_row(term, line);
        let logical = extract(term, line, last);
        if let Some(cells) = pick(search, &logical, Toward::Newer, |_| true) {
            return Some(cells);
        }
        line = last + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_word_is_judged_on_the_characters_around_it_in_any_script() {
        assert!(is_whole_word("a need.", 2..6));
        assert!(!is_whole_word("needle", 0..4));
        assert!(!is_whole_word("kneed", 1..5));
        assert!(is_whole_word("foo(bar)", 0..4), "«(» is its own boundary");
        assert!(!is_whole_word("xfoo(bar)", 1..5));
        assert!(!is_whole_word("日本語", 0..6));
        assert!(is_whole_word("語 日本", 4..10));
    }
}
