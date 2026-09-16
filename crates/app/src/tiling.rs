//! Rectangles, weights, and the arithmetic that shares an area out.
//!
//! **It knows no pane and no window.** What lives here answers «given this rectangle, this
//! orientation and these weights, where does each part go» — and nothing else. The tree that decides
//! *which* parts there are is [`crate::tree`]; no console is created, closed or read here.
//!
//! **It does know how small a console may be**, and that is the one exception rather than a hole in
//! the sentence above: [`can_split`] has to answer whether an area has room for two, and «room» is
//! measured in a console's columns and rows ([`MINIMUM_ROWS`], [`panel::MINIMUM_CONSOLE`]). It reads
//! that floor; it does not own it and it cannot reach a pane to ask.
//!
//! It came out of `lib.rs` under **D28**, which gives a module a ceiling of 600 production lines and
//! records what the window's core is at so the number can only go down.

use crate::painter::CellMetrics;
use crate::panel;
use eframe::egui;

/// How many points of width the line that separates the two consoles takes.
///
/// Two and not one: with one the line is visible, but it is also the drag's grab zone, and one
/// point is impossible to catch with the mouse.
///
/// **The grab zone is exactly this line, without widening**, and what that buys is that there is no
/// invisible sensitive surface: it is grabbed where it is seen. If two points turn out to be few
/// with the window in front of you, what is raised is this constant, and the line looks thicker as
/// a consequence.
///
/// **What it does not buy —and an earlier version of this comment said it did— is preventing the
/// same gesture from changing the focus.** The divider's edge *is* the edge of the console next to
/// it, and egui counts a point on the edge as a direct hit of the rectangle
/// (`Rect::distance_sq_to_pos` gives zero there), so in the two border columns both widgets are
/// hit: the divider receives the drag and the console receives the click. It is not corrected
/// because it is not a defect — whoever presses on the left edge of the line has pressed on the
/// right edge of the left-hand console, and giving the focus to that one is what is to be
/// expected—; it is written down so that nobody reads it as a guarantee that does not exist.
pub(crate) const DIVIDER_THICKNESS: f32 = 2.0;

/// Which side of a console the one arriving is put on.
///
/// **It exists so that inserting can say «before» as well as «after».** Splitting always puts the
/// new one behind, and that was enough while the only thing that inserted was the keyboard chord;
/// replacing needs all four, because dropping on the left and dropping on the right are different
/// layouts.
///
/// **Four variants and not an orientation plus a boolean**, even though inside it is exactly that:
/// whoever calls has a mouse gesture in their hand and knows how to say «top», not «in rows,
/// before».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// Which way it has to cut to put something on this side.
    pub(crate) fn cut_along(self) -> Orientation {
        match self {
            Self::Left | Self::Right => Orientation::Columns,
            Self::Top | Self::Bottom => Orientation::Rows,
        }
    }

    /// Whether the one arriving goes **before** the one that was already there, in the order they
    /// are painted in.
    pub(crate) fn before(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }

    /// The side that means «behind», which is the only thing inserting knew how to say before the
    /// four.
    ///
    /// The two splitting chords still want exactly that: `Ctrl+Shift+D` leaves the new console **on
    /// the right** and `+B` leaves it **below**, and this milestone does not change that promise.
    pub(crate) fn behind(cut_along: Orientation) -> Self {
        match cut_along {
            Orientation::Columns => Self::Right,
            Orientation::Rows => Self::Bottom,
        }
    }
}

/// What tiling the window opens with: the two consoles equal.
pub(crate) const INITIAL_FRACTION: f32 = 0.5;

/// The least height a console is left with when it is split, in rows.
///
/// **Three, and the number comes from what a console has to be able to show to go on being one:**
/// what is written, its answer, and the prompt coming back. With two, any command that answers one
/// line already wipes the previous prompt and there is nothing left to read.
///
/// It is the brother of [`panel::MINIMUM_CONSOLE`] —the twenty columns of width— for the other
/// axis, and the same number does not do: twenty rows are a whole window and twenty columns are
/// half a word. What they share is the rule, not the figure.
const MINIMUM_ROWS: f32 = 3.0;

/// Which way an area is cut when it is tiled.
///
/// **It is named for what it produces and not for the axis it cuts**, which is how it reads out
/// loud: tiling «in columns» leaves the parts side by side. Naming it for the axis —«horizontal»—
/// forces you to stop and think whether that means the cut or the result, and both readings are
/// defensible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Orientation {
    /// Side by side; the divider is a vertical line and what is tiled is the width.
    Columns,
    /// One above the other; the divider is a horizontal line and what is tiled is the height.
    Rows,
}

impl Orientation {
    /// How much the area measures in the direction it is tiled.
    pub(crate) fn length_of(self, area: egui::Rect) -> f32 {
        match self {
            Self::Columns => area.width(),
            Self::Rows => area.height(),
        }
    }

    /// Which cursor a border of this direction asks for: the one that stretches where it really
    /// moves.
    pub(crate) fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::Columns => egui::CursorIcon::ResizeHorizontal,
            Self::Rows => egui::CursorIcon::ResizeVertical,
        }
    }

    /// How much there is from the start of the area to that point, in the direction it is tiled.
    pub(crate) fn offset_from_the_start(self, area: egui::Rect, point: egui::Pos2) -> f32 {
        match self {
            Self::Columns => point.x - area.left(),
            Self::Rows => point.y - area.top(),
        }
    }

    /// The rectangle that goes from `starts_at` to `ends_at` measured in the direction it is tiled,
    /// taking up the whole area in the other.
    fn slice(self, area: egui::Rect, starts_at: f32, ends_at: f32) -> egui::Rect {
        match self {
            Self::Columns => egui::Rect::from_min_max(
                egui::pos2(area.left() + starts_at, area.top()),
                egui::pos2(area.left() + ends_at, area.bottom()),
            ),
            Self::Rows => egui::Rect::from_min_max(
                egui::pos2(area.left(), area.top() + starts_at),
                egui::pos2(area.right(), area.top() + ends_at),
            ),
        }
    }
}

/// What a part of the tiling gets, **in proportion to its siblings**.
///
/// **A type and not a loose `f32`, and it is not ceremony.** The tiling divides each part's weight
/// by the sum of those of all of them, so a sum of zero gives `NaN`; and a `NaN` put into a
/// rectangle propagates to everything painted from it, without failing in the place where it was
/// generated. Forbidding zero **in the constructor**, the case stops existing, instead of depending
/// on every path that creates weights remembering to check it. It is the same criterion with which
/// [`panel::PanelTiling`] keeps the panel's rectangle and its panel together: not leaving writable
/// a state that cannot happen.
///
/// **Weight and not a normalized fraction**, which is the other way of doing this: with fractions
/// that have to add up to one, removing a part forces sharing its gap out among the rest by hand,
/// and «they add up to one» becomes an invariant somebody will break. With weights, removing a part
/// is removing it from the list and the rest keep their proportion without touching a single
/// number.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Weight(f32);

impl Weight {
    /// A weight, if the number can be one.
    ///
    /// **It rejects zero, negatives and what is not a number.** Zero because a part with no weight
    /// is not a part; negatives because they would invert the order of the cuts; and `NaN` or
    /// infinity because they contaminate the sum and with it the whole tiling.
    ///
    /// **Visible in the whole crate since the layout is saved**, so that the module that validates
    /// what is read from disk goes through here instead of reimplementing the check. Two copies of
    /// this invariant is one too many: the sum of weights equal to zero puts a `NaN` in a
    /// rectangle, and this type exists so that it cannot be built.
    pub(crate) fn new(value: f32) -> Option<Self> {
        (value.is_finite() && value > 0.0).then_some(Self(value))
    }

    /// The smallest weight that is allowed to be built when rescaling; see [`Weight::scaled`].
    ///
    /// **The smallest normal one and not the smallest representable one**: below this are the
    /// subnormals, which are valid weights —positive and finite— but whose product reaches zero
    /// **in a single step**, so accepting them only postpones the problem by one tiling.
    pub(crate) const SMALLEST: f32 = f32::MIN_POSITIVE;

    /// The weight that comes out of rescaling another, **always**.
    ///
    /// **It exists because the tiling multiplies two weights and the product can stop being a
    /// weight.** Closing a pane raises the granddaughters to siblings with the scaled weight
    /// —`group_weight · (weight / total)`—, and that product **underflows to zero** with small
    /// numbers: checked by running it, `1·10⁻⁴⁰ · 1·10⁻⁴⁰ / 2·10⁻⁴⁰` gives **exactly zero**. And
    /// those numbers are reached **with gestures**, not only with a file: the drag clips the
    /// **fraction** to one per cent, not the weight, so what it leaves is one per cent of what the
    /// two neighbors add up to — a sum that shrinks with every drag and **has no floor**.
    ///
    /// **This used to be an `expect` with a comment claiming that no gesture got there, and it was
    /// false.** The damage was not the panic —for that you need weights of 10⁻⁴⁰— but the one next
    /// to it: while the column floor rescues the collapsed console on screen, **the real weight
    /// goes on dropping**, and a format that rejected it would throw away the user's layout without
    /// anything being seen. With the answer here, the impossible state stops existing in the place
    /// where it is produced and whoever saves or validates does not have to know anything about
    /// this.
    ///
    /// **A floor and not a clip on both sides, and the reason is NOT that weights do not grow.**
    /// That was claimed first and it is false: what a drag preserves is **the sum of the pair**, so
    /// an individual weight rises up to 99 % of what the two neighbors added up to —two weights of
    /// a million give 1.98 million in a single drag, measured with real `f32`—. What really does
    /// not grow is **the sum of a branch**: dragging preserves it, splitting divides a weight into
    /// two halves that add up to the same, and closing shares the group's weight out among its
    /// granddaughters. And that is the quantity the tiling depends on, which divides each weight by
    /// the sum of its branch.
    ///
    /// With the sum bounded and with closing's rescaling written as `group_weight · (w / total)`
    /// instead of `(group_weight · w) / total` —the left-hand factor does not go above one, so the
    /// product cannot go above `group_weight`—, **no path is left that overflows from above**, and
    /// a cap would be a mechanism without a scenario.
    ///
    /// And the floor covers what is not a number into the bargain, because `max` with a `NaN`
    /// returns the other operand. Clipping from below does not move any console perceptibly: the
    /// column floor rules many orders of magnitude earlier.
    ///
    /// **What is not finite is clipped even though there is no path that reaches here with an
    /// infinity**, and the reason is not defensive: it is that **this type cannot have two notions
    /// of a valid weight**. `new` demands finite, so a `scaled` that let an infinity through would
    /// build what the other constructor rejects, and the invariant would come to depend on which
    /// door was used.
    pub(crate) fn scaled(value: f32) -> Self {
        if !value.is_finite() {
            return Self(Self::SMALLEST);
        }
        Self(value.max(Self::SMALLEST))
    }

    pub(crate) fn value(self) -> f32 {
        self.0
    }
}

/// A line that separates two parts, with what is needed to be able to drag it.
///
/// **It carries the span the two it separates take up, and not its branch's area.** Dragging shares
/// the weight **between those two** preserving what they add up to, so the fraction has to be
/// measured inside its span: measured against the whole branch, moving a divider of three siblings
/// would give a jump as soon as the pointer went over the third.
#[derive(Clone, Copy)]
pub(crate) struct Divider {
    /// Where it is painted and where it is grabbed.
    pub(crate) rect: egui::Rect,
    /// Which way it tiles, which decides the drag's axis and the cursor's shape.
    pub(crate) orientation: Orientation,
    /// The span of the two parts it separates, counting the line in the middle.
    pub(crate) between_the_two: egui::Rect,
}

/// The rectangles the parts of an area take up and the lines that separate them.
///
/// **Two lists and not a pair of fields named for a side.** The names «left» and «right» stop
/// describing anything as soon as the cut can be horizontal or the parts can be three, and a field
/// that lies is worse than an index.
pub(crate) struct Tiling {
    /// One per part, in the order they were asked for.
    pub(crate) parts: Vec<egui::Rect>,
    /// One between each two consecutive parts, so there is **one fewer** than parts — and **none**
    /// when the part is a single one, which is what makes a pane with no company look the same as
    /// before the tiling existed.
    pub(crate) dividers: Vec<Divider>,
}

/// Tiles an area among its parts according to what each one weighs, without leaving any without a
/// place.
///
/// **The dividers' thickness is discounted first and what is left is tiled.** If the whole area
/// were split and then the lines subtracted, the part they were subtracted from would come out
/// narrower than the others without anybody having asked for that.
///
/// **Each part gets what it weighs out of what all of them weigh, and that is what makes the
/// proportion survive resizing**: on changing the area all the lengths are recomputed from the same
/// weights, instead of one keeping its points and the other eating the whole difference. That no
/// weight can be zero or `NaN` is guaranteed by [`Weight`], so the sum is **never zero** and the
/// division here cannot produce a `NaN` that propagates to the rectangles.
///
/// **What the `minimum` floor per part buys is modest, and it is best not to exaggerate it.** It
/// avoids no panic —the one in `Screen::resize` with zero columns is covered by the 1×1 minimum of
/// [`CellMetrics::cells_in`]— and it does not save the grid either: with zero pixels that same
/// function already gives one column, exactly the one it gives with the floor in place. And **the
/// handle is not the console, it is the divider**, which is still on screen and still grabbable
/// with the part collapsed, so it is recovered with the same gesture that collapsed it. What the
/// floor achieves is that the collapsed part **goes on showing its column** instead of nothing, so
/// that the collapse reads as something that has been done and not as a painting failure, and that
/// the handle does not end up stuck to the window's edge.
///
/// **The floor is lowered to tiling in equal parts when it does not fit for all of them.** That way
/// the dividers end up shared out and the parts come out equal —below the minimum, which is the
/// only thing possible there— without needing a separate branch. And it is not only brevity:
/// `f32::clamp` **panics if the minimum exceeds the maximum**, and lowering the floor makes that
/// impossible by construction, instead of depending on a guard covering every case.
///
/// The `max` with zero over `usable` keeps the rectangles well formed with an area narrower than
/// the dividers themselves. There the parts get down to zero points: there is no tiling possible,
/// and what is preserved is that none of them comes out inverted.
///
/// **With a single part there is no divider at all**, and therefore no thickness is discounted from
/// it either: it takes the whole area. It is what makes removing a pane's company leave it exactly
/// as it was before there was any tiling.
pub(crate) fn tile(
    area: egui::Rect,
    orientation: Orientation,
    weights: &[Weight],
    minimum: f32,
) -> Tiling {
    let how_many = weights.len();
    if how_many == 0 {
        return Tiling {
            parts: Vec::new(),
            dividers: Vec::new(),
        };
    }

    let length = orientation.length_of(area);
    let gaps = (how_many - 1) as f32;
    let usable = (length - gaps * DIVIDER_THICKNESS).max(0.0);
    let floor = minimum.min(usable / how_many as f32);
    let total: f32 = weights.iter().map(|weight| weight.value()).sum();

    // **The lengths are computed first and placed afterwards, and it is not a detour.** What each
    // part measures lives in the **usable** space —the area minus the dividers— while where it
    // starts lives in the **real** space. Mixing the two in a single accumulator is what makes the
    // last part fall short by exactly the lines' thickness, and that failure is not visible with
    // two equal parts.
    let mut lengths = Vec::with_capacity(how_many);
    let mut handed_out = 0.0;
    for (which, weight) in weights.iter().enumerate() {
        // **The floor is applied per part and not at the end**, so that one that takes almost
        // everything cannot leave the next without its minimum. The ceiling is what is left after
        // reserving the floor of the ones still missing, so the last one always finds a place.
        let left_to_place = (how_many - which - 1) as f32;
        let ceiling = (usable - handed_out - left_to_place * floor).max(floor);
        let length_of_this_one = (usable * weight.value() / total).clamp(floor, ceiling);
        lengths.push(length_of_this_one);
        handed_out += length_of_this_one;
    }

    let mut parts = Vec::with_capacity(how_many);
    let mut dividers = Vec::with_capacity(how_many - 1);
    let mut starts_at = 0.0;
    for (which, length_of_this_one) in lengths.iter().enumerate() {
        let is_last = which + 1 == how_many;
        // **The last one reaches the edge instead of reaching where it was due.** When some floor
        // has acted, the sum of the lengths does not give the exact usable and a slit would be left
        // against the edge; stretching it is what preserves that the parts and the dividers cover
        // the area **exactly**, which is the invariant this tiling exists for. It is the same the
        // tiling of two did by taking the right-hand one up to `area.right()`.
        let ends_at = if is_last {
            length.max(starts_at)
        } else {
            starts_at + length_of_this_one
        };

        parts.push(orientation.slice(area, starts_at, ends_at));
        if !is_last {
            // The span of the two it separates goes from where this one starts to where the next
            // one ends, and the next one is not placed yet: it is computed with its length, which
            // is known.
            let spans_to = ends_at + DIVIDER_THICKNESS + lengths[which + 1];
            dividers.push(Divider {
                rect: orientation.slice(area, ends_at, ends_at + DIVIDER_THICKNESS),
                orientation,
                between_the_two: orientation.slice(area, starts_at, spans_to),
            });
        }
        starts_at = ends_at + DIVIDER_THICKNESS;
    }

    Tiling { parts, dividers }
}

/// The two weights that tile an area between two parts according to the fraction the first one
/// takes.
///
/// **It exists because the fraction is what the tab saves today**, and translating it in one place
/// —instead of in every place that tiles— is what allows that the day the tab saves real weights
/// there is no need to hunt down loose conversions.
///
/// **The fraction is clipped before converting it, and that changes something observable**: until
/// now it could be worth exactly zero, and the length floor rescued the collapsed part on every
/// frame. A zero weight does not exist ([`Weight`]), so it is clipped to one per cent — which with
/// the column floor in place gives the same width in any normal window, and in a very wide one lets
/// the collapsed one grow a little instead of leaving it nailed to the minimum forever.
pub(crate) fn weights_of(fraction: f32) -> [Weight; 2] {
    const SMALLEST_FRACTION: f32 = 0.01;
    let first_part = if fraction.is_finite() {
        fraction.clamp(SMALLEST_FRACTION, 1.0 - SMALLEST_FRACTION)
    } else {
        INITIAL_FRACTION
    };

    [
        // **`expect` and not a silent fallback.** The clip above leaves `first_part` inside a range
        // where both weights are valid, so these two calls cannot fail today. An `unwrap_or` would
        // say the opposite: it would turn an impossibility into a plausible tiling in equal parts,
        // and the day somebody touched the clip the defect would show as «the divider has put
        // itself in the center on its own» — a symptom that does not point at its cause. Every test
        // of the tiling goes through here, so breaking the invariant falls loudly and on the spot.
        Weight::new(first_part)
            .expect("the clip leaves the fraction inside the range a weight accepts"),
        Weight::new(1.0 - first_part).expect("and so does its complement"),
    ]
}

/// The half of a console the one arriving by that side would take up.
///
/// **It is the half and not a thin strip**, because it is literally the place it is going to take
/// up: receiving another console splits the slot in half. A narrower mark would read as «it goes
/// here at the edge», which is what window managers where dropping on an edge docks to the screen
/// do — and that does not happen here.
pub(crate) fn half(pane_area: egui::Rect, side: Side) -> egui::Rect {
    let center = pane_area.center();
    match side {
        Side::Left => {
            egui::Rect::from_min_max(pane_area.min, egui::pos2(center.x, pane_area.bottom()))
        }
        Side::Right => {
            egui::Rect::from_min_max(egui::pos2(center.x, pane_area.top()), pane_area.max)
        }
        Side::Top => {
            egui::Rect::from_min_max(pane_area.min, egui::pos2(pane_area.right(), center.y))
        }
        Side::Bottom => {
            egui::Rect::from_min_max(egui::pos2(pane_area.left(), center.y), pane_area.max)
        }
    }
}

/// Which side of that console the pointer falls on: the edge it has nearest.
///
/// **They are compared in proportion to the side they belong to, and for that it multiplies
/// crosswise instead of dividing.** In points, a wide, low console has the top and bottom edges
/// much nearer than the side ones, so without normalizing it could only be dropped above or below
/// however close to the left edge it was dropped. Multiplying by the other side gives the same
/// comparison **and** avoids dividing by the width of a console that has been left at zero, which
/// would give a meaningless number precisely in the rare case.
///
/// Ties fall on the left and top side. It does not matter which: what matters is that it is always
/// the same one, so that the same point does not give two answers on two consecutive frames.
pub(crate) fn side_of(pane_area: egui::Rect, pointer: egui::Pos2) -> Side {
    let (to_the_left, to_the_right) = (pointer.x - pane_area.left(), pane_area.right() - pointer.x);
    let (to_the_top, to_the_bottom) = (pointer.y - pane_area.top(), pane_area.bottom() - pointer.y);

    let horizontal = to_the_left.min(to_the_right) * pane_area.height();
    let vertical = to_the_top.min(to_the_bottom) * pane_area.width();

    if horizontal <= vertical {
        if to_the_left <= to_the_right {
            Side::Left
        } else {
            Side::Right
        }
    } else if to_the_top <= to_the_bottom {
        Side::Top
    } else {
        Side::Bottom
    }
}

/// Whether a pane of this size can be split without leaving either of the two daughters below the
/// minimum that makes a console usable.
///
/// **The limit on the number of panes comes from here and not from an invented ceiling.** A maximum
/// —four, eight— cannot be justified with any real scenario: splitting demands a human gesture per
/// pane, and the memory each one costs (~1 MiB of queues) is nowhere near. What can be justified is
/// that a console of three columns is no use for anything, and that floor makes the limit **emerge
/// from the geometry**: in a normal window you end up unable to split long before anything runs
/// out.
///
/// **It is measured over the whole pane, panel included, and not over what is left to the
/// console.** The panel yields until it disappears before touching the console's columns
/// ([`panel::set_aside`]), so a daughter that keeps the pane's minimum keeps the console minimum
/// whatever happens with its producer — whereas measuring over today's console would make splitting
/// depend on whether somebody has published a `panel.json` in the last second.
pub(crate) fn can_split(
    pane_area: egui::Rect,
    cut_along: Orientation,
    metrics: &CellMetrics,
    headers: crate::header::Headers,
) -> bool {
    let minimum = match cut_along {
        Orientation::Columns => panel::MINIMUM_CONSOLE * metrics.width(),
        // **The strip counts towards the minimum, and it has to count here.** Each daughter gets one
        // of its own, so a pane that fits two consoles of three rows no longer fits them once each
        // owes a strip as well: without this the split is allowed and both consoles come out below
        // the floor this function exists to keep.
        Orientation::Rows => {
            MINIMUM_ROWS * metrics.height() + crate::header::height(metrics, headers)
        }
    };

    cut_along.length_of(pane_area) >= 2.0 * minimum + DIVIDER_THICKNESS
}

/// The weights a branch of `how_many` children is born with.
///
/// **With two they are [`INITIAL_FRACTION`]'s, and with more, equal.** That constant says where
/// **the** divider is born, and there is only «the» divider when there are two; tiling equal with
/// two as well would move the divider of the layout of always the day somebody adjusts the
/// constant, which is just what the constant exists to be able to do.
pub(crate) fn weights_for(how_many: usize) -> Vec<Weight> {
    if how_many == 2 {
        return weights_of(INITIAL_FRACTION).to_vec();
    }

    // **`expect` and not a fallback, for the same reason as in [`weights_of`]**: one is always a
    // valid weight, and an `unwrap_or` would turn an impossibility into a plausible tiling.
    vec![Weight::new(1.0).expect("one is a valid weight"); how_many]
}
