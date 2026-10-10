//! Closing a tab or a console: what leaves, what it takes with it, and what is collected.
//!
//! **It sits beside the window rather than inside its core for the reason D28 gives**: the core had
//! grown past its ratchet, and this is a coherent piece of it — everything that happens when a tab
//! or the focused console goes away. The methods still reach the window's private fields, which is
//! exactly what a child module of the crate root is for.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use eframe::egui;

use crate::measure;
use crate::tree::{Closing, Node};
use crate::{Pane, Window};

impl Drop for Window {
    fn drop(&mut self) {
        // Whole-window destruction does not run a closing frame that can rearm the watcher. Stop
        // it explicitly before Rust drops any pane-owned directory.
        self.watcher.stop_before_retirement();
    }
}

impl Window {
    /// Arms the watcher again over every pane that is left.
    ///
    /// The old generation is made inert first, including on remote retirement. Every surviving
    /// flag is then raised, so a publication during replacement cannot leave its panel stale.
    pub(super) fn rearm_the_watcher(&mut self, ctx: &egui::Context) {
        self.watcher.stop_before_retirement();
        let watcher = Self::watch(ctx, self.all_panes());
        self.watcher = watcher;
        for pane in self.all_panes() {
            pane.to_reread.raise();
        }
    }

    /// Banks a closing pane's channel counters on the window, **before it is dropped**.
    ///
    /// The report reads the live panes directly and adds [`Window::control`] on top, so a pane that
    /// closes has to hand its numbers over here or they leave with it.
    fn bank_control(&mut self, did: measure::Control) {
        self.control.retain(did);
    }

    /// Closes the tab that is being seen. If it is the only one, what is closed is the window.
    ///
    /// **Rearming the watcher is indispensable**: the old generation is made inert before the tab
    /// leaves, so without its replacement the surviving panes would no longer be watched and their
    /// next publications could remain unseen. There is a test that falls if it is removed.
    ///
    /// **The old watcher is stopped before the tab is dropped.** Rearming afterwards raises every
    /// survivor's reread flag, so no publication in that interval can leave a panel stale and the
    /// intentional deletions cannot be reported as lost tracking.
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
        // left with it undeclared. See [`what_this_frame_left`], which is where closing
        // a pane already solved it.
        let before = Instant::now();
        let its_cost: std::time::Duration = self.tabs[leaving]
            .panes_mut()
            .into_iter()
            .map(what_this_frame_left)
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

        // The tab leaves the layout first, then its old watcher is stopped before any directory is
        // removed. The replacement below watches only the survivors.
        let leaving_tab = self.tabs.remove(leaving);
        self.watcher.stop_before_retirement();

        // Here its programs die and its directories disappear, however many it had, through the
        // usual `Drop`s. Its watcher no longer contains those directories.
        drop(leaving_tab);

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
        // [`what_this_frame_left`].
        let its_cost = what_this_frame_left(&mut leaving);
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
        self.active_mut().focus = inherited;
        // **Closing a console by hand counts the same as closing a tab by hand**, and without this
        // line the same gesture protected the window or not depending on how many siblings the
        // console had: with only one, the path above delegates to closing the tab, which does raise
        // it. The real case is a console whose program has already finished —and whose last content
        // is being read— beside another with a live agent: on closing the agent's nothing alive is
        // left, and the usual rule would close the window taking what was being read with it.
        // Nobody asked for that: closing one console was asked for.
        self.some_closed_by_hand = true;
        self.watcher.stop_before_retirement();
        // Here its program dies and its directory disappears, through the usual `Drop`s. Its old
        // watcher no longer contains that directory.
        drop(leaving);
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
        let its_cost = what_this_frame_left(&mut leaving);
        if let Some(did) = leaving
            .control
            .as_ref()
            .map(crate::control::PaneControl::did_in_the_window)
        {
            self.bank_control(did);
        }
        if self.tabs[tab_index].focus == name {
            self.tabs[tab_index].focus = inherited;
        }
        self.some_closed_by_hand = true;
        self.watcher.stop_before_retirement();
        drop(leaving);
        self.rearm_the_watcher(ctx);
        self.timed_separately += its_cost;
        Ok(())
    }

    /// Detaches several named consoles after validating the whole removal.
    ///
    /// Nothing is changed until every name is known, unique, and leaves at least one console in its
    /// tab. Once those checks hold, `Node::close` cannot refuse: no earlier removal can erase a
    /// different named leaf, and the per-tab remainder keeps a root to close from.
    pub(crate) fn detach_panes_called(
        &mut self,
        names: &[usize],
        ctx: &egui::Context,
    ) -> Result<Vec<Pane>, String> {
        self.validate_detaching_panes(names)?;

        let mut detached = Vec::with_capacity(names.len());
        let mut pending_cost = std::time::Duration::ZERO;
        for &name in names {
            let tab = self
                .tabs
                .iter()
                .position(|tab| tab.panes().iter().any(|pane| pane.name == name))
                .expect("the complete retirement was validated before changing the tree");
            let Closing {
                mut leaving,
                focus: inherited,
            } = self.tabs[tab]
                .root
                .close(name)
                .expect("a validated retirement target remains in its tree");
            pending_cost += what_this_frame_left(&mut leaving);
            if let Some(did) = leaving
                .control
                .as_ref()
                .map(crate::control::PaneControl::did_in_the_window)
            {
                self.bank_control(did);
            }
            if self.tabs[tab].focus == name {
                self.tabs[tab].focus = inherited;
            }
            detached.push(leaving);
        }

        if !detached.is_empty() {
            self.rearm_the_watcher(ctx);
        }
        self.timed_separately += pending_cost;
        Ok(detached)
    }

    pub(crate) fn validate_detaching_panes(&self, names: &[usize]) -> Result<(), String> {
        let mut unique = HashSet::with_capacity(names.len());
        let mut per_tab: HashMap<usize, usize> = HashMap::new();
        for &name in names {
            if !unique.insert(name) {
                return Err("the retirement names one console twice".to_owned());
            }
            let tab = self
                .tabs
                .iter()
                .position(|tab| tab.panes().iter().any(|pane| pane.name == name))
                .ok_or_else(|| "a console owned by this connection is gone".to_owned())?;
            *per_tab.entry(tab).or_default() += 1;
        }
        for (tab, removing) in per_tab {
            if self.tabs[tab].panes().len() <= removing {
                return Err("retirement would leave a tab without a console".to_owned());
            }
        }
        Ok(())
    }
}

/// Takes **everything** this console has done in this frame and nobody has declared yet, for
/// the one that is leaving and is not going to publish any mean.
///
/// **There are two places, and one alone is not enough.** The frame's real order decides it:
/// `logic` first closes the previous frame —which empties what [`measure::Timings`] had
/// pending— and **then** pumps, which is what leaves in `pending_measurement` what this console
/// has just interpreted. The keyboard is attended at the start of the UI pass, **before the
/// painting**, so when the closing chord arrives, what the measurement had pending is empty and
/// what this frame interpreted is whole in `pending_measurement`: looking only at the first one
/// declares **zero** right in the frame where a dumping console is closed, and those
/// milliseconds are taken by «the rest of the frame», that is, the cost attributed to the UI
/// framework. The other one can indeed hold something when egui repeats the pass, so both are
/// picked up.
///
/// **It is a free function and not a method of `Pane`** because it is closing's work: it went out
/// of the crate root to pay `lib.rs`'s ratchet (D28), and `impl Pane` with it.
///
/// **It records no mean**, for the same reason as [`measure::Timings::take_pending`]: this
/// console is no longer going to publish spans, and closing it along the normal path would
/// force inventing a frame cost that is not going to finish.
fn what_this_frame_left(pane: &mut Pane) -> std::time::Duration {
    let without_recording = pane
        .pending_measurement
        .take()
        .map_or(std::time::Duration::ZERO, |(_, _, interpreted)| interpreted);

    pane.timings.take_pending()
        + without_recording
        + std::mem::take(&mut pane.panel_reread_unpainted)
        + std::mem::take(&mut pane.control_unpainted)
}
