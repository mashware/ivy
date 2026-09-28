//! The status panel of a pane: what its producer publishes, painted to the right of its
//! console.
//!
//! **This module decides the meaning of nothing.** The vocabulary —the marks, the styles, the phase— is
//! the contract's and the producer's; here it is only translated into color and position. That line is
//! D6, and it is the reason the panel works for an agent, a deployment or a script without knowing
//! anything about any of them.
//!
//! **Two producer values also leave interactive geometry here.** What a producer declares under
//! `actions` is painted with its command underneath, and a press on it writes that command into the
//! pane's program. A `line.link` gets a compact label whose exact target is disclosed on hover; this
//! module records that raw target but does not decide whether it is safe to open. The vocabulary is
//! still the producer's — the application reads no meaning out of the text — but where a gesture goes
//! is a fact about its own panes, so it is the application's to decide. **D42, D51**.
//!
//! Those are the places where being wrong costs more than a badly drawn panel, and it is why their
//! properties are held by mechanism rather than by care: what is painted under a button label is what
//! gets sent, character for character; and a gesture only reaches an interactive region this pass drew
//! **whole**, on a panel it did not have to lay out again.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

mod buttons;
mod color;
mod document;
mod driving;
mod links;
pub(crate) mod pane;
use buttons::buttons_drawn;
pub(crate) use document::Reader as DocumentReader;
use driving::RetirementState;
pub(crate) use driving::{Holder, Released, Retirement};
use links::{LaidOutLink, PaintedLink};

use eframe::egui::{Align2, Color32, Painter, Pos2, Rect};
use panel_contract::{Action, Attention, Line, Mark, Panel, Problem, Style, is_shown_as_itself};

use crate::painter::CellMetrics;
use crate::theme::Theme;

/// How many font cells the panel of a pane is born with.
///
/// **In cells and not in points**: a width in points narrows in columns when the desktop zoom is raised,
/// and what the panel has to preserve is **how many characters fit**, which is what decides whether its
/// text can be read. With thirty-six, the lines of the repo's real `panel.json` files fit without the
/// console losing its usual eighty columns in a normal window.
///
/// **Factory default, not fixed**: since the panel's border can be dragged, this is only the value a
/// pane that nobody has touched starts with. What rules is each pane's own.
///
/// The whole reasoning —why in cells, why it is the panel that yields, that the width belongs to each
/// pane, what the gesture's minimum is and why folding is a state apart— is in **D10** of
/// `docs/02-decisiones.md`, with what was discarded in each case.
pub(crate) const DEFAULT_WIDTH: usize = 36;

/// The least left to the console when its panel takes room from it, in cells.
///
/// **The panel yields, not the console.** The panel is margin and the console is the work: in a narrow
/// window what is spare is the status, not the program. With twenty columns a console is still usable;
/// with five, it is not.
///
/// **And it is also the floor that decides whether a pane can be split in two**, which is the other place
/// where it is needed to know when a console stops being of use. They are the same question, so they are
/// the same constant: two numbers for «the least a console needs» would end up saying different things.
pub(crate) const MINIMUM_CONSOLE: f32 = 20.0;

/// The least left to the panel when its border is dragged, in cells.
///
/// **With one cell the panel is almost invisible, and that is what is wanted: that there always be
/// somewhere to grab.** Reaching zero, the border would go away with the panel and the gesture that closed
/// it could not be undone with the mouse — a trip with no return ticket.
///
/// It is the floor of the **gesture**, not of the pane: [`set_aside`] can still return a panel **narrower
/// than this** —down to zero width— when the pane does not give for more. They are two distinct
/// questions, what the user asks and what fits, and confusing them would leave the console below its
/// minimum. (What `set_aside` never returns is *any* panel once it is asked for a width: in the extreme
/// case it returns the empty rectangle, and its test says so.)
///
/// **And since folding became possible, this is exactly the line between narrowing and folding**: asking
/// for less than this is what the gesture reads as «close it». Before folding existed, this same number
/// served the opposite —preventing reaching zero, because the border would have gone away with the panel—
/// and that reason is no longer its own: now what holds the return is the handle that survives the fold.
pub(crate) const DRAG_FLOOR: usize = 1;

/// What is left as a margin to the left of the text, in cells.
///
/// One cell and not zero: flush against the border, the text reads as a painting error. On the right it
/// is not needed, because there is nothing there — the panel is the last thing in the pane.
const MARGIN: f32 = 1.0;

/// How a pane's rectangle ends up divided among what is shown.
///
/// It has a name, and is not a tuple, for the same reason [`crate::tiling::Tiling`] has one, which does the same
/// between the two consoles: two divisions in the same frame that read differently would be two
/// languages for a single idea.
#[derive(Clone, Copy)]
pub struct PanelTiling {
    /// What is left to the console, which is what its grid has to be measured against.
    pub console: Rect,
    /// Where the panel goes, and **the panel that goes there**. They go together because one does not
    /// exist without the other: separating them leaves a branch writable —a gap without a panel— that
    /// cannot happen.
    pub panel: Option<Rect>,
}

/// Splits a pane's rectangle between its console and its panel.
///
/// **The deduction goes here and not after measuring the grid**, which is what makes the console measure
/// the gap it really has left: measuring the whole rectangle first, the program inside would believe it
/// had columns the panel has taken from it and would write beneath it.
///
/// With no panel to paint, the console keeps everything and there is no second rectangle: it is what makes
/// a pane whose producer has published nothing look exactly like it did before the panel existed.
///
/// With a panel, the gap is what is left over after the console's floor, so in a narrow window the one
/// that narrows is the panel and it can come to disappear. Never the other way around.
///
/// **`panel_cells` carries both things in a single parameter, and it is not for brevity.** «Whether there
/// is something to show» and «how much width it takes» are the same decision made in the same place: with
/// two parameters the writable state «there is no panel, but with thirty-six cells» exists, and this
/// module already paid once for having **two parallel criteria** for whether the panel is shown — one in
/// the width tiling and another in the painting, which contradicted each other.
///
/// The name says the unit on purpose: here cells and points coexist, and a single name for the two leaves
/// the reader without knowing which one they are looking at.
pub fn set_aside(pane: Rect, cell_width: f32, panel_cells: Option<usize>) -> PanelTiling {
    let Some(cells) = panel_cells else {
        return PanelTiling {
            console: pane,
            panel: None,
        };
    };

    let in_points = cells as f32 * cell_width;
    let available = (pane.width() - MINIMUM_CONSOLE * cell_width).max(0.0);
    let cut = pane.right() - in_points.min(available);

    PanelTiling {
        console: Rect::from_min_max(pane.left_top(), Pos2::new(cut, pane.bottom())),
        panel: Some(Rect::from_min_max(
            Pos2::new(cut, pane.top()),
            pane.right_bottom(),
        )),
    }
}

/// What a pane claims right now.
///
/// **Two things and not one**: *what* it claims —which its producer says and the application does not
/// deduce— and whether there is someone **holding it**, that is whether what is shown is trustworthy or is
/// the last thing that arrived from a producer that no longer answers. The two fit in a single symbol,
/// which is what is needed where there is no room for a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttentionState {
    /// Which of the contract's three.
    pub class: Attention,
    /// Whether there is someone behind what this panel says.
    ///
    /// It comes from the same place as the warning painted below, and not from a second criterion of «this
    /// no longer holds»: with two, the day they disagree the panel would say the claim is trustworthy
    /// while its own line confesses it is not. It covers the two ways of being left with no one behind
    /// —the long silence and the file that stopped being read—, which on screen mean the same thing.
    ///
    /// **With one exception, and it is in the decision log**: the *finished* is not left with no one
    /// behind by falling silent, because falling silent is what whoever finished does.
    pub backed: bool,
}

/// Which of two claims wins when **only one** has to be shown — a tab's card has one symbol and its
/// panes can claim different things.
///
/// **The class rules, and at equal class the one someone holds wins.** With two panes waiting for you
/// and one of them alive, the card has to say there is someone really waiting: painting the dimmed one
/// would send you to the one that is dead.
///
/// **By hand and not derived**, for the same reason as the ordering of [`Attention`]: a derived `Ord`
/// orders by the order of the fields in the file, so whoever reordered them —or put one in the middle—
/// would change which one wins **silently**.
impl Ord for AttentionState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // **It is destructured so that a new field does not compile.** Comparing by `self.field` by hand,
        // the day this type gains a third field the ordering would ignore it silently and stop being
        // coherent with the derived `Eq` — which is exactly what the ordering of [`Attention`] avoids with
        // its exhaustive `match`, and the reason there is no `derive` here.
        let Self { class, backed } = self;
        class.cmp(&other.class).then(backed.cmp(&other.backed))
    }
}

impl PartialOrd for AttentionState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// What a pane knows about its producer's panel.
///
/// **It keeps the last good panel on purpose.** The producer clobbers the whole file on each publication
/// —so says its contract— so a read can land right in the middle and not find valid JSON. Without this the
/// panel would flicker to empty on every write, which is a much more visible failure than the one it
/// avoids. **And for that very reason it has to be confessed**: keeping the last good one without saying
/// so hides a broken producer behind a plausible summary from an hour ago.
#[derive(Default)]
pub struct PanelState {
    last_good: Option<Arc<Panel>>,
    /// The producer document a connection may recover without asking the window to run a pass.
    ///
    /// **A pointer behind a mutex, not panel state behind a lock.** Painting, holds and presses stay
    /// owned by the interface thread. A connection only clones this immutable pointer under the
    /// guard and pays serialization after releasing it.
    committed: document::Committed,
    /// What was laid out, and what it was laid out for.
    ///
    /// **It exists for cost, and the scenario is the present**: while a console dumps, the window repaints
    /// every frame, and wrapping the panel to the width would be recalculated sixty times per second
    /// without its file having changed.
    layout: Option<PanelLayout>,
    /// Why the last read failed, if it failed. It is what tells apart «there is no producer here» from
    /// «there is one and it is broken», which look the same and are not the same thing.
    problem: Option<Problem>,
    /// What the last painting **actually drew**, or nothing when it did not paint.
    ///
    /// **One value and not two fields, because a press consults both and they must not be able to
    /// disagree.** The first version kept the rectangles and «was the layout reused» apart, and the
    /// pair went stale together the moment the panel was **folded**: the window then skips painting
    /// it altogether, neither field was touched, and a click on the console — where a button used to
    /// be — ran its command with nothing on screen. Written once at the end of a painting, and taken
    /// away by [`PanelState::nothing_was_drawn`] wherever the decision not to paint is made.
    ///
    /// **Recorded by the painting rather than derived again by whoever routes a press**, and that is
    /// a decision rather than an optimisation. The layout is a cache whose key is the width and the
    /// warning, and re-reading the file throws it away — so a second walk in the pointer pass
    /// silently yields the *new* layout, with nothing left to compare a press against.
    drawn: Option<Drawn>,
    /// The connection driving this panel, when one is.
    ///
    /// **This is D17's answer to «who rules», and the whole of it.** The contract has two transports
    /// and the file cannot identify its producer, so authority cannot be given to it: while a
    /// connection holds a pane it is the only writer, the file is not read at all, and a second
    /// source is refused by name. There is no sequence number anywhere and none is needed — one
    /// connection's newline-framed requests cannot reorder themselves, and with the file unread
    /// there is nobody else to order against.
    ///
    /// **It dies with the connection**, which is not a detail: without a release, an agent that
    /// crashed would freeze this pane's panel for ever and its file would never be read again.
    holder: Option<Holder>,
    /// The last intentional retirement, until another document is successfully committed.
    ///
    /// **It is an idempotency record, not authority that survives a generation.** Only the same
    /// connection may receive its original result, and [`PanelState::commit`] removes it for both
    /// channel and file publications before anybody can retire the new document.
    retirement: RetirementState,
    /// The date of the last file this panel actually looked at.
    ///
    /// **It is the baseline a hold compares against to tell a real file publication from a raised
    /// flag.** The watcher raises its notice for the exact path, but a split, an open tab, a close or
    /// the blind rescan raise it on every pane regardless — so the flag alone cannot say whether the
    /// file changed. This records the file's own date at the last read, and it is updated for free on
    /// the successful arm; [`PanelState::refresh`] uses it while a connection holds the pane.
    file_seen: Option<SystemTime>,
    /// Why an agent's access to this console ended, once the person took it over.
    ///
    /// **The pane's own flag is the truth and this is its copy for the painting**, which reads the
    /// panel state and has no way to reach the pane. It is set once, with the first byte of the
    /// person's input, and it never clears: what ended did not end back (**AC15**).
    taken_over: Option<String>,
    /// Whether an agent opened this console over the control channel.
    ///
    /// **It is what decides whether the person taking the console over is published here at all**
    /// (**AC15**). The sentence reports that an agent's access ended, so on a console no agent ever
    /// opened there is nothing that ended — and publishing it anyway made this panel **visible** over
    /// a console with no producer, which is what takes [`DEFAULT_WIDTH`] columns off it the first
    /// time anybody types.
    ///
    /// **It lives with the row it gates and not on the pane**, so the two are read in one place:
    /// whether there is a row to publish is a question about this panel, not about the console.
    opened_by_an_agent: bool,
}

impl PanelState {
    /// Records that an agent opened this console, **so its panel may carry the app's own row about
    /// losing it**.
    pub(crate) fn an_agent_opened_it(&mut self) {
        self.opened_by_an_agent = true;
    }

    /// Records that the person took this console over, **with the sentence the row will show**.
    ///
    /// **The application's own row and not a producer's**, so `header: false` cannot hide it: an agent
    /// that turned the identity row off would otherwise be able to hide from the person that a console
    /// of theirs had quietly gone back to them.
    pub(crate) fn the_person_took_it_over(&mut self, why: &str) {
        if self.opened_by_an_agent && self.taken_over.is_none() {
            self.taken_over = Some(why.to_owned());
        }
    }
}

/// Whether a refresh looked at the file at all.
///
/// **It exists so a caller can hand the notice back**, which is the whole of why it is a value and
/// not a bare return: the watcher raises a flag once per publication, and a refusal that swallowed
/// it would lose that publication for ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refreshed {
    /// The file was consulted — read, unchanged, or found broken.
    Read,
    /// A connection holds this panel, so nothing was read.
    Declined,
    /// A retired snapshot needs one later look to distinguish an identical metadata stamp from an
    /// in-place publication whose bytes changed.
    Deferred(std::time::Duration),
}

/// What one painting of a panel left behind for a press to land on.
#[derive(Debug, Clone, Default, PartialEq)]
struct Drawn {
    /// Whether that painting reused the layout it already had.
    ///
    /// **A press is only accepted on a settled panel**, and this is what makes «the command you read
    /// is the command that runs» true across frames. The panel is the one surface in this window
    /// that repaints from an outside writer with no user action, so between the frame a person read
    /// and the frame their click lands in, a producer can have replaced every button. It costs
    /// nothing to refuse that click: the panel is redrawn, the person reads it again, they press
    /// again.
    settled: bool,
    buttons: Vec<PaintedButton>,
    /// Link targets belong to the painting that produced their rectangles.
    ///
    /// **The target is owned here rather than recovered from `last_good`.** A producer can commit a
    /// new document after this frame and before its press is routed. Looking the target up by line
    /// number then would let new bytes retarget old pixels.
    links: Vec<PaintedLink>,
}

/// A button as it was drawn: which action it is, and the rectangle it occupied.
///
/// The rectangle is the union of the button's rows — its label and the wrapped command under it — so
/// a press anywhere on the command reaches the same button as a press on the label.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PaintedButton {
    action: usize,
    area: Rect,
}

/// A layout and the two things it depends on: if either changes, it has to be redone.
struct PanelLayout {
    width: usize,
    /// **The line the app puts in enters the cache key.** A panel that ages changes what has to be
    /// painted without its file having been touched, so a cache that only looked at the width would keep
    /// showing the layout from when the panel was fresh — and the warning would never appear.
    warning: Option<String>,
    rows: Vec<Row>,
}

impl PanelState {
    /// Reads the panel again if its file has changed.
    ///
    /// **It times nothing**: whoever measures is the window, with a single clock around reading and
    /// painting. Two clocks for a stretch that is published as one is the easiest way for the number to
    /// stop meaning what it says.
    pub fn refresh(&mut self, dir: &std::path::Path) -> Refreshed {
        self.refresh_file(dir)
    }

    fn refresh_file(&mut self, dir: &std::path::Path) -> Refreshed {
        // **The return is before the file is read, and that is the invariant rather than an
        // optimisation.** While a connection holds this pane there is exactly one writer, and reading
        // the file would put a second one back — settled, as it was before D17, by whichever of the
        // two wrote last. What it does do is **look at the file's date**, because the look-again flag
        // alone cannot say whether the file changed: a split, an open tab or the blind watcher raise
        // it on every pane, and latching the confession from it made a held panel say «the file is not
        // being read» for the rest of its life over a file nobody wrote.
        // The layout is not thrown away here: what this changes is the warning, and the warning is
        // in the cache's key — so a hundred writes to a file nobody reads redo the layout once.
        //
        // **Saying «declined» rather than just returning is what lets the caller keep the notice.**
        // A refusal that looked like a read would throw that publication away for ever: the file's
        // date never changes again on its own.
        if let Some(holder) = &mut self.holder {
            if driving::file_mtime(dir) != self.file_seen {
                holder.file_refused = true;
            }
            return Refreshed::Declined;
        }

        match self.refresh_after_retirement(dir) {
            driving::RetirementRefresh::NotRetired => {}
            driving::RetirementRefresh::Read => return Refreshed::Read,
            driving::RetirementRefresh::Deferred(delay) => return Refreshed::Deferred(delay),
        }

        let seen = self.last_good.as_ref().and_then(|panel| panel.written_at);

        match panel_contract::read_if_newer(dir, seen) {
            Ok(Some(panel)) => {
                // The file's own date, kept as the baseline a later hold compares against. The
                // contract's parser already carries it on the panel, so this is not a second read.
                self.file_seen = panel.written_at;
                self.commit(panel);
                self.problem = None;
                self.layout = None;
            }
            // **What there is stays valid, and that includes clearing the previous problem.** That the
            // read reaches here means the file could be asked about and that its date is that of the panel
            // already held: what is shown **is** the latest, and a failure from a while ago no longer
            // describes reality. Without this, a passing stumble —running out of descriptors, a network
            // mount that flickers— would accuse the producer forever, because its file is not going to
            // change its date again on its own.
            Ok(None) => self.problem = None,
            Err(problem) => {
                // A file that is not there is the baseline «nothing», so a file that appears during a
                // later hold reads as a change and is confessed. Any other failure leaves the last date
                // it had, which is still the file's date.
                if matches!(problem, Problem::Missing) {
                    self.file_seen = None;
                }
                self.problem = Some(problem);
            }
        }
        Refreshed::Read
    }

    /// The last panel that was understood, if there is one.
    ///
    /// **Only the tests use it, and that is why it is behind its flag.** Painting does not need it — it
    /// goes through [`PanelState::lay_out_if_needed`], which can compose a panel that is only the
    /// application's confession— and leaving it as a production method would be an open door to someone
    /// deciding «there is a panel» on their own again, which is exactly what [`PanelState::is_visible`]
    /// centralizes.
    #[cfg(test)]
    pub(crate) fn last_understood(&self) -> Option<&Panel> {
        self.last_good.as_deref()
    }

    /// The button that press reaches, if it reaches one.
    ///
    /// Three conditions, and each one is a decision rather than caution:
    ///
    /// - **the last pass painted this panel at all** — folded, or in a strip too short for one row,
    ///   it is not painted, and then there is nothing on screen for a press to have been aimed at;
    /// - **that painting reused the layout it already had**, so what is on screen now is what was on
    ///   screen when the person decided to press;
    /// - **the point is inside a button that painting recorded**, which excludes every button whose
    ///   rows did not all fit.
    pub fn button_at(&self, point: Pos2) -> Option<&Action> {
        let drawn = self.drawn.as_ref().filter(|drawn| drawn.settled)?;
        let pressed = drawn
            .buttons
            .iter()
            .find(|drawn| drawn.area.contains(point))?;
        self.last_good.as_ref()?.actions.get(pressed.action)
    }

    /// The buttons the last painting drew, in the order it drew them.
    ///
    /// **For the measurement and for the bench's injector**, which needs where to press and cannot
    /// derive it: a button's place comes out of wrapping and of how many there are, so a second
    /// derivation would be free to disagree with the one on screen.
    pub fn buttons_drawn(&self) -> impl Iterator<Item = Rect> + '_ {
        self.drawn
            .iter()
            .flat_map(|drawn| drawn.buttons.iter().map(|button| button.area))
    }

    /// Says that nothing was painted, so nothing is reachable.
    ///
    /// **Called wherever the window decides not to paint a panel** — a folded one, or a pane with no
    /// directory. Without it the previous frame's rectangles survive and a press lands on a button
    /// that is no longer on screen, which is the one thing this whole mechanism exists to prevent.
    pub fn nothing_was_drawn(&mut self) {
        self.drawn = None;
    }

    /// Whether this console has something to show, be it the producer's or the application's own.
    ///
    /// **It comes from the same thing that decides painting, and not from a parallel criterion.** With two
    /// predicates that say «there is a panel» on their own, the day they disagree a strip is reserved
    /// where nothing is painted, or something is painted outside the one that was reserved.
    ///
    /// **That the file does not exist yet does not count**, and that is the line that holds the promise
    /// that a console with no producer looks exactly like it did before this milestone: there is no panel,
    /// no width is deducted, there is nothing.
    pub fn is_visible(&self, now: SystemTime) -> bool {
        self.last_good.is_some() || self.warning(now).is_some()
    }

    /// What this pane claims **now**, if it claims anything.
    ///
    /// **It comes from the field the producer declared and from nothing else.** Not from the summary body,
    /// nor from the phase, nor from whether the console writes: deducing it is exactly what the contract
    /// forbids, and a line with `mark: block` from last week would leave the tab marked forever.
    ///
    /// **With no good panel there is no claim**, even if the application has something to confess: what is
    /// confessed then is that there is no panel, and a panel that could never be read said nothing to
    /// claim.
    pub fn attention(&self, now: SystemTime) -> Option<AttentionState> {
        self.attention_with(now, self.warning(now).is_none())
    }

    /// The same, for whoever already has the warning in hand.
    ///
    /// **It exists so the warning is computed once per pane and frame** and not twice: the painting path
    /// already needs it whole, and asking for it again here only to ask whether it is `None` costs a
    /// string format on every frame of a window that may be repainting at sixty.
    fn attention_with(&self, now: SystemTime, nothing_to_confess: bool) -> Option<AttentionState> {
        let class = self.last_good.as_ref()?.attention?;
        // **The finished needs no one behind it.** Whoever finished stops publishing *by definition*, so
        // the silence does not belie its «it finished and nobody looked»: it is its normal
        // state. Without this exception, half an hour after finishing everything finished would go out
        // — and finding in the morning what finished at night is exactly what it is for.
        // What does belie it is **not being able to read its file**, and that is the only thing looked at
        // here: `problem` is the same thing that separates the two classes of warning in
        // [`PanelState::warning`], from which `nothing_to_confess` comes.
        // **A held panel is backed unless what it shows has gone stale.** An open connection is the
        // most direct evidence of somebody standing behind the claim — and it is also exactly what a
        // wedged process holds. Giving it backing unconditionally kept a claim whose own line read
        // «no news for 12 min» painted bright, and painted its tab's card as waiting for somebody. The
        // warning is what already says the freshness is gone; this arm has to agree with it.
        let stale = self
            .last_good
            .as_ref()
            .is_some_and(|panel| panel.is_stale_at(now));
        let backed = nothing_to_confess
            || (self.holder.is_some() && !stale)
            || (class == Attention::Done && self.problem.is_none());
        Some(AttentionState { class, backed })
    }

    /// How long until **what this panel says about itself changes**, if it is going to change.
    ///
    /// **The window needs it to wake up in time.** A producer that dies or hangs writes nothing, so there
    /// is no file event to warn: without this alarm the warning of «this is old» would never appear in a
    /// window at rest, which is exactly when it is needed.
    ///
    /// **And it does not end when the panel is marked as old**: the warning carries a number that
    /// advances, so it has to come back when that number changes. Without that, a panel marked old at five
    /// minutes would keep saying «5 min» three hours later — right about the qualitative and lying just
    /// about the datum that serves to decide whether the agent is dead or only slow.
    pub fn how_long_until_warning(&self, now: SystemTime) -> Option<Duration> {
        // With a problem the warning is already set and does not change with time: there is nothing to
        // come back to.
        if self.problem.is_some() {
            return None;
        }
        let panel = self.last_good.as_ref()?;
        let silence = now.duration_since(panel.written_at?).ok()?;

        match panel.stale_after.checked_sub(silence) {
            // It is not old yet: the next thing that changes is that it is.
            Some(missing) => Some(missing),
            // It already is, and what is shown is a rounded number: it comes back when it ticks over.
            None => Some(until_the_next_step(silence)),
        }
    }

    /// The layout for this width, this warning and this claim, computing it only if the one there is no
    /// good.
    fn lay_out_if_needed(
        &mut self,
        id: &str,
        width: usize,
        theme: &Theme,
        warning: Option<String>,
        attention: Option<AttentionState>,
    ) -> Option<(&[Row], bool)> {
        if self.last_good.is_none() && warning.is_none() {
            return None;
        }

        // **The claim is not in the key, and it is on purpose**: it dims exactly when the warning appears
        // —which is in it— and its class only changes on re-reading the file, which throws away the whole
        // layout. Putting it in would be a term no test can knock down. Whoever changes the criterion of
        // «backed» and detaches it from the warning has to add it here.
        let still_valid =
            matches!(&self.layout, Some(done) if done.width == width && done.warning == warning);
        if !still_valid {
            let mut rows = lay_out(
                self.last_good.as_deref(),
                id,
                width,
                theme,
                warning.as_deref(),
                attention,
                self.holder.as_ref().map(|held| held.source.as_str()),
            );
            color::flag(&mut rows);
            self.layout = Some(PanelLayout {
                rows,
                width,
                warning,
            });
        }
        self.layout
            .as_ref()
            .map(|done| (done.rows.as_slice(), still_valid))
    }
}

/// Whether the problem is that the file is not there, which is the only one that is **not** shown on its
/// own.
pub(super) fn is_a_missing_file(problem: &Problem) -> bool {
    matches!(problem, Problem::Missing)
}

/// What is said about a panel that has been quiet longer than its due.
///
/// **Without a how-much when the deadline is under a minute**: the contract allows a `stale_after_minutes`
/// of zero, and «no news for 0 min» a second after publishing reads as an application failure, not as a
/// confession.
pub(super) fn no_news(panel: &Panel, now: SystemTime) -> String {
    match now.duration_since(panel.written_at.unwrap_or(now)) {
        Ok(silence) if silence.as_secs() >= 60 => {
            format!("no news for {}", in_words(silence))
        }
        _ => "no news".to_owned(),
    }
}

/// A while, said the way someone would say it: minutes until they are too many, and then hours.
///
/// Without seconds or decimals on purpose. What the warning has to convey is «a long time ago», and a
/// number that changes every second in a corner of the screen is noise, not information — and every
/// change costs a frame, because the window has to be woken to paint it.
fn in_words(how_much: Duration) -> String {
    let minutes = how_much.as_secs() / 60;
    match minutes {
        0..=119 => format!("{minutes} min"),
        _ => format!("{} h", minutes / 60),
    }
}

/// How long until the number of [`in_words`] changes.
///
/// It is what makes the warning not rot on screen without waking the window more than needed: while
/// minutes are said it comes back at the next minute, and when hours are said, at the next hour.
fn until_the_next_step(silence: Duration) -> Duration {
    let step = if silence.as_secs() < 2 * 60 * 60 {
        60
    } else {
        60 * 60
    };

    Duration::from_secs(step - silence.as_secs() % step)
}

/// The longest of a line that is painted, in characters.
///
/// **It is not cosmetic: it is the only thing that bounds the cost of painting, and it is measured.** The
/// size ceiling of the contract bounds what a file makes it reserve when *interpreted*, and its worst case
/// is thirty thousand tiny objects. Painting has the **inverse** worst case: a single line of 260,000
/// characters —254 KiB, comfortably below the ceiling, and one of the cheap ones to interpret— costs
/// **14.6 ms of tessellation on every frame** while it stays published, with a million vertices per frame.
/// The whole budget is 16.7 ms and the two consoles already spend 5.92.
///
/// **And clipping to the rectangle does not save you**: epaint discards text **by row, not by glyph**
/// (`tessellator.rs:2050`), so a row that starts inside the panel is copied whole to the mesh even if it
/// runs off to the right.
///
/// It is still needed with wrapping in place: wrapping spreads the text across rows, but **does not throw
/// it away**, so a million characters would be a million characters laid out.
const MAX_CHARACTERS: usize = 512;

/// Removes from an outside string what would knock the panel's grid out of square, and bounds what is
/// painted.
///
/// **It is measured in the source of epaint 0.36.1**: `break_on_newline` comes set by default in the
/// layout jobs `Painter::text` uses (`text_layout_types.rs:107`), so a single line with a `\n` inside
/// paints several rows, and the layout counts one. A tab is the other case: it advances the width of
/// several cells while the width measure counts it as zero.
///
/// The `panel.json` can come from a cloned repo or from a container, so this is not prudence against a
/// slip: it is the border with the outside. **It is applied to the reference too**, which is an outside
/// string all the same and since this PR is painted.
///
/// It is replaced with a space instead of deleted, so that two words separated by a line break do not end
/// up stuck together and read as one.
fn sanitize(text: &str) -> String {
    // **The same predicate the action parser uses, and for the same reason.** The control predicate
    // alone missed the format characters that are painted as nothing (U+200B), the direction
    // overrides that paint the rest backwards (U+202E), and the line separators: text arriving over
    // the wire — a `source` name above all — could reorder or spoof the line a person reads while
    // the bytes are the ones that were sent. One list, in the contract crate, so the two callers
    // cannot drift.
    text.chars()
        .take(MAX_CHARACTERS)
        .map(|c| if is_shown_as_itself(c) { c } else { ' ' })
        .collect()
}

/// The symbol each contract mark is painted with.
///
/// **Chosen by measuring the font, not from memory, and the width is the criterion.** Of the prototype's
/// six, `✓` does not exist in the monospaced one —zero width, just like an invented codepoint— and `⏸`
/// and `⚠` are only in the emoji fonts, where they measure 14 and 17.77 points against the 8.43 of a
/// cell: **they would knock the column out of square**, which reads worse than a box because it does not
/// look like a font failure but like a badly aligned panel. The other three of the prototype do serve as
/// they are.
///
/// `done` and `pending` share a shape on purpose —the filled circle and the dot— because they are the two
/// extremes of the same thing: the finished and what has not even started.
///
/// If some day the set falls short of expressiveness, the way out is written in the plan: painting the
/// marks as a shape instead of as a character. That is another milestone.
fn symbol(mark: Mark) -> char {
    match mark {
        Mark::Done => '●',
        Mark::Current => '▶',
        Mark::Pending => '·',
        Mark::Wait => '…',
        Mark::Block => '×',
        Mark::Info => '›',
    }
}

/// The symbol each class of claim is painted with.
///
/// **The three are their own and none is repeated with those of the body**, on purpose: the same sign in
/// two places invites reading that the application deduced one from the other, which is exactly what the
/// contract forbids — a `mark: block` from a line of the summary is **not** a claim of the pane.
///
/// **Chosen by measuring them against the width of an `M`**, with the same criterion and for the same
/// reason as those above. The test that checks it against the font tells the whole why.
///
/// `?` because they are asking you, `⊘` because something has it stopped, and `∎` because there is nothing
/// left there to do but look at it.
pub(crate) fn attention_symbol(class: Attention) -> char {
    match class {
        Attention::Wait => '?',
        Attention::Block => '⊘',
        Attention::Done => '∎',
    }
}

/// The symbol a button's label is painted with.
///
/// **It is a tenth sign and none of the other nine is reused** — the six a line can carry and the
/// three a claim can — for the reason those three already give: the same sign in two places invites reading that one was deduced from the other, and
/// here the two readings are «this line says something» and «pressing this runs a command». They are
/// not the same kind of thing and they must not look alike.
///
/// **Two constraints, and the second was found by the bench rather than reasoned.** Like the others
/// it is measured against the width of an `M`, because a sign the monospaced font draws wider than a
/// cell knocks the whole column out of square. And it has to be a character **the built-in fonts
/// already cover**: `»` was the first choice and it is not, so every frame that painted a button
/// sent the window off to search the machine for a face — which invalidated the measurement batch on
/// the rule that only the Japanese one may do that. A symbol nobody can see costs one glyph; a symbol
/// that triggers discovery costs the comparability of every published series.
fn action_symbol() -> char {
    '>'
}

/// What the claim's row says, in words.
///
/// **The symbol does not travel alone**: in the panel there is room, and a loose sign forces recalling a
/// table.
///
/// The three phrases are **the contract's, word for word**, including the «for you» of the first —
/// which is exactly what separates it from the second, «something that is **not** you». Shortening
/// them to fit is losing the opposition that tells them apart; what does not fit is wrapped, like
/// everything else in the panel.
fn attention_text(class: Attention) -> &'static str {
    match class {
        Attention::Wait => "it is waiting for you",
        Attention::Block => "stuck on something else, not you",
        Attention::Done => "it finished and nobody looked",
    }
}

/// The least left to the text when the indent and the column eat it all, in cells.
///
/// **Without a floor, wrapping does not progress**: with the gap at zero or negative, each turn of the
/// loop puts in zero characters and the window hangs. The prototype already learned it, and that is why
/// its wrapping carries the same `max`.
const MINIMUM_SLOT: usize = 8;

/// Wraps a text **without losing or inventing a single character**, unlike [`wrap`].
///
/// **The ordinary wrapping collapses whitespace, and for a button that is a defect rather than a
/// nicety.** It splits on whitespace and rejoins with one space, so `echo  a` is painted `echo a`
/// while `echo  a` is what gets sent — and for a button the painted text **is** the confirmation.
/// Two spaces mean nothing to a shell outside quotes and everything inside them, and either way a
/// painting that is not the sending has already broken the promise.
///
/// It is not [`sanitize`]d either: the contract already refuses every scalar that is not painted as
/// itself, so there is nothing left here to flatten — and flattening is precisely what would make
/// the two differ.
///
/// Cut **by characters and never by words**, which is right for one command and would be wrong for a
/// summary: a long path has nowhere to break and the panel is narrow.
fn wrap_verbatim(rows: &mut Vec<Row>, indent: usize, text: &str, color: Color32, width: usize) {
    // Kept as its own name rather than a flag on `wrap`: the two differ in a property that decides
    // whether a button is honest, and a boolean at the call site is exactly where that stops being
    // readable.
    let slot = width.saturating_sub(indent).max(MINIMUM_SLOT);
    let mut rest = text;
    loop {
        let cut = rest
            .char_indices()
            .nth(slot)
            .map_or(rest.len(), |(at, _)| at);
        let (row, after) = rest.split_at(cut);
        rows.push(Row::text(indent, row.to_owned(), color));
        if after.is_empty() {
            return;
        }
        rest = after;
    }
}

/// A row already laid out: which column it starts in, what it says, what color, and what it is for.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    column: usize,
    text: String,
    color: Color32,
    /// What this row is **for**, which is what lets the painting know which rectangle belongs to
    /// which button without laying the panel out a second time.
    role: RowRole,
    /// A compact link label painted on this row, independently of its body text.
    link: Option<LaidOutLink>,
    /// Which of its characters go to color, as ranges of character indices, set once the whole layout
    /// is done — the text is still being composed until then.
    color_spans: Vec<std::ops::Range<usize>>,
}

impl Row {
    /// A row of plain text, which is what every row is until a button or a link claims it.
    fn text(column: usize, text: String, color: Color32) -> Self {
        Self {
            column,
            text,
            color,
            role: RowRole::Text,
            link: None,
            color_spans: Vec::new(),
        }
    }
}

/// What a laid-out row is for.
///
/// **A button occupies more than one row** — its label, and the command it sends wrapped underneath —
/// and a press has to reach the whole of it. So the rows carry which button they belong to, rather
/// than the painting trying to work it out from their text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
enum RowRole {
    #[default]
    Text,
    /// The row carrying a button's label, with the index of the action it belongs to.
    ButtonLabel(usize),
    /// A row carrying part of the command that button sends.
    ButtonBody(usize),
}

/// Turns a panel into the rows that are going to be painted, at the given width in cells.
///
/// **Here is everything it costs and nothing it draws**, and that separation is what allows remembering
/// the result between frames: as long as the file and the width do not change, the layout is the same.
/// Without it, wrapping the panel would be recalculated sixty times per second while the console beside it
/// dumps.
fn lay_out(
    panel: Option<&Panel>,
    id: &str,
    width: usize,
    theme: &Theme,
    warning: Option<&str>,
    attention: Option<AttentionState>,
    held_by: Option<&str>,
) -> Vec<Row> {
    let mut rows = Vec::new();

    if panel.is_none_or(|panel| panel.header) {
        let phase = panel.and_then(|panel| panel.phase.as_deref());
        // **Who is driving goes on the identity row, beside the pane's own name and its phase.**
        // That row answers «what is this pane», and with two transports the answer includes which
        // one is in charge — a person looking at a panel that will not change from the file has to
        // be able to see why without editing anything to find out.
        let identity = match (phase, held_by) {
            (Some(phase), Some(source)) => format!("{id} · {phase} · {source}"),
            (Some(phase), None) => format!("{id} · {phase}"),
            (None, Some(source)) => format!("{id} · {source}"),
            (None, None) => id.to_owned(),
        };
        wrap(
            &mut rows,
            0,
            &sanitize(&identity),
            theme.panel_color(None, None),
            width,
        );
    }

    // **The claim goes in the application's row and not in the identity one**, and that frontier is the
    // reason this milestone exists: `header: false` hides the pane's identity, and the answer to «do I
    // have to go there?» cannot be turned off by the producer with a flag of its own. It is the same
    // frontier that the warning below already fulfills.
    if let Some(attention) = attention {
        wrap(
            &mut rows,
            0,
            &format!(
                "{} {}",
                attention_symbol(attention.class),
                attention_text(attention.class)
            ),
            theme.attention_color(attention.class, attention.backed),
            width,
        );
    }

    // **The warning goes on top and not at the end**, because below is where the clip eats what does not
    // fit: the warning that a panel is not trustworthy would be exactly the first thing to disappear from
    // a long panel.
    if let Some(warning) = warning {
        wrap(
            &mut rows,
            0,
            &sanitize(warning),
            theme.panel_color(None, Some(Style::Warn)),
            width,
        );
    }

    // **The buttons go above the body and below the warning, inside the block the application owns.**
    // Below is where the clip eats what does not fit, and a button whose command row got eaten would be
    // a label you can press over a command you cannot read — the one state the rule about seeing a
    // command before it runs forbids.
    // **A panel too narrow to show a command at all declares no buttons.** The wrapping has a floor
    // on its slot — without one it would put zero characters per row and hang — so below that floor
    // it lays the text out wider than the strip and the painting clips what does not fit: the person
    // would see a fragment and the press would send the whole string. Leaving these rows untagged is
    // the same mechanism as a button the clip cut in half: unreachable rather than refused.
    let shows_a_command = width.saturating_sub(MARGIN as usize) >= MINIMUM_SLOT;
    for (nth, action) in panel
        .iter()
        .flat_map(|panel| panel.actions.iter())
        .filter(|_| shows_a_command)
        .enumerate()
    {
        let first = rows.len();
        wrap(
            &mut rows,
            0,
            &sanitize(&format!("{} {}", action_symbol(), action.label())),
            theme.panel_color(None, None),
            width,
        );
        let body = rows.len();
        // **The command is painted in the ordinary ink and not in the dimmed one.** `clip`'s «… N more»
        // uses the dim colour to say «this is elided», and the one sentence on this panel that a person
        // has to read before pressing anything must not be styled as de-emphasis.
        wrap_verbatim(
            &mut rows,
            MARGIN as usize,
            action.send(),
            theme.panel_color(None, None),
            width,
        );
        for row in &mut rows[first..body] {
            row.role = RowRole::ButtonLabel(nth);
        }
        for row in &mut rows[body..] {
            row.role = RowRole::ButtonBody(nth);
        }
    }

    // The blank row is written by the layout, not by the painting: separated from the body, what the
    // application puts reads as what it is and not as the producer's first line.
    if !rows.is_empty() {
        rows.push(Row::text(0, String::new(), Color32::TRANSPARENT));
    }

    let Some(panel) = panel else {
        return rows;
    };

    for block in blocks(&panel.lines) {
        // **The column's width is computed per block, and that is the contract's rule.** With a global
        // width, a single block with a long reference would displace the text of all the others, and what
        // the panel gains by aligning it would lose in wasted margin.
        // **And the column is bounded against the panel's width.** The contract puts no ceiling on the
        // reference, so a single very long one would fix a column width that pushes the text of its whole
        // block outside the rectangle. What does not fit is cut, as the text is cut.
        let column = block
            .iter()
            .filter_map(|line| line.reference.as_deref())
            .map(|reference| sanitize(reference).chars().count())
            .max()
            .unwrap_or(0)
            .min(width.saturating_sub(MINIMUM_SLOT).saturating_sub(3));

        for line in block {
            rows.extend(lay_out_line(line, column, width, theme));
        }
    }

    rows
}

/// Splits the lines into blocks by the empty lines, which is what the contract uses to group them.
///
/// The gap **belongs to the block it closes**: this way it is kept as a row and the layout does not have
/// to remember to put it back between one block and the next.
fn blocks(lines: &[Line]) -> Vec<Vec<&Line>> {
    let mut blocks = vec![Vec::new()];
    for line in lines {
        let slot = line.text.is_empty() && line.reference.is_none() && line.mark.is_none();
        blocks
            .last_mut()
            .expect("there is always one open")
            .push(line);
        if slot {
            blocks.push(Vec::new());
        }
    }
    blocks
}

/// Lays out a line: its symbol, its aligned reference, its indent and its wrapped text.
fn lay_out_line(line: &Line, column: usize, width: usize, theme: &Theme) -> Vec<Row> {
    let color = theme.panel_color(line.mark, line.style);
    let reference = line.reference.as_deref().map(sanitize).unwrap_or_default();
    let text = sanitize(&line.text);

    // A real gap carries no prefix and takes no column: it is just a blank row.
    if text.is_empty() && reference.is_empty() && line.mark.is_none() && line.link.is_none() {
        return vec![Row::text(0, String::new(), color)];
    }

    // **The head is bounded before being composed, not after.** Symbol, reference and indent are painted
    // in the first row without going through wrapping, so a reference longer than the panel —legal form:
    // the contract puts no ceiling on it— pulled that whole row outside the rectangle and with it all its
    // continuations. What is spare of the reference is cut, just as the text is cut: what does not fit
    // cannot be read, and pretending it can displaces everything else.
    let head_cap = width.saturating_sub(MINIMUM_SLOT);
    // It is cut to the column's width, which already comes bounded against the panel: longer than its
    // column, the reference would push the text of its own line and break the block's alignment.
    let reference: String = reference.chars().take(column).collect();

    let mut head = String::new();
    match line.mark {
        Some(mark) => head.push(symbol(mark)),
        // The symbol's slot is reserved even without a mark: it is what keeps the text of all the lines in
        // the same column, which is what aligning is about.
        None => head.push(' '),
    }
    head.push(' ');
    if column > 0 {
        head.push_str(&format!("{reference:column$} "));
    }
    // The indent is bounded against the same: an `indent` of 40 cells is legal form —the contract admits
    // up to 64— and in a panel of 36 it would leave the continuation outside the rectangle, invisible and
    // counted as a visible row, eating rows and falsifying the warning of how many are missing.
    head.push_str(
        &" ".repeat(
            line.indent
                .min(head_cap.saturating_sub(head.chars().count())),
        ),
    );

    let indent = head.chars().count().min(head_cap);
    let mut rows = Vec::new();
    wrap(&mut rows, indent, &text, color, width);

    // The head is stuck to the first row; the continuation ones start already indented, which is what
    // aligns the wrapping under the text and not under the symbol.
    match rows.first_mut() {
        Some(first) => {
            first.text = format!("{head}{}", first.text);
            // **And the first row starts at zero, whether it carries text or not.** The empty-text branch
            // of the wrapping returns its indent —correct for a continuation, not for the row that carries
            // the head stuck to it—, so a line with a mark and no text came out displaced by the width of
            // its own symbol. It is legal form of the contract.
            first.column = 0;
        }
        None => rows.push(Row::text(0, head, color)),
    }
    if let Some(target) = line.link.as_deref() {
        links::attach(&mut rows, target, indent, width, color);
    }

    rows
}

/// Splits a text into chunks that fit, and adds them with the continuation indent in place.
///
/// It cuts **by words** when it can and by characters when a single word does not fit, which is the case
/// of a long path or of an identifier. Multiple spaces are collapsed, as the contract warns: they do not
/// serve to align.
fn wrap(rows: &mut Vec<Row>, indent: usize, text: &str, color: Color32, width: usize) {
    let slot = width.saturating_sub(indent).max(MINIMUM_SLOT);
    let mut pending: Vec<&str> = text.split_whitespace().rev().collect();

    if pending.is_empty() {
        rows.push(Row::text(indent, String::new(), color));
        return;
    }

    while let Some(word) = pending.pop() {
        let mut line = String::new();
        let mut word = word;
        loop {
            let room = slot - line.chars().count();
            if word.chars().count() > room {
                if line.is_empty() {
                    // A single word longer than the gap: it is split by characters, because leaving it
                    // whole would overflow the panel and jumping to a line would never make it fit.
                    let cut = word.char_indices().nth(room).map_or(word.len(), |(i, _)| i);
                    line.push_str(&word[..cut]);
                    pending.push(&word[cut..]);
                }
                break;
            }
            line.push_str(word);
            match pending.last() {
                Some(next) if line.chars().count() + 1 + next.chars().count() <= slot => {
                    line.push(' ');
                    word = pending.pop().expect("I just looked at it");
                }
                _ => break,
            }
        }
        // The first row is placed by the caller, sticking the head to it; the rest carry their indent.
        let column = if rows.is_empty() { 0 } else { indent };
        rows.push(Row::text(column, line, color));
    }
}

/// Paints a console's panel in the rectangle its painter is clipped to.
///
/// It borrows the state exclusively because **it lays out if needed and keeps it**: the alternative
/// —laying out outside and passing the rows— would force whoever paints to keep track of when the layout
/// expired, and that account belongs here.
///
/// The identifier is the pane's, and it is the only thing the application contributes of its own: the
/// identity line. **It is composed only with what the app already knows and with what the file itself
/// brings**, never by reading a neighboring `meta.json` — that would be deducing another's state.
///
/// The framing comes from the painter itself and not from a separate argument: with the two ways, nothing
/// would prevent them from ceasing to coincide one day.
pub fn paint(
    painter: &Painter,
    state: &mut PanelState,
    id: &str,
    theme: &Theme,
    metrics: &CellMetrics,
    now: SystemTime,
    discoverer: &mut crate::fonts::Discoverer,
) -> std::time::Duration {
    let rect = painter.clip_rect();
    painter.rect_filled(rect, 0.0, theme.panel_background());

    // The width in cells is deducted the margin of both sides even though only the left one is painted:
    // without the right one, the wrapping would fill to the exact edge of the panel and the text would be
    // stuck to the console beside it.
    let width = ((rect.width() / metrics.width()) as usize).saturating_sub(2 * MARGIN as usize);
    let warning = state.warning(now);
    let attention = state.attention_with(now, warning.is_none());
    // **Taken away first and put back only at the end.** Every way out of this function below is a
    // frame in which nothing was drawn, and a press must not land on what the previous one left.
    state.nothing_was_drawn();
    let Some((rows, settled)) = state.lay_out_if_needed(id, width, theme, warning, attention)
    else {
        return std::time::Duration::ZERO;
    };

    // **Without a whole row nothing is painted, not even the warning.** With a floor of one, a panel of
    // zero height queued «… N more» outside its rectangle: a warning that content is missing, in a place
    // where no content fits.
    let fit = (rect.height() / metrics.height()) as usize;
    if fit == 0 {
        return std::time::Duration::ZERO;
    }
    let (visible, leftover) = clip(rows, fit);
    let whole = rows.len() - leftover;
    // **Timed apart from the painting it sits inside, because it is not painting.** Laying out and
    // drawing a button's rows is what this span already means — rows are rows. Walking them
    // afterwards to work out what a press may reach is a **new kind of work**, and a frame's cost is
    // split by subtraction: a path that is not declared is published as the interface framework's,
    // which is the figure **D3** judges egui by.
    let walking = std::time::Instant::now();
    let mut drawn = buttons_drawn(rows, whole, rect, metrics);
    let walked = walking.elapsed();

    let text_left = rect.left() + MARGIN * metrics.width();
    let mut overflowed: std::collections::BTreeSet<RowRole> = std::collections::BTreeSet::new();
    let mut links_painted = Vec::new();
    let mut color_glyphs = Vec::new();
    for (n, row) in visible.iter().enumerate() {
        let x = text_left + row.column as f32 * metrics.width();
        let y = rect.top() + n as f32 * metrics.height();
        if !row.text.is_empty() {
            // **The panel discovers fonts too.** What it shows is what its producer publishes —free
            // text, in any language—, and without noting it a summary in Japanese stayed as boxes
            // forever because nothing triggered the search.
            discoverer.note_text(&row.text);
            let at = Pos2::new(x, y);
            let inked = color::paint_text(painter, at, row, metrics, discoverer, &mut color_glyphs);
            // **What a row really measures is asked of whoever drew it**, and that closes the last
            // way a press could send a command nobody could read. This module lays text out counting
            // characters, and a character is not a column: twenty-nine kanji are fifty-eight columns,
            // so a command in Japanese is drawn past the strip's edge and the painter clips what
            // sticks out — the person sees a fragment and the press would send the whole string.
            if inked.right() > rect.right() {
                overflowed.insert(row.role);
            }
        }

        if let Some(link) = &row.link {
            links_painted.push(links::paint_label(
                painter, link, y, metrics, theme, discoverer,
            ));
        }
    }

    if leftover > 0 {
        let y = rect.top() + (visible.len() as f32) * metrics.height();
        painter.text(
            Pos2::new(text_left, y),
            Align2::LEFT_TOP,
            format!("… {leftover} more"),
            metrics.font(),
            theme.panel_color(Some(Mark::Pending), None),
        );
    }
    // **After every piece of the panel's text**, so the images share one draw batch (**D25**).
    for glyph in &color_glyphs {
        glyph.paint(painter);
    }

    // **Recorded last, once nothing else borrows the layout.** What a press may reach is what this
    // pass actually drew, so it is written after the drawing rather than alongside it.
    // **A button whose ink left the strip is taken off the list**, for the same reason one the clip
    // cut in half never went on it: unreachable rather than refused, with no branch anywhere that
    // has to remember.
    drawn.retain(|button| {
        !overflowed.contains(&RowRole::ButtonLabel(button.action))
            && !overflowed.contains(&RowRole::ButtonBody(button.action))
    });
    // **D51 gives this to the panel's painting, not to the buttons' walk**: `walked` is published
    // under the buttons' name, so only `buttons_drawn` belongs inside the `walking` clock above. No
    // test can tell which span a few microseconds landed in; the clock's placement is the guard.
    let links = links::reachable(links_painted);
    state.drawn = Some(Drawn {
        settled,
        buttons: drawn,
        links,
    });
    links::disclose_hovered(painter, state.drawn.as_ref(), id);
    walked
}

/// How many rows are painted and how many are left out.
///
/// **The warning takes its own row**, so when something is left over one fewer has to be painted: without
/// deducting it, the last content line would be covered by the warning that says content is missing.
fn clip(rows: &[Row], fit: usize) -> (&[Row], usize) {
    if rows.len() <= fit {
        return (rows, 0);
    }
    let visible = fit.saturating_sub(1);
    (&rows[..visible], rows.len() - visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::markdown;
    use panel_contract::{PANEL_FILE, PanelDir};

    #[test]
    fn a_document_acquired_before_close_survives_but_a_later_read_does_not() {
        let mut state = PanelState::default();
        state.commit(panel_contract::parse_document(r#"{"lines":["kept"]}"#).expect("a panel"));
        let reader = state.reader();

        let acquired = reader.acquire().expect("the pane is alive");
        drop(state);

        assert!(
            reader.read().is_none(),
            "an admitted read kept later acquisitions alive after pane closure"
        );
        assert!(
            acquired
                .read()
                .expect("the acquired read finishes")
                .contains("kept")
        );
    }

    #[test]
    fn concurrent_reads_return_one_complete_committed_version() {
        let mut state = PanelState::default();
        let old = panel_contract::parse_document(r#"{"phase":"old","lines":["a"]}"#)
            .expect("the old panel");
        let new = panel_contract::parse_document(r#"{"phase":"new","lines":["b"]}"#)
            .expect("the new panel");
        let old_written = panel_contract::serialize(&old).expect("serialize the old panel");
        let new_written = panel_contract::serialize(&new).expect("serialize the new panel");
        state.commit(old.clone());
        let reader = state.reader();
        let start = Arc::new(std::sync::Barrier::new(9));

        let readers: Vec<_> = (0..8)
            .map(|_| {
                let reader = reader.clone();
                let start = Arc::clone(&start);
                let old_written = old_written.clone();
                let new_written = new_written.clone();
                std::thread::spawn(move || {
                    start.wait();
                    for _ in 0..100 {
                        let written = reader
                            .read()
                            .expect("the pane stays alive")
                            .expect("the committed document serializes");
                        assert!(
                            written == old_written || written == new_written,
                            "a read mixed or changed a committed document: {written}"
                        );
                        std::thread::yield_now();
                    }
                })
            })
            .collect();

        start.wait();
        for turn in 0..100 {
            state.commit(if turn % 2 == 0 {
                new.clone()
            } else {
                old.clone()
            });
            std::thread::yield_now();
        }
        for reader in readers {
            reader.join().expect("the concurrent reader finished");
        }
    }

    /// A panel read from a real file, to test the layout without going through the painting.
    fn panel_with(file: &str) -> Panel {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, file);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        state.last_understood().expect("there is a panel").clone()
    }

    pub(super) fn publish(dir: &PanelDir, content: &str) {
        std::fs::write(dir.path().join(PANEL_FILE), content).expect("publish the panel");
    }

    /// Advances the file's date so that the re-read sees it changed.
    ///
    /// By hand and not leaving it to the clock: two writes in a row fall within the same tick on some file
    /// systems, and then the test would pass or fail depending on the machine.
    pub(super) fn age_the_file(dir: &PanelDir, seconds: u64) {
        let path = dir.path().join(PANEL_FILE);
        std::fs::File::open(&path)
            .expect("open")
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(seconds))
            .expect("move the date");
    }

    /// What a cell of width measures in the pane tests: fixed, so that the floor comes out round and the
    /// failures read without calculating anything.
    const CELL_WIDTH: f32 = 8.0;

    fn pane_of(width: f32) -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(width, 300.0))
    }

    #[test]
    fn the_panel_takes_its_width_and_the_console_the_rest() {
        let slot = set_aside(pane_of(800.0), CELL_WIDTH, Some(DEFAULT_WIDTH));
        let panel = slot.panel.expect("there is a panel");

        assert_eq!(panel.width(), DEFAULT_WIDTH as f32 * CELL_WIDTH);
        assert_eq!(
            slot.console.width() + panel.width(),
            800.0,
            "between the two they have to cover the pane, with no gap or overlap"
        );
        assert_eq!(
            slot.console.right(),
            panel.left(),
            "the panel starts right where the console ends"
        );
    }

    /// The panel is margin and the console is the work, so in a narrow window what narrows is the panel.
    /// With the rule reversed, dragging the divider would leave an interactive program with no room to
    /// write while its panel keeps its thirty-six columns.
    #[test]
    fn in_a_narrow_pane_the_panel_yields_and_the_console_keeps_its_floor() {
        let narrow = (MINIMUM_CONSOLE + 4.0) * CELL_WIDTH;

        let slot = set_aside(pane_of(narrow), CELL_WIDTH, Some(DEFAULT_WIDTH));

        assert_eq!(
            slot.panel.expect("there is a panel").width(),
            4.0 * CELL_WIDTH,
            "the panel is the one that yields"
        );
        assert_eq!(slot.console.width(), MINIMUM_CONSOLE * CELL_WIDTH);
    }

    /// And the edge of the previous one: below the console's floor the panel disappears entirely rather
    /// than return an inverted rectangle, which is what a subtraction without a floor would do.
    #[test]
    fn in_a_pane_narrower_than_the_floor_the_panel_is_left_with_no_width() {
        let slot = set_aside(pane_of(16.0), CELL_WIDTH, Some(DEFAULT_WIDTH));
        let panel = slot.panel.expect("the rectangle exists, though empty");

        assert_eq!(panel.width(), 0.0);
        assert_eq!(slot.console.width(), 16.0);
        assert!(slot.console.left() <= slot.console.right() && panel.left() <= panel.right());
    }

    /// **The width is thirty-six columns, not two hundred eighty-eight points**, and with a single cell
    /// measure the two things give the same number: this module's first test passes just the same with a
    /// fixed width in points. The difference is only seen **between frames of different zooms** —the font
    /// changes size and the metric is asked again on each pass—, and it is exactly what the decision to
    /// count it in cells exists to preserve: how many characters fit.
    #[test]
    fn the_panel_width_is_counted_in_cells_and_not_in_points() {
        // Double the zoom, which is the jump a desktop makes when moving to a dense screen.
        for cell in [CELL_WIDTH, CELL_WIDTH * 2.0] {
            let slot = set_aside(pane_of(1600.0), cell, Some(DEFAULT_WIDTH));

            assert_eq!(
                slot.panel.expect("there is a panel").width() / cell,
                DEFAULT_WIDTH as f32,
                "with cells of {cell} points the panel stopped measuring its columns"
            );
        }
    }

    /// With nothing to paint no width is deducted and there is no second rectangle: the pane looks
    /// exactly like it did before the panel existed.
    #[test]
    fn with_no_panel_to_paint_the_console_keeps_everything() {
        let whole = pane_of(800.0);

        let slot = set_aside(whole, CELL_WIDTH, None);

        assert_eq!(slot.console, whole);
        assert!(slot.panel.is_none());
    }

    /// The room given to the panel in the painting tests: plenty of height so everything fits, so what
    /// does not come out is because it was not painted and not because it did not fit.
    const PLACE: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 600.0));

    /// Paints a real panel in the given place and returns **everything that got queued**.
    ///
    /// It returns the whole shapes and not just their text because two properties of the painting are not
    /// text: **the background**, which is the only thing that separates the panel from its console, and
    /// **where** each row **falls**.
    fn what_gets_queued(file: &str, place: Rect) -> Vec<eframe::egui::Shape> {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, file);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        assert!(
            state.last_understood().is_some(),
            "the test's file was not read"
        );

        paint_the_state(&mut state, place, SystemTime::now())
    }

    /// Paints the state it is given, at the moment it is told.
    ///
    /// Separated from [`what_gets_queued`] because the confession depends on **when** you look: the same
    /// panel is painted differently if its deadline has passed.
    pub(super) fn paint_the_state(
        state: &mut PanelState,
        place: Rect,
        now: SystemTime,
    ) -> Vec<eframe::egui::Shape> {
        paint_the_state_with(state, place, now, &mut crate::fonts::Discoverer::default())
    }

    /// The same, with the font discoverer the test hands it — which is how a test gets a full atlas.
    pub(super) fn paint_the_state_with(
        state: &mut PanelState,
        place: Rect,
        now: SystemTime,
        discoverer: &mut crate::fonts::Discoverer,
    ) -> Vec<eframe::egui::Shape> {
        let ctx = eframe::egui::Context::default();
        let mut output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrics = CellMetrics::measure(
                ui.ctx(),
                eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE),
            );
            paint(
                &ui.painter_at(place),
                state,
                "a1b2",
                &Theme::default(),
                &metrics,
                now,
                discoverer,
            );
        });

        // The shapes are taken **before** dropping the output: a UI pass dropped without applying its
        // textures panics, and here there is no screen to apply them on.
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();

        shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    /// A state with a panel published does what it is told.
    ///
    /// The date is moved by hand and the clock is not waited on, for the same reason as in [`age_the_file`]:
    /// a test that depended on how long it takes to run would give a different result on each machine.
    fn published_ago(file: &str, how_much: Duration) -> PanelState {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, file);
        std::fs::File::open(dir.path().join(PANEL_FILE))
            .expect("open")
            .set_modified(SystemTime::now() - how_much)
            .expect("move the date");

        let mut state = PanelState::default();
        state.refresh(dir.path());
        state
    }

    /// The text of what was painted, row by row.
    fn texts(shapes: Vec<eframe::egui::Shape>) -> Vec<String> {
        shapes
            .into_iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    /// **The producer that dies does not warn that it has died.** It is the case this warning exists to
    /// cover: it writes nothing, so there is no file event to give it away — only time.
    #[test]
    fn a_panel_that_has_been_quiet_longer_than_its_due_confesses_it() {
        // 45 min **and a half**: the silence is truncated to whole minutes, and with the date right on the
        // border a file system that rounded up would say «44 min» and the test would point at the code
        // without the code being at fault.
        let mut state = published_ago(
            r#"{"stale_after_minutes":5,"lines":["what it said"]}"#,
            Duration::from_secs(45 * 60 + 30),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            painted.iter().any(|row| row.contains("no news")),
            "the panel has gone 45 minutes untouched and is shown as if from now: {painted:?}"
        );
        assert!(
            painted.iter().any(|row| row.contains("45 min")),
            "the warning does not say how long it has been quiet: {painted:?}"
        );
        assert!(
            painted.iter().any(|row| row.contains("what it said")),
            "what the producer had published was lost: {painted:?}"
        );

        // **And it goes on top, not at the end.** Below is where the clip eats what does not fit: in a
        // long panel, the warning that it is not trustworthy would be the first to disappear.
        let where_at = |text: &str| painted.iter().position(|row| row.contains(text));
        assert!(
            where_at("no news") < where_at("what it said"),
            "the warning is painted behind the producer's body: {painted:?}"
        );
    }

    /// And the other half: **a panel that is up to date says nothing**. Without this, a warning set always
    /// would pass for good and the panel would have a line of permanent noise.
    #[test]
    fn a_just_published_panel_confesses_nothing() {
        let mut state = published_ago(
            r#"{"stale_after_minutes":5,"lines":["just posted"]}"#,
            Duration::from_secs(10),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            !painted.iter().any(|row| row.contains("no news")),
            "a panel from ten seconds ago announces itself as old: {painted:?}"
        );
    }

    /// **Keeping the last good panel without saying so hides a broken producer** behind a plausible
    /// summary from an hour ago. This is the half of AC7 that was missing: the window stays up, the last
    /// good one is kept, **and the panel says that is not the latest**.
    #[test]
    fn a_publication_that_is_not_understood_is_confessed_without_losing_the_good_one() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"lines":["the good stuff from before"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());

        publish(&dir, "this is not json");
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        // The confession is one logical line but the panel wraps it, and the English wording is long
        // enough to split «not understood» across two rows, so it is looked for in the joined painting
        // rather than on a single row.
        //
        // **And by position, not by membership.** Two `contains` over the joined painting pass with the
        // marker and its reason on unrelated rows: measured by swapping them in the production format,
        // where the membership form stayed green and this one goes red. What the test defends is that
        // the reason belongs to the marker, so the order is the assertion.
        //
        // **And both have to be there, which comparing two `Option`s does not ask.** `None` sorts before
        // `Some`, so `find(a) < find(b)` passes with the marker missing altogether — the very thing the
        // test exists to catch. The pair is matched instead.
        let one_line = painted.join(" ");
        let marker = one_line.find("not the latest");
        let reason = one_line.find("not understood");
        assert!(
            matches!((marker, reason), (Some(marker), Some(reason)) if marker < reason),
            "the old is still shown as if it were the latest: {painted:?}"
        );
        assert!(
            painted
                .iter()
                .any(|row| row.contains("the good stuff from before")),
            "confessing cannot cost the last thing that was understood: {painted:?}"
        );
    }

    /// And when there **never** was anything good, the panel is only the confession. Without this, a
    /// console whose producer publishes garbage from the start looks the same as one with no producer:
    /// like a bare console, without a word explaining why there is no panel.
    #[test]
    fn a_console_with_nothing_good_shows_the_warning_on_its_own() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, "not this either");
        let mut state = PanelState::default();
        state.refresh(dir.path());

        assert!(
            state.is_visible(SystemTime::now()),
            "there is a producer publishing something illegible and the pane looks as if there were none"
        );
        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        // **On one row, not on the joined painting.** This confession does fit a row —checked by
        // restoring the per-row form, which passes— so joining would only drop the requirement that the
        // reason accompany the warning. Its sibling above has to join because its wording wraps; this
        // one does not, and copying the concession there would silence what it defends.
        assert!(
            painted
                .iter()
                .any(|row| row.contains("no panel") && row.contains("not understood")),
            "it does not say why there is no panel: {painted:?}"
        );
    }

    /// **AC15's second half, against the rows a person reads**: once the person types into a console
    /// an agent opened, the application's own row says so — and `header: false` cannot hide it, which
    /// is the whole reason it is the application's row and not the producer's.
    ///
    /// **The console is made an agent's first, and that is the premise and not a formality**: on one
    /// the person opened themselves there is no access that ended and no row, which is what
    /// `only_a_console_an_agent_opened_says_on_its_panel_that_it_went_back` guards from the window's
    /// own doors.
    #[test]
    fn a_console_the_person_took_over_says_so_on_its_own_row() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, "{\"header\":false,\"lines\":[\"still publishing\"]}");
        let mut state = PanelState::default();
        state.refresh(dir.path());
        state.an_agent_opened_it();
        state
            .the_person_took_it_over("the person typed into this console, so it went back to them");

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        // **Joined, because the sentence wraps**, and the sibling tests above make the same concession
        // for the same reason: what is asserted is that the reason is painted, not that it fits a row.
        assert!(
            painted.join(" ").contains("went back to them"),
            "the console's own row does not say the person took it back: {painted:?}"
        );
    }

    /// **A file that goes over the ceiling is not painted, but does not go quiet either.** The ceiling is
    /// what prevents an outside `panel.json` from costing a whole frame to interpret; that it also left
    /// the console indistinguishable from one with no producer would be the failure this milestone comes
    /// to avoid, only caused by the defense itself.
    #[test]
    fn a_panel_that_goes_over_the_ceiling_is_not_painted_but_says_why() {
        let dir = PanelDir::create().expect("create the directory");
        let filler = "x".repeat(panel_contract::MAX_BYTES as usize);
        publish(&dir, &format!(r#"{{"lines":["{filler}"]}}"#));
        let mut state = PanelState::default();
        state.refresh(dir.path());

        assert!(
            state.is_visible(SystemTime::now()),
            "there is a producer publishing something huge and the pane looks as if there were none"
        );
        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            painted
                .iter()
                .any(|row| row.contains("no panel") && row.contains("KiB")),
            "it does not say why there is no panel: {painted:?}"
        );
        assert!(
            !painted.iter().any(|row| row.contains("xxxx")),
            "content was painted from a file that goes over the ceiling: {painted:?}"
        );
    }

    /// **The line that holds the milestone's promise**: a console whose producer has not published yet
    /// looks exactly like it did before the panel existed. That the file is not there is the normal
    /// startup state, not a breakdown to announce.
    #[test]
    fn a_console_with_no_file_shows_nothing_and_warns_of_nothing() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        state.refresh(dir.path());

        assert!(
            !state.is_visible(SystemTime::now()),
            "a console with no producer narrows to show a panel that does not exist"
        );
        assert!(state.warning(SystemTime::now()).is_none());
    }

    /// But if there **was** a panel and its file disappears, that is said: the producer deleted it or
    /// someone cleaned up underneath, and what is still shown no longer has backing on disk.
    #[test]
    fn that_the_file_disappears_afterward_is_confessed() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"lines":["I was here"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());

        std::fs::remove_file(dir.path().join(PANEL_FILE)).expect("delete the panel");
        state.refresh(dir.path());

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            painted.iter().any(|row| row.contains("no longer there")),
            "the file was deleted and the panel carries on as if nothing happened: {painted:?}"
        );
    }

    /// **`header: false` turns off the identity line, not the application's honesty.** If it turned it
    /// off, a producer would only have to set it for its broken panel not to look it.
    #[test]
    fn the_warning_comes_out_even_if_the_producer_turns_off_the_header() {
        let mut state = published_ago(
            r#"{"header":false,"stale_after_minutes":1,"lines":["without a header"]}"#,
            Duration::from_secs(60 * 60),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            painted.iter().any(|row| row.contains("no news")),
            "turning off the header hid the app's warning: {painted:?}"
        );
        assert!(
            !painted.iter().any(|row| row.contains("a1b2")),
            "`header: false` has to keep hiding the identity: {painted:?}"
        );
    }

    /// **The warning enters the stored layout's key.** A panel that ages changes what has to be painted
    /// without its file being touched, so a cache that only looked at the width would keep showing the
    /// layout from when it was fresh and the warning would never appear.
    #[test]
    fn aging_throws_away_the_layout_even_if_the_file_does_not_change() {
        let mut state = published_ago(
            r#"{"stale_after_minutes":10,"lines":["the same as always"]}"#,
            Duration::from_secs(60),
        );

        let fresh = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        let stale = texts(paint_the_state(
            &mut state,
            PLACE,
            SystemTime::now() + Duration::from_secs(30 * 60),
        ));

        assert!(!fresh.iter().any(|row| row.contains("no news")));
        assert!(
            stale.iter().any(|row| row.contains("no news")),
            "the panel aged and the layout from when it was fresh kept being painted: {stale:?}"
        );
    }

    /// **What the heartbeat needs to know**: how long until it is time to warn. Without this the window
    /// would not know when to wake up, and with the console quiet the warning would never arrive.
    #[test]
    fn how_long_until_it_is_time_to_warn_is_known() {
        let state = published_ago(
            r#"{"stale_after_minutes":10,"lines":["something"]}"#,
            Duration::from_secs(4 * 60),
        );

        let missing = state
            .how_long_until_warning(SystemTime::now())
            .expect("it is not old yet, so there is something to wait for");

        // Six minutes, with slack for how long the test itself takes.
        assert!(
            missing.as_secs() > 5 * 60 && missing.as_secs() <= 6 * 60,
            "{} s until warning about a panel from 4 min ago with a deadline of 10",
            missing.as_secs()
        );
    }

    /// **And a panel that is already old still asks for an alarm, because its warning carries a number
    /// that advances.** Without this, the agent that dies at 10:00 with a deadline of five minutes leaves
    /// on screen «no news for 5 min», and at 13:00 it still says five: right about the qualitative and
    /// lying about the datum that serves to decide whether it is dead or only slow.
    #[test]
    fn an_already_old_panel_wakes_again_when_its_number_changes() {
        let state = published_ago(
            r#"{"stale_after_minutes":1,"lines":["something"]}"#,
            Duration::from_secs(45 * 60 + 40),
        );

        let missing = state.how_long_until_warning(SystemTime::now()).expect(
            "the warning's number is going to change, so there is something to come back for",
        );

        // At the next minute, not in a minute: what matters is when the number ticks over.
        // Twenty seconds of margin between moving the date and asking: the narrowest of the suite was ten,
        // and it is the one that would fall first on a saturated machine.
        assert!(
            missing.as_secs() > 10 && missing.as_secs() <= 20,
            "it comes back in {} s, and the number has 20 to go before it ticks",
            missing.as_secs()
        );
    }

    /// When hours are said, it comes back at the next hour and not every minute: the number only changes
    /// when it changes, and every change costs waking the window.
    #[test]
    fn with_hours_of_silence_it_wakes_once_per_hour() {
        let state = published_ago(
            r#"{"stale_after_minutes":1,"lines":["something"]}"#,
            Duration::from_secs(5 * 60 * 60 + 10 * 60),
        );

        let missing = state
            .how_long_until_warning(SystemTime::now())
            .expect("the number keeps advancing");

        assert!(
            missing.as_secs() > 45 * 60,
            "with five hours of silence it wakes at {} s, which is once per minute",
            missing.as_secs()
        );
    }

    /// With no panel there is nothing to wait for.
    #[test]
    fn with_no_panel_no_alarm_is_asked_for() {
        assert!(
            PanelState::default()
                .how_long_until_warning(SystemTime::now())
                .is_none()
        );
    }

    /// **A passing failure cannot accuse the producer forever.** That the re-read says «what you have is
    /// still valid» means the file could be asked about and that its date is that of the good panel
    /// already held: the stumble from a while ago —running out of descriptors, a network mount that
    /// flickers— no longer describes anything. Without clearing it, the warning stayed stuck for life,
    /// because a file that nobody touches again never changes its date.
    #[test]
    fn a_passing_failure_does_not_leave_the_panel_accused_forever() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"lines":["the real latest"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());

        // The file becomes illegible for an instant and then recovers, without changing its date.
        let path = dir.path().join(PANEL_FILE);
        let date = std::fs::metadata(&path)
            .expect("look at the date")
            .modified()
            .expect("there is a date");
        std::fs::write(&path, "broken").expect("break it");
        std::fs::File::open(&path)
            .expect("open")
            .set_modified(date + Duration::from_secs(1))
            .expect("move the date");
        state.refresh(dir.path());
        publish(&dir, r#"{"lines":["the real latest"]}"#);
        std::fs::File::open(&path)
            .expect("open")
            .set_modified(date)
            .expect("restore the date");

        state.refresh(dir.path());

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            !painted.iter().any(|row| row.contains("not the latest")),
            "the file reads fine and its panel is still accused: {painted:?}"
        );
    }

    /// With a deadline under a minute —which the contract allows— the warning **does not say a number**:
    /// «no news for 0 min» a second after publishing reads as an application failure.
    #[test]
    fn with_a_deadline_of_zero_the_warning_does_not_say_zero_minutes() {
        let mut state = published_ago(
            r#"{"stale_after_minutes":0,"lines":["something"]}"#,
            Duration::from_secs(5),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            painted.iter().any(|row| row.contains("no news")),
            "with a deadline of zero the panel is old from the first second: {painted:?}"
        );
        assert!(
            !painted.iter().any(|row| row.contains("0 min")),
            "the warning says «0 min ago», which reads as a failure: {painted:?}"
        );
    }

    /// What a cell measures with the font the panel paints with.
    ///
    /// It is needed to assert about positions: the margin and the identity gap are counted in cells, so
    /// comparing them with a hand-written number would be checking this machine's font instead of the
    /// panel's arithmetic.
    fn cell() -> (f32, f32) {
        let ctx = eframe::egui::Context::default();
        let mut measured = (0.0, 0.0);
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrics = CellMetrics::measure(
                ui.ctx(),
                eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE),
            );
            measured = (metrics.width(), metrics.height());
        })
        .drop_without_applying_deltas();
        measured
    }

    /// Paints a real panel and returns **what got queued**: the text of each row and its color.
    ///
    /// Without this, everything that happens inside `paint` is invisible to the suite: the style can stop
    /// being read, everything painted the same color or **nothing painted at all**, and the
    /// `Theme::panel_color` tests stay green because they assert about the helper, not about the path.
    fn what_gets_painted(file: &str) -> Vec<(String, Color32)> {
        texts_and_colors(what_gets_queued(file, PLACE))
    }

    /// The same, for when the test needs to paint at a specific moment and cannot go through the file.
    fn texts_and_colors(shapes: Vec<eframe::egui::Shape>) -> Vec<(String, Color32)> {
        shapes
            .into_iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Text(text) => {
                    Some((text.galley.text().to_owned(), text.fallback_color))
                }
                _ => None,
            })
            .collect()
    }

    /// Where each text row falls, in the order they were queued.
    fn where_each_row_falls(file: &str, place: Rect) -> Vec<(Pos2, String)> {
        what_gets_queued(file, place)
            .into_iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Text(text) => Some((text.pos, text.galley.text().to_owned())),
                _ => None,
            })
            .collect()
    }

    /// **Each row in its own line, and the gap of the empty string counted.**
    ///
    /// The test below looks at the content and the order of the shape queue, and that **does not tie down
    /// where they fall**: removing the height advance, the three lines are painted one on top of another
    /// in the same line and the suite stayed green. It is the consequence AC18 names —«does not paint over
    /// the following lines»— and the half of AC2 that says «in order».
    ///
    /// The gap is checked **by the distance**, which is the only way to see it: an empty string does not
    /// queue any shape, so what proves it left its spot is that between its neighbors there are two lines
    /// and not one.
    #[test]
    fn each_row_falls_in_its_own_line_and_the_empty_one_leaves_its_own() {
        let (_, height) = cell();

        let rows = where_each_row_falls(
            r#"{"header":false,"lines":["the first","","the third"]}"#,
            PLACE,
        );

        assert_eq!(
            rows.len(),
            2,
            "the empty one queues no shape, the others do"
        );
        assert_eq!(rows[0].0.y, PLACE.top(), "the first starts at the top");
        assert_eq!(
            rows[1].0.y - rows[0].0.y,
            height * 2.0,
            "between the first and the third two lines have to fit: the gap is one of them"
        );
    }

    #[test]
    fn each_line_is_painted_in_its_order_and_the_empty_ones_leave_their_gap() {
        let rows = what_gets_painted(r#"{"header":false,"lines":["the first","","the third"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            ["  the first", "  the third"],
            "the gap is not painted, but the two there are go in order"
        );
    }

    /// **The color comes from the real path, not from the helper.** With this test, ignoring `line.style`
    /// in `paint` turns red; without it, the suite stayed green with the whole panel monochrome.
    #[test]
    fn each_line_color_comes_from_its_style_and_its_mark() {
        let theme = Theme::default();
        let rows = what_gets_painted(
            r#"{"header":false,"lines":[
                {"text":"a failure","style":"error"},
                {"text":"blocked","mark":"block"},
                {"text":"and its color is the data","mark":"info","style":"ok"},
                {"text":"without adjectives"}
            ]}"#,
        );

        assert_eq!(
            rows,
            vec![
                (
                    "  a failure".to_owned(),
                    theme.panel_color(None, Some(panel_contract::Style::Error))
                ),
                (
                    "× blocked".to_owned(),
                    theme.panel_color(Some(panel_contract::Mark::Block), None)
                ),
                (
                    "› and its color is the data".to_owned(),
                    theme.panel_color(
                        Some(panel_contract::Mark::Info),
                        Some(panel_contract::Style::Ok)
                    )
                ),
                (
                    "  without adjectives".to_owned(),
                    theme.panel_color(None, None)
                ),
            ]
        );
    }

    /// The identity line is written by the application with what it already knows: the pane's identifier
    /// and the phase the producer publishes. Nothing from a neighboring file.
    #[test]
    fn the_header_carries_the_panes_identity_and_its_phase() {
        let rows = what_gets_painted(r#"{"phase":"build","lines":["the body"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["a1b2 · build", "  the body"]);
    }

    /// A panel driven over the channel, painted.
    fn what_a_held_panel_paints(
        source: &str,
        document: &str,
        and_then: impl Fn(&mut PanelState),
    ) -> Vec<String> {
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                source,
                panel_contract::parse_document(document).expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("nobody was holding it");
        and_then(&mut state);

        texts_and_colors(paint_the_state(&mut state, PLACE, SystemTime::now()))
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }

    /// **Who is driving is on the identity row**, so a person can see it without editing anything to
    /// find out. Asserted on the whole row rather than with a `contains`: the source has to sit
    /// beside the pane's name and its phase, and a `contains` would pass with it painted anywhere.
    #[test]
    fn a_panel_somebody_is_driving_says_so_on_its_identity_row() {
        let rows =
            what_a_held_panel_paints("flow", r#"{"phase":"build","lines":["the body"]}"#, |_| {});

        assert_eq!(rows[0], "a1b2 · build · flow");
    }

    /// **AC9's other half**: a file publication refused while the panel is held is confessed, and the
    /// sentence names the source that is driving. Without it, somebody editing `panel.json` and
    /// seeing nothing change cannot tell «held by another producer» from «this terminal is broken».
    ///
    /// **And it says nothing until the file has actually been refused.** A standing warning over a
    /// panel that is working perfectly is noise, which is what the second half of this asserts.
    #[test]
    fn a_file_refused_while_the_panel_is_held_is_confessed_and_names_who_is_driving() {
        let quiet = what_a_held_panel_paints("flow", r#"{"lines":["the agent"]}"#, |state| {
            // **A refresh with no file written, which is what a rearm, a split or the blind watcher
            // produces.** The first version passed `|_| {}`, so `refresh` was never called: it proved
            // «does not confess without refreshing», never «does not confess without a refusal» — and
            // the flag a rearm raises was exactly what latched the confession in the field.
            let dir = PanelDir::create().expect("create the directory");
            state.refresh(dir.path());
        });
        assert!(
            !quiet.iter().any(|row| row.contains("not being read")),
            "it confessed a refusal that never happened: {quiet:?}"
        );

        let refused = what_a_held_panel_paints("flow", r#"{"lines":["the agent"]}"#, |state| {
            let dir = PanelDir::create().expect("create the directory");
            // The file's line says something no sentence of the application's could contain: with
            // «the file» in it, the last assertion below matched the warning's own wording and the
            // test failed on its own prose instead of on the code.
            publish(&dir, r#"{"lines":["published by hand"]}"#);
            state.refresh(dir.path());
        });

        assert!(
            refused
                .iter()
                .any(|row| row.contains("not being read") && row.contains("flow")),
            "the refusal was not confessed with its source: {refused:?}"
        );
        assert!(
            refused.iter().any(|row| row.contains("the agent")),
            "the held panel stopped being painted: {refused:?}"
        );
        assert!(
            !refused.iter().any(|row| row.contains("published by hand")),
            "the file was painted over a held panel: {refused:?}"
        );
    }

    /// **A held panel that has gone stale says so**, which is what D43 promised and the first
    /// version's warning order made unreachable: the file refusal returned before the staleness arm,
    /// so an agent that wedged with its socket open — and a person editing `panel.json` meanwhile —
    /// lost both lines. Held and stale with no file touched says the one thing that is true.
    #[test]
    fn a_held_panel_that_has_gone_stale_says_so() {
        let mut state = PanelState::default();
        let arrived = SystemTime::now();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(
                    r#"{"stale_after_minutes":1,"attention":"wait","lines":["the agent"]}"#,
                )
                .expect("the document is a panel"),
                arrived,
            )
            .expect("nobody was holding it");

        let rows = texts_and_colors(paint_the_state(
            &mut state,
            PLACE,
            arrived + Duration::from_secs(120),
        ))
        .into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>();

        assert!(
            rows.iter().any(|row| row.contains("no news")),
            "a held panel that went stale said nothing about it: {rows:?}"
        );
    }

    /// **A wedged holder's claim is dimmed once what it says has gone stale.** An open socket is what
    /// a process that stopped working also holds, so backing the claim from the connection alone
    /// painted the tab as waiting for somebody who is not there. The warning says the freshness is
    /// gone; the claim has to agree.
    #[test]
    fn a_held_claim_that_has_gone_stale_is_not_backed() {
        let mut state = PanelState::default();
        let arrived = SystemTime::now();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(
                    r#"{"stale_after_minutes":1,"attention":"wait","lines":["the agent"]}"#,
                )
                .expect("the document is a panel"),
                arrived,
            )
            .expect("nobody was holding it");

        assert!(
            state.attention(arrived).expect("it claims").backed,
            "a fresh held claim was dimmed"
        );
        assert!(
            !state
                .attention(arrived + Duration::from_secs(120))
                .expect("it still claims")
                .backed,
            "a wedged agent's claim was painted bright"
        );
    }

    /// A claim on a held panel is **backed**, because an open connection is the most direct evidence
    /// there is that somebody is still behind it — more direct than the file's date, which is what
    /// the other arms fall back on. Without this, the refusal's own warning would dim the claim.
    #[test]
    fn a_claim_on_a_held_panel_is_not_dimmed_by_the_refusal_it_causes() {
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(r#"{"attention":"wait","lines":["ready"]}"#)
                    .expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("nobody was holding it");
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"lines":["the file"]}"#);
        state.refresh(dir.path());

        let claim = state
            .attention(SystemTime::now())
            .expect("the producer declared a claim");

        assert!(claim.backed, "a claim somebody is holding up was dimmed");
    }

    /// Retirement removes every reachable form of the producer document in one state swap: the
    /// painter, an already-cloned off-frame reader, claims and old button hitboxes must all agree.
    #[test]
    fn the_holder_retires_the_document_and_every_route_to_its_old_state() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(
                    r#"{"attention":"wait","actions":[{"label":"Run","send":"make run"}],"lines":["ready"]}"#,
                )
                .expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("nobody was holding it");
        let reader = state.reader();
        state
            .lay_out_if_needed("a1b2", 40, &Theme::default(), None, None)
            .expect("there is a layout");
        state.drawn = Some(Drawn {
            settled: true,
            buttons: vec![PaintedButton {
                action: 0,
                area: PLACE,
            }],
            links: Vec::new(),
        });
        state.problem = Some(Problem::Invalid("old failure".to_owned()));
        let result = Retirement {
            closed: 2,
            preserved: 1,
        };

        assert_eq!(
            state
                .retire(
                    7,
                    PanelState::retirement_baseline(dir.path()).expect("snapshot the file"),
                    result,
                )
                .expect("the holder may retire"),
            result
        );

        assert!(state.last_understood().is_none());
        assert_eq!(
            state.document().expect_err("there is no document"),
            driving::NOTHING_PUBLISHED
        );
        assert_eq!(
            reader
                .read()
                .expect("the pane still exists")
                .expect_err("the committed reader was cleared"),
            driving::NOTHING_PUBLISHED
        );
        assert!(state.held_by().is_none());
        assert!(state.attention(SystemTime::now()).is_none());
        assert!(state.drawn.is_none());
        assert!(state.layout.is_none());
        assert!(state.problem.is_none());
        assert!(!state.is_visible(SystemTime::now()));
    }

    #[test]
    fn a_connection_that_does_not_hold_the_panel_cannot_retire_it() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                "same name",
                panel_contract::parse_document(r#"{"lines":["kept"]}"#)
                    .expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("nobody was holding it");

        let refusal = state
            .retire(
                8,
                PanelState::retirement_baseline(dir.path()).expect("snapshot the file"),
                Retirement::default(),
            )
            .expect_err("a fresh connection has no authority");

        assert!(
            refusal.contains("same name"),
            "the holder was not named: {refusal}"
        );
        assert_eq!(state.held_by(), Some("same name"));
        assert!(
            state
                .document()
                .expect("the document survived")
                .contains("kept")
        );
    }

    #[test]
    fn a_retirement_retry_keeps_its_result_until_a_new_mcp_publication() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        let panel = || {
            panel_contract::parse_document(r#"{"lines":["one generation"]}"#)
                .expect("the document is a panel")
        };
        state
            .deliver(7, "flow", panel(), SystemTime::now())
            .expect("publish the first generation");
        let baseline = PanelState::retirement_baseline(dir.path()).expect("snapshot the file");
        let original = Retirement {
            closed: 3,
            preserved: 1,
        };
        state
            .retire(7, baseline.clone(), original)
            .expect("retire once");

        assert_eq!(
            state
                .retire(7, baseline.clone(), Retirement::default())
                .expect("the retry is idempotent"),
            original
        );

        state
            .deliver(7, "flow", panel(), SystemTime::now())
            .expect("publish a new generation");
        let next = Retirement {
            closed: 1,
            preserved: 2,
        };
        assert_eq!(
            state
                .retire(7, baseline, next)
                .expect("the new generation may retire"),
            next
        );
    }

    #[test]
    fn retirement_suppresses_that_file_generation_but_a_later_one_invalidates_the_retry() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"lines":["the old file"]}"#);
        let retired_date = std::fs::metadata(dir.path().join(PANEL_FILE))
            .and_then(|metadata| metadata.modified())
            .expect("read the retired file date");
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(r#"{"lines":["the agent"]}"#)
                    .expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("publish over the file");
        let baseline = PanelState::retirement_baseline(dir.path()).expect("snapshot the file");
        state
            .retire(7, baseline.clone(), Retirement::default())
            .expect("retire the held generation");

        state.refresh(dir.path());
        assert!(
            state.last_understood().is_none(),
            "a queued notice resurrected the retired file generation"
        );
        state
            .retire(7, baseline.clone(), Retirement::default())
            .expect("the unchanged file did not end idempotency");

        publish(&dir, r#"{"lines":["the later file"]}"#);
        std::fs::File::open(dir.path().join(PANEL_FILE))
            .expect("open the later file")
            .set_modified(retired_date)
            .expect("keep the same timestamp");
        state.refresh(dir.path());

        assert_eq!(
            state
                .last_understood()
                .expect("the later file was read")
                .lines[0]
                .text,
            "the later file"
        );
        assert!(
            state.retire(7, baseline, Retirement::default()).is_err(),
            "a successful file publication left the old retirement authoritative"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_atomic_republication_of_identical_bytes_is_a_new_file_generation() {
        let dir = PanelDir::create().expect("create the directory");
        let document = r#"{"lines":["the same file"]}"#;
        publish(&dir, document);
        let path = dir.path().join(PANEL_FILE);
        let retired_date = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .expect("read the retired file date");
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(r#"{"lines":["the agent"]}"#)
                    .expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("publish over the file");
        let baseline = PanelState::retirement_baseline(dir.path()).expect("snapshot the file");
        state
            .retire(7, baseline, Retirement::default())
            .expect("retire the held generation");

        let replacement = dir.path().join("panel.next");
        std::fs::write(&replacement, document).expect("write an atomic replacement");
        std::fs::File::open(&replacement)
            .expect("open the replacement")
            .set_modified(retired_date)
            .expect("preserve its timestamp and bytes");
        std::fs::rename(replacement, path).expect("publish the replacement atomically");
        state.refresh(dir.path());

        assert_eq!(
            state
                .last_understood()
                .expect("the replacement was read")
                .lines[0]
                .text,
            "the same file"
        );
    }

    #[cfg(unix)]
    #[test]
    fn retirement_rejects_a_named_pipe_without_blocking_the_interface() {
        let dir = PanelDir::create().expect("create the directory");
        let path = dir.path().join(PANEL_FILE);
        let planted = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("launch mkfifo");
        assert!(planted.success(), "the pipe could not be planted");

        let (finished, result) = std::sync::mpsc::channel();
        let panel_dir = dir.path().to_path_buf();
        std::thread::spawn(move || finished.send(PanelState::retirement_baseline(&panel_dir)));
        let failure = result
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("opening the pipe blocked the interface")
            .expect_err("a pipe was accepted as a panel generation");
        assert_eq!(failure.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn an_ordinary_release_preserves_the_committed_document() {
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(r#"{"lines":["evidence"]}"#)
                    .expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("publish");
        let reader = state.reader();

        assert_eq!(state.release(7), Released::Given);
        assert!(state.held_by().is_none());
        assert!(
            state
                .document()
                .expect("the document stays")
                .contains("evidence")
        );
        assert!(
            reader
                .read()
                .expect("the pane stays alive")
                .expect("the committed document stays")
                .contains("evidence")
        );
    }

    #[test]
    fn with_the_header_removed_the_identity_is_not_painted() {
        let rows = what_gets_painted(r#"{"header":false,"phase":"build","lines":["the body"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["  the body"], "the phase goes inside the identity");
    }

    /// **The sanitizing, checked on what reaches the painter and not on the loose function.**
    #[test]
    fn what_reaches_the_painter_carries_no_line_breaks() {
        let rows = what_gets_painted(r#"{"header":false,"lines":["above\nbelow"]}"#);

        assert_eq!(rows[0].0, "  above below");
    }

    /// **The cost of painting has its own worst case, and it is the inverse of interpreting's.** A single
    /// very long line is cheap to read and cost 14.6 ms of tessellation **on every frame**, because epaint
    /// discards text by row and not by glyph. Without this cap, an outside `panel.json` leaves the window
    /// over its budget forever.
    #[test]
    fn an_absurdly_long_line_is_cut_before_being_painted() {
        let long = "A".repeat(60_000);

        let rows = what_gets_painted(&format!(r#"{{"header":false,"lines":["{long}"]}}"#));

        // **The total is counted, not the first row.** With wrapping in place, the cap is not seen in a
        // loose row: the line is spread across several, and what has to be bounded is how much text is
        // laid out in total. Counting only the first would let a thousand rows of forty characters through.
        let painted: usize = rows.iter().map(|(t, _)| t.chars().count()).sum();
        // The two of margin are the symbol's column, reserved even though the line carries no mark: it is
        // what keeps all their text aligned.
        assert!(
            painted <= MAX_CHARACTERS + 2,
            "{painted} characters of a 60,000-character line were laid out"
        );
        assert!(rows.len() > 1, "and it was wrapped across several rows");
    }

    /// **The background is painted even if there is nothing to say, and it is the only thing that
    /// separates the panel from its console** — there is no line or border between the two. Without it,
    /// where the panel should show, the console's background shows, with the console already narrowed: a
    /// black gap on the right that looks like a painting failure instead of a quiet producer.
    ///
    /// And the case is legal form of the contract: empty `lines` with the header removed leaves a panel in
    /// which the background is literally all there is.
    #[test]
    fn a_panel_with_no_lines_or_header_paints_its_background_and_nothing_else() {
        let shapes = what_gets_queued(r#"{"header":false,"lines":[]}"#, PLACE);

        let backgrounds: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Rect(rect) => Some((rect.rect, rect.fill)),
                _ => None,
            })
            .collect();
        assert_eq!(
            backgrounds,
            vec![(PLACE, Theme::default().panel_background())],
            "the panel has to cover its rectangle with its own background"
        );
        assert!(
            !shapes
                .iter()
                .any(|shape| matches!(shape, eframe::egui::Shape::Text(_))),
            "with no lines and no header there is no text to paint"
        );
    }

    /// A pane can be left with no height: the window squashed against the screen edge, or the reserve
    /// rectangle while the desktop has not yet said how much it measures. Then not even one row fits, and
    /// what is checked here is that **the loop stops**: without the cut by height, rows keep being queued
    /// outside the panel, and they are shapes that have to be tessellated on every frame to not be seen. A
    /// rectangle inside out —negative height— comes in by the same path and also cannot panic.
    #[test]
    fn in_a_panel_with_no_height_no_row_is_queued() {
        let no_height = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 0.0));
        let the_other_way_round = Rect::from_min_max(Pos2::new(0.0, 100.0), Pos2::new(400.0, 0.0));

        for place in [no_height, the_other_way_round] {
            let rows = where_each_row_falls(r#"{"header":false,"lines":["one","another"]}"#, place);

            assert!(
                rows.is_empty(),
                "in {place:?} no row fits and {rows:?} were queued"
            );
        }
    }

    /// **The identity line carries a producer string inside it**: the phase. The lines' sanitizing does
    /// not reach it —this string is composed by the application—, so a `\n` in the phase would paint two
    /// rows where one goes and would cover the body's first.
    #[test]
    fn the_phase_goes_through_the_same_sanitizing_as_the_lines() {
        let rows = what_gets_painted(r#"{"phase":"build\nfake","lines":["the body"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["a1b2 · build fake", "  the body"]);
    }

    /// And the cap too: a very long phase is the same tessellation worst case as a very long line —14.6 ms
    /// per frame while it stays published— and arrives by a string the lines' cap does not touch.
    ///
    /// Along the way, the edge the contract declares: with empty `lines` the panel shows **only** its
    /// identity line.
    #[test]
    fn an_absurdly_long_phase_is_not_painted_whole() {
        let long = "F".repeat(60_000);

        let rows = what_gets_painted(&format!(r#"{{"phase":"{long}","lines":[]}}"#));

        let painted: usize = rows.iter().map(|(t, _)| t.chars().count()).sum();
        // The cap is applied to the whole identity line, prefix included, so the prefix earns no
        // allowance on top of it. It used to be granted one, counted in bytes: eight where the
        // prefix is seven characters, so the bound sat eight characters above the cap.
        assert!(
            painted <= MAX_CHARACTERS,
            "{painted} characters of a 60,000-character phase were laid out"
        );
        assert!(
            rows.iter()
                .all(|(t, _)| t.starts_with('F') || t.starts_with("a1b2")),
            "with no lines, the only thing there is is the identity wrapped: {rows:?}"
        );
    }

    /// Two painting decisions that **are seen neither in the text nor in the color**, and that until now
    /// nothing tied down: the text does not go flush against the left border —flush it reads as a painting
    /// error— and the identity leaves a blank row before the body. The second matters because the identity
    /// is written by the application and the body is written by the producer: stuck together, the app's
    /// reads as the producer's first line.
    #[test]
    fn the_text_leaves_its_margin_and_the_identity_its_blank_row() {
        let (width, height) = cell();

        let rows = where_each_row_falls(r#"{"phase":"build","lines":["the body"]}"#, PLACE);

        let [(identity, _), (body, _)] = rows.as_slice() else {
            panic!("there had to be two rows and they are {rows:?}");
        };
        // The margin cell is written here and not as `MARGIN`: an expectation that comes from the same
        // constant it describes is met on its own, whatever value it is set to.
        assert_eq!(
            identity.x,
            PLACE.left() + width,
            "the text has to start one cell from the border"
        );
        assert_eq!(body.x, identity.x, "the body does not start where it does");
        assert_eq!(
            body.y - identity.y,
            2.0 * height,
            "between the identity and the body a blank row has to remain"
        );
    }

    /// **The six symbols, asking the font and not memory.**
    ///
    /// The criterion is **the width**, not `has_glyphs`: that predicate compares the resolved face against
    /// the replacement one, and with the monospaced one the base face *is* that, so it answers «no» even
    /// for the letter `A` — the design table was taken with it and came out inverted. A glyph the
    /// monospaced face has measures what the `M` measures; one that is missing measures zero; and one that
    /// comes from the emoji fonts measures more, which is what **knocks the column out of square**.
    #[test]
    fn the_six_symbols_are_in_the_font_and_measure_one_cell() {
        let marks = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ];

        let ctx = eframe::egui::Context::default();
        let mut widths = Vec::new();
        let mut cell = 0.0;
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let font = eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE);
            ui.ctx().fonts_mut(|fonts| {
                cell = fonts.glyph_width(&font, 'M');
                widths = marks
                    .iter()
                    .map(|m| (symbol(*m), fonts.glyph_width(&font, symbol(*m))))
                    .collect();
            });
        })
        .drop_without_applying_deltas();

        for (symbol, width) in widths {
            assert!(
                (width - cell).abs() < 0.01,
                "«{symbol}» measures {width} and a cell measures {cell}: {}",
                if width == 0.0 {
                    "it is not in the font"
                } else {
                    "it comes from the emoji one and would knock the column out of square"
                }
            );
        }
    }

    /// The six distinct from each other: with two marks sharing a symbol, the panel would say less than
    /// what the producer wrote and nobody would notice.
    /// **Each mark with its own, not just «the six exist and are distinct».** Swapping `done`'s with
    /// `wait`'s the suite stayed green: a panel that painted the ellipsis on the finished and the circle
    /// on what waits would say **the opposite** of what the producer wrote, and would contradict the table
    /// the decision log publishes.
    #[test]
    fn each_mark_carries_the_symbol_the_decision_assigned_it() {
        assert_eq!(
            [
                symbol(Mark::Done),
                symbol(Mark::Current),
                symbol(Mark::Pending),
                symbol(Mark::Wait),
                symbol(Mark::Block),
                symbol(Mark::Info),
            ],
            ['●', '▶', '·', '…', '×', '›']
        );
    }

    #[test]
    fn no_mark_shares_a_symbol_with_another() {
        let all: std::collections::HashSet<char> = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ]
        .iter()
        .map(|m| symbol(*m))
        .collect();

        assert_eq!(all.len(), 6);
    }

    /// **The contract rule that makes a panel legible: the column is computed per block.**
    ///
    /// With a global width, the block above —whose reference is long— would push the text of the one
    /// below, and what is gained by aligning would be lost in wasted margin. The blocks are separated by
    /// the empty lines.
    #[test]
    fn the_reference_column_aligns_per_block_and_one_block_does_not_widen_the_other() {
        let rows = what_gets_painted(
            r##"{"header":false,"lines":[
                {"ref":"#1","text":"short"},
                {"ref":"#10-#25","text":"long"},
                "",
                {"ref":"#2","text":"another block"}
            ]}"##,
        );

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            ["  #1      short", "  #10-#25 long", "  #2 another block"],
            "the second block has to return to its own width"
        );
    }

    /// The continuation is indented **to where the text starts**, not to the border: aligned to the
    /// border, the second row would read as a new producer line and the column would stop meaning
    /// anything.
    #[test]
    fn a_long_line_wraps_with_the_continuation_under_its_text() {
        let rows = where_each_row_falls(
            r##"{"header":false,"lines":[{"ref":"#1","mark":"done","text":"words enough to not fit in the panel in any way"}]}"##,
            PLACE,
        );

        assert!(rows.len() > 1, "it had to wrap: {rows:?}");
        let (first, rest) = rows.split_first().expect("there are rows");

        // **The property is the equality, not «further right».** With only `>`, any constant nonzero
        // indent passes —including one cell, i.e. under the symbol, which is literally what the criterion
        // forbids—. What has to be asserted is that the continuation falls where the **text** of the first
        // row starts, and that spot is computed from what is already held: the x of the first plus what
        // its head takes.
        let (cell_width, _) = cell();
        // In characters, not in bytes: the mark's symbol takes three bytes and a single cell.
        let head = first.1.chars().count()
            - first
                .1
                .split_once("words")
                .expect("the first row carries its head and then the text")
                .1
                .chars()
                .count()
            - "words".chars().count();
        let under_the_text = first.0.x + head as f32 * cell_width;

        for (where_at, text) in rest {
            assert!(
                (where_at.x - under_the_text).abs() < 0.01,
                "«{text}» started at {} and the text of the first starts at {under_the_text}",
                where_at.x
            );
        }
    }

    /// A single word longer than the gap is split by characters. Without this the wrapping does not
    /// progress: the word never fits, and jumping to a row does not make it fit.
    #[test]
    fn a_word_longer_than_the_gap_is_split_instead_of_hanging() {
        let rows = what_gets_painted(&format!(
            r#"{{"header":false,"lines":["{}"]}}"#,
            "x".repeat(200)
        ));

        assert!(rows.len() > 1);
        assert!(rows.iter().all(|(t, _)| !t.is_empty()), "no empty row");
    }

    /// **With the indent eaten whole, the wrapping has to keep advancing.** Without a floor, the gap comes
    /// out zero and the loop puts no character in per turn.
    ///
    /// **Its failure mode is hanging, not turning red**, and that has to be known before seeing it: with
    /// `MINIMUM_SLOT` at zero this test does not fail, it stays spinning and the whole suite dies of
    /// memory. Whoever sees it hung is seeing the failure, not a broken test. It is the same mode as the
    /// one of the named pipe in the contract crate.
    #[test]
    fn an_indent_larger_than_the_panel_does_not_hang_the_wrapping() {
        let rows = what_gets_painted(
            r#"{"header":false,"lines":[{"indent":64,"text":"this does not fit in any way"}]}"#,
        );

        assert!(!rows.is_empty(), "something has to be painted");
        assert!(rows.iter().all(|(t, _)| !t.is_empty()));
    }

    /// **What does not fit by height is said, not discarded silently.** A panel that is clipped quietly is
    /// a summary that lies by omission, which is exactly what the contract forbids the producer.
    #[test]
    fn when_they_do_not_all_fit_the_last_row_says_how_many_are_missing() {
        let (_, height) = cell();
        let four_rows =
            Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, height * 4.0));

        let rows: Vec<String> = where_each_row_falls(
            r#"{"header":false,"lines":["one","two","three","four","five","six"]}"#,
            four_rows,
        )
        .into_iter()
        .map(|(_, text)| text)
        .collect();

        assert_eq!(rows.len(), 4, "four lines, four rows");
        assert_eq!(
            rows.last().expect("there are rows"),
            "… 3 more",
            "and the last one counts them, including the one the warning itself displaces"
        );
    }

    /// The layout is remembered as long as the file and the width do not change: while the console beside
    /// it dumps, the window repaints sixty times per second and wrapping the panel cannot be paid each
    /// time.
    #[test]
    fn the_layout_is_remembered_and_thrown_away_when_the_width_changes() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"header":false,"lines":["something to wrap"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let first = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("there is a panel")
            .0
            .to_vec();
        let repeated = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("there is a panel")
            .0
            .to_vec();
        assert_eq!(first, repeated, "the same width gives the same layout");

        let narrow = state
            .lay_out_if_needed("a1b2", 12, &theme, None, None)
            .expect("there is a panel")
            .0
            .to_vec();
        assert_ne!(
            first, narrow,
            "with another width it has to be wrapped again"
        );
    }

    /// And it is thrown away on publishing: a layout that survives its file shows the previous forever.
    #[test]
    fn the_layout_is_thrown_away_when_the_producer_publishes() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"header":false,"lines":["the old"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();
        state.lay_out_if_needed("a1b2", 40, &theme, None, None);

        publish(&dir, r#"{"header":false,"lines":["the new"]}"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        let rows = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("there is a panel");
        assert!(
            rows.0.iter().any(|row| row.text.contains("the new")),
            "the layout kept the old: {rows:?}"
        );
    }

    /// **A patch that lands ages the panel and repaints it; one that is refused does neither.**
    ///
    /// Both halves had a claim in a docblock and no test. Without the stamp, a producer keeping a
    /// panel alive by patching would never go stale — the failure the notice exists to expose. Without
    /// the layout being dropped, the next pass paints the text from before the patch while a read
    /// over the channel hands back the text after it, and every assertion made through that channel
    /// still passes. **Asserted on what gets painted**, not on the cache field, for the same reason
    /// the publishing test above is.
    #[test]
    fn a_patch_ages_the_panel_and_repaints_it_and_a_refusal_does_neither() {
        let theme = Theme::default();
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                "flow",
                panel_contract::parse_document(r#"{"lines":[{"key":"one","text":"the old"}]}"#)
                    .expect("the document is a panel"),
                SystemTime::now(),
            )
            .expect("nobody was holding it");
        state.lay_out_if_needed("a1b2", 40, &theme, None, None);
        let published = state
            .last_understood()
            .expect("there is a panel")
            .written_at
            .expect("it was stamped on arrival");

        let ops = panel_contract::parse_ops(
            &serde_json::from_str(
                r#"[{"op":"replace","key":"one","line":{"key":"one","text":"the new"}}]"#,
            )
            .expect("the ops are JSON"),
        )
        .expect("the ops are read");
        state
            .patch(7, &ops, published + Duration::from_secs(1))
            .expect("the holder may patch");

        let rows = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("there is a panel");
        assert!(
            rows.0.iter().any(|row| row.text.contains("the new")),
            "the layout survived the patch, so the pane paints what the patch replaced: {rows:?}"
        );
        let after = state
            .last_understood()
            .expect("there is a panel")
            .written_at
            .expect("a patch stamps the date");
        assert!(
            after > published,
            "the patch left the panel ageing from the publication"
        );

        // And a refusal stamps nothing: a producer that fails to patch for an hour still goes stale.
        let nowhere = panel_contract::parse_ops(
            &serde_json::from_str(r#"[{"op":"delete","key":"nobody"}]"#).expect("the ops are JSON"),
        )
        .expect("the ops are read");
        state
            .patch(7, &nowhere, after + Duration::from_secs(600))
            .expect_err("a key that names nothing refuses");
        assert_eq!(
            state
                .last_understood()
                .expect("there is a panel")
                .written_at,
            Some(after),
            "a refused patch restamped the date, so a silent producer would never go stale"
        );
    }

    /// The reference is an outside string like the text, and since this PR is painted: a `\n` inside would
    /// knock the whole column out of square.
    #[test]
    fn the_reference_goes_through_the_same_sanitizing_as_the_text() {
        let rows = what_gets_painted(r#"{"header":false,"lines":[{"ref":"a\nb","text":"t"}]}"#);

        assert_eq!(rows[0].0, "  a b t");
    }

    /// **Nothing that comes from outside can hang or blow up the wrapping**, and this is the place with
    /// the most index arithmetic in the whole panel: there is a cut by bytes over an index that comes from
    /// walking characters, and a loop with two exits.
    #[test]
    fn the_wrapping_withstands_anything_without_panic_or_hang() {
        let many_tildes = "ñ".repeat(300);
        let cases = [
            "",
            " ",
            "                    ",
            "a",
            many_tildes.as_str(),
            "🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂",
            "e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}",
            "a short word",
            "averylongwordwithnospacesthatfitsinnogap",
            "मानक हिन्दी",
            "\u{200b}\u{200b}\u{200b}",
        ];

        for width in [0, 1, 2, 7, 8, 9, 40] {
            for case in cases {
                let mut rows = Vec::new();
                wrap(&mut rows, 0, case, Color32::WHITE, width);
                wrap(
                    &mut rows,
                    width.saturating_add(5),
                    case,
                    Color32::WHITE,
                    width,
                );
                assert!(
                    rows.len() < 5_000,
                    "with width {width} and «{case}» {} rows came out: the wrapping does not progress",
                    rows.len()
                );
            }
        }
    }

    /// **A legal contract indent cannot paint outside the panel.** The contract admits up to 64 cells and
    /// the panel measures 36: without bounding it against the width, the continuation was painted 42 cells
    /// from the border —clipped, i.e. invisible— and **`clip` counted it as a visible row**, so it ate
    /// rows and the «… N more» lied. A long reference reaches the same place.
    #[test]
    fn a_legal_contract_indent_does_not_take_the_text_out_of_the_panel() {
        let width = 36;
        let file =
            r#"{"lines":[{"indent":40,"text":"a long phrase that does not fit in any way"}]}"#;

        let rows = lay_out(
            Some(&panel_with(file)),
            "a1b2",
            width,
            &Theme::default(),
            None,
            None,
            None,
        );

        for row in &rows {
            assert!(
                row.column + row.text.chars().count() <= width,
                "«{}» starts at {} and measures {}: it runs off a panel of {width}",
                row.text,
                row.column,
                row.text.chars().count()
            );
        }
    }

    /// And by the other path: a very long reference pushes the head just like the indent.
    #[test]
    fn a_very_long_reference_does_not_take_the_text_out_of_the_panel_either() {
        let width = 36;
        let long = "R".repeat(50);
        let file = format!(r##"{{"lines":[{{"ref":"{long}","text":"something to say"}}]}}"##);

        let rows = lay_out(
            Some(&panel_with(&file)),
            "a1b2",
            width,
            &Theme::default(),
            None,
            None,
            None,
        );

        for row in &rows {
            assert!(
                row.column <= width,
                "a continuation started at column {} of a panel of {width}",
                row.column
            );
        }
    }

    /// **A line with a mark and no text is legal form, and came out displaced by its own symbol.** The
    /// empty-text branch of the wrapping returns its indent —correct for a continuation, not for the row
    /// that carries the head stuck to it—, so the symbol and the reference appeared five cells to the
    /// right of those of the line above, in the same block. It broke the column, which is exactly what
    /// this PR delivers.
    #[test]
    fn a_line_with_a_mark_and_no_text_is_not_displaced() {
        let rows = lay_out(
            Some(&panel_with(
                r##"{"header":false,"lines":[
                    {"mark":"done","ref":"#1","text":"with text"},
                    {"mark":"block","ref":"#2","text":""}
                ]}"##,
            )),
            "a1b2",
            36,
            &Theme::default(),
            None,
            None,
            None,
        );

        assert_eq!(
            rows[0].column, rows[1].column,
            "the two start in the same column or the column means nothing: {rows:?}"
        );
        assert_eq!(rows[0].column, 0);
    }

    /// **The head does not go through the wrapping, so it has to be bounded separately.** A reference
    /// longer than the panel —the contract puts no ceiling on it— pulled the whole first row outside the
    /// rectangle, and with it all its continuations: they were painted over the console.
    #[test]
    fn a_reference_longer_than_the_panel_does_not_overflow_the_first_row() {
        let width = 36;
        let long = "R".repeat(48);

        let rows = lay_out(
            Some(&panel_with(&format!(
                r##"{{"header":false,"lines":[{{"ref":"{long}","text":"any old text"}}]}}"##
            ))),
            "a1b2",
            width,
            &Theme::default(),
            None,
            None,
            None,
        );

        for row in &rows {
            assert!(
                row.column + row.text.chars().count() <= width,
                "«{}» starts at {} and measures {}: it runs off a panel of {width}",
                row.text,
                row.column,
                row.text.chars().count()
            );
        }
    }

    /// **The contract's indent is respected**, and nothing tied it down: ignoring it entirely left the
    /// suite green. It is a field the producer writes to group visually, so losing it in silence takes a
    /// tool from it without saying so.
    #[test]
    fn the_indent_the_producer_asks_for_displaces_its_line() {
        let rows = lay_out(
            Some(&panel_with(
                r#"{"header":false,"lines":[{"text":"flush"},{"indent":4,"text":"indented"}]}"#,
            )),
            "a1b2",
            36,
            &Theme::default(),
            None,
            None,
            None,
        );

        let flush = rows[0].text.find("flush").expect("it is there");
        let indented = rows[1].text.find("indented").expect("it is there");
        assert_eq!(
            indented - flush,
            4,
            "the second has to start four cells further right: {rows:?}"
        );
    }

    /// **It cuts by words, not by characters.** Cutting always by characters the panel still fits and no
    /// test fell, but the words end up split in half and the panel stops being readable at a glance, which
    /// is what this PR is about.
    #[test]
    fn the_wrapping_splits_by_words_while_it_can() {
        let rows = lay_out(
            Some(&panel_with(
                r#"{"header":false,"lines":["alfa beta gamma delta epsilon zeta"]}"#,
            )),
            "a1b2",
            16,
            &Theme::default(),
            None,
            None,
            None,
        );

        for row in &rows {
            let text = row.text.trim();
            assert!(
                text.split_whitespace().all(|word| {
                    ["alfa", "beta", "gamma", "delta", "epsilon", "zeta"].contains(&word)
                }),
                "«{text}» carries a split word: {rows:?}"
            );
        }
    }

    /// **The PR's headline, tied down.** The test next door compares two equal layouts, and that holds
    /// just the same recalculating from scratch. What has to be asserted is that **it is not
    /// recalculated**, and it is seen in the identity of the borrow: if it were re-laid-out, the vector's
    /// address would change.
    #[test]
    fn the_layout_is_not_recalculated_when_it_is_already_good() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, r#"{"header":false,"lines":["something to wrap"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let first = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("there is a panel")
            .0
            .as_ptr();
        let second = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("there is a panel")
            .0
            .as_ptr();

        assert_eq!(
            first, second,
            "it was laid out again without either the file or the width changing"
        );
    }

    /// The warning of what is missing goes **below** what does fit, not over the first row.
    #[test]
    fn the_warning_of_what_is_missing_goes_at_the_end_and_with_its_margin() {
        let (cell_width, height) = cell();
        let three_rows =
            Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, height * 3.0));

        let rows = where_each_row_falls(
            r#"{"header":false,"lines":["one","two","three","four"]}"#,
            three_rows,
        );

        let (where_at, text) = rows.last().expect("there are rows");
        assert_eq!(text, "… 2 more");
        assert!(
            where_at.y > rows[0].0.y,
            "the warning was painted over the first row"
        );
        assert!(
            (where_at.x - (three_rows.left() + cell_width)).abs() < 0.01,
            "the warning started at {} and the margin leaves the text at {}",
            where_at.x,
            three_rows.left() + cell_width
        );
    }

    /// **What closes a block is a real gap, not «a line without text».** A line with a mark and a
    /// reference but no text is legal form, and treating it as a gap splits the block in two: its
    /// reference stops counting for the column's width, and the one above is left with a column narrower
    /// than its neighbor needs. It breaks exactly what this PR delivers.
    #[test]
    fn a_line_with_a_mark_and_no_text_does_not_close_the_block() {
        // **Three** lines are needed for it to show: the one without text enters the block before closing
        // it, so with two the width comes out the same either way. With a third behind, if the middle one
        // closed the block, its long reference would not reach the first.
        let rows = lay_out(
            Some(&panel_with(
                r##"{"header":false,"lines":[
                    {"mark":"done","ref":"#1","text":"above"},
                    {"mark":"block","ref":"#22222"},
                    {"mark":"info","ref":"#3","text":"below"}
                ]}"##,
            )),
            "a1b2",
            36,
            &Theme::default(),
            None,
            None,
            None,
        );

        let column_of = |row: &Row, word: &str| {
            row.text.chars().count()
                - row
                    .text
                    .split_once(word)
                    .expect("carries its text")
                    .1
                    .chars()
                    .count()
                - word.chars().count()
        };
        assert_eq!(
            column_of(&rows[0], "above"),
            column_of(&rows[2], "below"),
            "the three are a single block: the middle one closes nothing, {rows:?}"
        );
        assert_eq!(
            column_of(&rows[0], "above"),
            "● #22222 ".chars().count(),
            "and the width is fixed by the longest reference of the three"
        );
    }

    #[test]
    fn with_no_file_there_is_no_panel_to_paint() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();

        state.refresh(dir.path());

        assert!(state.last_understood().is_none());
    }

    #[test]
    fn what_is_published_is_read_and_left_ready_to_paint() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        publish(&dir, r#"{"phase":"build","lines":["one","another"]}"#);

        state.refresh(dir.path());

        let panel = state.last_understood().expect("there is a panel");
        assert_eq!(panel.lines.len(), 2);
        assert_eq!(panel.phase.as_deref(), Some("build"));
    }

    /// **The brake, checked by the effect and not by a counter.** Something new is published leaving the
    /// file's date where it was: if it were re-read all the same, the panel would change. Since it is not
    /// re-read, the one from before stays — which is what is wanted, because the signal that there is
    /// something new is the date and that did not move.
    #[test]
    fn it_does_not_read_again_if_the_files_date_has_not_changed() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        publish(&dir, r#"{"lines":["the first"]}"#);
        state.refresh(dir.path());
        let date = state
            .last_understood()
            .and_then(|panel| panel.written_at)
            .expect("the read brings the file's date");

        publish(&dir, r#"{"lines":["the second"]}"#);
        std::fs::File::open(dir.path().join(PANEL_FILE))
            .expect("open")
            .set_modified(date)
            .expect("restore the date to where it was");
        state.refresh(dir.path());

        assert_eq!(
            state
                .last_understood()
                .expect("there is still a panel")
                .lines[0]
                .text,
            "the first",
            "it re-read a file whose date had not changed"
        );
    }

    #[test]
    fn with_the_date_moved_it_does_read_again() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        publish(&dir, r#"{"lines":["the first"]}"#);
        state.refresh(dir.path());

        publish(&dir, r#"{"lines":["the second"]}"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        assert_eq!(
            state.last_understood().expect("there is a panel").lines[0].text,
            "the second"
        );
    }

    /// The producer clobbers the whole file, so a read can land right in the middle. Without keeping the
    /// last good one, the panel would flicker to empty on **every** publication.
    #[test]
    fn a_half_written_write_does_not_erase_the_last_good_panel() {
        let dir = PanelDir::create().expect("create the directory");
        let mut state = PanelState::default();
        publish(&dir, r#"{"lines":["the good one"]}"#);
        state.refresh(dir.path());

        publish(&dir, r#"{"lines":[{"text":"half wr"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        assert_eq!(
            state
                .last_understood()
                .expect("the last good one is still there")
                .lines[0]
                .text,
            "the good one"
        );
    }

    /// **A line break inside an outside string paints several rows over the following ones.** It is
    /// measured in epaint's source, not assumed: `break_on_newline` comes set by default. Without
    /// sanitizing, a `panel.json` with a `\n` in its text covers the rest of the panel.
    #[test]
    fn a_line_break_in_the_text_cannot_paint_two_rows() {
        let sanitized = sanitize("first\nsecond");

        assert_eq!(sanitized, "first second");
        assert!(!sanitized.contains('\n'));
    }

    /// The tab is the other case: it advances the width of several cells while the width measure counts it
    /// as zero, so the column is knocked out of square without anything announcing it.
    #[test]
    fn tabs_and_the_carriage_return_do_not_survive_either() {
        assert_eq!(sanitize("one\ttwo\r\nthree"), "one two  three");
        assert_eq!(
            sanitize("\u{1b}[31mred"),
            " [31mred",
            "and the escape sequences"
        );
    }

    /// They are replaced with a space and not deleted: two words separated by a line break stuck together
    /// would read as one word the producer never wrote.
    #[test]
    fn what_is_sanitized_does_not_glue_words_that_were_separate() {
        assert_eq!(sanitize("hello\nworld"), "hello world");
    }

    #[test]
    fn normal_text_passes_through_intact() {
        let as_is = "F1.6: the panel · 35,392 tokens — 18 %";
        assert_eq!(sanitize(as_is), as_is);
    }

    /// **The predicate is the contract's now, not a second list.** The control predicate missed the
    /// format characters painted as nothing (U+200B), the direction overrides that paint the rest
    /// backwards (U+202E), and the line separators — so text arriving over the wire, a `source` name
    /// above all, could reorder or spoof the row a person reads while the bytes are the ones that
    /// were sent.
    #[test]
    fn the_scalars_that_are_not_shown_as_themselves_do_not_survive() {
        for hidden in ['\u{200b}', '\u{202e}', '\u{0890}', '\u{2028}'] {
            assert_eq!(
                sanitize(&format!("a{hidden}b")),
                "a b",
                "{hidden:?} survived the sanitizing"
            );
        }
    }

    /// **What the gesture of narrowing the panel makes reachable.** Before its border could be dragged, a
    /// panel of one or two cells only appeared in an absurdly narrow window; now you reach there with a
    /// gesture, and the drag's floor leaves exactly that case.
    ///
    /// **Measured, and the number is what decides:** a row's head —the symbol's slot, the column of the
    /// reference and its separator— takes **seven columns that cannot be compressed**, so with the panel
    /// below that there is no legible text that fits. What this test ties down is that that case **breaks
    /// nothing**: it lays out and clips without panic. That it also is not painted outside the panel is
    /// guaranteed by the painter's clipper, which belongs to the caller.
    ///
    /// What is **not** fixed here, and is said: between one and seven cells the panel is a strip with no
    /// information. It is the question D10 left open —what stops being painted in a very narrow panel— and
    /// its answer changes with folding, not with this.
    ///
    /// **And what this test does NOT assert, so that nobody reads it as stronger than it is.** It carried a
    /// check that the warning's count added up and it was removed on mutating it: `visible + leftover` is
    /// equal to the total with the good clip **and with the broken one**, because the two versions spread
    /// the same rows between the two numbers. The clip is tied down by its own tests; here what is proven
    /// is that the path is walked whole without breaking.
    #[test]
    fn with_the_panel_at_the_minimum_it_lays_out_and_clips_without_panic() {
        let theme = Theme::default();
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r##"{"lines":[{"ref":"#12","text":"a long line that does not fit in any way","mark":"current","indent":8},{"text":"another"},{"text":"and another"}]}"##,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());

        for width in [0usize, 1, 2, 3] {
            let rows = state
                .lay_out_if_needed("probe", width, &theme, None, None)
                .expect("there is a panel to lay out");
            assert!(
                !rows.0.is_empty(),
                "with a panel of {width} cells no row was laid out"
            );
            clip(rows.0, 1);
        }
    }

    /// **The three claim symbols, asking them of the font and not of memory**, with the same criterion as
    /// those of the body: what an `M` measures. A glyph the monospaced face does not have measures zero
    /// and comes out as a box; one that comes from the emoji fonts measures almost double and **knocks the
    /// column out of square**, which reads worse still because it does not look like a font failure but
    /// like a badly aligned panel. The obvious candidates of this vocabulary already fell that way.
    #[test]
    fn the_three_claim_symbols_measure_one_cell() {
        let ctx = eframe::egui::Context::default();
        let mut widths = Vec::new();
        let mut cell = 0.0;
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let font = eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE);
            ui.ctx().fonts_mut(|fonts| {
                cell = fonts.glyph_width(&font, 'M');
                widths = Attention::ALL
                    .iter()
                    .map(|class| {
                        let symbol = attention_symbol(*class);
                        (symbol, fonts.glyph_width(&font, symbol))
                    })
                    .collect();
            });
        })
        .drop_without_applying_deltas();

        for (symbol, width) in widths {
            assert!(
                (width - cell).abs() < 0.01,
                "«{symbol}» measures {width} and a cell measures {cell}: {}",
                if width == 0.0 {
                    "it is not in the font"
                } else {
                    "it comes from the emoji one and would knock the column out of square"
                }
            );
        }
    }

    /// **None of the three is that of a body mark**, and it is not cosmetic: the same sign in the two
    /// places invites reading that the application deduced one from the other, which is exactly what the
    /// contract forbids. With the set repeated, a panel with `mark: block` in a line and no claim would
    /// look the same as one that claims.
    #[test]
    fn each_claim_carries_its_symbol_and_none_is_repeated_with_the_bodys() {
        assert_eq!(
            [
                attention_symbol(Attention::Wait),
                attention_symbol(Attention::Block),
                attention_symbol(Attention::Done),
            ],
            ['?', '⊘', '∎']
        );

        let of_the_body = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ]
        .map(symbol);
        for class in Attention::ALL {
            let its_own = attention_symbol(class);
            assert!(
                !of_the_body.contains(&its_own),
                "«{its_own}» is already used by a body mark: the panel would say two things with a single sign"
            );
        }
    }

    /// The claim's row as the panel paints it, symbol and phrase.
    ///
    /// **Built from the two functions that paint it and never typed out**: the tests that use it are
    /// about where the claim goes and what colour it takes, not about its wording, and a translated
    /// phrase used to break all four of them. The wording has its own guard against the contract.
    fn claim_row(class: Attention) -> String {
        format!("{} {}", attention_symbol(class), attention_text(class))
    }

    /// **The producer's flag does not hide the claim.** `header: false` removes the identity line —the one
    /// that is its own— and the claim carries on, because the answer to «do I have to go there?» is the
    /// application's. Painting it in the identity line, a producer would turn it off and the pane would
    /// disappear from the radar without anyone noticing.
    #[test]
    fn the_claim_is_painted_even_if_the_producer_turns_off_its_header() {
        let theme = Theme::default();

        let rows = what_gets_painted(r#"{"header":false,"attention":"wait","lines":["whatever"]}"#);

        assert_eq!(
            rows,
            vec![
                (
                    claim_row(Attention::Wait),
                    theme.attention_color(Attention::Wait, true)
                ),
                ("  whatever".to_owned(), theme.panel_color(None, None)),
            ]
        );
    }

    /// And with the header in place, the claim goes **below the identity and above the body**: on top,
    /// which is where the clip does not reach.
    #[test]
    fn the_claim_goes_at_the_very_top_with_the_identity_in_place() {
        let rows =
            what_gets_painted(r#"{"phase":"build","attention":"done","lines":["the body"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            ["a1b2 · build", &claim_row(Attention::Done), "  the body"]
        );
    }

    /// **A body mark is not a claim of the pane**, and this test comes in through the painting to say it
    /// where it is seen: a `mark: block` is summary content —«I waited for approval on Tuesday»— and
    /// deducing from it that the pane claims would leave it marked forever.
    #[test]
    fn a_body_mark_does_not_paint_a_claim_in_the_application_row() {
        let rows = what_gets_painted(
            r#"{"header":false,"lines":[{"text":"I waited for approval on Tuesday","mark":"block"}]}"#,
        );

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["× I waited for approval on Tuesday"]);
    }

    /// **A claim nobody holds dims, and does not go out.** The language of this panel is to confess and
    /// not to hide —that is why a stale producer still has a panel—, so its claim keeps being painted: what
    /// changes is the color, which says «it claims, and there is no one behind it». The detail —since when—
    /// is still said by the warning, in its own line.
    #[test]
    fn a_claim_nobody_holds_dims_and_the_panel_confesses_it_separately() {
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r#"{"header":false,"attention":"block","stale_after_minutes":1,"lines":["whatever"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let rows = texts_and_colors(paint_the_state(
            &mut state,
            PLACE,
            SystemTime::now() + Duration::from_secs(60 * 60),
        ));

        assert_eq!(
            rows[0],
            (
                claim_row(Attention::Block),
                theme.attention_color(Attention::Block, false)
            )
        );
        assert_ne!(
            rows[0].1,
            theme.attention_color(Attention::Block, true),
            "with no one holding it it has to look different, or the dimming says nothing"
        );
        assert!(
            rows[1].0.contains("no news"),
            "the claim does not replace the confession, both go: {rows:?}"
        );
    }

    /// And while the producer holds it, the same panel is painted with the full color: without this half,
    /// dimming it always would pass for good.
    #[test]
    fn the_same_claim_is_painted_full_while_its_producer_holds_it() {
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r#"{"header":false,"attention":"block","stale_after_minutes":1,"lines":["whatever"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let rows = texts_and_colors(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert_eq!(
            rows[0].1,
            theme.attention_color(Attention::Block, true),
            "just published, nobody has anything to confess: {rows:?}"
        );
    }

    /// **With no good panel there is no claim**, even if the application has a lot to confess: what it
    /// confesses then is that there is no panel. A document that could never be read said nothing to
    /// claim, and marking the pane by it would be inventing the signal.
    #[test]
    fn a_panel_that_could_never_be_read_claims_nothing() {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, "this is not json");
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let now = SystemTime::now();

        assert!(
            state.warning(now).is_some(),
            "the application had to have something to confess here"
        );
        assert_eq!(state.attention(now), None);
    }

    /// The claim goes away when its producer removes it, and does not stay stuck to the previous layout.
    #[test]
    fn the_claim_disappears_when_the_producer_stops_claiming() {
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r#"{"header":false,"attention":"wait","lines":["whatever"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        paint_the_state(&mut state, PLACE, SystemTime::now());

        publish(&dir, r#"{"header":false,"lines":["whatever"]}"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());
        let rows = texts_and_colors(paint_the_state(&mut state, PLACE, SystemTime::now()));

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["  whatever"]);
    }

    /// **The claim's row wraps like any other.** Its phrases are the contract's, and the longest of
    /// them —the `done` one— measures thirty-one cells with its symbol: in a narrow panel —which is a
    /// width the user chooses by dragging— it has to split, not run off. Without this test, passing its
    /// wrapping a wrong width left the whole suite green, because the place of the other tests is wide.
    ///
    /// **What it does not cover is the width that ships**, and that is the sibling below: a phrase that
    /// splits at twelve tells nothing about a phrase that splits at thirty-four.
    #[test]
    fn the_claim_row_does_not_run_off_a_narrow_panel() {
        let width = 12;

        let rows = lay_out(
            Some(&panel_with(r#"{"header":false,"lines":["whatever"]}"#)),
            "a1b2",
            width,
            &Theme::default(),
            None,
            Some(AttentionState {
                class: Attention::Block,
                backed: true,
            }),
            None,
        );

        assert!(
            rows.len() > 2,
            "with this width the phrase had to split across several rows: {rows:?}"
        );
        for row in &rows {
            assert!(
                row.column + row.text.chars().count() <= width,
                "«{}» starts at {} and measures {}: it runs off a panel of {width}",
                row.text,
                row.column,
                row.text.chars().count()
            );
        }
    }

    /// **At the width that ships, each claim takes one row**, and that is a product property rather
    /// than a coincidence of wording.
    ///
    /// The three phrases are the contract's, and this test is what keeps a translation, or a reword,
    /// from quietly costing the factory panel a line of body. It cost one already: the first English
    /// wording measured 36 and 41 cells against a slot of 34, so `block` and `done` each took two rows
    /// and the second left a single orphaned word on the last one — with nothing failing, because
    /// [`the_claim_row_does_not_run_off_a_narrow_panel`] measures a width nobody has and the painting
    /// tests use a rect eleven cells wider than the default.
    ///
    /// **The slot is the panel minus both margins** — [`DEFAULT_WIDTH`] is 36 and only the left margin
    /// is painted, so what the text gets is 34. A row is the symbol, a space and the phrase, which
    /// leaves the phrase 32 cells.
    #[test]
    fn at_the_width_that_ships_every_claim_fits_one_row() {
        let width = DEFAULT_WIDTH - 2 * MARGIN as usize;

        for class in Attention::ALL {
            let rows = lay_out(
                Some(&panel_with(r#"{"header":false,"lines":["whatever"]}"#)),
                "a1b2",
                width,
                &Theme::default(),
                None,
                Some(AttentionState {
                    class,
                    backed: true,
                }),
                None,
            );

            // **The claim's rows are the ones before the first blank**, and the blank is what the layout
            // puts between what the application says and what the producer does. Counting by column
            // instead does not work: the body's indent lives inside its text, so every row is at zero.
            let claim = rows.iter().take_while(|row| !row.text.is_empty()).count();
            assert_eq!(
                claim,
                1,
                "«{} {}» takes {claim} rows at the width the panel ships with ({width} cells of text): \
                 the factory panel loses a line of body to the claim, and the reader gets a phrase cut \
                 in two — {rows:?}",
                attention_symbol(class),
                attention_text(class)
            );
        }
    }

    /// **When only one row fits, the one that survives is the claim's.** Of the two the application puts,
    /// the claim's is the one that answers «do I have to go there?» and the warning is the one that
    /// qualifies «and it is not fresh», so the order is that one and not the reverse. It is tied down here
    /// because the clip is silent: it takes the last rows without anything failing.
    #[test]
    fn in_a_single_line_panel_what_remains_is_the_claim() {
        let (_, height) = cell();
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r#"{"header":false,"attention":"wait","stale_after_minutes":1,"lines":["whatever"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());

        let rows = texts_and_colors(paint_the_state(
            &mut state,
            Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, 2.0 * height)),
            SystemTime::now() + Duration::from_secs(60 * 60),
        ));

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            [claim_row(Attention::Wait).as_str(), "… 3 more"],
            "the claim has to be the first thing painted, and what is missing has to be said"
        );
    }

    /// **The finished does not go out by falling silent.** Whoever finished stops publishing by
    /// definition, so the silence does not belie its «it finished and nobody looked»: it is
    /// its normal state. Without this exception, half an hour after finishing —the default deadline—
    /// everything finished would be painted dimmed, and finding in the morning what finished at night
    /// is exactly what it is for.
    ///
    /// **And the application keeps confessing the silence in its line**: what the exception changes is the
    /// color of the claim, not what the panel tells.
    #[test]
    fn the_finished_keeps_being_seen_whole_however_much_it_falls_silent() {
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r#"{"header":false,"attention":"done","stale_after_minutes":1,"lines":["whatever"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let rows = texts_and_colors(paint_the_state(
            &mut state,
            PLACE,
            SystemTime::now() + Duration::from_secs(60 * 60),
        ));

        assert_eq!(
            rows[0].1,
            theme.attention_color(Attention::Done, true),
            "an hour quiet, and the finished is still whole: {rows:?}"
        );
        assert!(
            rows[1].0.contains("no news"),
            "the silence is still confessed, which is another thing: {rows:?}"
        );
    }

    /// **But it does go out when its file stops being read**, which is the only thing that really belies
    /// what is being shown: without being able to read it, what is on screen is the last thing that
    /// arrived and nothing says it is still true. Without this half, the exception above would be «the
    /// finished does not go out ever», which is hiding.
    #[test]
    fn the_finished_does_go_out_when_its_file_stops_being_read() {
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r#"{"header":false,"attention":"done","lines":["whatever"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        publish(&dir, "this is no longer json");
        age_the_file(&dir, 1);
        state.refresh(dir.path());
        let rows = texts_and_colors(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert_eq!(
            rows[0].1,
            theme.attention_color(Attention::Done, false),
            "without being able to read its file, nobody holds what is shown: {rows:?}"
        );
    }

    /// And the exception is **only for the finished**: what waits for you and what is stuck keep going out
    /// on being left with no one behind, because there the silence does belie what they say.
    #[test]
    fn the_exception_for_the_finished_does_not_reach_the_other_two_classes() {
        let theme = Theme::default();

        for class in Attention::ALL
            .into_iter()
            .filter(|class| *class != Attention::Done)
        {
            let value = class.name();
            let dir = PanelDir::create().expect("create the directory");
            publish(
                &dir,
                &format!(
                    r#"{{"header":false,"attention":"{value}","stale_after_minutes":1,"lines":["x"]}}"#
                ),
            );
            let mut state = PanelState::default();
            state.refresh(dir.path());

            let rows = texts_and_colors(paint_the_state(
                &mut state,
                PLACE,
                SystemTime::now() + Duration::from_secs(60 * 60),
            ));

            assert_eq!(
                rows[0].1,
                theme.attention_color(class, false),
                "{class:?} had to go out after an hour quiet: {rows:?}"
            );
        }
    }

    /// **A word that does not exist marks nothing, and does not take the rest of the panel with it.** The
    /// field is written by a third party by hand, so `waiting` for `wait` is a matter of time: the
    /// application cannot guess which of the three it meant —guessing it would be deducing— nor punish the
    /// rest of the document for a loose word.
    ///
    /// **It comes in through the painting because that is where it is seen.** The contract crate already
    /// asserts that field is read empty, but not that the identity and the body keep being painted: they
    /// are two distinct things and this is the one for this milestone.
    #[test]
    fn a_claim_that_does_not_exist_does_not_mark_and_the_rest_of_the_panel_is_painted_whole() {
        let rows =
            what_gets_painted(r#"{"attention":"waiting","phase":"validate","lines":["the body"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["a1b2 · validate", "  the body"]);
    }

    /// **What [`PanelState::attention`] answers from outside is the same as what the panel paints.** They
    /// are two paths: the painting already has the warning in hand and asks inside; the outside one —the
    /// door through which whoever accumulates a tab's claims will ask— has to compute it itself.
    ///
    /// **Without this test nobody looks at the outside one**: checked by inverting there the warning's
    /// question —taking as backed exactly what is not and the reverse— with the whole suite green. What is
    /// asserted is the value, not that the two doors coincide, because coinciding in the false also
    /// coincides.
    #[test]
    fn what_the_claim_answers_from_outside_is_what_the_panel_paints() {
        let dir = PanelDir::create().expect("create the directory");
        publish(
            &dir,
            r#"{"header":false,"attention":"wait","stale_after_minutes":1,"lines":["whatever"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();
        let just_now = SystemTime::now();
        let gone_quiet = just_now + Duration::from_secs(60 * 60);

        assert_eq!(
            state.attention(just_now),
            Some(AttentionState {
                class: Attention::Wait,
                backed: true
            }),
            "just published there is nothing to confess about this panel"
        );
        assert_eq!(
            state.attention(gone_quiet),
            Some(AttentionState {
                class: Attention::Wait,
                backed: false
            }),
            "after an hour quiet there is no longer anyone behind what is shown"
        );

        for when in [just_now, gone_quiet] {
            let answer = state.attention(when).expect("it claims");
            let rows = texts_and_colors(paint_the_state(&mut state, PLACE, when));
            assert_eq!(
                rows[0].1,
                theme.attention_color(answer.class, answer.backed),
                "what is painted is not what is answered from outside: {rows:?}"
            );
        }
    }

    /// **The three phrases live four times**: in the code that paints them, and in three places of the
    /// documents — the contract's `attention` table, the contract's «How it looks» bullet, and D18's
    /// own table in the decisions record — word for word in all of them. The guard used to read the
    /// contract's table only, so of the four places a phrase is written, only one was checked.
    ///
    /// It is the same kind of coupling as that of the measurement script with the report: files that
    /// have to say the same thing and no place where that is asserted.
    #[test]
    fn the_three_claim_phrases_are_the_ones_the_contract_publishes() {
        const CONTRACT: &str = include_str!("../../../docs/04-contrato-panel.md");
        const DECISIONS: &str = include_str!("../../../docs/02-decisiones.md");
        const HEADER: &[&str] = &["Value", "What it says", "Example"];

        // D18's table has the contract's header too, so it is read from D18's heading to the next
        // decision's: anchoring on the header alone would read whichever of the record's tables came
        // first, and reading to the end would find a later decision's table if D18 lost its own.
        let d18 = DECISIONS
            .split_once("## D18 —")
            .expect("the decisions record no longer carries D18")
            .1;
        let d18 = d18.split_once("\n## ").map_or(d18, |(it, _)| it);
        let tables = [
            ("the contract", markdown::table(CONTRACT, HEADER)),
            ("D18", markdown::table(d18, HEADER)),
        ];
        // The bullet ends at the next bullet or at a blank line, whichever comes first, so that being
        // the last of its list cannot hand it the sections below. Its lines are trimmed before they
        // are joined, so a phrase the text wraps still reads whole.
        let mut lines = CONTRACT
            .lines()
            .skip_while(|line| !line.starts_with("- **How it looks**"));
        let bullet = lines
            .next()
            .into_iter()
            .chain(lines.take_while(|line| !line.trim().is_empty() && !line.starts_with("- ")))
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            !bullet.is_empty(),
            "the contract no longer carries its «How it looks» bullet"
        );
        // Both directions of the vocabulary: every class the parser knows has a row, below, and no
        // row names a value the parser does not know, here.
        let names: Vec<String> = Attention::ALL
            .into_iter()
            .map(|class| format!("`{}`", class.name()))
            .collect();
        for (place, table) in &tables {
            assert_eq!(
                table.column("Value"),
                names,
                "{place} does not publish the values the parser reads, in the parser's order"
            );
        }

        // **It is searched in the row of ITS key, not in the file.** A `contains` over the whole document
        // passes with the three phrases **rotated among classes** —checked by measuring it— and with any
        // loose fragment: «for you» is inside «it is waiting for you». The row is identified by the
        // contract's key, which is the value the parser reads.
        for class in Attention::ALL {
            let (key, phrase) = (class.name(), attention_text(class));
            for (place, table) in &tables {
                let cell = table
                    .cell("Value", &format!("`{key}`"), "What it says")
                    .unwrap_or_else(|| {
                        panic!(
                            "{place} has no row for «{key}»: either it changed shape, or it stopped \
                             publishing a value the application does paint"
                        )
                    });
                // **And the WHOLE phrase is compared, not whether it is inside.** A `contains` over the
                // row passes with the phrase truncated —«it is waiting» is inside «it is waiting for
                // you»—, and the «for you» is what separates it from the second.
                //
                // **What the colon cuts off is the gloss, and that is the row's shape rather than a
                // shortcut.** The `wait` row reads «**it is waiting for you**: a question, a
                // permission, a decision»: the phrase is what the panel paints and the rest explains it
                // to whoever writes the file. So a row growing a gloss is accepted on purpose; a row
                // changing the words before the colon is not.
                let published = cell.replace("**", "");
                let published = published.split(':').next().unwrap_or(&published).trim();
                assert_eq!(
                    published, phrase,
                    "the panel paints «{phrase}» for {class:?} and the «{key}» row of {place} publishes \
                     «{published}»: the producer that reads it expects one text and sees another"
                );
            }
            // The bullet writes what the row of the application paints, symbol and phrase, each in
            // backticks: the backticks are what make it whole rather than a fragment.
            let painted = format!("`{} {phrase}`", attention_symbol(class));
            assert!(
                bullet.contains(&painted),
                "the panel paints {painted} for {class:?} and the contract's «How it looks» bullet does \
                 not show it: «{bullet}»"
            );
        }
    }
}

/// What a button is on screen, and what a press on it reaches.
///
/// Named for the gesture rather than for the thing, because the thing already names the module the
/// hit list lives in.
///
/// **These go through a real published document and a real painting**, never through a hand-pinned
/// layout: a panel nobody published takes up no columns at all, so a test that invents the strip
/// exercises something the window never does.
#[cfg(test)]
mod pressing {
    use super::tests::{age_the_file, paint_the_state, publish};
    use super::*;
    use panel_contract::PanelDir;

    const RUN: &str = r#"{"actions": [{"label": "Run tests", "send": "npm test"}],
                          "lines": ["the body"]}"#;

    /// A state with that document published, painted once at the given size.
    fn painted(document: &str, place: Rect) -> PanelState {
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, document);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        assert!(
            state.is_visible(SystemTime::now()),
            "the document was not understood, so there is no strip and this test would be vacuous"
        );
        // **Twice, and that is the subject rather than setup.** The first pass builds the layout, so
        // the panel is not settled and a press would be refused; the second reuses it, which is the
        // ordinary state of a panel nobody is writing to.
        paint_the_state(&mut state, place, SystemTime::now());
        paint_the_state(&mut state, place, SystemTime::now());
        state
    }

    fn a_wide_strip() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), eframe::egui::vec2(400.0, 600.0))
    }

    /// Somewhere inside the first button that was drawn.
    fn on_the_first_button(state: &PanelState) -> Pos2 {
        state
            .buttons_drawn()
            .next()
            .expect("a button was drawn")
            .center()
    }

    fn on_the_first_link(state: &PanelState) -> Pos2 {
        state
            .links_drawn()
            .next()
            .expect("a link was drawn")
            .center()
    }

    #[test]
    fn a_compact_link_sits_beside_a_short_line_and_below_a_full_one() {
        let beside = panel_contract::parse_document(
            r#"{"header":false,"lines":[{"text":"Review","link":"https://example.test/pull/257"}]}"#,
        )
        .expect("a panel");
        let beside = lay_out(
            Some(&beside),
            "a1b2",
            30,
            &Theme::default(),
            None,
            None,
            None,
        );
        let row = beside
            .iter()
            .find(|row| row.link.is_some())
            .expect("the line carries its link");
        assert!(!row.text.is_empty(), "the short line should share its row");
        assert!(
            row.link.as_ref().expect("a link").column > row.text.chars().count(),
            "the compact label was not placed beside the text"
        );

        let below = panel_contract::parse_document(
            r#"{"header":false,"lines":[{"text":"This line consumes every available cell in its first row","link":"https://example.test/pull/257"}]}"#,
        )
        .expect("a panel");
        let below = lay_out(
            Some(&below),
            "a1b2",
            20,
            &Theme::default(),
            None,
            None,
            None,
        );
        let row = below
            .iter()
            .find(|row| row.link.is_some())
            .expect("the line carries its link");
        assert!(
            row.text.is_empty(),
            "a label that cannot fit beside the text should hang below it"
        );
    }

    #[test]
    fn a_settled_painted_link_reaches_the_exact_raw_target() {
        let target = "https://example.test/panel?a=1&b=two";
        let document = format!(r#"{{"lines":[{{"text":"Review","link":"{target}"}}]}}"#);
        let state = painted(&document, a_wide_strip());

        assert_eq!(state.link_at(on_the_first_link(&state)), Some(target));
    }

    #[test]
    fn a_new_document_cannot_retarget_the_previous_paintings_link_region() {
        let strip = a_wide_strip();
        let dir = PanelDir::create().expect("create the directory");
        let old = "https://example.test/old";
        publish(
            &dir,
            &format!(r#"{{"lines":[{{"text":"Review","link":"{old}"}}]}}"#),
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        paint_the_state(&mut state, strip, SystemTime::now());
        paint_the_state(&mut state, strip, SystemTime::now());
        let old_pixels = on_the_first_link(&state);

        age_the_file(&dir, 1);
        publish(
            &dir,
            r#"{"lines":[{"text":"Review","link":"https://example.test/new"}]}"#,
        );
        state.refresh(dir.path());

        assert_eq!(
            state.link_at(old_pixels),
            Some(old),
            "the current document retargeted geometry painted from the previous one"
        );
    }

    #[test]
    fn a_label_the_strip_cuts_at_its_side_is_not_reachable() {
        let document = r#"{"lines":[{"text":"Review","link":"https://example.test/releases/a-very-long-release-name"}]}"#;
        assert_eq!(
            painted(document, a_wide_strip()).links_drawn().count(),
            1,
            "the control strip reached no label, so the narrow one would prove nothing"
        );

        // Too narrow for the label beside the line, so it hangs below — and runs past the right edge.
        let narrow = Rect::from_min_size(a_wide_strip().min, eframe::egui::vec2(120.0, 600.0));
        let cut = painted(document, narrow);
        assert_eq!(
            cut.links_drawn().count(),
            0,
            "a label cut at the strip's side stayed reachable"
        );
    }

    #[test]
    fn an_address_the_gate_refuses_leaves_its_label_unreachable() {
        for refused in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "htps://example.test/typo",
        ] {
            let document = format!(r#"{{"lines":[{{"text":"Review","link":"{refused}"}}]}}"#);
            let state = painted(&document, a_wide_strip());
            assert_eq!(
                state.links_drawn().count(),
                0,
                "{refused:?} got a reachable label, so it would show a hand and a tooltip for a click \
                 that opens nothing"
            );
        }
    }

    #[test]
    fn a_reachable_label_carries_the_form_the_browser_receives() {
        let state = painted(
            r#"{"lines":[{"text":"Review","link":"HTTPS://Example.Test/Panel"}]}"#,
            a_wide_strip(),
        );
        assert_eq!(
            state.link_at(on_the_first_link(&state)),
            Some("https://example.test/Panel")
        );
    }

    #[test]
    fn clipped_and_unsettled_links_are_inert() {
        let document = r#"{"lines":[{"text":"Review","link":"https://example.test/panel"}]}"#;
        let strip = a_wide_strip();
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, document);
        let mut unsettled = PanelState::default();
        unsettled.refresh(dir.path());
        paint_the_state(&mut unsettled, strip, SystemTime::now());
        assert_eq!(unsettled.links_drawn().count(), 1);
        assert!(unsettled.link_at(on_the_first_link(&unsettled)).is_none());

        let clipped = painted(
            document,
            Rect::from_min_size(strip.min, eframe::egui::vec2(strip.width(), 1.0)),
        );
        assert_eq!(clipped.links_drawn().count(), 0);
        assert!(clipped.link_at(strip.center()).is_none());

        let mut hidden = painted(document, strip);
        let old_pixels = on_the_first_link(&hidden);
        hidden.nothing_was_drawn();
        assert_eq!(hidden.links_drawn().count(), 0);
        assert!(hidden.link_at(old_pixels).is_none());
    }

    #[test]
    fn a_button_that_was_drawn_is_reached_by_a_press_on_it() {
        let state = painted(RUN, a_wide_strip());

        let reached = state
            .button_at(on_the_first_button(&state))
            .expect("the press reaches the button that was drawn");

        assert_eq!(reached.send(), "npm test");
    }

    #[test]
    fn a_press_beside_the_button_reaches_nothing() {
        let strip = a_wide_strip();
        let state = painted(RUN, strip);
        let below = Pos2::new(strip.center().x, strip.bottom() - 1.0);

        assert!(
            state.button_at(below).is_none(),
            "a press at the bottom of the strip, where the body is, reached a button"
        );
    }

    /// **The criterion the whole design rests on.** The rows the painting produced, reassembled, have
    /// to be the string that would be written to the program — because that painted text *is* the
    /// confirmation, and a filter or a wrapping that changed it would mean the bytes that ran are not
    /// the bytes that were read.
    ///
    /// **It asserts against the laid-out rows and not against the parsed string**, which would be the
    /// same value compared with itself and would pass however the painting mangled it.
    #[test]
    fn what_is_painted_under_the_label_is_what_would_be_sent() {
        for command in [
            "npm test",
            // Runs of spaces: the ordinary wrapping collapses them, which is what this rule found.
            "echo  two  spaces",
            // Longer than the strip, so it wraps and the rows have to rejoin exactly.
            "cargo test --workspace --all-targets --no-fail-fast -- --nocapture --test-threads=1",
            // Not ASCII, so a cut by bytes would split a scalar and a cut by words would move it.
            "grep 日本語 ~/documentos/notas.txt",
            // Quoted whitespace, where the difference is not cosmetic at all.
            r#"echo "a  b""#,
        ] {
            let document = format!(
                r#"{{"actions": [{{"label": "x", "send": "{}"}}], "lines": ["b"]}}"#,
                command.replace('"', "\\\"")
            );
            let mut state = PanelState::default();
            let dir = PanelDir::create().expect("create the directory");
            publish(&dir, &document);
            state.refresh(dir.path());
            let rows = state
                .lay_out_if_needed("a1b2", 30, &Theme::default(), None, None)
                .expect("there is a panel")
                .0
                .to_vec();

            let reassembled: String = rows
                .iter()
                .filter(|row| matches!(row.role, RowRole::ButtonBody(0)))
                .map(|row| row.text.as_str())
                .collect();

            assert_eq!(
                reassembled, command,
                "what the panel drew is not what the program would receive"
            );
        }
    }

    /// **A button whose rows did not all fit is not in the frame's reach**, and this pins the
    /// mechanism rather than a number.
    ///
    /// **The first version of it passed for the wrong reason and that is worth recording.** It
    /// invented a cell height of 15 points where the real one is about 16, so the strip fitted two
    /// rows instead of three and **not one row of the button ever reached the screen** — the button
    /// was absent because nothing of it was drawn, which is not the case this test is named after.
    /// The defect and the fix gave the same result, which is the disguise **D30** lists second.
    ///
    /// So the height comes from the metrics the window uses, and the assertion is in two halves: the
    /// label **is** on screen, and the button is **not** reachable. Only the two together say that a
    /// half-drawn button was refused.
    #[test]
    fn a_button_that_did_not_fit_whole_cannot_be_pressed() {
        let whole = painted(RUN, a_wide_strip());
        assert_eq!(
            whole.buttons_drawn().count(),
            1,
            "the case needs the button drawn whole"
        );

        // Three rows of the **real** cell: the identity row, the label, and one more that `clip`
        // spends on «… N more». The command's row is what does not fit.
        // The metrics the window itself uses, asked of a real context: a height invented here is how
        // the first version of this test passed for the wrong reason.
        let ctx = eframe::egui::Context::default();
        let mut cell = 0.0;
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            cell = CellMetrics::measure(
                ui.ctx(),
                eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE),
            )
            .height();
        });
        output.drop_without_applying_deltas();

        let short = Rect::from_min_size(
            Pos2::new(100.0, 50.0),
            eframe::egui::vec2(400.0, cell * 3.0),
        );
        let clipped = painted(RUN, short);
        let rows = clipped
            .layout
            .as_ref()
            .expect("there is a layout")
            .rows
            .clone();
        let label = rows
            .iter()
            .position(|row| matches!(row.role, RowRole::ButtonLabel(_)))
            .expect("the button has a label row");
        assert!(
            label < 2,
            "the case needs the label on screen and its command off it; the label is at row {label} \
             and only 2 rows survive the clip"
        );

        assert_eq!(
            clipped.buttons_drawn().count(),
            0,
            "a button the clip cut in half is still reachable by a press"
        );
    }

    /// **A press is refused on a panel that was rebuilt this frame.** The panel is the one surface in
    /// this window that repaints because somebody else wrote, so between the frame a person read and
    /// the frame their click lands in, a producer can have replaced every button.
    #[test]
    fn a_press_is_refused_while_the_panel_is_not_settled() {
        let strip = a_wide_strip();
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, RUN);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        paint_the_state(&mut state, strip, SystemTime::now());
        paint_the_state(&mut state, strip, SystemTime::now());
        let over_a_button = on_the_first_button(&state);
        assert!(
            state.button_at(over_a_button).is_some(),
            "the case needs a press that would otherwise land"
        );

        // The producer publishes something else and the window paints it: that pass rebuilds the
        // layout, so it is not settled.
        age_the_file(&dir, 1);
        publish(
            &dir,
            r#"{"actions": [{"label": "Deploy", "send": "make deploy"}], "lines": ["b"]}"#,
        );
        state.refresh(dir.path());
        paint_the_state(&mut state, strip, SystemTime::now());

        assert!(
            state.button_at(over_a_button).is_none(),
            "a press landed on the frame the panel was rewritten in, which is the one frame where \
             what is under the pointer is not what the person read"
        );

        // And on the next frame, with nothing having changed, it is pressable again.
        paint_the_state(&mut state, strip, SystemTime::now());
        assert!(
            state.button_at(on_the_first_button(&state)).is_some(),
            "the panel never became pressable again"
        );
    }

    /// **A command drawn past the strip's edge is not pressable**, and this is the last way a press
    /// could have sent something nobody could read.
    ///
    /// This module lays text out counting **characters**, and a character is not a column: a kanji
    /// is two. So a command in Japanese that fits by the character count is drawn beyond the strip
    /// and the painter clips what sticks out — the person reads a fragment, and pressing would send
    /// the whole string.
    ///
    /// **The width is asked of whoever drew the text**, which is the only authority there is: this
    /// crate carries no table of glyph widths and does not need one.
    #[test]
    fn a_command_drawn_past_the_strip_is_not_pressable() {
        // Narrow enough that the character count fits and the columns do not: the strip takes about
        // thirty cells and every one of these is two wide.
        let wide = "日".repeat(40);
        let document =
            format!(r#"{{"actions": [{{"label": "Wide", "send": "{wide}"}}], "lines": ["b"]}}"#);
        let strip = Rect::from_min_size(Pos2::new(100.0, 50.0), eframe::egui::vec2(260.0, 600.0));

        let narrow = painted(&document, strip);

        assert!(
            narrow.buttons_drawn().count() == 0,
            "a command drawn outside the strip is still reachable by a press"
        );
        // And the case is not vacuous: the same document in a strip wide enough **is** pressable.
        let roomy = painted(
            &document,
            Rect::from_min_size(Pos2::new(100.0, 50.0), eframe::egui::vec2(1400.0, 600.0)),
        );
        assert_eq!(
            roomy.buttons_drawn().count(),
            1,
            "the case needs the same command to be pressable once there is room, or it proves nothing"
        );
    }

    /// **The same rule for a command whose characters go to color**: that row is laid out by another
    /// path, and what it inked still has to reach the check.
    #[test]
    fn a_command_of_emoji_drawn_past_the_strip_is_not_pressable() {
        let wide = "\u{1F525}".repeat(20);
        let document =
            format!(r#"{{"actions": [{{"label": "Hot", "send": "{wide}"}}], "lines": ["b"]}}"#);
        let strip = Rect::from_min_size(Pos2::new(100.0, 50.0), eframe::egui::vec2(260.0, 600.0));

        assert_eq!(
            painted(&document, strip).buttons_drawn().count(),
            0,
            "a command of emoji drawn outside the strip is still reachable by a press"
        );
        let roomy = Rect::from_min_size(Pos2::new(100.0, 50.0), eframe::egui::vec2(1400.0, 600.0));
        assert_eq!(
            painted(&document, roomy).buttons_drawn().count(),
            1,
            "the case needs the same command to be pressable once there is room, or it proves nothing"
        );
    }

    /// A producer that declares no buttons costs nothing and reaches nothing.
    #[test]
    fn a_panel_with_no_buttons_has_nothing_to_press() {
        let state = painted(r#"{"lines": ["just a summary"]}"#, a_wide_strip());

        assert_eq!(state.buttons_drawn().count(), 0);
        assert_eq!(state.links_drawn().count(), 0);
        assert!(state.button_at(a_wide_strip().center()).is_none());
    }
}
