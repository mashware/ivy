//! The strip of cards at the top: painting it, clicking it, and the space it takes from the grid.
//!
//! **One strip, coupled to nothing else.** It decides where the bar sits, what each card's rectangle
//! is, which card a click landed on, and what the pass cost. Everything a card *says* comes from
//! elsewhere — the claim a tab carries is its panes' ([`crate::tree::Tab::attention`]), and the
//! symbols and colors are the panel's.
//!
//! **With a single tab there is no bar**, and [`set_the_tab_bar_aside`] is where that is decided: the
//! grid then is identical to the one before tabs existed, which is what makes the measurements from
//! either side of **D13** comparable at all.
//!
//! **The bar is painted last in the frame**, and that is an invariant rather than an ordering
//! preference: the panels are not re-read until the end of the pass, so painting it earlier left
//! every card one frame behind — which in a window that only redraws on change is forever.
//!
//! It came out of `lib.rs` under **D28**, and the rectangle arithmetic came with it. A reviewer of an
//! earlier delivery in this train pointed at [`tab_rect`] and [`set_the_tab_bar_aside`] as geometry
//! that belonged in [`crate::tiling`]; they are geometry, but they are **this strip's** geometry, and
//! `tiling` is the module that knows no console and no tab.

use crate::painter::CellMetrics;
use crate::{Window, WindowRequest, panel};
use eframe::egui;
use std::time::{Instant, SystemTime};

/// What the window takes away from having painted the tab bar.
///
/// **Both are worth it for the report, and that is why they travel together**: the cost is
/// subtracted from the frame and the marked cards are what tells apart a batch that measured the
/// bar **with** its new work from one that measured it without it.
#[must_use = "it carries the cost the frame has to subtract and the cards the report has to declare. \
              Dropping the cost publishes it as the framework's, which is the figure D3 judges egui \
              by; dropping the count publishes ZERO marked cards, and the measurement script \
              invalidates a whole run over a bar that marked none"]
pub(crate) struct TabBarOutcome {
    pub(crate) cost: std::time::Duration,
    /// How many cards carried a symbol in this pass.
    pub(crate) marked: usize,
}

/// How much air the bar leaves above and below the tab's number.
///
/// It is the only constant thing about its height: the rest comes from the font's metric, so the
/// bar grows with the font size instead of falling short when it is raised.
///
/// **`pub(crate)` for the tests, for the same reason as [`tab_rect`] and with the same refutation
/// behind it**: its outside callers are the pane-floor test and a shared helper, both of which sit
/// with the window builders in `lib.rs`'s test module. See that item's note.
pub(crate) const TAB_BAR_MARGIN: f32 = 4.0;

/// Sets aside the bar's place at the top, and returns what is left to the consoles.
///
/// **With a single tab there is no bar, and that is not an aesthetic convenience.** It is what
/// leaves the grid identical to the one from before the tabs, and that equality is the control
/// batch of D3's annex: without it, the bar would take height off the consoles just as the panel
/// took columns off them, the frame would come out **cheaper** by painting fewer cells and the
/// annex would publish an improvement that does not exist.
///
/// **The cut is bounded, because [`egui::Rect::split_top_bottom_at_y`] does not do it** — read in
/// its source (`emath-0.36.1/src/rect.rs:671`), where it builds the two rectangles with the cut as
/// it comes. With the window lower than the bar, a cut below the area would leave the consoles'
/// rectangle **inverted**, with its top edge below its bottom one. It is the same care [`crate::tiling::tile`]
/// already takes with the width, and the same edge case that is tested there.
pub(crate) fn set_the_tab_bar_aside(
    area: egui::Rect,
    tabs: usize,
    metrics: &CellMetrics,
) -> (Option<egui::Rect>, egui::Rect) {
    if tabs < 2 {
        return (None, area);
    }

    let height = metrics.height() + 2.0 * TAB_BAR_MARGIN;
    let cut_along = (area.top() + height).min(area.bottom());
    let (tab_bar, consoles) = area.split_top_bottom_at_y(cut_along);

    (Some(tab_bar), consoles)
}

/// Where a tab's card falls inside the bar.
///
/// **All of the same width, and with no scrolling.** The number is clipped when they stop fitting
/// —hence whoever paints it clips to this rectangle—, which leaves it legible up to some fifteen or
/// twenty tabs. Beyond that you navigate by shortcut, which is how you navigate anyway as soon as
/// there are more than three.
///
/// There is no need to guard against `how_many` being zero: there are only cards when there is a
/// bar, and there is only a bar with two tabs or more.
///
/// **It is `pub(crate)` for the tests and it stays that way, which needs saying because the opposite
/// was tried.** No production code outside this module calls it. The visibility was reported as
/// avoidable — [`crate::pointer`]'s header sets that standard, and the delivery that wrote it
/// collapsed its own surface from eighteen items to two — and closing it does compile, so the
/// reading looked right. What refutes it is where the callers are: **eight of them, across three
/// different subjects**, and none of them is about this strip.
///
/// - Five are the tab card's **claim rollup** (`a_background_tabs_card_claims_…` and its four
///   siblings): a different milestone, which uses this only to find the card it is asserting about.
/// - One is the **pane floor** (`with_the_tab_bar_in_place_a_pane_stops_fitting_below`), which is
///   about splitting, not about the bar.
/// - Two are shared test helpers (`clear_color`, `tab_bar_of`) that half the suite builds on.
///
/// All eight live in `lib.rs`'s test module **with the window builders** — `window_of_tabs`,
/// `area_of`, `ui_pass`, `with_metrics` — so moving them here would mean opening those four instead,
/// and trading two items for four is not a narrower seam. **A closed item whose tests cannot reach
/// it is not progress.**
///
/// Watch out for how the compiler reports this: closing the item breaks the single `use` in
/// `lib.rs`'s test module and the error names **that import**, not the eight call sites behind it.
/// One error reads like one easy fix.
pub(crate) fn tab_rect(tab_bar: egui::Rect, which: usize, how_many: usize) -> egui::Rect {
    let width = tab_bar.width() / how_many as f32;
    let left_x = tab_bar.left() + width * which as f32;

    egui::Rect::from_min_max(
        egui::pos2(left_x, tab_bar.top()),
        egui::pos2(left_x + width, tab_bar.bottom()),
    )
}

impl Window {
    /// Changes tab when a tab card is clicked.
    ///
    /// It lives apart from the painting for the same reason as [`Self::handle_the_mouse`], and it
    /// uses `Sense::CLICK` for the same reason too: `Sense::click()` adds `FOCUSABLE`, that is, the
    /// card **would ask for egui's keyboard focus** and would compete with the application's
    /// routing, which is geographic.
    pub(crate) fn handle_tab_bar_clicks(
        &mut self,
        ui: &egui::Ui,
        tab_bar: egui::Rect,
        metrics: &CellMetrics,
    ) {
        for which_tab in 0..self.tabs.len() {
            let id = ui.id().with(("tab", which_tab));
            if ui
                .interact(
                    tab_rect(tab_bar, which_tab, self.tabs.len()),
                    id,
                    egui::Sense::CLICK,
                )
                .clicked()
            {
                // **Through the same door as the keyboard, instead of writing `active` by hand.**
                // They are the same transition, and having it in two places already cost a defect:
                // the click did not ask for a frame, so with two quiet consoles —this milestone's
                // case— the tab just shown was stuck with the start-up grid until input from
                // something else arrived.
                self.apply(
                    WindowRequest::Go(which_tab as isize - self.active as isize),
                    ui.ctx(),
                    ui.max_rect(),
                    metrics,
                );
            }
        }
    }

    /// Paints the bar: one card per tab, with its number inside.
    ///
    /// **The numbers that are seen start at one**, not at zero like the consoles' in the report:
    /// that is an instrument and this is what whoever has the window in front of them reads.
    ///
    /// The clip to the card is what holds up the equal-shares tiling with no scrolling: when the
    /// number stops fitting it is cut off inside its own card instead of invading the neighbor's.
    ///
    /// **And each card says whether that tab claims you**, with the same symbol and the same color
    /// as the panel of the pane that claims it. **The symbol goes flush left and the number stays
    /// where it was**: centring the two together would move the number every time a producer
    /// publishes or stops publishing, and a card whose number dances reads worse than one with a
    /// gap on the left.
    ///
    /// **A single clock for all the cards**, passed in from outside. It is not the same one as the
    /// panels' —each pane takes its own when it is painted, and the background ones share another—,
    /// and it does not need to be: what this avoids is **two cards on the same bar** disagreeing
    /// about the minute in which one of them turns stale.
    ///
    /// **It returns what it cost, and the stopwatch lives in here and not in the caller.** This
    /// project has already turned down the clock in the caller twice, with the reason measured:
    /// from outside, the span covers the whole call whatever is done inside, so **taking work out
    /// of the measured region breaks no test**. Checked again here — with the clock outside, moving
    /// the walk over the claims out left the whole suite green, and that walk is the half this
    /// milestone added.
    pub(crate) fn paint_the_tab_bar(
        &self,
        ui: &egui::Ui,
        tab_bar: egui::Rect,
        metrics: &CellMetrics,
        now: SystemTime,
    ) -> TabBarOutcome {
        let started = Instant::now();
        let mut marked = 0;
        ui.painter_at(tab_bar)
            .rect_filled(tab_bar, 0.0, self.theme.background());

        for which_tab in 0..self.tabs.len() {
            let tab_area = tab_rect(tab_bar, which_tab, self.tabs.len());
            let painter = ui.painter_at(tab_area);
            let is_the_active_one = which_tab == self.active;

            // The active one is filled with the same color that marks the console with focus: both
            // say «the keyboard goes here», at two different scales, and giving them two colors
            // would say they are two states with nothing to do with each other.
            if is_the_active_one {
                painter.rect_filled(tab_area, 0.0, self.theme.focus());
            }

            painter.text(
                tab_area.center(),
                egui::Align2::CENTER_CENTER,
                // The number is passed **without converting it to text**: `Painter::text` takes an
                // `impl ToString` and calls `to_string` inside, so giving it a ready-made `String`
                // clones it and allocates twice per tab and per frame.
                which_tab + 1,
                metrics.font(),
                if is_the_active_one {
                    self.theme.background()
                } else {
                    self.theme.foreground()
                },
            );

            // **The active one is marked too.** In the tab you are looking at the claim can come
            // from the pane next door, and not marking it would be one more conditional rule to
            // maintain.
            if let Some(attention) = self.tabs[which_tab].attention(now) {
                // **The symbol is painted over the bar's background, in the active card too.**
                // That card is filled with the focus color, and on top of it these colors **cannot
                // be seen**: measured, the blocking red gives 1.01 of contrast and the dimmed
                // yellow 1.02 — that is, the same color as far as reading it goes. The number
                // beside it is already inverted for this very reason, but inverting the symbol
                // would be no good: **its color is the class**, and changing it would say something
                // else. What is changed is what lies behind.
                if is_the_active_one {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            tab_area.left_top(),
                            egui::vec2(2.0 * TAB_BAR_MARGIN + metrics.width(), tab_area.height()),
                        ),
                        0.0,
                        self.theme.background(),
                    );
                }
                painter.text(
                    egui::pos2(tab_area.left() + TAB_BAR_MARGIN, tab_area.center().y),
                    egui::Align2::LEFT_CENTER,
                    panel::attention_symbol(attention.class),
                    metrics.font(),
                    self.theme
                        .attention_color(attention.class, attention.backed),
                );
                marked += 1;
            }
        }

        TabBarOutcome {
            cost: started.elapsed(),
            marked,
        }
    }
}

#[cfg(test)]
mod tests {
    //! **The one thing in this module that is not a move, guarded.**
    //!
    //! Everything else here came out of `lib.rs` unchanged and is covered by the tests that were
    //! already watching it from there. The `#[must_use]` on [`super::TabBarOutcome`] is the
    //! exception: it was added by the delivery that created this module, and **deleting it left the
    //! whole suite green** — the shape **D30** exists to refuse.

    /// How many lines after a call the outcome has to have been spent.
    ///
    /// It is not a style rule: the three writes are deliberately on consecutive lines in the caller,
    /// with a comment saying why («noting and subtracting are the same decision, so they are taken
    /// on the same line»). A window this size is loose enough not to break when somebody adds a
    /// comment between them and tight enough that spending the outcome in another function is not
    /// mistaken for spending it here.
    const LINES_TO_SPEND_IT: usize = 12;

    /// **Both numbers are spent at every call, which is the failure `#[must_use]` cannot catch.**
    ///
    /// The attribute fires when the whole value is dropped, and that is *not* the failure the
    /// caller's own comment fears: it fears **half** of it — subtracting the cost and forgetting to
    /// note the marked cards, or the other way round. `#[must_use]` is silent on that, and silent on
    /// `let _ = …` as well, so on its own it guards the narrow case and leaves the likely one open.
    ///
    /// What makes the omission expensive is the subtraction. A frame's cost is split by taking each
    /// declared piece off the total (**D13**), so a bar whose cost is never subtracted is published
    /// as **the framework's** — the figure **D3** judges egui by. And the marked cards are what tell
    /// a two-tab batch that measured the claim rollup from one that measured it before there was
    /// one: the measurement script invalidates a run whose bar declares nothing.
    ///
    /// So this walks the callers instead of trusting the lint. It is a text walk and it says so:
    /// it cannot prove the lint fires, only that no call site spends less than the whole outcome.
    #[test]
    fn every_caller_spends_both_halves_of_the_outcome() {
        let core = include_str!("lib.rs");
        let lines: Vec<&str> = core.lines().collect();

        let calls: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains("self.paint_the_tab_bar("))
            .map(|(at, _)| at)
            .collect();

        // **A floor, because this test passes by looking at nothing otherwise.** It is how the
        // guards of this suite die: the method is renamed, the walk matches nothing and every
        // assertion below is vacuously true.
        assert!(
            !calls.is_empty(),
            "no call to `paint_the_tab_bar` was found in `lib.rs`: the walk stopped matching and \
             this guard is checking nothing"
        );

        for at in calls {
            let after = lines[at..(at + LINES_TO_SPEND_IT).min(lines.len())].join("\n");
            assert!(
                after.contains(".cost"),
                "the call at `lib.rs:{}` does not spend the outcome's cost within {LINES_TO_SPEND_IT} \
                 lines: the bar's time stops being subtracted from the frame and is published as the \
                 interface framework's, which is the number D3 judges it by",
                at + 1
            );
            assert!(
                after.contains(".marked"),
                "the call at `lib.rs:{}` does not spend the outcome's marked cards within \
                 {LINES_TO_SPEND_IT} lines: the report stops declaring them and a batch that \
                 measured the claim rollup becomes indistinguishable from one taken before it \
                 existed",
                at + 1
            );
        }
    }

    /// **And the attribute itself is still there.**
    ///
    /// The walk above is the one that guards the real failure; this one guards the narrow case the
    /// attribute does cover — a future call site written as a bare statement, which drops both
    /// numbers at once and which the walk above would also catch only because it looks for the
    /// fields. Kept separate so that removing the attribute fails with the reason for the
    /// attribute, and not with a message about spending halves.
    #[test]
    fn the_outcome_still_refuses_to_be_dropped() {
        let source = include_str!("tab_bar.rs");
        let declaration = source.find("struct TabBarOutcome").expect(
            "`TabBarOutcome` is not declared in this file any more: this guard moved house",
        );
        // **Only what is immediately above the declaration counts.** Searching the whole file would
        // be satisfied by a `#[must_use]` on any other item, which is the disguise this project has
        // on file as «asserting on the field where a value is recorded instead of on the effect».
        // The window is the attribute plus its own message, which wraps over two lines.
        let attached = &source[declaration.saturating_sub(400)..declaration];

        assert!(
            attached.contains("#[must_use"),
            "`TabBarOutcome` has no `#[must_use]` attached to it: `self.paint_the_tab_bar(…);` as a \
             bare statement would compile in silence and drop the cost and the marked cards \
             together. It was the delivery's only behavioural change and deleting it left the whole \
             suite green until this test existed.\nWhat sits above the declaration:\n{attached}"
        );
    }
}
