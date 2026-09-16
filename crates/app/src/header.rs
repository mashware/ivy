//! The strip over every console: the space it takes from the pane, and the handle it is grabbed by.
//!
//! **One row per pane, and it is the price of the gesture.** Until now a console was taken somewhere
//! else with `Ctrl+Shift` and the left button (**D20**), which is a chord you have to be told about:
//! nothing on screen says a console can be moved at all. The strip is the thing that says it, and
//! saying it costs every console a row of its own — which is exactly what D20 weighed and refused
//! **as part of F2.6**. What is amended here is not that arithmetic but what pays for it: the
//! measurement bench switches the strip off, so the eighteen published batches keep the grid they
//! were measured with, and what the strip costs **will be** measured in a batch of its own. That
//! batch does not exist yet: no batch of `scripts/measure-frame.sh` asks for a strip, so the span
//! this module returns is structurally zero in every run there is today.
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

/// Whether this session paints a strip over each console.
///
/// **A type and not a `bool`, for the reason the four call sites give**: `can_split(area, rows,
/// metrics, true)` says nothing about what the `true` is, and the one thing that must never happen
/// here is a call site guessing. Same argument as [`crate::settings::Entry`], one step further
/// along: that one says which door the session came in through, this one says what was decided
/// because of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Headers(bool);

impl Headers {
    /// What every session that is not a measurement batch gets, and what the tests build with.
    pub(crate) const PAINTED: Self = Self(true);

    /// A measured run, which is the only regime that goes without.
    pub(crate) const NOT_PAINTED: Self = Self(false);

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
        self.0
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

/// How tall the strip is, or zero where none is painted.
///
/// **The same number feeds the geometry and the painting, and there is no regime in between.** A
/// height reserved for a strip nobody draws is a row stolen from every console, and a strip drawn
/// over a row nobody reserved is a strip drawn on top of somebody's text.
pub(crate) fn height(metrics: &CellMetrics, headers: Headers) -> f32 {
    if !headers.are_painted() {
        return 0.0;
    }
    metrics.height() + 2.0 * MARGIN
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
/// — the very figure **D3** judges egui by. With the gate on, no measured run reaches this function
/// and the number it returns is zero in every published batch; that is the point, not an excuse for
/// leaving it undeclared.
#[must_use = "it carries what the strip cost the frame. Dropping it publishes that cost as the \
              framework's, which is the figure D3 judges egui by (D13)"]
pub(crate) fn paint(
    painter: &egui::Painter,
    header: egui::Rect,
    theme: &crate::theme::Theme,
    metrics: &CellMetrics,
) -> std::time::Duration {
    let started = std::time::Instant::now();

    // **The panel's background and not a colour of its own.** The strip is chrome the application
    // owns, exactly like the status panel beside it, and a new colour would have to be chosen in
    // both themes, added to the twenty-two of the settings file (**D26**) and explained in its
    // manual — for a band whose whole job is to be distinguishable from the console under it.
    painter.rect_filled(header, 0.0, theme.panel_background());

    // **The grip is centred on the strip and not flush with a corner**, because the corners are
    // spoken for: the close button lands on the right in the next delivery, and the left of a pane
    // is where the tree's divider ends.
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
