//! Searching what a console still holds, its history included — and, through [`Pattern`], any
//! other text the application searches the way the console does.
//!
//! **The grid is read a logical line at a time, as text**, and a regex engine that never gives up
//! looks through it. The terminal engine's own search walked the grid cell by cell with a lazy DFA
//! that, on a pattern too large for its cache, stopped and reported no match where there was one,
//! saying so only in a log (D61). Wrapped lines and wide characters are [`line`]'s; walking from one
//! line to the next is [`walk`]'s; whole word, which no engine judges the way the box means it, is
//! decided on each match there.

mod line;
pub(crate) mod walk;

use std::borrow::Cow;
use std::ops::Range;

use regex_automata::Input;
use regex_automata::meta::{self, Regex};
use regex_automata::util::syntax;

/// The largest automaton a query may build: what the terminal engine's search allowed, and a
/// ceiling that keeps a pattern compiled on every keystroke from costing tens of milliseconds to
/// build — the engine's own default is five times this, and a short Unicode pattern reaches it.
const SIZE_LIMIT: usize = 2 * 1024 * 1024;

/// A query compiled: the text to find, escaped, or a pattern, with the box's case.
///
/// **Opaque on purpose**: the engine's type does not leave this crate, for the same reason as
/// [`Scroll`](crate::Scroll). The console's search and Files' name search both build one, so a
/// query means the same thing in either place and is refused for the same reasons.
pub struct Pattern {
    regex: Regex,
}

impl Pattern {
    /// Compiles `text` the way `options` says: as the text to find, escaped, or as a pattern. Only
    /// `regex` and `match_case` are read; whole word and wrapping are how a console walks.
    ///
    /// **Case is set on the engine's syntax** rather than with an inline flag ahead of the query,
    /// so a pattern's own flag still overrides the box and the parser's complaints point at what
    /// the person typed.
    pub fn new(text: &str, options: Options) -> Result<Self, Refused> {
        if text.is_empty() {
            return Err(Refused::Empty);
        }
        let pattern = if options.regex {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(regex_syntax::escape(text))
        };
        Regex::builder()
            .configure(Regex::config().nfa_size_limit(Some(SIZE_LIMIT)))
            .syntax(syntax::Config::new().case_insensitive(!options.match_case))
            .build(&pattern)
            .map(|regex| Self { regex })
            .map_err(refused)
    }

    /// Where `haystack` first holds a match, in bytes. **An empty match is no match**: it has
    /// nothing to select or mark, and a pattern such as `x*` would otherwise match everywhere.
    pub fn find(&self, haystack: &str) -> Option<Range<usize>> {
        self.find_in(haystack, 0..haystack.len())
    }

    /// The first non-empty match that lies inside `span` of `haystack`. **What lies around the
    /// span is still read** by `^`, `$` and `\b`, so a span cut out of a longer line does not turn
    /// its edges into a line's start or end.
    pub(crate) fn find_in(&self, haystack: &str, span: Range<usize>) -> Option<Range<usize>> {
        let mut at = span.start;
        while at <= span.end {
            let found = self
                .regex
                .find(Input::new(haystack).range(at..span.end))?
                .range();
            if !found.is_empty() {
                return Some(found);
            }
            at = next_char(haystack, found.start)?;
        }
        None
    }
}

/// The byte where the character after the one at `at` starts, if there is one.
pub(crate) fn next_char(text: &str, at: usize) -> Option<usize> {
    text[at..].chars().next().map(|c| at + c.len_utf8())
}

/// What a console is searched for, compiled once and kept for as long as the search is open.
pub struct Search {
    pub(crate) pattern: Pattern,
    pub(crate) whole_word: bool,
    pub(crate) wrap: bool,
}

/// How a query is matched and walked: the choices the search box offers.
///
/// **Not part of the query's text**, so changing one never edits what the person typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Off, case is ignored always — not the terminal engine's old rule, which turned
    /// case-sensitive the moment the query held an uppercase letter. An explicit box means what it
    /// says (**D61**). A pattern's own inline flag, `(?i)` or `(?-i)`, still wins over it.
    pub match_case: bool,
    /// A match counts only when each of its ends that is a word character does not touch another
    /// one — the boundary rule of `\b`.
    pub whole_word: bool,
    /// Past the last match, carry on from the other end of the history.
    pub wrap: bool,
    /// The query is a pattern rather than the text to find.
    pub regex: bool,
}

impl Default for Options {
    /// What a search did before it had options, except for case, which D61's amendment changed.
    fn default() -> Self {
        Self {
            match_case: false,
            whole_word: false,
            wrap: true,
            regex: false,
        }
    }
}

/// Why there is nothing to search with — never the same thing as a search that found nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The query is empty. An empty pattern matches between every two characters, which is no
    /// answer to anything a person typed.
    Empty,
    /// The automaton would pass [`SIZE_LIMIT`]. A literal of ordinary length never gets here; a
    /// short pattern can — `\w{50}` does, because each `\w` is every word character of Unicode.
    TooLarge,
    /// The pattern does not parse, and why, in the parser's words.
    Invalid(String),
}

impl Search {
    /// Compiles `text` as [`Pattern::new`] does, and keeps how the console walks it.
    pub fn new(text: &str, options: Options) -> Result<Self, Refused> {
        Pattern::new(text, options).map(|pattern| Self {
            pattern,
            whole_word: options.whole_word,
            wrap: options.wrap,
        })
    }
}

/// What a failed build tells the person.
fn refused(error: meta::BuildError) -> Refused {
    if let Some(syntax) = error.syntax_error() {
        // The parser's full message draws the pattern with a caret under the fault over several
        // lines; the box has one, so only what is wrong is kept.
        return Refused::Invalid(match syntax {
            regex_syntax::Error::Parse(error) => error.kind().to_string(),
            regex_syntax::Error::Translate(error) => error.kind().to_string(),
            other => other.to_string(),
        });
    }
    if error.size_limit().is_some() {
        return Refused::TooLarge;
    }
    Refused::Invalid(error.to_string())
}

/// Which rows of the view a step keeps its match off: the ones the search's box takes, at the top
/// or at the bottom of the console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepClear {
    Top(usize),
    Bottom(usize),
}

impl KeepClear {
    /// No rows: the match lands where the walk puts it.
    pub const NONE: Self = Self::Top(0);
}

/// What a step through the console came back with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// A match, now the selection.
    Match,
    /// Nothing anywhere in what the console holds.
    Nothing,
    /// A match that counts lies only past the end the step walked to, and wrapping is off.
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
        Search::new(text, options).expect("an ordinary literal builds")
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

        assert_eq!(
            screen.find(&literal("needle"), Toward::Older, KeepClear::NONE),
            Found::Match
        );

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

        assert_eq!(
            screen.find(&literal("a.c"), Toward::Older, KeepClear::NONE),
            Found::Match
        );

        assert_eq!(screen.selected_text().as_deref(), Some("a.c"));
    }

    #[test]
    fn each_step_moves_on_from_the_current_match_and_back() {
        let mut screen = screen_with(6, &["item 1", "item 2", "item 3"]);
        let search = literal("item");
        let row_text = |screen: &Screen| selected_row(screen).map(|(_, text)| text);

        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(row_text(&screen).as_deref(), Some("item 3"));
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(
            row_text(&screen).as_deref(),
            Some("item 2"),
            "the second press found the match it started on"
        );
        assert_eq!(
            screen.find(&search, Toward::Newer, KeepClear::NONE),
            Found::Match
        );
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
        let search = literal("item");
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("item 3")
        );

        for n in 0..5 {
            screen.advance(format!("noise {n}\r\n").as_bytes());
        }
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );

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
        assert_eq!(
            screen.find(&literal("item"), Toward::Older, KeepClear::NONE),
            Found::Match
        );
        let scroll = screen.view().scroll();
        assert!(scroll > 0, "the premise: the view moved to the match");

        assert_eq!(
            screen.find(&literal("absent"), Toward::Older, KeepClear::NONE),
            Found::Nothing
        );

        assert_eq!(screen.selected_text().as_deref(), Some("item"));
        assert_eq!(screen.view().scroll(), scroll);
    }

    #[test]
    fn a_match_pushed_out_of_the_history_is_no_longer_found() {
        let mut screen = screen_with(4, &["needle"]);
        assert_eq!(
            screen.find(&literal("needle"), Toward::Older, KeepClear::NONE),
            Found::Match,
            "the premise: it is found while kept"
        );
        screen.clear_selection();

        for n in 0..SCROLLBACK + 10 {
            screen.advance(format!("filler {n}\r\n").as_bytes());
        }

        assert_eq!(
            screen.find(&literal("needle"), Toward::Older, KeepClear::NONE),
            Found::Nothing
        );
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
        screen.mark_matches(&mut view, &literal("needle"));

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
        screen.mark_matches(&mut view, &literal("日本"));

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
        screen.mark_matches(&mut view, &literal("needle"));

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
            screen.find(&literal("needle"), Toward::Older, KeepClear::NONE),
            Found::Match
        );
        let mut screen = screen_with(4, &["needle"]);
        assert_eq!(
            screen.find(&literal("Needle"), Toward::Older, KeepClear::NONE),
            Found::Match,
            "an uppercase letter made the search case-sensitive with the box unchecked"
        );

        let exact = Options {
            match_case: true,
            ..Options::default()
        };
        let mut screen = screen_with(4, &["Needle"]);
        assert_eq!(
            screen.find(&with("needle", exact), Toward::Older, KeepClear::NONE),
            Found::Nothing
        );
        // The newer line is the lowercase one, so a search that ignored case would stop there.
        let mut screen = screen_with(4, &["Needle", "needle"]);
        assert_eq!(
            screen.find(&with("Needle", exact), Toward::Older, KeepClear::NONE),
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
            screen.find(&with("need", whole_word()), Toward::Older, KeepClear::NONE),
            Found::Nothing,
            "a word containing the query was taken for the word"
        );

        let mut screen = screen_with(4, &["a need.", "needle"]);
        assert_eq!(
            screen.find(&with("need", whole_word()), Toward::Older, KeepClear::NONE),
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
                    screen.find(&with("日本", whole_word()), toward, KeepClear::NONE),
                    Found::Nothing,
                    "«日本» was a whole word inside «{line}» walking {toward:?}"
                );
            }
        }
        let mut screen = screen_with(4, &["語 日本"]);
        assert_eq!(
            screen.find(&with("日本", whole_word()), Toward::Older, KeepClear::NONE),
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
                wrapped("abcdeneed").find(&with("need", whole_word()), toward, KeepClear::NONE),
                Found::Nothing,
                "the letter before the wrap did not count as touching the match, walking {toward:?}"
            );
            assert_eq!(
                wrapped(" needcde").find(&with("need", whole_word()), toward, KeepClear::NONE),
                Found::Nothing,
                "the letter after the wrap did not count as touching the match, walking {toward:?}"
            );
            assert_eq!(
                wrapped("abcd need").find(&with("need", whole_word()), toward, KeepClear::NONE),
                Found::Match
            );
            // «日» has no room after four letters, so the row ends in a spacer of its own.
            assert_eq!(
                wrapped("abcd日本").find(&with("日本", whole_word()), toward, KeepClear::NONE),
                Found::Nothing,
                "the spacer a wrap leaves before a wide character hid the letter in front, walking \
                 {toward:?}"
            );
            assert_eq!(
                wrapped("abc 日本").find(&with("日本", whole_word()), toward, KeepClear::NONE),
                Found::Match
            );
        }
    }

    #[test]
    fn whole_word_with_only_rejected_candidates_ends() {
        let mut screen = screen_with(4, &["needle", "needles", "kneed"]);
        for toward in [Toward::Older, Toward::Newer] {
            assert_eq!(
                screen.find(&with("need", whole_word()), toward, KeepClear::NONE),
                Found::Nothing
            );
        }
    }

    #[test]
    fn whole_word_marks_only_whole_words() {
        let screen = screen_with(4, &["need needle"]);

        let mut view = screen.view();
        screen.mark_matches(&mut view, &with("need", whole_word()));

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
        let search = with(
            "item",
            Options {
                wrap: false,
                ..Options::default()
            },
        );
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(row_text(&screen).as_deref(), Some("item 1"));
        let scroll = screen.view().scroll();

        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::NoMore
        );
        assert_eq!(row_text(&screen).as_deref(), Some("item 1"));
        assert_eq!(screen.view().scroll(), scroll);
        // The same end with a line above the oldest match, so the step does not fall off the
        // history's edge and only the match's place tells the wrap apart.
        let mut above = vec!["header"];
        above.extend_from_slice(&lines);
        let mut screen = screen_with(4, &above);
        for _ in 0..2 {
            assert_eq!(
                screen.find(&search, Toward::Older, KeepClear::NONE),
                Found::Match
            );
        }
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::NoMore
        );
        assert_eq!(row_text(&screen).as_deref(), Some("item 1"));
        // And walking newer from the newest match.
        assert_eq!(
            screen.find(&search, Toward::Newer, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(
            screen.find(&search, Toward::Newer, KeepClear::NONE),
            Found::NoMore
        );
        assert_eq!(row_text(&screen).as_deref(), Some("item 2"));

        let mut screen = screen_with(4, &lines);
        let search = literal("item");
        for _ in 0..2 {
            assert_eq!(
                screen.find(&search, Toward::Older, KeepClear::NONE),
                Found::Match
            );
        }
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(
            row_text(&screen).as_deref(),
            Some("item 2"),
            "with wrapping on, the walk did not carry on from the other end"
        );
    }

    #[test]
    fn whole_word_asks_only_an_end_that_is_a_word_character_to_stand_apart() {
        for (line, query) in [
            ("foo(bar)", "foo("),
            ("a /path/x", "/path/"),
            ("say hi", " hi"),
        ] {
            let mut screen = screen_with(4, &[line]);
            assert_eq!(
                screen.find(&with(query, whole_word()), Toward::Older, KeepClear::NONE),
                Found::Match,
                "«{query}» in «{line}» ends on a boundary of its own"
            );
        }
        let mut screen = screen_with(4, &["xfoo(bar)"]);
        assert_eq!(
            screen.find(&with("foo(", whole_word()), Toward::Older, KeepClear::NONE),
            Found::Nothing,
            "the word end of «foo(» touches «x»"
        );
    }

    #[test]
    fn without_wrap_only_a_match_that_counts_is_more_matches() {
        // Whole word rejects both candidates; with wrapping off, the one past the end must not
        // be reported as «no more matches» when nothing anywhere is a whole word.
        let lines: Vec<String> = std::iter::once("kneed".to_owned())
            .chain((0..12).map(|n| format!("filler {n}")))
            .chain(std::iter::once("kneed".to_owned()))
            .chain((0..12).map(|n| format!("filler {n}")))
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut screen = screen_with(4, &lines);
        screen.scroll_by(Scroll::Lines(14));
        let search = with(
            "need",
            Options {
                whole_word: true,
                wrap: false,
                ..Options::default()
            },
        );
        for toward in [Toward::Older, Toward::Newer] {
            assert_eq!(
                screen.find(&search, toward, KeepClear::NONE),
                Found::Nothing
            );
        }
    }

    #[test]
    fn a_step_keeps_its_match_below_the_rows_it_is_asked_to_keep_clear() {
        let lines: Vec<String> = (0..40)
            .map(|n| {
                if n % 10 == 3 {
                    "needle".to_owned()
                } else {
                    format!("line {n}")
                }
            })
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let row = |screen: &Screen| selected_row(screen).map(|(row, _)| row);

        let mut screen = screen_with(8, &lines);
        let search = literal("needle");
        let top = KeepClear::Top(3);
        // The newest match is on the live screen: the view stays where the program writes, and
        // the box is the one that moves.
        assert_eq!(screen.find(&search, Toward::Older, top), Found::Match);
        assert_eq!(
            screen.view().scroll(),
            0,
            "a match on the live screen scrolled the view"
        );
        for _ in 0..2 {
            assert_eq!(screen.find(&search, Toward::Older, top), Found::Match);
            let on = row(&screen).expect("the match is on screen");
            assert!(on >= 3, "the match was left under the box, on row {on}");
        }
        // The oldest match cannot be brought further down: the history ends above it.
        assert_eq!(screen.find(&search, Toward::Older, top), Found::Match);
        let oldest = screen.view().scroll();
        screen.scroll_by(Scroll::ToTop);
        assert_eq!(
            screen.view().scroll(),
            oldest,
            "the premise: the oldest line is in view"
        );
        // Without rows to keep clear, the step leaves the match where the engine put it.
        let mut screen = screen_with(8, &lines);
        assert_eq!(
            screen.find(&literal("needle"), Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(
            screen.find(&literal("needle"), Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(row(&screen), Some(0));
    }

    #[test]
    fn with_the_box_at_the_bottom_a_step_keeps_its_match_above_it() {
        let lines: Vec<String> = (0..40)
            .map(|n| {
                if n % 10 == 3 {
                    "needle".to_owned()
                } else {
                    format!("line {n}")
                }
            })
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut screen = screen_with(8, &lines);
        screen.scroll_by(Scroll::ToTop);
        let search = literal("needle");
        let bottom = KeepClear::Bottom(3);
        // Walking newer from the top, the engine lands each match on the last row.
        for _ in 0..3 {
            assert_eq!(screen.find(&search, Toward::Newer, bottom), Found::Match);
            let (on, _) = selected_row(&screen).expect("the match is on screen");
            assert!(
                on <= 4,
                "the match was left under the box at the bottom, on row {on}"
            );
        }
    }

    fn regex() -> Options {
        Options {
            regex: true,
            ..Options::default()
        }
    }

    #[test]
    fn with_regular_expression_on_the_query_is_a_pattern() {
        // Walking older from the bottom, the pattern meets «abc» first; as text it would be «a.c».
        let mut screen = screen_with(4, &["a.c abc"]);
        assert_eq!(
            screen.find(&with("a.c", regex()), Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(screen.selected_text().as_deref(), Some("abc"));
        assert_eq!(
            screen.find(&with("a.c", regex()), Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(screen.selected_text().as_deref(), Some("a.c"));
    }

    #[test]
    fn a_pattern_that_does_not_parse_is_said_and_never_searched() {
        let Err(Refused::Invalid(why)) = Search::new("(", regex()) else {
            panic!("an unclosed group was not refused as invalid");
        };
        assert!(why.contains("unclosed") && !why.contains('\n'), "{why:?}");
        assert!(
            Search::new("(", Options::default()).is_ok(),
            "as text, «(» is just a character"
        );
    }

    #[test]
    fn a_pattern_too_large_to_build_is_refused_as_such() {
        // Every `\w` is every word character of Unicode, so fifty of them pass the limit.
        assert_eq!(
            Search::new(r"\w{50}", regex()).err(),
            Some(Refused::TooLarge)
        );
        assert!(Search::new(r"\w{5}", regex()).is_ok());
    }

    #[test]
    fn a_pattern_the_terminal_engine_gave_up_on_is_found() {
        use alacritty_terminal::grid::Dimensions;
        use alacritty_terminal::index::{Column, Direction, Point};
        use alacritty_terminal::term::search::{RegexIter, RegexSearch};

        const HOSTILE: &str = r"\p{L}.{170}~";
        let mut screen = Screen::new(Size {
            rows: 10,
            cols: 180,
        });
        // **Text that varies is what makes the automaton grow**: over a repeated line its states
        // repeat too and the old engine never gave up. Lines of 170 characters cannot hold a match.
        let alphabet: Vec<char> = "ab cd1 ü日~".chars().collect();
        let mut seed: u64 = 7;
        for _ in 0..400 {
            let line: String = (0..170)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    alphabet[((seed >> 33) % alphabet.len() as u64) as usize]
                })
                .collect();
            screen.advance(format!("{line}\r\n").as_bytes());
        }
        let needle = format!("x{}~", "y".repeat(170));
        screen.advance(format!("{needle}\r\n").as_bytes());
        for n in 0..5 {
            screen.advance(format!("tail {n}\r\n").as_bytes());
        }

        // The premise: the terminal engine's own search, over the same grid, reports nothing.
        let term = screen.term();
        let mut old = RegexSearch::new(HOSTILE).expect("the old engine builds it");
        let start = Point::new(term.grid().topmost_line(), Column(0));
        let end = Point::new(term.grid().bottommost_line(), term.grid().last_column());
        assert!(
            RegexIter::new(start, end, Direction::Right, term, &mut old)
                .next()
                .is_none(),
            "the premise: the terminal engine gives up on this pattern over this history"
        );

        for toward in [Toward::Older, Toward::Newer] {
            screen.clear_selection();
            assert_eq!(
                screen.find(&with(HOSTILE, regex()), toward, KeepClear::NONE),
                Found::Match
            );
            assert_eq!(screen.selected_text().as_deref(), Some(needle.as_str()));
        }
    }

    #[test]
    fn a_pattern_over_a_wrap_and_wide_characters_selects_and_marks_their_cells() {
        // Five columns: «本» has no room in the last one, so the row ends in a spacer of its own.
        let mut screen = Screen::new(Size { rows: 3, cols: 5 });
        screen.advance("ab日本cd".as_bytes());

        let search = with("b.本c", regex());
        let mut view = screen.view();
        screen.mark_matches(&mut view, &search);
        assert_eq!(marked(&view)[0], vec![1, 2, 3, 4]);
        assert_eq!(marked(&view)[1], vec![0, 1, 2]);

        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(screen.selected_text().as_deref(), Some("b日本c"));
    }

    #[test]
    fn whole_word_and_wrap_hold_for_patterns_too() {
        let mut screen = screen_with(4, &["needle need"]);
        let search = with(
            "ne+d",
            Options {
                whole_word: true,
                ..regex()
            },
        );
        for toward in [Toward::Older, Toward::Newer] {
            assert_eq!(screen.find(&search, toward, KeepClear::NONE), Found::Match);
            assert_eq!(
                selected_row(&screen).map(|(_, text)| text).as_deref(),
                Some("needle need")
            );
            assert_eq!(screen.selected_text().as_deref(), Some("need"));
        }

        let mut screen = screen_with(4, &["item 1", "item 2"]);
        let search = with(
            r"item \d",
            Options {
                wrap: false,
                ..regex()
            },
        );
        for expected in [Found::Match, Found::Match, Found::NoMore] {
            assert_eq!(
                screen.find(&search, Toward::Older, KeepClear::NONE),
                expected
            );
        }
        assert_eq!(screen.selected_text().as_deref(), Some("item 1"));
    }

    #[test]
    fn a_pattern_that_only_matches_nothing_finds_and_marks_nothing() {
        let mut screen = screen_with(4, &["abc"]);
        for pattern in ["x*", "^", r"\b"] {
            let search = with(pattern, regex());
            let mut view = screen.view();
            assert_eq!(screen.mark_matches(&mut view, &search), 0, "{pattern}");
            assert_eq!(
                screen.find(&search, Toward::Older, KeepClear::NONE),
                Found::Nothing,
                "{pattern}"
            );
        }
    }

    #[test]
    fn a_pattern_s_own_case_flag_wins_over_the_box() {
        let mut screen = screen_with(4, &["needle"]);
        assert_eq!(
            screen.find(&with("NEEDLE", regex()), Toward::Older, KeepClear::NONE),
            Found::Match
        );
        screen.clear_selection();
        assert_eq!(
            screen.find(
                &with("(?-i)NEEDLE", regex()),
                Toward::Older,
                KeepClear::NONE
            ),
            Found::Nothing
        );
        let exact = Options {
            match_case: true,
            ..regex()
        };
        assert_eq!(
            screen.find(&with("(?i)NEEDLE", exact), Toward::Older, KeepClear::NONE),
            Found::Match
        );
    }

    #[test]
    fn a_match_never_crosses_a_line_the_program_ended() {
        // The first line fills its row, so no padding stands between «foo» and «bar»: only the
        // break does.
        let full = format!("{}foo", "x".repeat(17));
        let mut screen = screen_with(4, &[full.as_str(), "bar"]);
        for search in [literal("foobar"), with(r"foo\s*bar", regex())] {
            let mut view = screen.view();
            assert_eq!(screen.mark_matches(&mut view, &search), 0);
            assert_eq!(
                screen.find(&search, Toward::Older, KeepClear::NONE),
                Found::Nothing
            );
        }
        // The control: the same rows joined by a wrap are one line.
        let mut joined = Screen::new(Size { rows: 4, cols: 20 });
        joined.advance(format!("{full}bar").as_bytes());
        assert_eq!(
            joined.find(&literal("foobar"), Toward::Older, KeepClear::NONE),
            Found::Match
        );
    }

    /// The columns of the visible row the selection sits on.
    fn selected_columns(screen: &Screen) -> Vec<usize> {
        let view = screen.view();
        let cols = view.size().cols as usize;
        view.cells()
            .chunks_exact(cols)
            .find(|row| row.iter().any(|cell| cell.selected))
            .map(|row| (0..cols).filter(|&col| row[col].selected).collect())
            .unwrap_or_default()
    }

    #[test]
    fn on_one_line_each_step_takes_the_nearest_match_that_way() {
        let mut screen = screen_with(4, &["x", "item item", "y"]);
        screen.scroll_by(Scroll::ToTop);
        let search = literal("item");
        for expected in [vec![0, 1, 2, 3], vec![5, 6, 7, 8]] {
            assert_eq!(
                screen.find(&search, Toward::Newer, KeepClear::NONE),
                Found::Match
            );
            assert_eq!(selected_columns(&screen), expected);
        }

        // Adjacent matches: the step starts one cell past the current one, not two.
        let mut screen = screen_with(4, &["aa"]);
        let search = literal("a");
        for (toward, expected) in [
            (Toward::Older, vec![1]),
            (Toward::Older, vec![0]),
            (Toward::Newer, vec![1]),
        ] {
            assert_eq!(screen.find(&search, toward, KeepClear::NONE), Found::Match);
            assert_eq!(selected_columns(&screen), expected, "{toward:?}");
        }
    }

    #[test]
    fn without_wrap_a_match_behind_the_start_on_the_same_line_is_no_more() {
        let mut screen = screen_with(4, &["a a"]);
        screen.scroll_by(Scroll::ToTop);
        let search = with(
            "a",
            Options {
                wrap: false,
                ..Options::default()
            },
        );
        for expected in [Found::Match, Found::Match, Found::NoMore] {
            assert_eq!(
                screen.find(&search, Toward::Newer, KeepClear::NONE),
                expected
            );
        }
        assert_eq!(selected_columns(&screen), vec![2]);
    }

    #[test]
    fn a_whole_word_overlapping_a_rejected_candidate_is_still_found() {
        for (line, query, options, expected) in [
            (
                "xa b",
                "a b|b",
                Options {
                    whole_word: true,
                    ..regex()
                },
                "b",
            ),
            ("v1.1.1", "1.1", whole_word(), "1.1"),
        ] {
            for toward in [Toward::Older, Toward::Newer] {
                let mut screen = screen_with(4, &[line]);
                assert_eq!(
                    screen.find(&with(query, options), toward, KeepClear::NONE),
                    Found::Match,
                    "«{query}» in «{line}» walking {toward:?}"
                );
                assert_eq!(screen.selected_text().as_deref(), Some(expected));
            }
        }
    }

    #[test]
    fn a_long_run_of_rejected_candidates_is_walked_once() {
        // Forty thousand digits after an «x»: every candidate inside the run is rejected. Going on
        // one character per rejection read the rest of the run each time — some eight hundred
        // million steps, a test that never ends — where skipping the run reads it once.
        let mut screen = Screen::new(Size { rows: 3, cols: 200 });
        screen.advance(format!("x{} 7", "1".repeat(40_000)).as_bytes());
        let search = with(
            r"\d+",
            Options {
                whole_word: true,
                ..regex()
            },
        );
        let started = std::time::Instant::now();
        assert_eq!(
            screen.find(&search, Toward::Older, KeepClear::NONE),
            Found::Match
        );
        assert_eq!(screen.selected_text().as_deref(), Some("7"));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "the run was read once per rejected candidate: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn marking_judges_the_rows_it_cuts_by_what_lies_past_the_cut() {
        use alacritty_terminal::index::Line;

        // Five columns: «abcdk» wraps into «need», so the second row starts inside «kneed».
        let marked_in = |text: &str, search: &Search, first: i32, last: i32| {
            let mut screen = Screen::new(Size { rows: 3, cols: 5 });
            screen.advance(text.as_bytes());
            let mut cells = 0;
            walk::mark(screen.term(), search, Line(first), Line(last), |_| {
                cells += 1
            });
            cells
        };
        let need = with(
            "need",
            Options {
                whole_word: true,
                ..Options::default()
            },
        );
        assert_eq!(
            marked_in("abcdkneed", &need, 1, 1),
            0,
            "the cut read as a word's start"
        );
        assert_eq!(marked_in("abcd need", &need, 1, 1), 1, "the control");
        assert_eq!(
            marked_in("abcdkneed", &with("^need", regex()), 1, 1),
            0,
            "the cut read as the line's start"
        );
        assert_eq!(
            marked_in("abcdkneed", &with("k$", regex()), 0, 0),
            0,
            "the cut read as the line's end"
        );
    }

    #[test]
    fn marking_reaches_no_further_below_the_screen_than_above_it() {
        let rows = 4;
        let mut screen = Screen::new(Size { rows, cols: 20 });
        let filler = "y".repeat(20 * (crate::screen::MARKING_REACH + 50));
        screen.advance(format!("S{filler}E").as_bytes());
        screen.scroll_by(Scroll::ToTop);
        let mut view = screen.view();
        assert!(
            view.cells().iter().any(|cell| cell.c == 'S'),
            "the premise: the line's start is on screen"
        );

        assert_eq!(
            screen.mark_matches(&mut view, &with("S[^E]*E", regex())),
            0,
            "marking read the wrapped line beyond its reach below the screen"
        );
    }

    #[test]
    fn marking_reads_a_long_wrapped_line_only_within_its_reach() {
        // One logical line, twenty columns a row: «S» far above the screen, then filler, then
        // «E» on the screen. A pattern from one to the other only matches if marking read the
        // line past its reach.
        let rows = 4;
        let mut screen = Screen::new(Size { rows, cols: 20 });
        let filler = "y".repeat(20 * (crate::screen::MARKING_REACH + 50));
        screen.advance(format!("S{filler}E").as_bytes());
        let search = with("S[^E]*E", regex());
        let mut view = screen.view();

        assert_eq!(
            screen.mark_matches(&mut view, &search),
            0,
            "marking read the wrapped line beyond its reach"
        );
        // The control: the same line inside the reach is marked.
        let mut near = Screen::new(Size { rows, cols: 20 });
        near.advance(format!("S{}E", "y".repeat(20 * 3)).as_bytes());
        let mut view = near.view();
        assert!(near.mark_matches(&mut view, &search) > 0);
    }

    #[test]
    fn nothing_typed_is_not_a_search() {
        assert_eq!(
            Search::new("", Options::default()).err(),
            Some(Refused::Empty)
        );
    }
}
