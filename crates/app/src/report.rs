//! The frame's accounting, and what gets published of it.
//!
//! **The window's own accounting, not the bench's.** [`crate::injected`] is the half that pretends to
//! be a hand; [`crate::measure`] holds the spans and the guards that read them. What lives here is the
//! moment they meet: closing each frame — folding every console's spans and keeping what none of them
//! timed — the window walking every tab and every console at exit to say what each one cost, and the
//! pass that stops a round escaping that sum by never being painted.
//!
//! **One idea and three methods, and what the third one changed is worth saying.** The module landed
//! with two, on the argument that only what the window is **able to say** belongs here; a blind idiom
//! audit then pointed out that the argument was drawn too tightly, because the sentence «a frame's
//! cost is split by subtraction» describes a **subtraction somebody performs**, and that somebody was
//! still in `lib.rs`. [`Window::close_the_frame`] is that subtraction, and it arrived under **D28**
//! with the delivery that declared the strips. The idea is unchanged and its boundary moved one step
//! out: the arithmetic of a frame, and the report that publishes it.
//!
//! **What was kept out, and it is the lesson.** `measure_the_ones_out_of_sight` first came here for
//! company, on the reading that both are about measuring. The same audit sent it back: that method
//! gives the consoles of an unseen tab the grid they would really have, so the program inside formats
//! for its own width instead of for the eighty start-up columns. **That is the milestone's promise,
//! not an accounting of it** — only the stopwatch inside it is measurement, and a name with «measure»
//! in it is what made the mistake easy.
//!
//! Why any of it is declared at all: a frame's cost is split by subtraction, so work that is not
//! declared is published as the framework's cost — the figure **D3** judges egui by.
//!
//! **Not into `measure.rs`, which is the module whose name matches it**, and the first telling of
//! this put the weak reason first. The strong one is that `measure.rs` **holds no `impl Window` at
//! all** — types and free functions, nothing that reaches into a window — so the seam was already
//! there and this only lands on the near side of it. The arithmetic agreed rather than decided: at
//! the time `measure.rs` had no room for these 54 lines, so the ceiling would have refused the move
//! anyway. **The number it used to state here is deliberately gone**: `measure.rs` has since been
//! split and the figure was wrong twice over, which is what a comment asserting a count always
//! becomes. Consulting the ceiling before choosing a home is what **D28** asks for; leading with it
//! made a boundary look like a coincidence of size.
//!
//! It came out of `lib.rs` under **D28**.

use crate::tiling::DIVIDER_THICKNESS;
use crate::{Window, measure};

/// What the strips have cost — the frame in progress on one side, the whole run on the other.
///
/// **One type because the two are one decision taken twice a frame, and splitting them across the
/// window is how half of it gets forgotten.** The half in progress is summed per pane and per pass;
/// the half that is published is closed once per frame. Keeping them apart on [`Window`] is what the
/// tab bar does, and the tab bar is also where the rule below lives only as a comment.
#[derive(Default)]
pub(crate) struct Strips {
    /// What the strips of the frame in progress have cost, still unrecorded, or nothing when no pane
    /// has cut one.
    ///
    /// **`None` and `Some(ZERO)` are different answers, and [`Strips::close_the_frame`] reads them
    /// apart**: `None` is a frame in which no pane cut a strip at all, `Some(ZERO)` one in which a
    /// pane drew a band faster than the clock could measure. Counting the second as the first would
    /// make the number the measurement script reads answer a question nobody asked.
    pending: Option<std::time::Duration>,
    /// What the run has published so far, frame by frame.
    run: measure::Strips,
}

impl Strips {
    /// Takes what one pane's strip cost this frame: **into** the frame's tally and **out of** the
    /// frame's rest, in one statement.
    ///
    /// **The two halves live in one method because forgetting either publishes a lie, and they are
    /// different lies.** Without the tally the report cannot name what the strips cost, and it would
    /// have to be got by subtracting two batches. Without the subtraction the strips are left inside
    /// «the rest of the frame», which is published as the interface framework's cost — the figure
    /// **D3** judges egui by (**D13**). The tab bar's accumulation states the same rule in a comment
    /// one file away; here it is a method, which is the difference between a rule and a rule a test
    /// can hold.
    ///
    /// **It sums and does not replace**, because a frame has as many strips as its tab has panes and
    /// what the report publishes is what **one frame** pays for all of them. With a replacement and
    /// two consoles the published mean is half the real one, and nothing else in the report
    /// disagrees with it.
    ///
    /// **The rest of the frame arrives as an argument and is not reached through the window**,
    /// because the only caller is the walk over the panes, which holds the active tab mutably for as
    /// long as it runs: two disjoint fields can be borrowed there, a `&mut Window` cannot. And the
    /// cost arrives as the `Option` the painting handed back, so «no strip was cut» is answered in
    /// the one place that also answers what a strip costs.
    pub(crate) fn a_strip_was_painted(
        &mut self,
        rest_of_the_frame: &mut std::time::Duration,
        cost: Option<std::time::Duration>,
    ) {
        let Some(cost) = cost else {
            return;
        };
        *self.pending.get_or_insert_default() += cost;
        *rest_of_the_frame += cost;
    }

    /// Closes the frame: what its passes accumulated becomes one sample of the run.
    ///
    /// **Once per frame and not once per pass**, for the reason the tab bar's own close gives: egui
    /// repeats the pass over the same input and both passes really paint, so recording there would
    /// publish a mean per pass under a label that says «per frame». The work of both passes is
    /// summed, which is what [`Strips::a_strip_was_painted`] does; the frame is still one.
    pub(crate) fn close_the_frame(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.run.record(pending);
        }
    }

    /// What the run has to publish, with the session's gate on it.
    fn published(&self, gate: crate::header::Headers) -> measure::Strips {
        self.run.under(gate)
    }

    /// What the frame in progress has accumulated and nobody has recorded yet.
    ///
    /// **It exists so a test can assert over the real walk** — that a painted strip reached this
    /// tally at all — rather than over a call it made itself, which proves the accumulator works and
    /// not that the painting feeds it. Same reason as [`measure::Timings::recorded_panel`].
    #[cfg(test)]
    pub(crate) fn pending(&self) -> Option<std::time::Duration> {
        self.pending
    }

    /// How many frames of this run painted a strip, as the report will publish it.
    #[cfg(test)]
    pub(crate) fn frames(&self) -> usize {
        self.run.frames
    }
}

impl Window {
    /// Closes the previous frame: each console closes its spans and the window keeps whatever none
    /// of them timed.
    ///
    /// **It came out of `lib.rs` under D28 with the delivery that declared the strips**, and it
    /// lands here rather than anywhere else because this module is already the one that holds what
    /// the window can *say* it did: the subtraction this performs is the mechanism the report's
    /// «rest of the frame» is, and [`Window::close_the_unpainted_rounds`] beside it exists to stop
    /// a round escaping that same sum.
    pub(crate) fn close_the_frame(&mut self, cpu_cost: std::time::Duration) {
        let timed = self
            .all_panes_mut()
            .filter_map(|pane| pane.timings.close_frame(cpu_cost))
            .reduce(|one, other| one + other);

        // **It is always emptied, whether there is a rest to record or not.** Dragging it into the
        // next frame would be subtracting from one frame work that happened in another, which is
        // the kind of error this instrument is after. See [`Window::timed_separately`] for why it
        // exists.
        let separately = std::mem::take(&mut self.timed_separately);

        // **The mirror goes into the subtraction AND into its series, and both are needed.** The
        // accumulator above discounts but publishes no row at all; a series publishes but discounts
        // nothing. With only one of the two, this work —walking the tree of every tab, on every
        // frame— would end up inside the figure D3 judges the framework with, or auditable but
        // without subtracting.
        let mirror = std::mem::take(&mut self.mirror_this_frame);
        if !mirror.is_zero() {
            self.saving.record_mirror(mirror);
        }

        // **The bar is recorded here and not where it is painted**, for the same reason as the
        // mirror: the thing above happens once per **pass** and this once per **frame**, so
        // counting there would publish an average per pass under the «per frame» label. What is
        // added up is the work of both passes, which happened in full.
        let tab_bar = std::mem::take(&mut self.tab_bar_this_frame);
        if !tab_bar.is_zero() {
            self.tab_bar_cost.record(tab_bar);
        }

        // **The strips are recorded here too, and for the bar's reason**: this runs once per frame
        // and the painting once per pass. What is added up is the work of both passes, which
        // happened in full, under a count that really says frames.
        //
        // **And the guard is «was anything painted», not «did it cost anything».** A strip painted
        // in under the clock's resolution is a frame that had one, and skipping it would make the
        // count the script reads answer a question nobody asked — see [`measure::Strips`].
        self.strips.close_the_frame();

        // **And the raw-input hook's is declared here, AFTER emptying.** See
        // [`Window::spent_injecting`]: it ran after the frame being closed had finished, so it
        // belongs to the one that is starting. Adding it above would subtract it from the one that
        // did not pay for it.
        self.timed_separately += std::mem::take(&mut self.spent_injecting);

        // With nobody's spans there is no rest to record: the whole frame would be counted as rest
        // and the series would end up measuring above all the frames in which nothing was
        // happening.
        if let Some(timed) = timed {
            self.rest_of_frame
                .record(cpu_cost, timed + separately + mirror);
        }
    }
    /// Writes the session report to whatever destination it is given.
    ///
    /// **It receives the destination instead of printing**, for the same reason as each console's
    /// span table: it is the only way for a test to assert about **what the report really says**
    /// and not about the constants it is written with. Checking the constant proves it is well
    /// worded, not that anyone prints it — and that, in this project, has already let things
    /// through with the suite green.
    pub(crate) fn publish_the_report(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.all_panes().any(|pane| pane.timings.has_any()) {
            return Ok(());
        }

        let grids: Vec<_> = self
            .tabs
            .iter()
            .map(|tab| {
                (
                    tab.name,
                    tab.panes()
                        .into_iter()
                        .map(|pane| (pane.name, pane.screen.size()))
                        .collect(),
                )
            })
            .collect();
        measure::header(
            to,
            measure::SizeInPoints {
                width: self.window_size.x,
                height: self.window_size.y,
            },
            &grids,
            self.active().name,
            measure::SessionHeader {
                drags: self
                    .injected_drags
                    .as_ref()
                    .map(|injected_drags| measure::Drags {
                        injected: injected_drags.completed,
                        effective: injected_drags.with_effect,
                        marked: injected_drags.selected,
                        reported: injected_drags.reported,
                        gesture: injected_drags.which.as_published(),
                    }),
                restoring: self.restoring,
                settings: self.reading_settings,
                chosen_font: self.chosen_font.clone(),
                tab_bar: self.tab_bar_cost,
                reflow: self.reflow,
                blink: measure::Blinking {
                    frames: self.blink.frames_painted(),
                    ceiling: self.blink.ceiling,
                },
                buttons: self.buttons,
                // **What was painted comes from the counter and what was asked for from the gate,
                // and the two are never derived from each other.** That disagreement is the one
                // thing this line exists to be able to report: a run that asked for strips and drew
                // none, and a run that drew some without asking, are both thrown away by
                // `scripts/measure-frame.sh`, and neither is visible from the other half.
                strips: self.strips.published(self.headers),
                // **Summed here, at the end, out of each pane's own count, plus what the panes that
                // already closed banked on the window.** The channel is drained from two places —
                // the painting and the sweep over the consoles nobody looks at — and a counter each
                // of those had to remember to bump is a counter one of them eventually forgets. The
                // window's share is not optional: without it a pane or a tab closed during the run
                // disappears from these three numbers, and the script can invalidate a correct run.
                control: self
                    .all_panes()
                    .filter_map(|pane| pane.control.as_ref())
                    .map(crate::control::PaneControl::did)
                    .fold(self.control, |so_far, did| measure::Control {
                        applied: so_far.applied + did.applied,
                        refused: so_far.refused + did.refused,
                        released: so_far.released + did.released,
                    }),
            },
        )?;

        // Where the divider ended up, and not «half and half»: since it can be dragged, the spans
        // below can come from two consoles of very different widths, and a span table with no idea
        // which tiling it was taken with compares with nothing.
        //
        // **The fraction asked for is what is published, not the resulting widths.** The real
        // widths are above, in each console's grid, which is what was used to interpret; repeating
        // them here out of the fraction would make them disagree in the one case where the floor
        // acts.
        //
        // **And of the tab that was being seen, because it is the only one whose divider and focus
        // mean anything**: in one that is not painted the divider is not dragged and nothing is
        // typed.
        let visible = self.active();
        // **Every divider and how many there are, not just the first.** Since splitting is
        // possible, three consoles are one gesture away, and a line publishing the first fraction
        // and calling it «the divider» would be exactly the wrongly labelled number this project
        // has already paid for once. Each one goes with **its own span** as reference because that
        // is what it shares out: a divider between three siblings is not measured against the width
        // of the window.
        let dividers = visible.divider_fractions();
        if !dividers.is_empty() {
            let at: Vec<String> = dividers
                .iter()
                .map(|fraction| format!("{:.0} %", fraction * 100.0))
                .collect();
            // **The line says which tab it is talking about, and before it did not.** It describes
            // the tree of the one that was being seen at exit, while the tables below are of
            // **all** of them; with a single divider per tab that made no difference because it
            // said the same about any of them, but since splitting is possible each tab has its
            // own, and whoever read the report would match this tiling with consoles it does not
            // describe.
            //
            // **«Thickness» and not «width», and the difference shows in the first batch of four
            // panes in a square**: of its three dividers, one is vertical and two are horizontal,
            // so calling the two points of the horizontal ones a width publishes a dimension that
            // is not the one it measures. It is the word the constant already uses.
            writeln!(
                to,
                "dividers:     {} in tab {}, at {} of their shareable span at exit, and they are \
                 {DIVIDER_THICKNESS} points thick each. If it was dragged during the session, the \
                 spans below mix tilings",
                dividers.len(),
                visible.name,
                at.join(" · ")
            )?;
        }
        // **By the pane's name, not by its position.** Position identifies well while the panes are
        // two and fixed; the name keeps identifying when they stop being that, and publishing right
        // away the one that is going to hold is what keeps the batches from before and the ones
        // after from being read differently.
        //
        // **And a focus that points at nobody is said in those words, not with «no side».** They
        // are two very different things and before they came out with the same text: that there are
        // three panes and the side no longer describes anything is normal; that the focus does not
        // exist means the keys were reaching no console at all, and that has to be readable in the
        // report.
        let how_many_visible = visible.panes().len();
        let where_it_looks = match visible.where_the_focus_is() {
            Some(at) => measure::side(how_many_visible, at).map_or_else(
                // **With the count up front, instead of «with more than two».** While a tab always
                // had two consoles or more —it was born with two and splitting only added—, «no
                // side» could only mean that there were too many. Since **closing** is possible,
                // the tab with one is reachable, and that text would say exactly the opposite of
                // what is happening. The count tells the truth in both cases, and along the way
                // there is no longer any need to count lines to know how many there were.
                || format!("no side: there are {how_many_visible} and a side only describes something with two"),
                str::to_string,
            ),
            None => "DOES NOT EXIST: the keyboard reached no console".to_string(),
        };
        writeln!(
            to,
            "focus at exit: pane {} ({where_it_looks}) of tab {}",
            visible.focus, visible.name
        )?;

        // **Every tab, not just the one that was being seen.** A background console times its
        // interpreting just like the rest, and publishing only the visible ones would throw away
        // exactly the numbers the two-tab batch exists to take. **And each table is titled with the
        // name of its pane.** It is the place where getting it wrong costs the most: whoever reads
        // the report sees a table of numbers under a label and has no way of checking that they are
        // that console's. **And the side hint goes quiet as soon as they are not two, instead of
        // indexing by the position.** With three panes —which since splitting is possible is one
        // gesture away— indexing the array of two sides would be a panic **inside the report that
        // is published on exit**, that is, losing at a stroke everything measured in the session.
        // It is the same case the header already solved, through the same door.
        for tab in &self.tabs {
            let which_tab = tab.name;
            let its_panes = tab.panes();
            for (which, pane) in its_panes.iter().enumerate() {
                let side = measure::side(its_panes.len(), which)
                    .map_or_else(String::new, |side| format!(" ({side})"));
                writeln!(to, "\n── tab {which_tab}, pane {}{side} ──", pane.name)?;
                pane.timings.publish_spans(to)?;
            }
        }

        self.rest_of_frame.publish(to)?;
        // **The two saving spans go after the rest, and each with a row of its own.** They are
        // discounted from that rest —they go into the window's accumulator—, so without these rows
        // the number would exist and **nobody could audit it**.
        self.saving.publish(to)?;
        // **And discovery the same: its row goes after the rest because it too is discounted from
        // it.** It is the most expensive span this project adds outside a console, so without
        // declaring it D3's number would swallow it whole.
        self.discovery.publish(to)?;
        // **And the color one goes last, for the same reason as the three above**: it is subtracted
        // from the painting span, so without this row the number would exist and nobody could audit
        // it.
        self.color.publish(to)
    }

    /// Closes the round of the consoles that **interpreted and were not painted**, so that their
    /// work does not end up published as interface framework cost.
    ///
    /// **The bias this avoids is large and silent.** The frame's cost is shared out by subtraction:
    /// each console declares its own and what is left over is attributed to the framework. A
    /// console of a tab that is not seen interprets bytes just like the rest, but does not go
    /// through the painting, which was the only place that closed its round — so its time was
    /// handed to the rest as a gift, and with a single background tab dumping it would multiply
    /// that **several times over**. And it left no trace: the next frame's pumping overwrites what
    /// is pending, so the sample was not doubled, it was **lost**.
    ///
    /// **Zero queueing is the truth, not a filler**: that console queued no painting at all. **The
    /// panel's span is no longer zero**, and that is the difference from the first version of this
    /// sweep: a background console **does** reread its `panel.json` —how much width it has left
    /// depends on whether there is a panel—, and [`Window::measure_the_ones_out_of_sight`] times
    /// that read and hands it over here. And **the wait is declared as `None` on purpose**, which
    /// is what keeps the four averages from being touched —see [`measure::Timings::close_frame`]—;
    /// what is kept is what was interpreted and what the panel cost, which is exactly what the
    /// subtraction needs. That is why this **carries no new series**.
    ///
    /// **It goes at the end of the interface pass and not after pumping**, because until the
    /// painting has happened it is not known what was painted: a shortcut can change the visible
    /// tab in that very frame.
    ///
    /// With the window minimized or occluded there is no interface pass, so this does not run and
    /// nobody declares anything. **It is not a hole**: then the visible console does not declare
    /// either, so [`Window::close_the_frame`] finds nothing to subtract and records no rest — the
    /// frame is left unmeasured, which is the correct thing, instead of measured short.
    pub(crate) fn close_the_unpainted_rounds(&mut self) {
        for pane in self.all_panes_mut() {
            // **The panel's span travels with the round, and not in a record of its own.** It is
            // not a preference: [`measure::Timings::record_round`] **overwrites** what is pending,
            // so a second record in the same frame would take the good round with it. Here it does
            // not get to happen —`pump` always leaves something pending and the two things are
            // declared together—, and for that very reason it is worth writing down: the safe way
            // to add a span to this round is to put it inside, never beside it.
            if let Some((_, collected_at, interpreted)) = pane.pending_measurement.take() {
                pane.timings.record_round(
                    measure::Spans::of_a_round(
                        None,
                        collected_at,
                        interpreted,
                        std::time::Duration::ZERO,
                        std::mem::take(&mut pane.panel_reread_unpainted),
                    )
                    .and_its_control(std::mem::take(&mut pane.control_unpainted)),
                );
            }
        }
    }
}
