//! Searching what a console still holds, its history included.
//!
//! **The engine already knows how to search its own grid**, wrapped lines and wide characters
//! included, so this module only decides what is searched for and in which direction. The search
//! itself and the rules for what a match is stay in `alacritty_terminal`, the way the rules for
//! what «the text» is stay there for copying (D22).

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::{Match, RegexSearch};

/// What a console is searched for, compiled once and kept for as long as the search is open.
///
/// **Opaque on purpose**: the engine's type does not leave this crate, for the same reason as
/// [`Scroll`](crate::Scroll). And **kept rather than rebuilt per step**, because the engine's lazy
/// automata learn as they walk the grid — throwing them away would pay the learning again on every
/// Enter.
pub struct Search {
    pub(crate) regex: RegexSearch,
    pub(crate) whole_word: bool,
    pub(crate) wrap: bool,
}

/// How a query is matched and walked: the three choices the search box offers.
///
/// **Not part of the query's text**, so changing one never edits what the person typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Off, case is ignored always — not the engine's own rule, which turns case-sensitive the
    /// moment the query holds an uppercase letter. An explicit box means what it says (**D61**).
    pub match_case: bool,
    /// A match counts only when neither of its ends touches a word character.
    pub whole_word: bool,
    /// Past the last match, carry on from the other end of the history.
    pub wrap: bool,
}

impl Default for Options {
    /// What a search did before it had options, except for case, which D61's amendment changed.
    fn default() -> Self {
        Self {
            match_case: false,
            whole_word: false,
            wrap: true,
        }
    }
}

/// Why there is nothing to search with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The query is empty. An empty pattern matches between every two characters, which is no
    /// answer to anything a person typed.
    Empty,
    /// The engine would not build the automaton. A literal of ordinary length never gets here; a
    /// query long enough to pass the engine's size limit does, and saying so beats a search that
    /// silently finds nothing.
    TooLarge,
}

impl Search {
    /// Searches for exactly these characters, the way `options` says.
    ///
    /// **Literal, never a pattern**: what a person types into a terminal's search is text they saw,
    /// and a `.` or a `(` in it means that character. A pattern can also build an automaton large
    /// enough that the engine gives up mid-search and reports no match where there is one, which
    /// it only says in a log nobody reads — the reason and what was discarded are in **D61**.
    ///
    /// **Case is set by an inline flag**, which overrides the case rule the engine builds its
    /// syntax with: that rule looks for an uppercase letter in the pattern, and would otherwise
    /// decide for the person whatever the box says.
    pub fn literal(text: &str, options: Options) -> Result<Self, Refused> {
        if text.is_empty() {
            return Err(Refused::Empty);
        }
        let case = if options.match_case { "(?-i)" } else { "(?i)" };
        RegexSearch::new(&format!("{case}{}", regex_syntax::escape(text)))
            .map(|regex| Self {
                regex,
                whole_word: options.whole_word,
                wrap: options.wrap,
            })
            .map_err(|_| Refused::TooLarge)
    }
}

/// What a step through the console came back with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// A match, now the selection.
    Match,
    /// Nothing anywhere in what the console holds.
    Nothing,
    /// Matches exist, but only behind the end the step walked to, and wrapping is off.
    NoMore,
}

/// Which way through the console a search walks.
///
/// **Older first, because that is where a terminal's answers are**: what is being looked for has
/// usually scrolled away, and the newest line is the one already in front of the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    /// Up, toward the history.
    Older,
    /// Down, toward where the program writes.
    Newer,
}

/// Whether neither end of `found` touches a word character on its own logical line.
///
/// **A word character is a letter, a digit or `_`, in any script**, which is why this is a check
/// on the match and not a `\b` in the pattern: an ASCII boundary takes every non-ASCII character
/// for a non-word one, so `日本` would match inside `日本語`, and the engine's lazy automata do not
/// take a Unicode boundary over non-ASCII text.
pub(crate) fn is_whole_word<T>(term: &Term<T>, found: &Match) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    !neighbour(term, *found.start(), Direction::Left).is_some_and(word)
        && !neighbour(term, *found.end(), Direction::Right).is_some_and(word)
}

/// The character next to `point` that way, on the same logical line, if there is one.
///
/// **Spacer cells are not characters and are stepped over.** A wide character takes two cells,
/// and the engine's two walks leave a match's ends on different halves of it: walking newer a
/// match ends on its last wide character's spacer, walking older one starts on the wide cell, so
/// the cell before it can be the spacer of the wide character in front. A wrap that had no room
/// for a wide character leaves a spacer of its own in the last column, before the line goes on.
fn neighbour<T>(term: &Term<T>, point: Point, toward: Direction) -> Option<char> {
    let grid = term.grid();
    let last = grid.last_column();
    let wraps = |line: Line| grid[line][last].flags.contains(Flags::WRAPLINE);
    let mut at = point;
    loop {
        at = match toward {
            Direction::Left if at.column > Column(0) => Point::new(at.line, at.column - 1),
            Direction::Left if at.line > grid.topmost_line() && wraps(at.line - 1) => {
                Point::new(at.line - 1, last)
            }
            Direction::Right if at.column < last => Point::new(at.line, at.column + 1),
            Direction::Right if at.line < grid.bottommost_line() && wraps(at.line) => {
                Point::new(at.line + 1, Column(0))
            }
            _ => return None,
        };
        let cell = &grid[at];
        if !cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            return Some(cell.c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SCROLLBACK, Screen, Scroll, Size};

    fn screen_with(rows: u16, lines: &[&str]) -> Screen {
        let mut screen = Screen::new(Size { rows, cols: 20 });
        for line in lines {
            screen.advance(format!("{line}\r\n").as_bytes());
        }
        screen
    }

    fn literal(text: &str) -> Search {
        with(text, Options::default())
    }

    fn with(text: &str, options: Options) -> Search {
        Search::literal(text, options).expect("an ordinary literal builds")
    }

    /// The visible row the selection sits on, and that row's text — which is what tells two
    /// matches of the same word apart, since the selected text of both is the word itself.
    fn selected_row(screen: &Screen) -> Option<(u16, String)> {
        let view = screen.view();
        let cols = view.size().cols as usize;
        view.cells()
            .chunks_exact(cols)
            .enumerate()
            .find(|(_, row)| row.iter().any(|cell| cell.selected))
            .map(|(index, row)| {
                let text: String = row.iter().map(|cell| cell.c).collect();
                (index as u16, text.trim_end().to_owned())
            })
    }

    #[test]
    fn a_match_in_the_history_is_scrolled_to_and_selected() {
        let mut filler: Vec<String> = (0..10).map(|n| format!("filler {n}")).collect();
        filler.insert(0, "the needle".to_owned());
        let lines: Vec<&str> = filler.iter().map(String::as_str).collect();
        let mut screen = screen_with(4, &lines);
        assert_eq!(
            screen.view().scroll(),
            0,
            "the premise: nothing scrolled yet"
        );
        assert!(selected_row(&screen).is_none());

        assert_eq!(screen.find(&mut literal("needle"), Toward::Older), Found::Match);

        assert_eq!(screen.selected_text().as_deref(), Some("needle"));
        assert!(
            screen.view().scroll() > 0,
            "the match is in the history and the view stayed at the bottom"
        );
        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("the needle"),
            "the match is selected but not on screen"
        );
    }

    #[test]
    fn the_query_is_text_and_never_a_pattern() {
        // **The order matters**: walking older from the bottom, a pattern meets «abc» first, so only a
        // literal search can come back with «a.c».
        let mut screen = screen_with(4, &["a.c abc"]);

        assert_eq!(screen.find(&mut literal("a.c"), Toward::Older), Found::Match);

        assert_eq!(screen.selected_text().as_deref(), Some("a.c"));
    }

    #[test]
    fn each_step_moves_on_from_the_current_match_and_back() {
        let mut screen = screen_with(6, &["item 1", "item 2", "item 3"]);
        let mut search = literal("item");
        let row_text = |screen: &Screen| selected_row(screen).map(|(_, text)| text);

        assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        assert_eq!(row_text(&screen).as_deref(), Some("item 3"));
        assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        assert_eq!(
            row_text(&screen).as_deref(),
            Some("item 2"),
            "the second press found the match it started on"
        );
        assert_eq!(screen.find(&mut search, Toward::Newer), Found::Match);
        assert_eq!(row_text(&screen).as_deref(), Some("item 3"));
    }

    #[test]
    fn output_arriving_under_an_open_search_does_not_lose_the_place() {
        let lines: Vec<String> = ["item 1", "item 2", "item 3"]
            .iter()
            .map(|line| line.to_string())
            .chain((0..6).map(|n| format!("filler {n}")))
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut screen = screen_with(4, &lines);
        let mut search = literal("item");
        assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("item 3")
        );

        for n in 0..5 {
            screen.advance(format!("noise {n}\r\n").as_bytes());
        }
        assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);

        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("item 2"),
            "new output moved the place the next step starts from"
        );
    }

    #[test]
    fn no_match_leaves_the_selection_and_the_view_where_they_were() {
        let lines: Vec<String> = std::iter::once("item".to_owned())
            .chain((0..8).map(|n| format!("filler {n}")))
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut screen = screen_with(4, &lines);
        assert_eq!(screen.find(&mut literal("item"), Toward::Older), Found::Match);
        let scroll = screen.view().scroll();
        assert!(scroll > 0, "the premise: the view moved to the match");

        assert_eq!(screen.find(&mut literal("absent"), Toward::Older), Found::Nothing);

        assert_eq!(screen.selected_text().as_deref(), Some("item"));
        assert_eq!(screen.view().scroll(), scroll);
    }

    #[test]
    fn a_match_pushed_out_of_the_history_is_no_longer_found() {
        let mut screen = screen_with(4, &["needle"]);
        assert_eq!(
            screen.find(&mut literal("needle"), Toward::Older),
            Found::Match,
            "the premise: it is found while kept"
        );
        screen.clear_selection();

        for n in 0..SCROLLBACK + 10 {
            screen.advance(format!("filler {n}\r\n").as_bytes());
        }

        assert_eq!(screen.find(&mut literal("needle"), Toward::Older), Found::Nothing);
        assert_eq!(screen.selected_text(), None);
    }

    /// Which columns of each visible row are marked as matches.
    fn marked(view: &crate::View) -> Vec<Vec<usize>> {
        let cols = view.size().cols as usize;
        view.cells()
            .chunks_exact(cols)
            .map(|row| (0..cols).filter(|&col| row[col].matched).collect())
            .collect()
    }

    #[test]
    fn every_match_on_screen_is_marked_and_nothing_else() {
        let screen = screen_with(4, &["a needle", "none here", "needle needle"]);

        let mut view = screen.view();
        screen.mark_matches(&mut view, &mut literal("needle"));

        assert_eq!(
            marked(&view),
            vec![
                vec![2, 3, 4, 5, 6, 7],
                vec![],
                (0..6).chain(7..13).collect(),
                vec![]
            ]
        );
        assert!(
            marked(&screen.view()).iter().all(Vec::is_empty),
            "the plain view marked matches with no search open"
        );
    }

    #[test]
    fn a_wide_character_is_marked_whole() {
        let screen = screen_with(4, &["日本 x"]);

        let mut view = screen.view();
        screen.mark_matches(&mut view, &mut literal("日本"));

        assert_eq!(
            marked(&view)[0],
            vec![0, 1, 2, 3],
            "half of a wide character was left unmarked"
        );
    }

    #[test]
    fn a_match_wrapped_across_the_top_edge_is_marked_on_the_part_that_shows() {
        // Ten columns: the needle starts at the end of one row and the wrap carries its tail to the
        // next. Scrolled so that only the tail is on screen.
        let mut screen = Screen::new(Size { rows: 3, cols: 10 });
        screen.advance(b"xxxxxxxxneedle\r\n");
        for n in 0..5 {
            screen.advance(format!("filler {n}\r\n").as_bytes());
        }
        screen.scroll_by(Scroll::Lines(4));
        let top: String = screen.view().cells()[..4]
            .iter()
            .map(|cell| cell.c)
            .collect();
        assert_eq!(top, "edle", "the premise: the wrapped tail is the top row");

        let mut view = screen.view();
        screen.mark_matches(&mut view, &mut literal("needle"));

        assert_eq!(marked(&view)[0], vec![0, 1, 2, 3]);
    }

    fn whole_word() -> Options {
        Options {
            whole_word: true,
            ..Options::default()
        }
    }

    #[test]
    fn match_case_off_ignores_case_whatever_the_query_holds_and_on_respects_it() {
        let mut screen = screen_with(4, &["Needle"]);
        assert_eq!(
            screen.find(&mut literal("needle"), Toward::Older),
            Found::Match
        );
        let mut screen = screen_with(4, &["needle"]);
        assert_eq!(
            screen.find(&mut literal("Needle"), Toward::Older),
            Found::Match,
            "an uppercase letter made the search case-sensitive with the box unchecked"
        );

        let exact = Options {
            match_case: true,
            ..Options::default()
        };
        let mut screen = screen_with(4, &["Needle"]);
        assert_eq!(
            screen.find(&mut with("needle", exact), Toward::Older),
            Found::Nothing
        );
        // The newer line is the lowercase one, so a search that ignored case would stop there.
        let mut screen = screen_with(4, &["Needle", "needle"]);
        assert_eq!(
            screen.find(&mut with("Needle", exact), Toward::Older),
            Found::Match
        );
        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("Needle")
        );
    }

    #[test]
    fn whole_word_skips_a_query_inside_a_longer_word() {
        let mut screen = screen_with(4, &["needle kneed"]);
        assert_eq!(
            screen.find(&mut with("need", whole_word()), Toward::Older),
            Found::Nothing,
            "a word containing the query was taken for the word"
        );

        let mut screen = screen_with(4, &["a need.", "needle"]);
        assert_eq!(
            screen.find(&mut with("need", whole_word()), Toward::Older),
            Found::Match
        );
        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("a need.")
        );
    }

    #[test]
    fn whole_word_reads_past_a_wide_character_s_spacer_on_both_sides_and_both_ways() {
        for line in ["日本語", "語日本"] {
            for toward in [Toward::Older, Toward::Newer] {
                let mut screen = screen_with(4, &[line]);
                assert_eq!(
                    screen.find(&mut with("日本", whole_word()), toward),
                    Found::Nothing,
                    "«日本» was a whole word inside «{line}» walking {toward:?}"
                );
            }
        }
        let mut screen = screen_with(4, &["語 日本"]);
        assert_eq!(
            screen.find(&mut with("日本", whole_word()), Toward::Older),
            Found::Match
        );
    }

    #[test]
    fn whole_word_follows_a_wrap_to_the_character_on_the_other_row() {
        // Five columns, so the text wraps after five characters.
        let wrapped = |text: &str| {
            let mut screen = Screen::new(Size { rows: 3, cols: 5 });
            screen.advance(text.as_bytes());
            screen
        };
        for toward in [Toward::Older, Toward::Newer] {
            assert_eq!(
                wrapped("abcdeneed").find(&mut with("need", whole_word()), toward),
                Found::Nothing,
                "the letter before the wrap did not count as touching the match, walking {toward:?}"
            );
            assert_eq!(
                wrapped(" needcde").find(&mut with("need", whole_word()), toward),
                Found::Nothing,
                "the letter after the wrap did not count as touching the match, walking {toward:?}"
            );
            assert_eq!(
                wrapped("abcd need").find(&mut with("need", whole_word()), toward),
                Found::Match
            );
            // «日» has no room after four letters, so the row ends in a spacer of its own.
            assert_eq!(
                wrapped("abcd日本").find(&mut with("日本", whole_word()), toward),
                Found::Nothing,
                "the spacer a wrap leaves before a wide character hid the letter in front, walking \
                 {toward:?}"
            );
            assert_eq!(
                wrapped("abc 日本").find(&mut with("日本", whole_word()), toward),
                Found::Match
            );
        }
    }

    #[test]
    fn whole_word_with_only_rejected_candidates_ends() {
        let mut screen = screen_with(4, &["needle", "needles", "kneed"]);
        for toward in [Toward::Older, Toward::Newer] {
            assert_eq!(
                screen.find(&mut with("need", whole_word()), toward),
                Found::Nothing
            );
        }
    }

    #[test]
    fn whole_word_marks_only_whole_words() {
        let screen = screen_with(4, &["need needle"]);

        let mut view = screen.view();
        screen.mark_matches(&mut view, &mut with("need", whole_word()));

        assert_eq!(marked(&view)[0], vec![0, 1, 2, 3]);
    }

    #[test]
    fn without_wrap_the_last_match_is_the_last_and_nothing_moves() {
        let lines: Vec<String> = ["item 1", "item 2"]
            .iter()
            .map(|line| line.to_string())
            .chain((0..6).map(|n| format!("filler {n}")))
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let row_text = |screen: &Screen| selected_row(screen).map(|(_, text)| text);

        let mut screen = screen_with(4, &lines);
        let mut search = with(
            "item",
            Options {
                wrap: false,
                ..Options::default()
            },
        );
        assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        assert_eq!(row_text(&screen).as_deref(), Some("item 1"));
        let scroll = screen.view().scroll();

        assert_eq!(screen.find(&mut search, Toward::Older), Found::NoMore);
        assert_eq!(row_text(&screen).as_deref(), Some("item 1"));
        assert_eq!(screen.view().scroll(), scroll);
        // The same end with a line above the oldest match, so the step does not fall off the
        // history's edge and only the match's place tells the wrap apart.
        let mut above = vec!["header"];
        above.extend_from_slice(&lines);
        let mut screen = screen_with(4, &above);
        for _ in 0..2 {
            assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        }
        assert_eq!(screen.find(&mut search, Toward::Older), Found::NoMore);
        assert_eq!(row_text(&screen).as_deref(), Some("item 1"));
        // And walking newer from the newest match.
        assert_eq!(screen.find(&mut search, Toward::Newer), Found::Match);
        assert_eq!(screen.find(&mut search, Toward::Newer), Found::NoMore);
        assert_eq!(row_text(&screen).as_deref(), Some("item 2"));

        let mut screen = screen_with(4, &lines);
        let mut search = literal("item");
        for _ in 0..2 {
            assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        }
        assert_eq!(screen.find(&mut search, Toward::Older), Found::Match);
        assert_eq!(
            row_text(&screen).as_deref(),
            Some("item 2"),
            "with wrapping on, the walk did not carry on from the other end"
        );
    }

    #[test]
    fn nothing_typed_is_not_a_search() {
        assert_eq!(Search::literal("", Options::default()).err(), Some(Refused::Empty));
    }
}
