//! Closing a tab or a console: what leaves, what it takes with it, and what is collected.
//!
//! **It sits beside the window rather than inside its core for the reason D28 gives**: the core had
//! grown past its ratchet, and this is a coherent piece of it — everything that happens when a tab
//! or the focused console goes away. The methods still reach the window's private fields, which is
//! exactly what a child module of the crate root is for.

use std::time::Instant;

use eframe::egui;

use crate::measure;
use crate::tree::{Closing, Node};
use crate::{Pane, Window};

impl Window {
    /// Banks a closing pane's channel counters on the window, **before it is dropped**.
    ///
    /// The report reads the live panes directly and adds [`Window::control`] on top, so a pane that
    /// closes has to hand its numbers over here or they leave with it.
    fn bank_control(&mut self, did: measure::Control) {
        self.control.retain(did);
    }

    /// Closes the tab that is being seen. If it is the only one, what is closed is the window.
    ///
    /// **Rearming the watcher is indispensable**: without it it is left following the directories
    /// of the tab that is leaving, however many it had, sees what it was watching disappear and
    /// declares itself blind. From then on the window would reread every panel on every frame, for
    /// ever and in the degraded mode, with nothing on screen saying so. There is a test that falls
    /// if it is removed.
    ///
    /// **It is rearmed after dropping the tab, and the design asked for the opposite.** Its reason
    /// was that the old watcher should not get to see the deletion and declare itself blind;
    /// checked by inverting it, **the observable result is the same**, because the new watcher
    /// brings its own blindness flag and the old one is discarded whole along with its own. With no
    /// difference to defend, it is done in the order that does not need to know which tab is
    /// leaving.
    pub(super) fn close_the_active_tab(&mut self, ctx: &egui::Context) {
        // **The last one is not removed from the list, and it is not a capricious exception:**
        // asking for the close does not close on the spot —frames are still running, and the report
        // is published on exit—, so an empty vector would leave `active` indexing out of range on
        // every one of them.
        if self.tabs.len() == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let leaving = self.active;
        self.some_closed_by_hand = true;

        // **What its consoles had already declared of this frame is collected before dropping
        // them.** They leave with their `Timings` inside, and in there there can be a round
        // recorded for this very frame: bytes that really were interpreted an instant ago. If it
        // leaves without being declared, the subtraction attributes it to the interface framework
        // and «the rest» comes out inflated precisely on the frame in which a tab is closed. The
        // cost of the closing itself —killing the programs and deleting their directories, which
        // happens when the tab is dropped— goes into the same bag for the same reason.
        //
        // **And the three places where this frame's work lives are collected, not just the recorded
        // one.** Looking only at what the measurement already had noted down declared **zero**
        // precisely at the instant of the chord: the real order of the frame is closing the
        // previous one —which empties what the measurement had pending—, pumping —which fills what
        // was interpreted without recording it— and attending to the keyboard **before** the
        // painting. So what the console had just interpreted, and what rereading its panel cost,
        // left with it undeclared. See [`Pane::take_what_this_frame_left`], which is where closing
        // a pane already solved it.
        let before = Instant::now();
        let its_cost: std::time::Duration = self.tabs[leaving]
            .panes_mut()
            .into_iter()
            .map(Pane::take_what_this_frame_left)
            .sum();

        // **Its channel counters are banked before it leaves**, or a tab closed mid-run takes what
        // its consoles published with it and the report under-counts — which can invalidate a run
        // that was correct. Read here, while the panes still exist.
        let its_control: Vec<_> = self.tabs[leaving]
            .panes_mut()
            .into_iter()
            .filter_map(|pane| pane.control.as_ref())
            .map(crate::control::PaneControl::did_in_the_window)
            .collect();
        for did in its_control {
            self.bank_control(did);
        }

        // Here its programs die and its directories disappear, however many it had, through the
        // usual `Drop`s.
        self.tabs.remove(leaving);

        // **Rearming the watcher goes inside the stopwatch**, and before it was left outside by a
        // line drawn in the wrong place: it rebuilds the watching over the panes of every tab that
        // is left and makes them reread their `panel.json`, which is the expensive half of closing.
        // No pane declares it, so without this it was published as interface framework cost.
        self.rearm_the_watcher(ctx);
        // On removing the last one in the list the index is left outside; in any other case, it is
        // left pointing at the one that took its place, which is the one on the right.
        self.active = leaving.min(self.tabs.len() - 1);
        self.timed_separately += its_cost + before.elapsed();
    }

    /// Closes the console that has the keyboard, and its siblings share out its place.
    ///
    /// **Nothing has to be shared out by hand, and that is the reason the layout carries weights
    /// and not fractions that add up to one**: removing one child from the list leaves the rest
    /// with the weights they already had, and the tiling places them in proportion over the whole
    /// area. With normalized fractions they would have to be renormalized on every close.
    ///
    /// **With a single console, closing it is closing the tab** —there is no layout left to
    /// reabsorb its place—, and if it was also the only tab, the window. It is delegated to the
    /// path that already exists instead of repeating its rule, which is not obvious: the last tab
    /// is not removed from the list, because asking for the close does not close on the spot and an
    /// empty vector would leave `active` indexing out of range.
    ///
    /// **What it costs is declared in full**, and they are three things no living pane is going to
    /// declare: the tree walk, what the one that is leaving had measured of this frame, and killing
    /// its program and deleting its directory. Without declaring them, the subtraction attributes
    /// them to the interface framework — the figure D3 decides with.
    pub(super) fn close_the_focused(&mut self, ctx: &egui::Context) {
        let before = Instant::now();
        // **The tab having a single console is its root being a leaf**, and asking the type costs
        // what looking at a tag costs. Counting the leaves would walk the whole tree and allocate a
        // vector only to throw it away, and it would also say for a second time —and more weakly—
        // an invariant the structure already guarantees: a branch is born with two children and
        // collapses when it is left with one.
        if matches!(self.active().root, Node::Leaf(_)) {
            self.timed_separately += before.elapsed();
            self.close_the_active_tab(ctx);
            return;
        }

        let focus = self.active().focus;
        let closed = self.active_mut().root.close(focus);
        let Some(Closing {
            mut leaving,
            focus: inherited,
        }) = closed
        else {
            // **It cannot happen**: the focus is the name of one of this tab's leaves, and with
            // more than one console none of them is the root. It is said out loud instead of kept
            // quiet because the chord has swallowed the key and closed nothing, and without this
            // line no trace of why would be left.
            eprintln!("the focused pane was not found when closing; nothing has been closed");
            // **The stopwatch is closed after writing**, not before: writing to the error output
            // costs whatever whoever reads at the other end of the pipe takes, and what is not
            // declared is published as interface framework cost.
            self.timed_separately += before.elapsed();
            return;
        };

        // **What it had done in this frame is collected before dropping it.** See [`Closing`] and
        // [`Pane::take_what_this_frame_left`].
        let its_cost = leaving.take_what_this_frame_left();
        // **And its channel counters, for the same reason and here rather than at report time.** A
        // console closed mid-run would otherwise take its `applied`/`refused`/`released` out of the
        // live set the report walks.
        if let Some(did) = leaving
            .control
            .as_ref()
            .map(crate::control::PaneControl::did_in_the_window)
        {
            self.bank_control(did);
        }
        // Here its program dies and its directory disappears, through the usual `Drop`s.
        drop(leaving);
        self.active_mut().focus = inherited;
        // **Closing a console by hand counts the same as closing a tab by hand**, and without this
        // line the same gesture protected the window or not depending on how many siblings the
        // console had: with only one, the path above delegates to closing the tab, which does raise
        // it. The real case is a console whose program has already finished —and whose last content
        // is being read— beside another with a live agent: on closing the agent's nothing alive is
        // left, and the usual rule would close the window taking what was being read with it.
        // Nobody asked for that: closing one console was asked for.
        self.some_closed_by_hand = true;
        // Without rearming, the watcher is left looking at a directory that has just been deleted:
        // it sees what it was watching disappear, declares itself blind, and from then on the
        // window rereads every panel on every frame, for ever and with nothing saying so.
        self.rearm_the_watcher(ctx);
        self.timed_separately += its_cost + before.elapsed();
    }

    /// Closes a console by its name, for a connection that opened it, **and refuses the two cases
    /// the chord cannot produce**.
    ///
    /// **It refuses rather than closing the tab**, unlike the chord's path: a person who presses
    /// `+X` on the last console of a tab means to close the tab, and an agent that guesses the same
    /// would take a whole tab of consoles away on a handle typo (**AC16**). And **the focus moves
    /// only if the console that leaves was holding it**, so closing one the person is not typing in
    /// does not steal their keyboard.
    pub(crate) fn close_a_pane_called(
        &mut self,
        name: usize,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.panes().iter().any(|pane| pane.name == name))
            .ok_or_else(|| "that console is gone".to_owned())?;
        if matches!(self.tabs[tab_index].root, Node::Leaf(_)) {
            return Err(
                "that console is its tab's only one; closing it would close the tab".to_owned(),
            );
        }

        let closed = self.tabs[tab_index].root.close(name);
        let Some(Closing {
            mut leaving,
            focus: inherited,
        }) = closed
        else {
            return Err("the console was not found when closing; nothing was closed".to_owned());
        };

        // The same two things the chord's close collects, and for the same reasons: what the console
        // had measured of this frame, and the channel counters that would leave with it.
        //
        // **The elapsed time is not added here.** It is the drain that times this call and adds it to
        // `timed_separately`, because it is the drain that runs inside the frame; adding it in both
        // places would subtract the same microseconds twice from the number the framework is judged
        // by (**D13**) — the opposite of the mistake declared in this file, and just as wrong.
        let its_cost = leaving.take_what_this_frame_left();
        if let Some(did) = leaving
            .control
            .as_ref()
            .map(crate::control::PaneControl::did_in_the_window)
        {
            self.bank_control(did);
        }
        drop(leaving);
        if self.tabs[tab_index].focus == name {
            self.tabs[tab_index].focus = inherited;
        }
        self.some_closed_by_hand = true;
        self.rearm_the_watcher(ctx);
        self.timed_separately += its_cost;
        Ok(())
    }
}
