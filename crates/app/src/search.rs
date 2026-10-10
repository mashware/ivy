//! Searching a console's scrollback: what is typed into the search, what each key does to it, and
//! the row it is painted on. **D61**.
//!
//! **A module of its own and not more of `keyboard.rs`** because the keyboard's walk already
//! decides, event by event, whether a key is the application's or the program's — and a search is a
//! third owner for some keys while it is open. Its rules live here and the walk asks them one
//! question per event, so `impl Pane` (over its ceiling, D28) grows by the asking and not by the
//! rules.

use eframe::egui::{self, Key, Painter, Rect, pos2, vec2};
use term_core::{Refused, Screen, Search, Toward, View};

use crate::painter::CellMetrics;
use crate::theme::Theme;

/// What one console keeps about searching it: the search open on it, if any, and what its presses
/// cost since the last time it was painted.
///
/// **It belongs to the console**, like a held paste: switching focus or tab leaves a search standing
/// on the console it was opened on, and closing that console drops it.
///
/// **The presses are kept here and not on the search** because a search can close in the same batch
/// as the press that cost something — `Enter` and then `Escape` — and the bench still has to publish
/// that press.
#[derive(Default)]
pub(crate) struct OnThisConsole {
    pub(crate) open: Option<Mode>,
    pressed: Presses,
    /// What marking cost on the painting in progress, kept between marking and handing it over so
    /// the painter cannot hand the window another console's.
    marking: Option<Marking>,
}

/// What marking one painting's matches cost, and how many cells it marked.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Marking {
    pub(crate) spent: std::time::Duration,
    pub(crate) cells: usize,
}

/// What a console's presses of `Enter` cost, for the bench (D13). Summed until the painting hands
/// them to the window.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Presses {
    pub(crate) count: usize,
    pub(crate) found: usize,
    pub(crate) total: std::time::Duration,
    pub(crate) slowest: std::time::Duration,
}

impl Presses {
    fn one(&mut self, spent: std::time::Duration, found: bool) {
        self.count += 1;
        self.found += usize::from(found);
        self.total += spent;
        self.slowest = self.slowest.max(spent);
    }
}

/// What searching a console cost on one painting, handed from the painter to the window.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Cost {
    /// The presses since the last painting. **Published, never subtracted here**: they ran in the
    /// keyboard's pass, which the window times whole and subtracts there.
    pub(crate) presses: Presses,
    /// What marking the matches cost on this painting, and nothing when no search was open.
    ///
    /// **An `Option` and not a zero**, for the strips' reason: a marking faster than the clock can
    /// see is still a painting that had one, and the script counts them.
    pub(crate) marking: Option<Marking>,
}

impl Cost {
    /// Hands this painting's search costs to the window: the marking is subtracted from the frame
    /// and published, the presses only published — see [`Self::presses`].
    pub(crate) fn hand_over(
        self,
        to: &mut crate::measure::Searching,
        timed_separately: &mut std::time::Duration,
    ) {
        to.presses += self.presses.count;
        to.found += self.presses.found;
        to.pressing += self.presses.total;
        to.slowest = to.slowest.max(self.presses.slowest);
        if let Some(marking) = self.marking {
            *timed_separately += marking.spent;
            to.paintings += 1;
            to.marking += marking.spent;
            // **Counted only where a match was on screen**: a painting with a search open and
            // nothing marked is the frame after the chord, or a query that matches nothing — and
            // counting it would let the bench's «it marked» guard pass over a marking that never
            // marked a cell.
            to.markings += usize::from(marking.cells > 0);
        }
    }
}

impl OnThisConsole {
    pub(crate) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Marks the open search's matches in the snapshot about to be painted, if a search is open,
    /// and says what that cost so the painter can keep it out of its own span.
    pub(crate) fn mark(&mut self, screen: &Screen, view: &mut View) -> Option<std::time::Duration> {
        self.marking = self.open.as_mut().map(|open| open.mark(screen, view));
        self.marking.map(|marking| marking.spent)
    }

    /// What this painting hands the window: the presses since the last one, and the marking.
    pub(crate) fn cost(&mut self) -> Cost {
        Cost {
            presses: std::mem::take(&mut self.pressed),
            marking: self.marking.take(),
        }
    }
}

/// A search that is open on one console.
#[derive(Default)]
pub(crate) struct Mode {
    query: String,
    /// The query compiled, **kept between presses** so the engine's automata keep what they learnt
    /// about this grid. Emptied whenever the query changes.
    compiled: Option<Result<Search, Refused>>,
    outcome: Outcome,
}

/// What the row says about the last press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Outcome {
    /// Nothing pressed since the query last changed.
    #[default]
    Typing,
    Found,
    NotFound,
    Refused(Refused),
}

/// One thing a key asked of the search, in the order the keys arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// The search chord: opens it, or closes it if it was open.
    Toggle,
    Type(String),
    Backspace,
    /// `Ctrl+W` and `Ctrl+Backspace`: what they do on a shell's line.
    DeleteWord,
    /// `Ctrl+U`, the same.
    Clear,
    Go(Toward),
    Close,
}

/// What the keyboard's walk records about the search during one pass.
///
/// **It tracks whether the search is open event by event**, because the batch itself opens and
/// closes it: with the chord and a letter in the same batch, the letter belongs to the search.
pub(crate) struct Steps {
    open: bool,
    asked: Vec<Step>,
}

impl Steps {
    pub(crate) fn new(open: bool) -> Self {
        Self {
            open,
            asked: Vec::new(),
        }
    }

    pub(crate) fn toggle(&mut self) {
        self.open = !self.open;
        self.asked.push(Step::Toggle);
    }

    /// Takes a paste into the query while the search is open, which is what a paste means there.
    /// Returns whether it took it.
    pub(crate) fn took_a_paste(&mut self, text: &str) -> bool {
        if self.open {
            self.asked.push(Step::Type(text.to_owned()));
        }
        self.open
    }

    /// Takes the event if it is one of the search's own keys while the search is open, and returns
    /// whether it did. **Asked after the application's chords**, so a chord without `Ctrl` has
    /// already swallowed the letter it would also have typed.
    pub(crate) fn took(&mut self, event: &egui::Event, chord: egui::Modifiers) -> bool {
        let Some(step) = self.open.then(|| step_for(event, chord)).flatten() else {
            return false;
        };
        self.open = step != Step::Close;
        self.asked.push(step);
        true
    }

    /// The program is getting bytes from this pass, so **the search closes**. A key that was not
    /// the search's was meant for the program, and leaving the search open would leave the person
    /// typing into a console they believe is searching.
    pub(crate) fn the_program_got_bytes(&mut self) {
        if std::mem::take(&mut self.open) {
            self.asked.push(Step::Close);
        }
    }
}

/// Which of the search's own keys this event is, if any.
///
/// **The search's gestures and the editing a shell's line already taught everybody** — and nothing
/// else, so the search never deafens a console: every key not listed here is the program's, and
/// reaching the program closes the search. Modifiers are not looked at beyond the ones named: a
/// `Ctrl+Enter` is still a step older, an `Alt+Backspace` still deletes a character.
fn step_for(event: &egui::Event, chord: egui::Modifiers) -> Option<Step> {
    match event {
        egui::Event::Text(text) => Some(Step::Type(text.clone())),
        egui::Event::Key {
            key, pressed: true, ..
        } => match key {
            Key::Escape => Some(Step::Close),
            Key::Enter if chord.shift => Some(Step::Go(Toward::Newer)),
            Key::Enter => Some(Step::Go(Toward::Older)),
            Key::Backspace if chord.ctrl => Some(Step::DeleteWord),
            Key::Backspace => Some(Step::Backspace),
            Key::W if chord.ctrl => Some(Step::DeleteWord),
            Key::U if chord.ctrl => Some(Step::Clear),
            _ => None,
        },
        _ => None,
    }
}

impl Mode {
    /// Marks this search's matches in the snapshot about to be painted, and says what that walk
    /// cost — **so whoever paints can keep it out of its own span** and the bench can publish it
    /// under its own name (D13).
    ///
    /// **It marks while the query is being typed**, compiling it here the first time it is needed:
    /// seeing every match light up as the query narrows is what tells a person whether `Enter` is
    /// worth pressing. A query the engine refuses marks nothing. **Compiling is inside the span**,
    /// and the span is named for the search and not for the walk alone: it is the search's work
    /// either way, and what matters for D13 is that it is declared.
    pub(crate) fn mark(&mut self, screen: &Screen, view: &mut View) -> Marking {
        let before = std::time::Instant::now();
        let query = &self.query;
        let cells = match self.compiled.get_or_insert_with(|| Search::literal(query)) {
            Ok(search) => screen.mark_matches(view, search),
            Err(_) => 0,
        };
        Marking {
            spent: before.elapsed(),
            cells,
        }
    }
}

/// Applies what the keys of one pass asked, in order.
pub(crate) fn apply(console: &mut OnThisConsole, screen: &mut Screen, steps: Steps) {
    let OnThisConsole {
        open: mode,
        pressed,
        ..
    } = console;
    for step in steps.asked {
        if matches!(step, Step::Toggle) {
            *mode = match mode.take() {
                Some(_) => None,
                None => Some(Mode::default()),
            };
            continue;
        }
        if matches!(step, Step::Close) {
            *mode = None;
            continue;
        }
        let Some(open) = mode.as_mut() else {
            continue;
        };
        match step {
            // **Control characters never reach the query.** The ones a paste ends in — the newline
            // of a copied line above all — are dropped: the engine never matches across a hard line
            // break, so a pasted line would never be found. One inside the text becomes a space,
            // so `foo\nbar` does not search for a word nobody wrote.
            Step::Type(text) => open.query.extend(
                text.trim_end_matches(char::is_control)
                    .chars()
                    .map(|c| if c.is_control() { ' ' } else { c }),
            ),
            Step::Backspace => {
                open.query.pop();
            }
            Step::DeleteWord => {
                let kept = open
                    .query
                    .trim_end()
                    .trim_end_matches(|c: char| !c.is_whitespace());
                open.query.truncate(kept.len());
            }
            Step::Clear => open.query.clear(),
            Step::Go(toward) => {
                let compiled = open
                    .compiled
                    .get_or_insert_with(|| Search::literal(&open.query));
                open.outcome = match compiled {
                    Ok(search) => {
                        let before = std::time::Instant::now();
                        let found = screen.find(search, toward);
                        pressed.one(before.elapsed(), found);
                        if found {
                            Outcome::Found
                        } else {
                            Outcome::NotFound
                        }
                    }
                    Err(refused) => Outcome::Refused(*refused),
                };
                continue;
            }
            Step::Toggle | Step::Close => unreachable!("handled above"),
        }
        open.compiled = None;
        open.outcome = Outcome::Typing;
    }
}

/// What the row reads.
fn the_row(mode: &Mode) -> String {
    // A query the engine already refused while marking says so before anybody presses Enter: its
    // matches are not lit, and the row is the only place that can say why.
    let refused_while_typing = matches!(mode.compiled, Some(Err(Refused::TooLarge)));
    let status = match mode.outcome {
        Outcome::Typing if refused_while_typing => "too long to search  -  Esc: close",
        Outcome::Typing => "Enter: older  Shift+Enter: newer  Esc: close",
        Outcome::Found => "found  -  Enter: older  Shift+Enter: newer  Esc: close",
        Outcome::NotFound => "no match  -  Esc: close",
        Outcome::Refused(Refused::Empty) => "type what to look for  -  Esc: close",
        Outcome::Refused(Refused::TooLarge) => "too long to search  -  Esc: close",
    };
    format!("search: {}_    {status}", mode.query)
}

/// Where the row goes over a console.
///
/// **Over its last row, never cut out of the console.** Cutting a row would resize the program
/// every time the search opens and shift every pointer-to-cell mapping by a row (D61). The search
/// scrolls a match off that row when the view is not already at the bottom; when it is, and the
/// match the last press found is on the last line, **the row moves to the top** rather than cover
/// what it just found. Before any press has found a match, a selection made with the mouse does not
/// move it; after one, the row reads the selection, whoever made it, as the match.
pub(crate) fn the_row_over(mode: &Mode, view: &View, metrics: &CellMetrics, area: Rect) -> Rect {
    let size = view.size();
    let cols = size.cols as usize;
    let last = size.rows.saturating_sub(1) as usize;
    let match_on_the_last_row = mode.outcome == Outcome::Found
        && view.cells()[last * cols..].iter().any(|cell| cell.selected);
    let row = if match_on_the_last_row { 0 } else { last };
    Rect::from_min_size(
        pos2(area.left(), area.top() + row as f32 * metrics.height()),
        vec2(area.width(), metrics.height()),
    )
}

/// Paints the search's row over the console.
pub(crate) fn paint_the_row(
    painter: &Painter,
    mode: &Mode,
    view: &View,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
) {
    let row = the_row_over(mode, view, metrics, area);
    let clipped = painter.with_clip_rect(row.intersect(painter.clip_rect()));
    clipped.rect_filled(row, 0.0, theme.panel_background());
    let text = clipped.layout_no_wrap(the_row(mode), metrics.font(), theme.foreground());
    clipped.galley(row.left_top(), text, theme.foreground());
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_core::Size;

    fn steps(open: bool, asked: Vec<Step>) -> Steps {
        Steps { open, asked }
    }

    fn opened_with(screen: &mut Screen, asked: Vec<Step>) -> Option<Mode> {
        let mut console = OnThisConsole {
            open: Some(Mode::default()),
            ..OnThisConsole::default()
        };
        apply(&mut console, screen, steps(true, asked));
        console.open
    }

    fn a_screen(lines: &str) -> Screen {
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        screen.advance(lines.as_bytes());
        screen
    }

    #[test]
    fn the_query_is_edited_the_way_a_shell_line_is() {
        let mut screen = a_screen("");
        let query = |asked: Vec<Step>, screen: &mut Screen| {
            opened_with(screen, asked).expect("still open").query
        };

        assert_eq!(
            query(
                vec![Step::Type("needle x".into()), Step::DeleteWord],
                &mut screen
            ),
            "needle "
        );
        assert_eq!(
            query(
                vec![Step::Type("needle  ".into()), Step::DeleteWord],
                &mut screen
            ),
            ""
        );
        assert_eq!(
            query(
                vec![Step::Type("needle".into()), Step::Backspace],
                &mut screen
            ),
            "needl"
        );
        assert_eq!(
            query(vec![Step::Type("needle".into()), Step::Clear], &mut screen),
            ""
        );
    }

    #[test]
    fn the_row_says_what_the_last_press_found() {
        let mut screen = a_screen("needle\r\n");
        let said = |asked: Vec<Step>, screen: &mut Screen| {
            the_row(&opened_with(screen, asked).expect("still open"))
        };

        assert!(
            said(
                vec![Step::Type("absent".into()), Step::Go(Toward::Older)],
                &mut screen
            )
            .contains("no match")
        );
        assert!(said(vec![Step::Go(Toward::Older)], &mut screen).contains("type what to look for"));
        let found = said(
            vec![Step::Type("needle".into()), Step::Go(Toward::Older)],
            &mut screen,
        );
        assert!(
            found.starts_with("search: needle_") && found.contains("found  -"),
            "{found}"
        );
        // Changing the query takes back what the row said about the old one.
        let edited = said(
            vec![
                Step::Type("absent".into()),
                Step::Go(Toward::Older),
                Step::Backspace,
            ],
            &mut screen,
        );
        assert!(!edited.contains("no match"), "{edited}");
    }

    #[test]
    fn the_chord_toggles_and_escape_closes() {
        let mut screen = a_screen("");
        let mut console = OnThisConsole::default();
        apply(&mut console, &mut screen, steps(false, vec![Step::Toggle]));
        assert!(console.is_open());
        apply(&mut console, &mut screen, steps(true, vec![Step::Toggle]));
        assert!(!console.is_open());

        apply(
            &mut console,
            &mut screen,
            steps(
                false,
                vec![Step::Toggle, Step::Close, Step::Type("x".into())],
            ),
        );
        assert!(
            !console.is_open(),
            "a step after Escape reopened the search"
        );
    }

    #[test]
    fn every_press_is_counted_for_the_bench_even_when_the_search_closes_with_it() {
        let mut screen = a_screen("needle\r\n");
        let mut console = OnThisConsole::default();
        apply(
            &mut console,
            &mut screen,
            steps(
                false,
                vec![
                    Step::Toggle,
                    Step::Type("needle".into()),
                    Step::Go(Toward::Older),
                    Step::Type("x".into()),
                    Step::Go(Toward::Older),
                    Step::Close,
                ],
            ),
        );

        let cost = console.cost();
        assert_eq!((cost.presses.count, cost.presses.found), (2, 1));
        assert!(
            cost.presses.slowest <= cost.presses.total
                && cost.presses.slowest > std::time::Duration::ZERO
        );
        assert_eq!(
            console.cost().presses,
            Presses::default(),
            "the presses were handed over twice"
        );
    }

    #[test]
    fn the_marking_is_subtracted_and_the_presses_only_published() {
        let ms = std::time::Duration::from_millis;
        let mut to = crate::measure::Searching::default();
        let mut timed_separately = ms(1);
        let presses = Presses {
            count: 2,
            found: 1,
            total: ms(30),
            slowest: ms(20),
        };

        Cost {
            presses,
            marking: Some(Marking {
                spent: ms(5),
                cells: 6,
            }),
        }
        .hand_over(&mut to, &mut timed_separately);
        Cost {
            presses: Presses::default(),
            marking: Some(Marking {
                spent: ms(3),
                cells: 0,
            }),
        }
        .hand_over(&mut to, &mut timed_separately);

        assert_eq!(
            timed_separately,
            ms(9),
            "only the two markings may be subtracted: the presses ran in the keyboard's pass, which is \
             subtracted whole"
        );
        assert_eq!(
            (to.presses, to.found, to.pressing, to.slowest),
            (2, 1, ms(30), ms(20))
        );
        assert_eq!(
            (to.paintings, to.markings, to.marking),
            (2, 1, ms(8)),
            "a painting that marked nothing was counted as one that marked a match"
        );
    }

    #[test]
    fn control_characters_never_reach_the_query() {
        let mut screen = a_screen("");
        let open = opened_with(&mut screen, vec![Step::Type("need\tle\n\u{1b}".into())]);
        assert_eq!(open.expect("still open").query, "need le");
    }

    #[test]
    fn the_row_moves_to_the_top_only_for_a_match_on_the_last_row() {
        let metrics = crate::painter::CellMetrics::of(10.0, 20.0);
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 80.0));
        // Four rows and the view at the bottom: the last row is the line under the cursor.
        let mut screen = a_screen("one\r\ntwo\r\nthree\r\nneedle");
        let mut console = OnThisConsole::default();
        apply(
            &mut console,
            &mut screen,
            steps(
                false,
                vec![
                    Step::Toggle,
                    Step::Type("needle".into()),
                    Step::Go(Toward::Older),
                ],
            ),
        );
        let mode = console.open.as_ref().expect("still open");
        assert_eq!(
            mode.outcome,
            Outcome::Found,
            "the premise: the match was found"
        );
        assert_eq!(
            the_row_over(mode, &screen.view(), &metrics, area).top(),
            0.0
        );

        // The same cells selected, under a search that has not found anything yet: the row stays
        // at the bottom.
        let typing = Mode::default();
        assert_eq!(
            the_row_over(&typing, &screen.view(), &metrics, area).top(),
            60.0
        );
    }

    #[test]
    fn only_the_search_s_own_keys_are_taken_and_only_while_it_is_open() {
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let none = egui::Modifiers::NONE;
        let shift = egui::Modifiers::SHIFT;
        let ctrl = egui::Modifiers::CTRL;

        assert_eq!(
            step_for(&key(Key::Enter), none),
            Some(Step::Go(Toward::Older))
        );
        assert_eq!(
            step_for(&key(Key::Enter), shift),
            Some(Step::Go(Toward::Newer))
        );
        assert_eq!(step_for(&key(Key::Escape), none), Some(Step::Close));
        assert_eq!(step_for(&key(Key::W), ctrl), Some(Step::DeleteWord));
        assert_eq!(step_for(&key(Key::U), ctrl), Some(Step::Clear));
        assert_eq!(step_for(&key(Key::Backspace), ctrl), Some(Step::DeleteWord));
        assert_eq!(step_for(&key(Key::Backspace), none), Some(Step::Backspace));
        assert_eq!(
            step_for(&key(Key::C), ctrl),
            None,
            "the interrupt is the program's"
        );
        assert_eq!(step_for(&key(Key::ArrowUp), none), None);

        let mut closed = Steps::new(false);
        assert!(!closed.took(&egui::Event::Text("x".into()), none));
        let mut open = Steps::new(true);
        assert!(open.took(&egui::Event::Text("x".into()), none));
        open.the_program_got_bytes();
        assert!(
            !open.took(&egui::Event::Text("y".into()), none),
            "the search stayed open after bytes went out"
        );
        assert_eq!(open.asked, vec![Step::Type("x".into()), Step::Close]);
    }
}
