//! The strip over every console: the space it takes from the pane, and the handle it is grabbed by.
//!
//! **One row per pane, and it is the price of the gesture.** Until now a console was taken somewhere
//! else with `Ctrl+Shift` and the left button (**D20**), which is a chord you have to be told about:
//! nothing on screen says a console can be moved at all. The strip is the thing that says it, and
//! saying it costs every console a row of its own — which is exactly what D20 weighed and refused
//! **as part of F2.6**. What is amended here is not that arithmetic but what pays for it: the
//! measurement bench switches the strip off, so the eighteen published batches keep the grid they
//! were measured with, and what the strip costs is measured in a batch of its own — `T` of
//! `scripts/measure-frame.sh`, the one run that sets [`HEADER_VARIABLE`], against `C` as its
//! control. **The two cannot be told apart by their spans alone**, because a strip takes a row off
//! every console and so `T`'s grid is not `C`'s: that is why the report declares what was painted
//! and what was asked for on a line of its own, and why the script throws a run away in either
//! direction (see [`crate::measure::Strips`]).
//!
//! **The mould is the tab bar's, and copying it is deliberate.** [`crate::tab_bar`] already cuts a
//! height off the top of an area and hands back what is left, already clamps that cut, and already
//! answers `None` when there is nothing to paint — the three things this needs. What is not copied
//! is its reason for the `None`: there the bar disappears with a single tab so that the grid stays
//! comparable with the one from before tabs existed; here the strip disappears **only** under the
//! measurement gate, because the product decision taken at the start of this milestone is that every
//! console carries one, single console included.
//!
//! **Whether it is painted is a value resolved once**, at startup, beside the settings the same gate
//! already vetoes ([`crate::settings::to_use`]) — never an `Entry` threaded down to whoever needs a
//! height. [`crate::tiling::can_split`] has four production callers and not one of them holds the
//! gate's enum; all four hold a [`Headers`], which is what a resolved decision looks like once it
//! stops being a question.

use crate::painter::CellMetrics;
use eframe::egui;

pub(crate) mod actions;
pub(crate) mod controls;
pub(crate) mod integrated;
pub(crate) mod menu;
mod splitting;
pub(crate) mod title_text;
pub(crate) use splitting::split as split_named;

/// Whether this session paints a strip over each console.
///
/// **A type and not a `bool`, for the reason the four call sites give**: `can_split(area, rows,
/// metrics, true)` says nothing about what the `true` is, and the one thing that must never happen
/// here is a call site guessing. Same argument as [`crate::settings::Entry`], one step further
/// along: that one says which door the session came in through, this one says what was decided
/// because of it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Headers {
    painted: bool,
    integrated: bool,
    title_line_height: f32,
    title_size: f32,
}

impl Headers {
    /// What a session gets unless the gate takes it away — every ordinary one, and the one measured
    /// batch that asks for strips on purpose. The tests build with it.
    pub(crate) const PAINTED: Self = Self {
        painted: true,
        integrated: false,
        title_line_height: 0.0,
        title_size: 12.0,
    };

    /// A measured run that did not ask for them, which is every batch but `T`.
    pub(crate) const NOT_PAINTED: Self = Self {
        painted: false,
        ..Self::PAINTED
    };

    pub(crate) fn integrated(self, enabled: bool) -> Self {
        Self {
            integrated: enabled && self.painted,
            ..self
        }
    }

    pub(crate) fn with_title_metrics(self, size: f32, line_height: f32) -> Self {
        Self {
            title_line_height: line_height,
            title_size: size,
            ..self
        }
    }

    pub(crate) fn title_size(self) -> f32 {
        self.title_size
    }

    /// Space reserved on both sides of an integrated pane, including its outline.
    pub(crate) fn pane_inset(self) -> f32 {
        if self.integrated { 2.5 } else { 0.0 }
    }

    /// The decision, taken once from the environment the session was launched with.
    ///
    /// **The knob is an opt-in and it lives outside [`crate::MEASUREMENT_VARIABLES`]**, which is the
    /// shape the script's own per-batch switches already have — the ones that turn on the Japanese
    /// dump, the emoji dump and the panel's buttons. It cannot be one of those five: the script
    /// exports the window size on **every** batch, so the gate holds in every run, and a batch built
    /// to measure what the strip costs could never paint one.
    ///
    /// It takes what it reads instead of reading it, for the reason its neighbours do: setting a
    /// variable from a test is undefined behaviour in a process with threads, and this suite runs in
    /// parallel.
    ///
    /// **The door arrives as [`crate::settings::Entry`] and not as a `bool`**, which is the whole
    /// reason that type exists one file away: `resolved(true, false)` says nothing about which
    /// `true` it is, and two adjacent booleans are swapped in a call without the compiler noticing.
    /// The one that is left is unambiguous — it is the knob, and there is nothing beside it to
    /// confuse it with.
    pub(crate) fn resolved(entry: crate::settings::Entry, asked_for: bool) -> Self {
        if entry == crate::settings::Entry::Measuring && !asked_for {
            Self::NOT_PAINTED
        } else {
            Self::PAINTED
        }
    }

    /// Whether a strip is painted at all.
    pub(crate) fn are_painted(self) -> bool {
        self.painted
    }
}

/// The environment variable a measurement batch asks for headers with.
///
/// **Its absence is what keeps the published series comparable**, so it is the one gate of this
/// milestone that is read the other way round from the rest: everything else here is on by default
/// and this is off by default — but only inside a measured run, which is the only place the default
/// differs from what a person sees.
///
/// Written without an eñe, like its sisters: it is what a shell script types without encoding
/// surprises.
pub(crate) const HEADER_VARIABLE: &str = "MEASURE_HEADER";

/// How much air the strip leaves above and below its handle.
///
/// The same shape as [`crate::tab_bar::TAB_BAR_MARGIN`] and for the same reason: the rest of the
/// height comes from the font's own metric, so the strip grows with the font size instead of
/// falling short when somebody raises it. It is smaller than the bar's four points because the bar
/// carries a number and this carries a grip.
pub(crate) const MARGIN: f32 = 3.0;

/// How wide the grip is, in cells.
///
/// **In cells and not in points**, so a window opened with a bigger font gets a bigger handle
/// instead of the same speck: it is the same rule the panel's width follows (**D10**), where
/// counting in characters is what keeps the desktop's zoom from changing what you see.
const GRIP_CELLS: f32 = 3.0;

/// How many lines the grip is drawn with, and how thick each one is.
///
/// Three lines is the affordance every desktop already uses for «drag me by this», and it is the
/// whole of what the strip says in this delivery: a title, a state or the focus would each be a
/// product decision this milestone did not take.
const GRIP_LINES: usize = 3;
const GRIP_THICKNESS: f32 = 1.0;

/// How wide the close button is, in cells.
///
/// **In cells like the grip, and for the same reason** (**D10**): a window opened with a bigger font
/// gets a bigger target instead of the same speck. Two and not three because it is a square-ish
/// corner and the grip is a band.
const BUTTON_CELLS: f32 = 2.0;

/// How much of the button's shorter side each arm of the cross reaches from its centre, and how
/// thick the two strokes are.
const CROSS_REACH: f32 = 0.22;
const CROSS_THICKNESS: f32 = 1.0;

/// How much of the strip's right-hand end the button leaves alone, **and it is the folded panel's
/// handle, taken from the two constants that build it** rather than copied as a number.
///
/// **That corner is spoken for and the delivery before this one paid a review finding for it**: with
/// the panel folded, its handle sits exactly this far in from the pane's right edge, and cutting the
/// strip after tiling the panel left that handle running the pane's whole height and swallowing the
/// corner of the one band that exists to be grabbed. It is driven by a test that presses at this
/// very abscissa (`the_strip_is_grabbed_at_the_corner_the_folded_panels_handle_would_take`).
///
/// **A button flush against the edge would take that press instead — and end a program with it.**
/// The first version of this delivery did exactly that, and the test caught it: the console closed
/// where somebody had reached to move it. Taken from the constants so the button follows the corner
/// if either one ever moves.
const CORNER_LEFT_TO_THE_GRAB: f32 = crate::PANEL_GRIP + crate::tiling::DIVIDER_THICKNESS;

/// Whether this pane's strip carries the button that closes it.
///
/// **A pane with no sibling carries none, and that does not contradict [`Headers`] at the top of
/// this module.**
/// The strip is painted always, single console included, because a strip that came and went would
/// teach nobody that a console can be moved (**D45**). The button is the opposite case: on a pane
/// with no sibling it would not close a console, it would close the **tab** — and on the last tab,
/// the window, with whatever an agent was doing inside it — one click away, on a target two cells
/// wide. That gesture keeps the three-key chord it already has.
///
/// **A type and not a `bool`**, for the reason its neighbours give: it travels beside [`Headers`]
/// into the same call, and two adjacent booleans are swapped without the compiler saying a word.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Button(bool);

impl Button {
    /// What every pane of a tab showing these consoles carries.
    ///
    /// **The count is the question, not the tree's shape.** A tab whose root is a leaf has one
    /// console and every other tab has more, so both callers — the painting and the pointer — read
    /// it off the list of rectangles they are already walking, and neither one walks the tree a
    /// second time to ask something the length already answers.
    ///
    /// **And they hand over the list rather than its length**, which is not ceremony: with a `usize`
    /// in the signature, the call sites of both callers have other `usize`s within reach — an index,
    /// a count of effects — and passing the wrong one compiles and paints a button on a console that
    /// must not carry one. A slice of rectangles is a thing neither of them can confuse.
    pub(crate) fn in_a_tab_of(consoles: &[egui::Rect]) -> Self {
        Self(consoles.len() > 1)
    }

    pub(crate) fn is_drawn(self) -> bool {
        self.0
    }
}

/// A strip's close button: the one rectangle, and the only way to ask whether a press hit it.
///
/// **A type and not a bare [`egui::Rect`], because the rule on its right edge is not optional.**
/// [`egui::Rect::contains`] is inclusive on both edges, and the button's right edge is the abscissa
/// where the corner left to the grab begins: asked with `contains`, that single column answered yes
/// to the button **and** to the grab, and the press
/// `the_strip_is_grabbed_at_the_corner_the_folded_panels_handle_would_take` drives lands exactly
/// there — it closed the console it had come to move. With the rectangle handed out bare, writing
/// `contains` again is one line away and nothing fails until somebody presses that column. Handed
/// out as this, the only question it answers is the right one. It is the same shape
/// [`crate::panel::pane::PaneChrome::strip_holds`] gives the tie on the strip's bottom edge.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct CloseButton(egui::Rect);

impl CloseButton {
    /// Where the cross is painted.
    pub(crate) fn rect(self) -> egui::Rect {
        self.0
    }

    /// Whether a press at that point hit it.
    ///
    /// **Only the right edge is decided here.** To the left is strip you may grab, and the bottom
    /// edge is the strip's own tie with the console's first row, which
    /// [`crate::panel::pane::PaneChrome::strip_holds`] settles in the one place both callers read —
    /// so the pointer asks that first and this never sees a press from the row below.
    pub(crate) fn holds(self, at: egui::Pos2) -> bool {
        self.0.contains(at) && at.x < self.0.right()
    }
}

/// The button of that strip, or nothing where the pane carries none **or has no room for one**.
///
/// **One place decides it and both callers read it here**: the painting draws the cross inside this
/// rectangle and the pointer decides a press by it. With the geometry written twice, a delivery that
/// moved the button in one of them would leave the other answering for where it used to be — a
/// button that is painted in one corner and pressed in another.
///
/// **It is not flush with the right edge**: see [`CORNER_LEFT_TO_THE_GRAB`], which is the corner the
/// previous delivery fought for and the reason this one does not take it.
///
/// **And there is no button at all unless the strip still has its grip to the left of it**, which is
/// this project's rule about destructive surfaces applied to the other axis. Nothing stops a window
/// being dragged narrow — no minimum size is asked of the desktop, which is why the cut itself is
/// clamped one function above — and with the button simply squeezed, a strip a few points wide is
/// **entirely** a close button: every press on a console's top row would end its program, and the
/// band that exists to be grabbed could not be grabbed anywhere. The pane that is too narrow keeps
/// the handle and loses the button; the chord still closes it.
pub(crate) fn button_within(
    strip: egui::Rect,
    metrics: &CellMetrics,
    button: Button,
) -> Option<CloseButton> {
    if !button.is_drawn() {
        return None;
    }

    let right = strip.right() - CORNER_LEFT_TO_THE_GRAB;
    let left = right - BUTTON_CELLS * metrics.width();
    // The grip is centred, so what the button may not reach is the centre plus half of it.
    let grip_ends = strip.center().x + GRIP_CELLS * metrics.width() / 2.0;

    (left > grip_ends).then(|| {
        CloseButton(egui::Rect::from_min_max(
            egui::pos2(left, strip.top()),
            egui::pos2(right, strip.bottom()),
        ))
    })
}

/// How tall the strip is, or zero where none is painted.
///
/// **The same number feeds the geometry and the painting, and there is no regime in between.** A
/// height reserved for a strip nobody draws is a row stolen from every console, and a strip drawn
/// over a row nobody reserved is a strip drawn on top of somebody's text.
pub(crate) fn height(metrics: &CellMetrics, headers: Headers) -> f32 {
    if !headers.are_painted() {
        return 0.0;
    }
    if headers.integrated {
        (metrics.height() + 2.0 * MARGIN)
            .max(33.0)
            .max(headers.title_line_height + 12.0)
    } else {
        metrics.height() + 2.0 * MARGIN
    }
}

/// Sets aside the strip's place at the top of a pane, and returns what is left below it.
///
/// **The cut is bounded, because [`egui::Rect::split_top_bottom_at_y`] does not bound it** — read in
/// its source (`emath-0.36.1/src/rect.rs:671`), where it builds both rectangles with the cut as it
/// comes. A window dragged shorter than one pane's strip would leave the console's rectangle
/// **inverted**, its top edge below its bottom one. [`crate::tiling::can_split`] does not cover
/// this: it guards splitting, and nobody splits anything while dragging the window's corner.
pub(crate) fn set_the_header_aside(
    area: egui::Rect,
    metrics: &CellMetrics,
    headers: Headers,
) -> (Option<egui::Rect>, egui::Rect) {
    if !headers.are_painted() {
        return (None, area);
    }

    let cut_along = (area.top() + height(metrics, headers)).min(area.bottom());
    let (header, console) = area.split_top_bottom_at_y(cut_along);

    (Some(header), console)
}

/// Paints one pane's strip, and returns what it cost.
///
/// **Its own span and not part of the console's**, for the rule **D13** states: the frame's cost is
/// split by subtraction, so any work that is not declared is published as the interface framework's
/// — the very figure **D3** judges egui by. Under the gate only one batch reaches this function,
/// which is what keeps the rest of the published series comparable; that is the point, not an
/// excuse for leaving the span undeclared. What it costs there is published on the report's own
/// `header:` line (see [`crate::measure::Strips`]).
#[must_use = "it carries what the strip cost the frame. Dropping it publishes that cost as the \
              framework's, which is the figure D3 judges egui by (D13)"]
pub(crate) fn paint(
    painter: &egui::Painter,
    header: egui::Rect,
    theme: &crate::theme::Theme,
    metrics: &CellMetrics,
    button: Button,
) -> std::time::Duration {
    let started = std::time::Instant::now();

    // **The panel's background and not a colour of its own.** The strip is chrome the application
    // owns, exactly like the status panel beside it, and a new colour would have to be chosen in
    // both themes, added to the twenty-two of the settings file (**D26**) and explained in its
    // manual — for a band whose whole job is to be distinguishable from the console under it.
    painter.rect_filled(header, 0.0, theme.panel_background());

    // **The grip is centred on the strip and not flush with a corner**, because the corners are
    // spoken for: the close button lands on the right, and the left of a pane is where the tree's
    // divider ends.
    let grip_width = GRIP_CELLS * metrics.width();
    let centre = header.center();
    // **The span is the INK, not the distance between the first line and the last.** Measured from
    // one line's top to the last line's top, the drawing comes out half a thickness low: the middle
    // line's top lands on the centre instead of the line itself. It is half a point, and it is the
    // kind of half point this project has already paid for in the grid (**D34**).
    let span = (GRIP_LINES - 1) as f32 * (2.0 * GRIP_THICKNESS) + GRIP_THICKNESS;
    for line in 0..GRIP_LINES {
        let y = centre.y - span / 2.0 + line as f32 * 2.0 * GRIP_THICKNESS;
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(centre.x - grip_width / 2.0, y),
                egui::pos2(centre.x + grip_width / 2.0, y + GRIP_THICKNESS),
            ),
            0.0,
            theme.divider(),
        );
    }

    // **The cross is drawn in the text's ink and the grip in the divider's, and the difference is
    // deliberate.** The grip is an affordance: it has to be there when you look for it and quiet
    // when you do not. The button ends a program, so it has to be findable at a glance — and a
    // colour of its own would have to be chosen in both themes, join the twenty-two of the settings
    // file (**D26**) and be explained in its manual, for a mark two cells wide.
    if let Some(button) = button_within(header, metrics, button) {
        let button = button.rect();
        let reach = CROSS_REACH * button.height().min(button.width());
        let centre = button.center();
        let stroke = egui::Stroke::new(CROSS_THICKNESS, theme.foreground());
        painter.line_segment(
            [
                centre - egui::vec2(reach, reach),
                centre + egui::vec2(reach, reach),
            ],
            stroke,
        );
        painter.line_segment(
            [
                centre + egui::vec2(reach, -reach),
                centre - egui::vec2(reach, -reach),
            ],
            stroke,
        );
    }

    started.elapsed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A metric with round numbers, so the arithmetic below reads as it is written. It deliberately
    /// does not go through the font: asking it needs a window, and what is checked here is the
    /// arithmetic.
    fn metrics() -> CellMetrics {
        CellMetrics::of(8.0, 16.0)
    }

    fn area() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(100.0, 50.0), egui::pos2(500.0, 450.0))
    }

    #[test]
    fn the_strip_is_the_cell_plus_its_two_margins() {
        // **The number is written out and not computed from the constants under test.** An
        // expectation built by calling what is being mutated measures that the code agrees with
        // itself: 16 + 3 + 3, with the metric above.
        assert_eq!(
            height(&metrics(), Headers::PAINTED),
            22.0,
            "the strip's height stopped being the cell's plus its margins, and every console's grid \
             moves with it"
        );
    }

    #[test]
    fn integrated_height_reserves_the_demo_spacing_without_changing_legacy() {
        assert_eq!(height(&metrics(), Headers::PAINTED.integrated(true)), 33.0);
        assert_eq!(height(&metrics(), Headers::PAINTED), 22.0);
        assert_eq!(
            height(&metrics(), Headers::NOT_PAINTED.integrated(true)),
            0.0
        );
    }

    #[test]
    fn a_measured_run_reserves_nothing_and_paints_nothing() {
        assert_eq!(
            height(&metrics(), Headers::NOT_PAINTED),
            0.0,
            "a measured run would lose a row per pane and the eighteen published batches would \
             stop being comparable with it"
        );
        let (header, console) = set_the_header_aside(area(), &metrics(), Headers::NOT_PAINTED);
        assert!(
            header.is_none(),
            "there is a strip to paint in a run that reserved no room for one"
        );
        assert_eq!(
            console,
            area(),
            "the console under the gate has to be the whole pane, which is the grid every published \
             batch was measured with"
        );
    }

    #[test]
    fn the_console_loses_exactly_one_strip_from_its_top() {
        let (header, console) = set_the_header_aside(area(), &metrics(), Headers::PAINTED);
        let header = header.expect("a session that paints headers sets a strip aside");

        assert_eq!(header.top(), area().top());
        assert_eq!(header.height(), 22.0);
        assert_eq!(
            console.top(),
            area().top() + 22.0,
            "the console starts where the strip ends, or the two overlap by whatever the difference is"
        );
        assert_eq!(
            console.bottom(),
            area().bottom(),
            "the cut took height from the bottom as well, which is not what a strip on top does"
        );
        // **The two are contiguous and neither overlaps the other**, which is the property the
        // pointer depends on: a gap would be a band that belongs to nobody, and an overlap a band
        // that answers twice.
        assert_eq!(header.bottom(), console.top());
        assert_eq!(header.left(), console.left());
        assert_eq!(header.right(), console.right());
    }

    #[test]
    fn a_window_shorter_than_the_strip_does_not_invert_the_console() {
        // Ten points of pane against a strip of twenty-two: the cut falls past the bottom, which is
        // what `split_top_bottom_at_y` does not defend against.
        let tiny = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 10.0));
        let (header, console) = set_the_header_aside(tiny, &metrics(), Headers::PAINTED);

        assert!(
            console.top() <= console.bottom(),
            "the console's rectangle came out inverted, and every cell computed from it is nonsense"
        );
        assert!(
            header.expect("a strip is still set aside").bottom() <= tiny.bottom(),
            "the strip is painted past the pane it belongs to, over whatever is below"
        );
        assert_eq!(
            console.height(),
            0.0,
            "a console with no room left has to be empty, not negative"
        );
    }

    #[test]
    fn the_button_is_at_the_right_end_of_the_strip_and_inside_it() {
        let (strip, _) = set_the_header_aside(area(), &metrics(), Headers::PAINTED);
        let strip = strip.expect("a session that paints headers sets a strip aside");
        let button = button_within(
            strip,
            &metrics(),
            Button::in_a_tab_of(&[egui::Rect::NOTHING; 2]),
        )
        .expect("a console with a sibling carries a button")
        .rect();

        // **The corner itself is not the button's**, and that is the point of the assertion: with
        // the button flush against the edge, the press this project already drives at that abscissa
        // — the one the folded panel's handle wants — closed the console instead of moving it.
        assert_eq!(
            button.right(),
            strip.right() - CORNER_LEFT_TO_THE_GRAB,
            "the button took the strip's right-hand corner, where a press means «move this console»"
        );
        assert_eq!(button.width(), 16.0, "two cells of the metric above");
        assert_eq!(
            (button.top(), button.bottom()),
            (strip.top(), strip.bottom()),
            "the button is shorter than the strip it lives in, so there is a band of it that looks \
             pressable and is not"
        );
    }

    #[test]
    fn the_column_the_button_and_the_grab_share_belongs_to_the_grab() {
        // **One abscissa of ambiguity, and it is the one this project already drives a press at.**
        // `Rect::contains` is inclusive, so the button's right edge — which is where the corner left
        // to the folded panel's handle begins — answered yes to both. What it costs is a console
        // **closed** where somebody had reached to move it.
        let (strip, _) = set_the_header_aside(area(), &metrics(), Headers::PAINTED);
        let button = button_within(
            strip.expect("a session that paints headers sets a strip aside"),
            &metrics(),
            Button::in_a_tab_of(&[egui::Rect::NOTHING; 2]),
        )
        .expect("a console with a sibling carries a button");
        let edge = button.rect().right();

        assert!(
            !button.holds(egui::pos2(edge, button.rect().center().y)),
            "the column the button shares with the grab closes the console"
        );
        assert!(
            button.holds(egui::pos2(edge - 1.0, button.rect().center().y)),
            "the column beside it does not close the console either, so the button is not pressable"
        );
    }

    #[test]
    fn a_console_with_no_sibling_carries_no_button() {
        // **AC5's arithmetic half.** What it costs if this stops holding is not a cosmetic
        // difference: on a tab of one console the close would take the tab, and on the last tab the
        // window, with whatever was running inside it.
        let (strip, _) = set_the_header_aside(area(), &metrics(), Headers::PAINTED);
        let strip = strip.expect("a session that paints headers sets a strip aside");

        assert!(
            button_within(
                strip,
                &metrics(),
                Button::in_a_tab_of(&[egui::Rect::NOTHING])
            )
            .is_none(),
            "the only console of its tab carries a button, and one click on it closes the tab"
        );
        assert!(
            button_within(
                strip,
                &metrics(),
                Button::in_a_tab_of(&[egui::Rect::NOTHING; 2])
            )
            .is_some(),
            "a console with a sibling carries no button, so the milestone's gesture does not exist"
        );
    }

    #[test]
    fn a_strip_with_no_room_for_both_keeps_the_handle_and_loses_the_button() {
        // **The narrow window, which nothing stops**: no minimum size is asked of the desktop, and
        // the cut one function above is clamped for exactly that case. Squeezed instead of dropped,
        // the button would take a strip a few points wide **whole** — every press on a console's top
        // row ending its program, and the band that exists to be grabbed grabbable nowhere.
        let narrow = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(40.0, 400.0));
        let (strip, _) = set_the_header_aside(narrow, &metrics(), Headers::PAINTED);
        let strip = strip.expect("a strip is still set aside");

        assert!(
            button_within(
                strip,
                &metrics(),
                Button::in_a_tab_of(&[egui::Rect::NOTHING; 2])
            )
            .is_none(),
            "a strip with no room for the handle and the button still carries the button, so the \
             whole of it closes the console"
        );
        // **And the pane one point wider than the floor does carry one**, or the assertion above
        // would be satisfied by a button that never exists at all.
        let roomy = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(120.0, 400.0));
        let (roomy_strip, _) = set_the_header_aside(roomy, &metrics(), Headers::PAINTED);
        assert!(
            button_within(
                roomy_strip.expect("a strip is set aside"),
                &metrics(),
                Button::in_a_tab_of(&[egui::Rect::NOTHING; 2])
            )
            .is_some(),
            "a pane wide enough for both carries no button"
        );
    }

    #[test]
    fn a_measured_run_paints_headers_only_when_the_batch_asks_for_them() {
        assert!(
            Headers::resolved(crate::settings::Entry::Normal, false).are_painted(),
            "whoever opens the terminal to use it would get no strip, which is the product decision \
             taken the other way"
        );
        assert!(
            !Headers::resolved(crate::settings::Entry::Measuring, false).are_painted(),
            "a measured run would paint strips and the whole published series would stop being \
             comparable"
        );
        assert!(
            Headers::resolved(crate::settings::Entry::Measuring, true).are_painted(),
            "the batch that exists to measure what the strip costs could not paint one"
        );
        // A knob set outside a measured run changes nothing: the strip is already painted.
        assert!(Headers::resolved(crate::settings::Entry::Normal, true).are_painted());
    }
}
