//! Searching a console's scrollback: what is typed into the search, what each key and each press
//! on its box does to it, and the options it is matched with. **D61**. The box itself — where it
//! sits, how it is painted and what a press on it reaches — is [`floating`].
//!
//! **A module of its own and not more of `keyboard.rs`** because the keyboard's walk already
//! decides, event by event, whether a key is the application's or the program's — and a search is a
//! third owner for some keys while it is open. Its rules live here and the walk asks them one
//! question per event, so `impl Pane` (over its ceiling, D28) grows by the asking and not by the
//! rules.

use eframe::egui::{self, Key};
use term_core::{Found, Options, Refused, Screen, Search, Toward, View};

pub(crate) mod floating;

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
    /// How the query is matched. **Kept here and not on the open search**, so closing the box and
    /// opening it again finds the choices where the person left them. **Private, and changed only
    /// by [`Step::Flip`]**, which also drops the compiled query: a write anywhere else would leave
    /// the search matching with the old options.
    options: Options,
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

/// What a console's steps through its matches cost — `Enter`, `Shift+Enter` and the box's previous
/// and next — for the bench (D13). Summed until the painting hands
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
    /// keyboard's pass or, from the box's buttons, in the mouse pass, and the window times each of
    /// those whole and subtracts it there.
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
        let options = self.options;
        self.marking = self
            .open
            .as_mut()
            .map(|open| open.mark(screen, view, options));
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
    /// about this grid. Emptied whenever the query or an option changes.
    compiled: Option<Result<Search, Refused>>,
    outcome: Outcome,
    /// Whether the options' list is unfolded beside the box. Changed only by the steps.
    list: bool,
    /// Where the box was painted, and in which pass: **what a press is judged against**, never a
    /// layout worked out again by the pointer, which has no snapshot to place the box by.
    pub(crate) painted: Option<floating::Painted>,
}

/// One of the three choices the options' list offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    MatchCase,
    WholeWord,
    Wrap,
}

impl Choice {
    pub(crate) const ALL: [Self; 3] = [Self::MatchCase, Self::WholeWord, Self::Wrap];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::MatchCase => "Match case",
            Self::WholeWord => "Whole word only",
            Self::Wrap => "Wrap around",
        }
    }

    pub(crate) fn is_on(self, options: Options) -> bool {
        match self {
            Self::MatchCase => options.match_case,
            Self::WholeWord => options.whole_word,
            Self::Wrap => options.wrap,
        }
    }

    fn flip(self, options: &mut Options) {
        let field = match self {
            Self::MatchCase => &mut options.match_case,
            Self::WholeWord => &mut options.whole_word,
            Self::Wrap => &mut options.wrap,
        };
        *field = !*field;
    }
}

/// What the box says about the last press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Outcome {
    /// Nothing pressed since the query last changed.
    #[default]
    Typing,
    Found,
    NotFound,
    /// A match that counts lies only past the end the press walked to, and wrapping is off.
    NoMore,
    Refused(Refused),
}

/// One thing a key or a press on the box asked of the search, in the order they arrived. The first
/// seven come from keys and from the box's previous, next and close; [`Step::List`],
/// [`Step::FoldList`] and [`Step::Flip`] only from the pointer.
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
    /// The box's options button: unfolds the list, or folds it again.
    List,
    /// A press anywhere else while the list is unfolded folds it.
    FoldList,
    Flip(Choice),
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
    pub(crate) fn mark(&mut self, screen: &Screen, view: &mut View, options: Options) -> Marking {
        let before = std::time::Instant::now();
        let query = &self.query;
        let cells = match self
            .compiled
            .get_or_insert_with(|| Search::literal(query, options))
        {
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
        options,
        pressed,
        ..
    } = console;
    for step in steps.asked {
        // **An option belongs to the console**, so it flips whether or not the box is open.
        if let Step::Flip(choice) = step {
            choice.flip(options);
        }
        match step {
            Step::Toggle => {
                *mode = match mode.take() {
                    Some(_) => None,
                    None => Some(Mode::default()),
                };
                continue;
            }
            Step::Close => {
                *mode = None;
                continue;
            }
            _ => {}
        }
        let Some(open) = mode.as_mut() else {
            continue;
        };
        match step {
            Step::Toggle | Step::Close => {}
            Step::List => {
                open.list = !open.list;
                continue;
            }
            Step::FoldList => {
                open.list = false;
                continue;
            }
            // Flipped above; what follows is what any change to how the query matches does.
            Step::Flip(_) => {}
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
                // The rows the box takes at the top of the console, as it was last painted: the
                // step keeps the match below them wherever the view can scroll.
                let clear_rows = open
                    .painted
                    .as_ref()
                    .map_or(0, |painted| painted.layout.clear_rows);
                let compiled = open
                    .compiled
                    .get_or_insert_with(|| Search::literal(&open.query, *options));
                open.outcome = match compiled {
                    Ok(search) => {
                        let before = std::time::Instant::now();
                        let found = screen.find(search, toward, clear_rows);
                        pressed.one(before.elapsed(), found == Found::Match);
                        match found {
                            Found::Match => Outcome::Found,
                            Found::Nothing => Outcome::NotFound,
                            Found::NoMore => Outcome::NoMore,
                        }
                    }
                    Err(refused) => Outcome::Refused(*refused),
                };
                continue;
            }
        }
        open.compiled = None;
        open.outcome = Outcome::Typing;
    }
}

impl Mode {
    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn list(&self) -> bool {
        self.list
    }

    /// What the box says about the last press, beside the query: nothing while typing or after a
    /// match, and why there is nothing to show otherwise.
    pub(crate) fn status(&self) -> &'static str {
        // A query the engine already refused while marking says so before anybody presses Enter:
        // its matches are not lit, and the box is the only place that can say why.
        let refused_while_typing = matches!(self.compiled, Some(Err(Refused::TooLarge)));
        match self.outcome {
        Outcome::Typing if refused_while_typing => "too long to search",
        Outcome::Typing | Outcome::Found => "",
        Outcome::NotFound => "no match",
        Outcome::NoMore => "no more matches",
        Outcome::Refused(Refused::Empty) => "type what to look for",
        Outcome::Refused(Refused::TooLarge) => "too long to search",
        }
    }
}

impl OnThisConsole {
    pub(crate) fn options(&self) -> Options {
        self.options
    }
}

/// Opens the search on a console, or closes it if it was open: what the chord, the console's
/// header button and the application menu all do.
pub(crate) fn toggle_on(console: &mut OnThisConsole, screen: &mut Screen) {
    let mut steps = Steps::new(console.is_open());
    steps.toggle();
    apply(console, screen, steps);
}

/// Applies one press on the box. Previous, next and close are the steps their keys ask for, so a
/// click and a key can never do two different things; the options have no key.
pub(crate) fn press(console: &mut OnThisConsole, screen: &mut Screen, step: Step) {
    let open = console.is_open();
    apply(
        console,
        screen,
        Steps {
            open,
            asked: vec![step],
        },
    );
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
    fn the_box_says_what_the_last_press_found() {
        let mut screen = a_screen("needle\r\nneedle\r\n");
        let said = |asked: Vec<Step>, screen: &mut Screen| {
            opened_with(screen, asked).expect("still open").status()
        };

        assert_eq!(
            said(
                vec![Step::Type("absent".into()), Step::Go(Toward::Older)],
                &mut screen
            ),
            "no match"
        );
        assert_eq!(
            said(vec![Step::Go(Toward::Older)], &mut screen),
            "type what to look for"
        );
        assert_eq!(
            said(
                vec![Step::Type("needle".into()), Step::Go(Toward::Older)],
                &mut screen
            ),
            ""
        );
        // Changing the query takes back what the box said about the old one.
        assert_eq!(
            said(
                vec![
                    Step::Type("absent".into()),
                    Step::Go(Toward::Older),
                    Step::Backspace,
                ],
                &mut screen,
            ),
            ""
        );
    }

    #[test]
    fn without_wrap_the_box_says_there_are_no_more_matches() {
        let mut screen = a_screen("needle\r\n");
        let mut console = OnThisConsole::default();
        apply(
            &mut console,
            &mut screen,
            steps(
                false,
                vec![
                    Step::Toggle,
                    Step::Flip(Choice::Wrap),
                    Step::Type("needle".into()),
                    Step::Go(Toward::Older),
                    Step::Go(Toward::Older),
                ],
            ),
        );
        assert_eq!(
            console.open.as_ref().expect("still open").status(),
            "no more matches"
        );
    }

    #[test]
    fn an_option_changes_how_the_query_matches_and_takes_back_what_the_box_said() {
        let mut screen = a_screen("Needle\r\n");
        let mut console = OnThisConsole::default();
        apply(
            &mut console,
            &mut screen,
            steps(
                false,
                vec![
                    Step::Toggle,
                    Step::Type("needle".into()),
                    Step::Flip(Choice::MatchCase),
                    Step::Go(Toward::Older),
                ],
            ),
        );
        let open = console.open.as_ref().expect("still open");
        assert_eq!(
            open.status(),
            "no match",
            "match case did not reach the engine"
        );

        let mut view = screen.view();
        assert_eq!(
            console
                .open
                .as_mut()
                .unwrap()
                .mark(&screen, &mut view, console.options)
                .cells,
            0,
            "the marking ignored match case"
        );
        apply(
            &mut console,
            &mut screen,
            steps(true, vec![Step::Flip(Choice::MatchCase)]),
        );
        assert_eq!(
            console.open.as_ref().expect("still open").status(),
            "",
            "flipping an option left the old answer standing"
        );
        let mut view = screen.view();
        assert_eq!(
            console
                .open
                .as_mut()
                .unwrap()
                .mark(&screen, &mut view, console.options)
                .cells,
            6,
            "the marking kept the query compiled with the old option"
        );
    }

    #[test]
    fn the_options_belong_to_the_console_and_outlive_the_box() {
        let mut screen = a_screen("");
        let mut one = OnThisConsole::default();
        apply(
            &mut one,
            &mut screen,
            steps(
                false,
                vec![Step::Toggle, Step::Flip(Choice::WholeWord), Step::Close],
            ),
        );
        toggle_on(&mut one, &mut screen);
        assert!(one.is_open());
        assert!(one.options.whole_word, "closing the box forgot the option");
    }

    #[test]
    fn the_options_button_unfolds_the_list_and_a_press_elsewhere_folds_it() {
        let mut screen = a_screen("");
        let mut console = OnThisConsole::default();
        toggle_on(&mut console, &mut screen);
        press(&mut console, &mut screen, Step::List);
        assert!(console.open.as_ref().unwrap().list);
        press(&mut console, &mut screen, Step::Flip(Choice::Wrap));
        assert!(
            console.open.as_ref().unwrap().list,
            "choosing an option folded the list"
        );
        assert!(!console.options.wrap);
        press(&mut console, &mut screen, Step::List);
        assert!(!console.open.as_ref().unwrap().list);
        press(&mut console, &mut screen, Step::List);
        press(&mut console, &mut screen, Step::FoldList);
        assert!(!console.open.as_ref().unwrap().list);
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
