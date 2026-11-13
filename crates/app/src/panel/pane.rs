//! How a pane gets its panel: its directory, the identity it hands its program, what arrives for it,
//! and how much of its rectangle the panel takes.
//!
//! **Four pieces that were in the window's core and are not about the window.** They left it under
//! D28's rule — a delivery that needs room in a file takes a piece out of it first — and they came
//! here rather than anywhere else because every one of them answers the same question: what this
//! pane's panel is fed with. The one that decides *who* may feed it lives beside this, in
//! [`super::driving`].

use std::time::SystemTime;

use panel_contract::PanelDir;

use eframe::egui;

use crate::painter::CellMetrics;
use crate::tiling::{DIVIDER_THICKNESS, Orientation};
use crate::{PANEL_GRIP, Pane, panel};

/// Creates the interface-owned state before its channel receives read and attestation capabilities.
pub(crate) fn state_and_control_with_authority(
    ctx: &egui::Context,
    dir: Option<&PanelDir>,
    reads: crate::control::ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    authority: Option<std::sync::Arc<panel::overlay::Authority>>,
    inspector: panel_control::PublicationInspector,
) -> (panel::PanelState, Option<crate::control::PaneControl>) {
    let state = panel::PanelState::default();
    let control = dir.and_then(|dir| {
        let waker = ctx.clone();
        let wake = move || waker.request_repaint();
        crate::control::open_with_authority(
            dir, &state, reads, sessions, authority, inspector, wake,
        )
    });
    (state, control)
}

/// What one refresh of a pane's panel cost, split by **which transport it was spent on**.
///
/// The two never travel as one number, and that is the whole reason this type exists rather than a
/// bare duration: see [`Pane::refresh_its_panel`].
pub(crate) struct PanelRefresh {
    /// What applying what arrived over the control channel cost.
    pub(crate) applying: std::time::Duration,
    /// What re-reading `panel.json` cost. **The stopwatch runs whether or not the file was read**,
    /// because the flag is taken inside it: a frame where nobody published still asks, and a span
    /// that only existed when something was read would leave that asking undeclared.
    pub(crate) reading: std::time::Duration,
}

/// A directory that could not be created **does not stop the console from opening**.
///
/// It is the same decision as the fallback to another shell when the user's cannot be launched, and
/// with more reason: without a panel, a console is still a complete terminal. It is a function so
/// that this can be asserted — from a test the creation cannot be made to fail for real without
/// touching the whole process's environment.
pub(crate) fn panel_or_none(created: std::io::Result<PanelDir>) -> Option<PanelDir> {
    match created {
        Ok(panel) => Some(panel),
        Err(failure) => {
            eprintln!(
                "this console goes without a panel: its directory could not be created ({failure})"
            );
            None
        }
    }
}

/// What is done with the panel's two variables when launching a console's program.
///
/// **With no directory they are removed, and removing them is not the same as not setting them.**
/// The program inherits this application's environment, so if the application runs inside a console
/// of its own —which happens as soon as it is used for work— the child console would receive its
/// mother's identity: its producer would publish in the mother's mailbox and two panes would share
/// an identifier, which is exactly what the identity comes to prevent.
///
/// There are always two, with a directory or without one: it is what makes the half-identity case
/// impossible.
pub(crate) fn identity_of(
    panel: Option<&PanelDir>,
) -> [(&'static str, Option<&std::ffi::OsStr>); 2] {
    match panel {
        Some(panel) => panel.env().map(|(name, value)| (name, Some(value))),
        None => panel_contract::ENV_NAMES.map(|name| (name, None)),
    }
}

/// Keep the complete tiling outside the oversized Pane implementation (D28).
fn tile_pane(
    pane: &Pane,
    rect: egui::Rect,
    metrics: &CellMetrics,
    now: SystemTime,
) -> panel::PanelTiling {
    // **The width comes from the pane itself, and that is what keeps the two callers in
    // agreement.** One measures the grid of the consoles that are not painted and the other
    // paints the one on show; if the width travelled as a parameter, each could bring its own
    // and a background console would again be measured with a width that is not its own — the
    // defect that already cost 36 columns.
    //
    // Folding and an unpublished panel leave the same outer console rectangle; both still
    // reserve inner padding before grid sizing, painting and pointer coordinates use it.
    let mut split = panel::set_aside(
        rect,
        metrics.width(),
        (!pane.folded && pane.panel_state.is_visible(now)).then_some(pane.panel_width),
    );
    split.console = metrics.console_in(split.console);
    split
}

impl Pane {
    /// Reads this console's `panel.json` again **if somebody has given notice** that it changed.
    ///
    /// The flag is always picked up, whether there is a directory or not: leaving it raised because
    /// this console has nowhere to read from would mean another frame's read picked it up, and then
    /// the notice that was saved would belong to nobody.
    ///
    /// With no directory there is nothing to read: it is the case of the console that could not
    /// create its own, which has to keep looking as it did before the panel existed.
    ///
    /// **It returns the two spans separately, and returning them separately is the point.** Both
    /// callers used to time this whole call under the «panel» label and subtract the channel's share
    /// back out, which made the correct number depend on every caller remembering to subtract — and
    /// a caller that forgot would publish a document applied over a socket as the cost of reading a
    /// file that was never read, which **D13** rates worse than no number at all. There is now no
    /// subtraction anywhere for anybody to forget, and the order the two happen in is not the
    /// caller's to get wrong either.
    pub(crate) fn refresh_its_panel(&mut self, ctx: &egui::Context) -> PanelRefresh {
        // **What arrived over the channel is applied first, and the order is the decision.** A
        // connection that closed this frame gives its pane back before the file is consulted, so the
        // next file publication takes over in the same pass instead of waiting for the one after;
        // and a connection that published this frame claims it before, so the read below declines
        // rather than racing it.
        //
        // **Zero when this console has no channel**, rather than a stopwatch around nothing: a pane
        // that cannot be published to did not spend those nanoseconds on the control channel, and
        // the same reasoning already keeps the panel's own stopwatch off a console with no
        // directory.
        let applying = self
            .control
            .as_mut()
            .map_or(std::time::Duration::ZERO, |control| {
                let started = std::time::Instant::now();
                control.apply(&mut self.panel_state, &mut self.signals, SystemTime::now());
                started.elapsed()
            });

        let started = std::time::Instant::now();
        let deferred_probe_is_due = self.panel_state.retired_probe_is_due();
        if let (true, Some(dir)) = (
            self.to_reread.take() || deferred_probe_is_due || self.panel_state.file_probe_pending(),
            self.panel.as_ref(),
        ) {
            // **A refusal gives the notice back, and without that the publication is lost for
            // good.** The file's date does not change again on its own, so a notice swallowed while
            // the panel was held would mean the first pass after the release had nothing to go back
            // for: the person who edited `panel.json` would see their change only if they saved a
            // second time. Raised again, that pass reads it — and it is the very pass the hold ends
            // in, because the release is applied above, before this line.
            //
            // **And it is given back only when there was a refusal to protect**, which is what keeps
            // the notice from becoming a confession: the flag is also raised by a split, by an open
            // tab and by the blind watcher, and handing it back for those made a held panel look at
            // the file every frame for the life of the hold and confess a publication nobody made.
            match self.panel_state.refresh(dir.path()) {
                panel::Refreshed::Declined if self.panel_state.refused_the_file() => {
                    self.to_reread.raise();
                }
                panel::Refreshed::Deferred(delay) => {
                    // The watcher already spent its one notice. Its deadline stays in the panel
                    // state and this arms the exact pass that will compare bytes, even if the
                    // window is idle. The notice itself stays down, so an active window does not
                    // stat the same file once per frame until that deadline.
                    ctx.request_repaint_after(delay);
                }
                panel::Refreshed::Read | panel::Refreshed::Declined => {}
            }
        }

        PanelRefresh {
            applying,
            reading: started.elapsed(),
        }
    }

    /// Splits this pane's rectangle between its console and its panel.
    ///
    /// **The clock is passed in from outside and is the same one the painting uses**: whether a
    /// panel is stale depends on the time, and whether there is anything to show depends on that.
    /// Two different clocks in the same frame could set aside a strip where nothing is then
    /// painted.
    pub(crate) fn tile_with_its_panel(
        &self,
        rect: egui::Rect,
        metrics: &CellMetrics,
        now: SystemTime,
    ) -> panel::PanelTiling {
        tile_pane(self, rect, metrics, now)
    }

    /// How a pane's rectangle is shared out between its strip, its console and its panel.
    ///
    /// **One function makes both cuts, and that is the whole reason this exists.** The grid a
    /// program is told about is measured from `console` ([`crate::painter::paint_pane`]) and every
    /// pointer is counted from that same rectangle ([`Pane::console_within`]); with the two cuts
    /// made in two places, a delivery that changes one of them slides every selection and every
    /// mouse report by a row without anything failing.
    ///
    /// The clock is passed in for the reason [`Pane::tile_with_its_panel`] gives: two clocks in one
    /// pass can disagree about whether a panel is still there.
    pub(crate) fn chrome_within(
        &self,
        rect: egui::Rect,
        metrics: &CellMetrics,
        now: SystemTime,
        headers: crate::header::Headers,
    ) -> PaneChrome {
        // **The strip is taken off first, and the order is the decision.** The panel is cut from
        // the right and the strip from the top, so arithmetically the order changes nothing; what
        // it changes is what the panel's own border is registered over. Cut the other way round,
        // that border's grip would run the full height of the pane and would cover the right-hand
        // end of the strip — and being registered later, it would win the tie: the one band that
        // exists to be grabbed would not be grabbable at its own corner.
        let (header, below) = crate::header::set_the_header_aside(rect, metrics, headers);

        PaneChrome {
            header,
            below,
            split: self.tile_with_its_panel(below, metrics, now),
        }
    }

    /// Attends the border between this console and its panel, and returns whether the width
    /// changed.
    ///
    /// **It came out of `lib.rs` with the delivery that cut the strip** (**D28**: a delivery that
    /// needs room in a file takes a piece out of it first), and it came **here** because this module
    /// is the one that already answers how much of its rectangle a pane's panel takes. It became
    /// `pub(crate)` on the way, which widens nothing: it lived in the crate's root module, where
    /// every module of the crate could already reach it.
    ///
    /// **The identifier carries the pane's name, never its place.** egui holds a drag between
    /// frames by that identifier, and this one lasts as long as the button is held down. The chords
    /// for splitting, closing and switching tabs are applied **at the start of the same pass**,
    /// long before the mouse is attended: with the position inside, grabbing this border and
    /// pressing one of those chords without letting go would narrow another console's panel. The
    /// reasoning that does allow the tree's dividers to be indexed by position —that their drag is
    /// resolved within the same frame— does not hold here.
    ///
    /// **The handle is an invisible strip straddling the cut**, wider than the line a tree divider
    /// paints but painting none. Neither the geometry nor the look changes: between the console and
    /// its panel there is no line, and putting one in would deduct its thickness from the tiling
    /// and would move every console's grid — the comparison with the earlier measurements would be
    /// lost to gain an ornament.
    ///
    /// **And it is not mounted if the panel has no width.** There its border coincides with the
    /// pane's edge, that is, with the divider that tiles consoles; as this control is registered
    /// afterwards, it would win the tie and would leave dead the border that does move something —
    /// in exchange for resizing a panel that is not on show.
    ///
    /// **The width is asked for in cells from the pointer's position, not by accumulating its
    /// displacement.** An accumulator falls out of step with the mouse as soon as the floor or the
    /// ceiling clips one frame: the pointer would go on descending and the panel would not, and on
    /// coming back the whole of that stretch would have to be retraced before the border moved.
    pub(crate) fn drag_its_panel_divider(
        &mut self,
        ui: &egui::Ui,
        pane_area: egui::Rect,
        panel: Option<egui::Rect>,
        metrics: &CellMetrics,
        now: SystemTime,
    ) -> bool {
        // **Folded there is no panel, and even so somewhere to grab has to be left**: if the border
        // went away with it, the gesture that closed it could not be undone with the mouse. The
        // handle stays stuck to the pane's right edge, **inwards**.
        //
        // **Inwards and not straddling, unlike the one of a border with a panel.** A pane's right
        // edge is where the divider that tiles consoles begins, and this control is registered
        // afterwards: straddling, it would win the tie and would leave dead the border that does
        // move something. The price of putting it inwards is declared and is small: on a folded
        // console, the band at the right edge grabs its panel and not the console, so there it
        // cannot be picked up to be carried elsewhere.
        let cut_along = match panel {
            // **A panel narrower than the handle itself is not grabbable by its border**, and the
            // threshold is the handle and not zero: with zero, a panel of two points left the band
            // of eight **covering the divider** that tiles the consoles, and as this control is
            // registered afterwards it won the tie — the console could not be widened again. The
            // review found it, with the exact-zero test in the green.
            Some(panel_rect) if panel_rect.width() <= PANEL_GRIP => return false,
            Some(panel_rect) => panel_rect.left(),
            // **Folded there is a handle, but only if there is a panel to unfold.** If the producer
            // stopped publishing while it was folded there is nothing left to show, and setting
            // aside a band of eight points there would take from the console its right edge —the
            // place it is grabbed by to be carried elsewhere— in exchange for unfolding a panel
            // that does not exist.
            None if self.folded && self.panel_state.is_visible(now) => {
                // **And one divider thickness is withdrawn from the edge**, so that the band does
                // not reach the point where that divider begins. egui's rectangles contain both
                // their ends and ties are won by the last one registered, which is this one:
                // without the withdrawal, the exact point of the edge resized the panel instead of
                // moving the console tiling. It is the same tie the narrow-panel guard avoids, and
                // the price D10 declares is only over the console, not over the divider.
                pane_area.right() - PANEL_GRIP - DIVIDER_THICKNESS
            }
            // With no panel and not folded there is nothing to show nor to adjust: it is a console
            // whose producer has published nothing.
            None => return false,
        };

        let grip = egui::Rect::from_min_max(
            egui::pos2(cut_along - PANEL_GRIP, pane_area.top()),
            egui::pos2(cut_along + PANEL_GRIP, pane_area.bottom()),
        );

        // `Sense::DRAG` and not `Sense::drag()`: the second adds `FOCUSABLE`, that is, the border
        // would ask for egui's keyboard focus and would compete with the application's geographic
        // routing.
        //
        // **And grabbing this border does not give its console the focus, unlike grabbing the
        // console.** It is deliberate and goes with the other borders: moving the divider that
        // tiles two consoles does not change where the keys go either. What gives the focus is
        // grabbing **a console**, because that does say which one you want to work in; adjusting a
        // panel's width does not say it.
        let id = ui.id().with(("panel-border", self.name));
        let response = ui
            .interact(grip, id, egui::Sense::DRAG)
            // While it is being dragged too, and above all in the frame where it is released: there
            // the drag has already ended and the pointer may have been left outside the handle,
            // where the cursor would flicker to an arrow right at the end of the maneuver.
            .on_hover_and_drag_cursor(Orientation::Columns.cursor());

        // **Only the primary button, and only if the pointer has moved.** The two conditions cover
        // two different holes, and both are in egui 0.36.1's source:
        //
        // - `interaction.rs:162` matches `PointerEvent::Pressed { .. }` **without looking at which
        //   button it was**, so the middle one —X11's paste— and the right one —the selection in
        //   almost any terminal— arm the drag as well. It is the same guard the console drag
        //   already has and documents.
        // - A control that only senses drags is marked as dragged **on the press itself**, with no
        //   movement (`interaction.rs:219-220`, with its comment). And that, over a pane where the
        //   tiling is clipping the panel, is destructive: the handle is painted at the **clipped**
        //   cut, so a bare click would read that cut as intent and would wipe out the width the
        //   user had asked for when the window was wider. Demanding movement is what separates «I
        //   have pressed here» from «I want this width».
        if !response.dragged_by(egui::PointerButton::Primary) || response.drag_delta().x == 0.0 {
            return false;
        }

        let Some(pointer) = response.interact_pointer_pos() else {
            return false;
        };

        // **The ceiling is what the tiling can honor today, not what the pane measures.** During a
        // gesture what the user asks for is what they see, and the tiling is not going to give them
        // more than what is spare after the console's floor: saving more would be saving a width
        // they never saw, and which would appear all at once on widening the window.
        //
        // **And the ceiling is applied only on writing, which is what makes shrinking the window
        // not erase what was asked for.** With no gesture this is not gone through, so a panel of
        // thirty-six cells saved in a wide window survives a narrow one: it is painted clipped and
        // comes back whole on widening.
        //
        // The pane's number of cells comes from [`CellMetrics::cells_in`] so as not to repeat here
        // a conversion that function already does with its floor and its ceiling. **But its cap is
        // not the one that is needed and it is worth saying so:** it is a thousand columns, a grid
        // limit, not «the rectangle corresponds to a real window». With the ten thousand points of
        // reserve egui returns while the desktop has not said its size, this ceiling comes out at
        // almost a thousand cells and **lets through** a width no real window is going to have.
        // What really stops that from being saved is the guard below: a drag demands that the
        // pointer has moved, and in those first two frames there is nobody dragging.
        let fits = metrics
            .cells_in(pane_area.size() - egui::vec2(2.0 * crate::painter::CONSOLE_PADDING, 0.0))
            .cols as usize;
        let ceiling = fits
            .saturating_sub(panel::MINIMUM_CONSOLE as usize)
            .max(panel::DRAG_FLOOR);

        // **No guard against a non-finite value, and this time with the four paths enumerated.**
        // The review warned that, since asking for less than the floor means **folding**, a zero is
        // no longer raised by the floor: it would fold the panel. The warning is correct in its
        // reasoning and the scenario **is not reachable**; the guard was put in, it was mutated,
        // and **the mutation survived**. Enumerating where a non-finite value could come from:
        //
        // - **The pane's rectangle with no number.** The handle is built from its coordinates, so
        //   it comes out with no number too, and egui **does not deliver the drag of a control like
        //   that** — checked: the method leaves through the «nobody is dragging me» door and never
        //   gets this far.
        // - **The pointer's position with no number.** Impossible: `interact_pointer_pos` returns
        //   `None` when the position holds a `NaN` (`response.rs:545-548`), and that has already
        //   been returned from.
        // - **A cell of zero width, with the pointer anywhere but the edge.** It gives infinity,
        //   which on being converted to an integer saturates at the maximum and the ceiling brings
        //   it down. It does not fold.
        // - **A cell of zero width, with the pointer right on the right edge.** It gives `NaN`,
        //   which falls to zero and folds — and **asking for zero from the right edge is exactly
        //   asking for it to be folded**.
        //
        // All four give the correct result or do not happen, so the guard would be a mechanism with
        // no scenario. Its sibling the border drag does need it, and that is why it has it: there
        // the state is a fraction and a clip over a `NaN` returns it untouched. **What separates «I
        // have pressed here» from «I want this width»: that the pointer has crossed at least half a
        // cell from where the panel is cut right now.** And it has to be said why it is *this* and
        // not what the review proposed, because both things were tried.
        //
        // **The hole was real.** The handle is drawn where the tiling **clips** the panel, which in
        // a narrow pane is not the width the user asked for; reading the pointer's position as
        // intent turns a press into «I want the clipped width» and wipes out for ever what had
        // been asked for with the window wide. The first protection demanded that the width asked
        // for *departed* from the painted one, and it **only moved the hole**: a half-cell tremble
        // already makes them different, and the two tests watching it used zero and one point — the
        // suite was green with the defect inside, which is the small-margin disguise the project's
        // hard rule names.
        //
        // **And the fix the review proposed —anchoring to the saved width and adding the
        // displacement to it— was implemented, was mutated and did NOT change a single outcome.**
        // The reason can be followed with a pencil: when the tiling clips, what it paints **is**
        // the ceiling (`fits - MINIMUM_CONSOLE`), so the direct reading gives `ceiling +
        // displacement` and the anchor gives `saved + displacement`, and the `min(ceiling)` below
        // leaves both at `ceiling`. And where there is no clipping, both coincide by construction.
        // A mechanism that cannot change an outcome is a mechanism with no scenario, so out it
        // goes.
        //
        // What really closes the hole is the guard here: without half a cell crossed the state is
        // not touched, **be the tremble one point or four**.
        let painted_cut = panel.map_or(pane_area.right(), |panel_rect| panel_rect.left());
        if ((painted_cut - pointer.x) / metrics.width()).round() == 0.0 {
            return false;
        }
        // The negative and what is not a number fall to zero on being converted to an integer,
        // which is what makes pushing beyond the right edge ask for zero — that is, ask for a fold.
        let requested = ((pane_area.right() - pointer.x) / metrics.width()).round() as usize;

        // **Folding is dragging to the end, which is how the project's written decision defines it:
        // asking for less than the floor.** And unfolding is the symmetric thing — pulling the
        // handle inwards.
        //
        // **Neither one nor the other touches the saved width, and there lies the detail that makes
        // unfolding give back what the user asked for.** Folding merely stops asking for it;
        // unfolding asks again for **the same one**, so the panel reappears with its earlier width
        // and not with the factory one. From that frame on the drag goes on ruling, which is what
        // was decided: your width, and the mouse ruling from there.
        //
        // > **Declared blind spot:** egui can repeat the pass with the pointer where it was, and
        // > the second one no longer sees the panel folded, so it writes the pointer's width
        // > instead of keeping the saved one. The two are halves of what was decided —the earlier
        // > width, and the mouse ruling— so no state of our own is invented to force one of the two. It is
        // > left said because it is the class of difference between frames this project has already
        // > paid for once without seeing it.
        if requested < panel::DRAG_FLOOR {
            let changed = !self.folded;
            self.folded = true;
            return changed;
        }
        if self.folded {
            self.folded = false;
            return true;
        }

        // `min` and not `clamp`: the floor was already applied by the folding branch, seven lines
        // above. A `clamp` here would read as if this line defended it, and it defends nothing —
        // and along the way the form that panics when the minimum exceeds the maximum disappears.
        let new = requested.min(ceiling);
        let changed = new != self.panel_width;
        self.panel_width = new;
        changed
    }

    /// The rectangle its console really occupies inside the pane's, which is **the one a pointer is
    /// measured against**.
    ///
    /// **It exists so that there is one answer to «where does the console start», and not two.** The
    /// grid a program is told about comes out of this same split ([`Pane::chrome_within`]), and
    /// whoever turns a pointer's position into a cell has to start counting from the same corner:
    /// with the pane's rectangle the two agree only for as long as nothing is taken off the top, and
    /// the first thing that is —the strip over each console— slides every selection and every mouse
    /// report by a row without anything failing.
    pub(crate) fn console_within(
        &self,
        rect: egui::Rect,
        metrics: &CellMetrics,
        now: SystemTime,
        headers: crate::header::Headers,
    ) -> egui::Rect {
        self.chrome_within(rect, metrics, now, headers).console()
    }
}

/// What a pane's rectangle is shared out into.
///
/// **The panel's own division travels whole and is not flattened into this one.** Taking its two
/// rectangles apart here forced the painting to put a [`panel::PanelTiling`] back together field by
/// field in order to hand it on — a second place deciding what that type is, in the one delivery
/// whose whole point is that there is a single place deciding where a console starts.
///
/// **The fields are private and read through their accessors**, which is [`CellMetrics`]' standard
/// in this crate: with them open anybody can build a chrome whose strip and whose console overlap,
/// and the one thing this type is for is that the two cuts agree. The only way to make one is
/// [`Pane::chrome_within`].
#[derive(Clone, Copy)]
pub(crate) struct PaneChrome {
    header: Option<egui::Rect>,
    below: egui::Rect,
    split: panel::PanelTiling,
}

impl PaneChrome {
    /// The strip over the console, or nothing where none is painted — which is every measured run.
    pub(crate) fn header(&self) -> Option<egui::Rect> {
        self.header
    }

    /// What is left of the pane once the strip is off: the console and its panel together.
    ///
    /// **It is what the panel's border is registered over**, and what the panel's width is measured
    /// against. Recomputing it at either of those call sites would be the second place that decides
    /// where the strip ends.
    pub(crate) fn below(&self) -> egui::Rect {
        self.below
    }

    /// What is left to the console, **and what its grid and every pointer are measured against**.
    pub(crate) fn console(&self) -> egui::Rect {
        self.split.console
    }

    /// The panel's own division of what is below the strip, whole.
    pub(crate) fn split(&self) -> panel::PanelTiling {
        self.split
    }

    /// Whether a press at that point belongs to the strip.
    ///
    /// **Exclusive of the bottom edge, and that is the whole of this method.**
    /// [`egui::Rect::contains`] is inclusive on both edges and the cut leaves the strip's bottom
    /// **equal** to the console's top, so the one line the two share answered yes twice: a press
    /// there both fell on the strip and resolved to row 0 of the console. What that costs is a
    /// press that moves the pane where somebody meant to mark, and a program that asked for the
    /// mouse never being told about a click on its own first row. It is the same tie the panel's
    /// grip documents twice, decided here in the one place both callers read.
    pub(crate) fn strip_holds(&self, at: egui::Pos2) -> bool {
        self.header
            .is_some_and(|strip| strip.contains(at) && at.y < strip.bottom())
    }
}

impl panel::PanelState {
    /// What the application has to say of its own about this panel, if it has something to say.
    ///
    /// **It is the app's line, not a producer's line**, and that is why it does not depend on
    /// `header`: that flag decides whether the pane's identity is painted, not whether the
    /// application can be honest. A producer that turned it off to hide that its panel is broken
    /// would be exactly the case this comes to prevent.
    ///
    /// **It lives here and not with the rest of the state** because a delivery that needs room in
    /// `panel.rs` takes a piece out of it first (**D28**), and this is the piece: everything the
    /// application has to say about a panel, in one place.
    pub(super) fn warning(&self, now: SystemTime) -> Option<String> {
        warning(self, now)
    }
}

fn warning(state: &panel::PanelState, now: SystemTime) -> Option<String> {
    if state.is_retired() {
        return None;
    }
    // **The person's line is joined with whatever the panel has to confess**, not chosen over it:
    // the two say different things, and a console can have been taken over while its file is also
    // unreadable.
    match (&state.taken_over, warning_about_the_panel(state, now)) {
        (None, said) => said,
        (Some(why), None) => Some(why.clone()),
        (Some(why), Some(said)) => Some(format!("{why} · {said}")),
    }
}

/// The rest of the warning: what this panel has to say about itself.
fn warning_about_the_panel(state: &panel::PanelState, now: SystemTime) -> Option<String> {
    // **A held panel's two confessions are independent, and both have to be able to appear.** A
    // held panel says nothing on its own — the identity row already carries who is driving it,
    // and a standing warning over a panel that is working perfectly is noise — but it can have two
    // things to say, and the first version returned the file refusal and made the other
    // unreachable for the whole life of the hold: an agent that wedged with its socket open kept
    // painting a claim as fresh, and D43 promises the panel goes stale where a person can read
    // that it has. They are joined rather than one chosen, because they say different things.
    if let Some(holder) = &state.holder
        && !holder.yielded
    {
        let mut said: Vec<String> = Vec::new();
        if holder.file_refused {
            said.push(format!(
                "the file is not being read: \"{}\" is publishing here",
                holder.source
            ));
        }
        if state.problem.is_none()
            && let Some(shown) = &state.last_good
            && shown.is_stale_at(now)
        {
            said.push(panel::no_news(shown, now));
        }
        return (!said.is_empty()).then(|| said.join(" · "));
    }

    // **The file that does not exist yet is the exception, and it goes first for that.** The rest
    // is ordered by what has to be said: first the read failures, which are the most recent thing
    // known, and only if there is none is the clock looked at.
    match (&state.problem, &state.last_good) {
        (Some(problem), _) if panel::is_a_missing_file(problem) => state
            .last_good
            .as_ref()
            .map(|_| "the panel file is no longer there".to_owned()),
        (Some(problem), Some(_)) => Some(format!("what is shown is not the latest: {problem}")),
        (Some(problem), None) => Some(format!("no panel: {problem}")),
        (None, Some(shown)) if shown.is_stale_at(now) => Some(panel::no_news(shown, now)),
        (None, _) => None,
    }
}
