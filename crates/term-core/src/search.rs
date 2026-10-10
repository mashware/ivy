//! Searching what a console still holds, its history included.
//!
//! **The engine already knows how to search its own grid**, wrapped lines and wide characters
//! included, so this module only decides what is searched for and in which direction. The search
//! itself and the rules for what a match is stay in `alacritty_terminal`, the way the rules for
//! what «the text» is stay there for copying (D22).

use alacritty_terminal::term::search::RegexSearch;

/// What a console is searched for, compiled once and kept for as long as the search is open.
///
/// **Opaque on purpose**: the engine's type does not leave this crate, for the same reason as
/// [`Scroll`](crate::Scroll). And **kept rather than rebuilt per step**, because the engine's lazy
/// automata learn as they walk the grid — throwing them away would pay the learning again on every
/// Enter.
pub struct Search {
    pub(crate) regex: RegexSearch,
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
    /// Searches for exactly these characters.
    ///
    /// **Literal, never a pattern**: what a person types into a terminal's search is text they saw,
    /// and a `.` or a `(` in it means that character. A pattern can also build an automaton large
    /// enough that the engine gives up mid-search and reports no match where there is one, which
    /// it only says in a log nobody reads — the reason and what was discarded are in **D61**.
    ///
    /// **Case is the engine's rule**: insensitive unless the query holds an uppercase letter.
    /// Escaping adds no letters, so the rule sees exactly what was typed.
    pub fn literal(text: &str) -> Result<Self, Refused> {
        if text.is_empty() {
            return Err(Refused::Empty);
        }
        RegexSearch::new(&regex_syntax::escape(text))
            .map(|regex| Self { regex })
            .map_err(|_| Refused::TooLarge)
    }
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
        Search::literal(text).expect("an ordinary literal builds")
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

        assert!(screen.find(&mut literal("needle"), Toward::Older));

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

        assert!(screen.find(&mut literal("a.c"), Toward::Older));

        assert_eq!(screen.selected_text().as_deref(), Some("a.c"));
    }

    #[test]
    fn case_is_ignored_unless_the_query_holds_an_uppercase_letter() {
        let mut screen = screen_with(4, &["Needle"]);
        assert!(
            screen.find(&mut literal("needle"), Toward::Older),
            "a lowercase query did not find the capitalised word"
        );
        assert_eq!(screen.selected_text().as_deref(), Some("Needle"));

        // The newer line is the lowercase one, so a search that ignored case would stop there first.
        let mut screen = screen_with(4, &["Needle", "needle"]);
        assert!(screen.find(&mut literal("Needle"), Toward::Older));
        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("Needle"),
            "a query with an uppercase letter matched another case"
        );
    }

    #[test]
    fn each_step_moves_on_from_the_current_match_and_back() {
        let mut screen = screen_with(6, &["item 1", "item 2", "item 3"]);
        let mut search = literal("item");
        let row_text = |screen: &Screen| selected_row(screen).map(|(_, text)| text);

        assert!(screen.find(&mut search, Toward::Older));
        assert_eq!(row_text(&screen).as_deref(), Some("item 3"));
        assert!(screen.find(&mut search, Toward::Older));
        assert_eq!(
            row_text(&screen).as_deref(),
            Some("item 2"),
            "the second press found the match it started on"
        );
        assert!(screen.find(&mut search, Toward::Newer));
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
        assert!(screen.find(&mut search, Toward::Older));
        assert_eq!(
            selected_row(&screen).map(|(_, text)| text).as_deref(),
            Some("item 3")
        );

        for n in 0..5 {
            screen.advance(format!("noise {n}\r\n").as_bytes());
        }
        assert!(screen.find(&mut search, Toward::Older));

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
        assert!(screen.find(&mut literal("item"), Toward::Older));
        let scroll = screen.view().scroll();
        assert!(scroll > 0, "the premise: the view moved to the match");

        assert!(!screen.find(&mut literal("absent"), Toward::Older));

        assert_eq!(screen.selected_text().as_deref(), Some("item"));
        assert_eq!(screen.view().scroll(), scroll);
    }

    #[test]
    fn a_match_pushed_out_of_the_history_is_no_longer_found() {
        let mut screen = screen_with(4, &["needle"]);
        assert!(
            screen.find(&mut literal("needle"), Toward::Older),
            "the premise: it is found while kept"
        );
        screen.clear_selection();

        for n in 0..SCROLLBACK + 10 {
            screen.advance(format!("filler {n}\r\n").as_bytes());
        }

        assert!(!screen.find(&mut literal("needle"), Toward::Older));
        assert_eq!(screen.selected_text(), None);
    }

    #[test]
    fn a_match_brought_up_from_below_is_kept_off_the_last_row() {
        let lines: Vec<String> = (0..10)
            .map(|n| format!("above {n}"))
            .chain(std::iter::once("needle".to_owned()))
            .chain((0..10).map(|n| format!("below {n}")))
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut screen = screen_with(4, &lines);
        screen.scroll_by(Scroll::ToTop);

        assert!(screen.find(&mut literal("needle"), Toward::Newer));

        let (row, text) = selected_row(&screen).expect("the match is on screen");
        assert_eq!(text, "needle");
        assert!(
            row < 3,
            "the match landed on the last row, under the search's own line"
        );
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

    #[test]
    fn nothing_typed_is_not_a_search() {
        assert_eq!(Search::literal("").err(), Some(Refused::Empty));
    }
}
