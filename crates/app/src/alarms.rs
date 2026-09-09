//! **The two deadlines the window computes from its own state.**
//!
//! One for writing the layout to disk, one for saying that a status panel has gone stale. Both are
//! *derived*: how long is left until something that is already scheduled falls due. They came out of
//! the window's core as a piece, unchanged, because that core is over its ceiling and **D28** records
//! the order its pieces leave in.
//!
//! **A third repaint deadline stays in `lib.rs` on purpose**, and it is not one of these: the
//! degraded-mode poll, asked for inside the branch that discovers the watcher has gone blind. It is
//! not computed from anything — it is a fixed interval, and it exists only for as long as that
//! branch is taken — so it belongs where the degradation is detected rather than here. Say «the
//! deadlines the window arms» and you have said something false; these two are the ones it works out.
//!
//! **What belongs here is a deadline, not a clock.** The blinking cursor's own deadline will join
//! these; **its phase does not** — whether the cursor is lit at this instant is how the cursor
//! looks, which is the painter's question, and a module named for the deadlines it arms is one that
//! can say no to it.
//!
//! **A deadline armed here is work at rest**, which is exactly what every published control batch of
//! `scripts/measure-frame.sh` measures (**D13**): it changes how many frames a run has, so anything
//! added is declared in the report rather than left to be published as the interface framework's
//! cost.

use crate::Window;
use eframe::egui;
use std::time::{Instant, SystemTime};

impl Window {
    /// **The heartbeat: waking up in time to say that a panel is old.**
    ///
    /// The watcher sees what changes; this sees what **stops** changing, and they are different
    /// things — a producer that dies or hangs writes nothing, so it produces no file event at all.
    /// Without this alarm clock the warning would never appear in a window at rest, which is
    /// exactly when it is needed.
    ///
    /// **Only what is left until the first one falls due** is asked for: it is not a clock that
    /// runs all the time, it is an alarm that is set when there is something to wait for and is not
    /// set when there is not.
    ///
    /// **It is called twice per frame, and both are needed.** The panels are reread in the
    /// interface pass, which runs *after* the logic half: arming the alarm only in the first, the
    /// frame in which a panel is read for the first time would compute it with the state from
    /// before reading it —that is, with no deadline to wait for— and the window would sleep for
    /// ever. Measured: the delay published in that frame was `Duration::MAX`. And arming it only in
    /// the second would leave out the rounds in which the window is covered and only the logic half
    /// runs. Asking for too much costs nothing: the window keeps the shortest of all the deadlines
    /// it is asked for.
    ///
    /// **And it looks only at the tab that is seen**, unlike the pumping. The alarm exists so that
    /// the «this is old» label **appears on screen**, and a tab that is not painted has no screen:
    /// waking the window for it would be spending a frame on a warning nobody is going to see.
    /// Nothing is lost, because whether the panel looks stale is decided **when it is painted**, so
    /// on going back to that tab the warning shows in that very frame.
    pub(crate) fn set_the_alarm(&self, ctx: &egui::Context) {
        let now = SystemTime::now();

        // **The saving deadline wakes the window, and both halves have to ask for it.** The logic
        // half starts by setting the deadline to the longest possible and is the only one that runs
        // with the window covered: without the call over there, every round of a hidden window
        // would erase this alarm and the layout would not be saved until someone moved the mouse.
        //
        // **And this changes how many frames a measurement run has**, so it goes declared in the
        // annex: a window that wakes up twice a second while it is being dragged does not have the
        // same number of frames as one that only responds to input.
        if let Some(due) = self.save_due {
            ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
        }

        if let Some(missing) = self
            .active()
            .panes()
            .into_iter()
            // **A folded panel wakes nobody**, for the same reason the panel of a tab that is not
            // being looked at wakes nobody: the warning that its content is stale **is not
            // painted**, so spending a frame on it is spending it on something nobody is going to
            // see. Without this, folding a console whose producer goes quiet leaves the window
            // waking up every minute for nothing.
            .filter(|pane| !pane.folded)
            .filter_map(|pane| pane.panel_state.how_long_until_warning(now))
            .min()
        {
            ctx.request_repaint_after(missing);
        }
    }
}
