//! **The three deadlines the window computes from its own state.**
//!
//! One for writing the layout to disk, one for saying that a status panel has gone stale, one for
//! flipping a blinking cursor. All three are *derived*: how long is left until something that is
//! already scheduled falls due. The first two came out of the window's core as a piece, unchanged,
//! because that core is over its ceiling and **D28** records the order its pieces leave in.
//!
//! **A fourth repaint deadline stays in `lib.rs` on purpose**, and it is not one of these: the
//! degraded-mode poll, asked for inside the branch that discovers the watcher has gone blind. It is
//! not computed from anything — it is a fixed interval, and it exists only for as long as that
//! branch is taken — so it belongs where the degradation is detected rather than here. Say «the
//! deadlines the window arms» and you have said something false; these three are the ones it works
//! out.
//!
//! **What belongs here is a deadline, not a clock.** The blinking cursor's own deadline joined
//! these; **its phase did not** — whether the cursor is lit at this instant is how the cursor looks,
//! which is the painter's question, and a module named for the deadlines it arms is one that could
//! say no to it. What lives here is the state the deadline is computed from and the arming itself;
//! the phase arithmetic, the period and the fifteen-second stop are all in `painter.rs`.
//!
//! **A deadline armed here is work at rest**, which is exactly what every published control batch of
//! `scripts/measure-frame.sh` measures (**D13**): it changes how many frames a run has, so anything
//! added is declared in the report rather than left to be published as the interface framework's
//! cost.

use crate::Window;
use crate::painter::Blink;
use eframe::egui;
use std::time::{Duration, Instant, SystemTime};

/// What the window keeps between frames so that a cursor can blink.
///
/// **The three pieces are one subject**, and they travel together for a reason the ratchet decides
/// rather than taste: the window's core is `src/lib.rs`, which cannot grow (**D28**), so three
/// fields there would have to be paid for with three lines out of it.
pub(crate) struct BlinkState {
    /// What the settings file settles over the program's head. `None` leaves it to the program.
    pub(crate) ceiling: Option<bool>,
    /// When a keystroke last reached a console.
    ///
    /// **Reachable from the crate so a test can choose a phase.** There is no clock trait in this
    /// repository and this delivery does not add one; the seam it follows instead is `panel.rs`'s,
    /// where `now` is an argument and the caller reads the real clock once. Here the caller is
    /// [`Self::now`], and moving this back is how a test asks for the dark half without sleeping
    /// six hundred milliseconds in the suite.
    pub(crate) typed_at: Instant,
    /// **Whether the last frame really drew a blinking cursor** — not whether a program asked for
    /// one.
    ///
    /// This is the whole reason the deadline below is honest. The engine's blink bit is independent
    /// of whether the cursor is shown, so every full-screen program — they all send `CSI ?25l` —
    /// reports a blink nobody can see, and with `cursor.blink = on` the ceiling forces it. Arming
    /// from the request would wake the window at 1.7 Hz to paint nothing, which is exactly the cost
    /// this whole delivery exists to bound.
    ///
    /// **One frame of lag can only arm a deadline early** — egui keeps the shortest it is asked for
    /// — and the next frame clears it.
    pub(crate) painted: bool,
    /// Frames already closed in which a blinking cursor **was painted**.
    ///
    /// **Painted, not «woken», and the two are not the same number.** They part company when the
    /// desktop takes the focus away between the painting half and the alarm: the flag is still true,
    /// the deadline already answers nothing, and that frame painted a blink without waking anybody.
    /// The report says «painted» because that is what this counts, and **D13** rates a mislabelled
    /// number worse than no number at all.
    ///
    /// **The frame being painted is not in here yet**; [`Self::frames_painted`] adds it. Counted so
    /// the report can declare it: work nobody names is published as the interface framework's cost.
    frames_painted: usize,
}

impl Default for BlinkState {
    fn default() -> Self {
        Self {
            ceiling: None,
            typed_at: Instant::now(),
            painted: false,
            frames_painted: 0,
        }
    }
}

impl BlinkState {
    /// What the painter needs to decide the ink, with the clock read **once**, here.
    pub(crate) fn now(&self) -> Blink {
        Blink {
            ceiling: self.ceiling,
            since_a_key: self.typed_at.elapsed(),
            // **The window's own snapshot answers for no console in particular.** Each one puts its
            // own answer on it with [`Blink::on`] as it is handed over to be painted; here it is
            // «elsewhere», so a caller which forgets gets the still cursor and never a false blink —
            // and its notice says «focus this console», which is wrong on the focused one. Forgetting
            // is not harmless; this only picks the failure that shows.
            keys: crate::painter::Keys::Elsewhere,
        }
    }

    /// Starts a painting pass: the frame that is ending is counted and the flag is cleared.
    ///
    /// **Clearing is the half that matters.** Without it the flag only ever goes up, so a console
    /// that stops blinking — its program asks for a steady cursor, or the settings switch it off —
    /// goes on waking the window until the fifteen seconds run out, every time. Mutated: deleting
    /// the reset left the whole suite green.
    pub(crate) fn new_frame(&mut self) {
        self.frames_painted += usize::from(self.painted);
        self.painted = false;
    }

    /// Notes what one console's painting answered.
    pub(crate) fn saw(&mut self, blinking: bool) {
        self.painted |= blinking;
    }

    /// What the report declares, **including the frame being painted right now**.
    pub(crate) fn frames_painted(&self) -> usize {
        self.frames_painted + usize::from(self.painted)
    }

    /// Puts the phase back to the start, so the cursor is lit **the instant somebody types**.
    pub(crate) fn saw_a_key(&mut self) {
        self.typed_at = Instant::now();
    }

    /// How long until the window has to wake to flip a cursor, or nothing if none is blinking.
    ///
    /// **The desktop's focus is asked for again here, and it is not the same question the painting
    /// answered.** [`Self::painted`] is set by the painting half, which is documented as not running
    /// while the window is minimized or occluded — `Window::set_the_alarm` says why, and it is the
    /// premise this repository already relies on — so the flag keeps the last painted frame's answer
    /// for as long as the window stays hidden, and without this the window rearms every 600 ms to
    /// flip a cursor nobody can see. **Bounded, not unbounded**: the anchor is refreshed by that same
    /// half, so the fifteen-second stop ends it, and what this removes is on the order of
    /// twenty-five needless frames per minimize. Found by a design challenger against the code, and
    /// it had a test failing on it before it had a fix.
    fn due(&self, the_desktop_is_looking: bool) -> Option<Duration> {
        (self.painted && the_desktop_is_looking)
            .then(|| self.now().until_it_flips())
            .flatten()
    }
}

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
        if let Some(due) = self.persistence.save_due {
            ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
        }

        // **The blink's deadline, and it is armed from what the painting drew.** See
        // [`BlinkState::painted`] for why the program's own request is the wrong thing to ask.
        //
        // **The desktop's focus is asked for again, and what it bounds is smaller than it looks.**
        // Without it a hidden window rearms every 600 ms off a flag the painting half is no longer
        // maintaining — but only until the fifteen-second stop runs out, because the anchor is
        // refreshed by that same half. So the cost it removes is about twenty-five needless frames
        // per minimize, not an unbounded spin. That correction is worth keeping: the first version
        // of this comment, of **D37** and of the commit that fixed it all said «for ever».
        // **`raw.focused` and not `focused`, and the difference is the whole point.** The processed
        // field is refreshed when a pass begins, which is the interface half; `Context::run_logic`
        // (egui 0.36.1, `context.rs:926`) deliberately writes `raw.focused` and says in a comment
        // that it leaves the ui input alone. Reading the processed one here answers with the last
        // painted frame's focus, which in a minimized window is «yes» for ever — the same staleness
        // this guard exists to undo, arrived at by a different door. Measured: the test for it
        // still failed with the wrong field.
        if let Some(flip) = self.blink.due(ctx.input(|input| input.raw.focused)) {
            ctx.request_repaint_after(flip);
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
