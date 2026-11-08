//! Pane creation shared by shortcuts and the header controls.

use crate::painter::CellMetrics;
use crate::tiling::{Orientation, Side, can_split};
use crate::{Pane, Window};
use eframe::egui;
use std::time::Instant;

impl Window {
    pub(crate) fn split_the_focused(
        &mut self,
        cut_along: Orientation,
        ctx: &egui::Context,
        whole_window: egui::Rect,
        metrics: &CellMetrics,
    ) {
        let before = Instant::now();
        #[cfg(test)]
        let before = crate::tests::OperationClock::start(before);
        split(
            self,
            self.active().focus,
            cut_along,
            ctx,
            whole_window,
            metrics,
        );
        self.timed_separately += before.elapsed();
    }

    #[cfg(test)]
    pub(crate) fn graft_the_new_one(
        &mut self,
        opened: term_core::Result<Pane>,
        cut_along: Orientation,
        ctx: &egui::Context,
    ) {
        graft(self, self.active().focus, opened, cut_along, ctx);
    }
}

/// Splits the named pane and gives focus to the newborn only after a successful graft.
///
/// **Focus passes to the new one because splitting is opening a console**, and whoever opens a
/// console wants to write in it; it is the same thing creating a tab does by putting it in
/// front. Staying on the old one would force a click to use what has just been asked for.
///
/// **If it does not fit, nothing is split and nothing is spent**: not a name, not a `fork`. A
/// child below the minimum cannot be used, so it is refused before launching a program.
///
/// **What it costs is declared in full**, and they are two things no pane is going to declare:
/// the tree walk to find the target, and the new program's `fork`/`exec`.
/// Without this, the frame's cost is shared out by subtraction and both would end up published
/// as interface framework cost — the figure D3 judges it with. The shortcut times this call;
/// a header press is already inside the mouse stopwatch and must not subtract it twice.
pub(crate) fn split(
    window: &mut Window,
    target: usize,
    cut_along: Orientation,
    ctx: &egui::Context,
    whole_window: egui::Rect,
    metrics: &CellMetrics,
) {
    // The bar is set aside with the tabs there are **now**: if the same batch of events has
    // just created one, the height left for the consoles is already a different one.
    let area = crate::tab_bar::geometry(window, whole_window, metrics).consoles;
    let target_area = window
        .active()
        .panes()
        .iter()
        .position(|pane| pane.name == target)
        .map(|at| {
            // The leaves and their rectangles are matched by position: the two walks go in the same
            // order, which is the invariant the whole painting already rests on.
            window.active().root.place(area, metrics.width()).parts[at]
        });

    // A target removed before its queued action is applied cannot open a replacement console.
    let Some(its_area) = target_area else {
        return;
    };
    if !can_split(its_area, cut_along, metrics, window.headers) {
        // **It is said out loud, because the chord has swallowed the key and nothing has
        // happened.** Without this line, in a narrow window the shortcut reads as broken: it
        // neither splits, nor reaches the program inside, nor leaves a trace of why. It is the
        // same as the sibling path already does —the console that could not be opened—, and for
        // now it is all that can be done: saying it on screen would need a way of warning that
        // this phase does not have.
        eprintln!(
            "it is not split: the pane cannot give both children their minimum. Make the window bigger \
                 or move the divider"
        );
        return;
    }

    let name = window.next_pane_name;
    let opened = Pane::new_counting_reads(ctx, name, window.control.read_activity());
    // **The name is spent even if the program never gets to start**, just as when opening a
    // tab: whatever is opened afterwards must not be called the same as what never got to
    // exist, because the report may already have named the failed attempt on the error output.
    window.next_pane_name += 1;

    // **The stopwatch is not closed before the graft, and that line was drawn in the wrong
    // place.** Putting the leaf into the tree and rearming the watcher are the two
    // **expensive** parts of splitting —the second rebuilds the watcher over the panes of every
    // tab and makes them reread their `panel.json`—, and they happen inside the interface pass
    // without any pane declaring them. Closing it here, that work fell into «the rest of the
    // frame», which is what this project calls the interface framework cost: the figure D3
    // decides with.
    graft(window, target, opened, cut_along, ctx);
}

/// Puts the console that has just been opened next to its target, or leaves the window as
/// it was if it could not be opened.
///
/// **It receives the attempt already made instead of making it**, just like
/// [`Window::bring_to_front`] with the tabs and for the same reason: opening a console only
/// fails when the system gives no more pseudoterminals or descriptors, and that cannot be
/// provoked from a test without touching the whole process. Splitting it here is what makes it
/// possible to assert what the case promises — that a failure leaves no half pane, does not
/// move the focus and rearms nothing.
fn graft(
    window: &mut Window,
    target: usize,
    opened: term_core::Result<Pane>,
    cut_along: Orientation,
    ctx: &egui::Context,
) {
    let new = match opened {
        Ok(new) => new,
        Err(failure) => {
            eprintln!("the pane could not be split ({failure})");
            return;
        }
    };

    // **The name focus will go to is taken from the console being grafted**, and not from a
    // parameter saying which one it is. The two would be worth the same as long as the caller
    // took them from the same place, but the day they come apart the focus would point at a
    // pane that does not exist: the keys would reach no console at all —they would be lost in
    // silence—, the highlight would not be painted, and the next splitting chord would find
    // nothing to split.
    let name = new.name;
    let tab = window.active_mut();
    if let Some(leftover) = tab.root.split(target, Side::behind(cut_along), new) {
        // **It cannot happen**: the caller has just found the target among the leaves of this
        // very tab, and nothing has touched the tree in between. It is said out loud instead of
        // kept quiet because the console really did get opened, and on dropping it here its
        // program dies and its directory disappears: without this line, a shell opened and
        // closed in the same frame would leave no trace at all.
        drop(leftover);
        eprintln!("the pane was not found when splitting; nothing has been split");
        return;
    }

    tab.focus = name;
    // Without rearming, the watcher knows nothing about the directory of the one that has just
    // been born and its panel would stay frozen with nothing saying so. It is the same as
    // opening a tab does.
    window.rearm_the_watcher(ctx);
}
