//! Publishing the measurement report, and the two passes that keep it honest.
//!
//! **The window's own accounting, not the bench's.** [`crate::injected`] is the half that pretends to
//! be a hand; [`crate::measure`] holds the spans and the guards that read them. What lives here is the
//! moment they meet: the window walking every tab and every console at exit to say what each one cost,
//! plus the two passes that make sure nothing goes unaccounted — measuring the consoles you cannot see
//! and closing the rounds that were never painted.
//!
//! **Two methods and not three, and the third is the lesson.** Publishing the report first came here
//! with `measure_the_ones_out_of_sight` for company, on the reading that both are about measuring. A
//! blind idiom audit sent it back: that method gives the consoles of an unseen tab the grid they
//! would really have, so the program inside formats for its own width instead of for the eighty
//! start-up columns. **That is the milestone's promise, not an accounting of it** — only the
//! stopwatch inside it is measurement, and a name with «measure» in it is what made the mistake easy.
//!
//! What is left is one idea: a frame's cost is split by subtraction, so work that is not declared is
//! published as the framework's cost — the figure **D3** judges egui by. Publishing the report says
//! what each console cost, and closing the rounds nobody painted stops a round escaping that sum.
//! Not what the window does; what it is able to say it did.
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

impl Window {
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
