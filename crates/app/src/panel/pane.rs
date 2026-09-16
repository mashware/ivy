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

use crate::Pane;
use crate::panel;

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
    pub(crate) fn refresh_its_panel(&mut self) -> PanelRefresh {
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
                control.apply(&mut self.panel_state, SystemTime::now());
                started.elapsed()
            });

        let started = std::time::Instant::now();
        if let (true, Some(dir)) = (self.to_reread.take(), self.panel.as_ref()) {
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
            if self.panel_state.refresh(dir.path()) == panel::Refreshed::Declined
                && self.panel_state.refused_the_file()
            {
                self.to_reread.raise();
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
        cell_width: f32,
        now: SystemTime,
    ) -> panel::PanelTiling {
        // **The width comes from the pane itself, and that is what keeps the two callers in
        // agreement.** One measures the grid of the consoles that are not painted and the other
        // paints the one on show; if the width travelled as a parameter, each could bring its own
        // and a background console would again be measured with a width that is not its own — the
        // defect that already cost 36 columns.
        //
        // **Folded asks for no width, and that is why folding leaves the console the whole
        // rectangle**: it is the same path a pane whose producer has published nothing goes down,
        // so the two look alike without there being two ways of having no panel.
        panel::set_aside(
            rect,
            cell_width,
            (!self.folded && self.panel_state.is_visible(now)).then_some(self.panel_width),
        )
    }

    /// The rectangle its console really occupies inside the pane's, which is **the one a pointer is
    /// measured against**.
    ///
    /// **It exists so that there is one answer to «where does the console start», and not two.** The
    /// grid a program is told about comes out of this same split
    /// ([`Pane::tile_with_its_panel`]), and whoever turns a pointer's position into a cell has to
    /// start counting from the same corner: with the pane's rectangle the two agree only for as
    /// long as nothing is taken off the top, and the first thing that is —a header over each
    /// console— would slide every selection and every mouse report by a row without anything
    /// failing.
    ///
    /// The clock is passed in for the reason the split gives: two clocks in one pass can disagree
    /// about whether a panel is still there.
    pub(crate) fn console_within(
        &self,
        rect: egui::Rect,
        cell_width: f32,
        now: SystemTime,
    ) -> egui::Rect {
        self.tile_with_its_panel(rect, cell_width, now).console
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
        // **The person's line is joined with whatever the panel has to confess**, not chosen over it:
        // the two say different things, and a console can have been taken over while its file is also
        // unreadable.
        match (&self.taken_over, self.warning_about_the_panel(now)) {
            (None, said) => said,
            (Some(why), None) => Some(why.clone()),
            (Some(why), Some(said)) => Some(format!("{why} · {said}")),
        }
    }

    /// The rest of the warning: what this panel has to say about itself.
    fn warning_about_the_panel(&self, now: SystemTime) -> Option<String> {
        // **A held panel's two confessions are independent, and both have to be able to appear.** A
        // held panel says nothing on its own — the identity row already carries who is driving it,
        // and a standing warning over a panel that is working perfectly is noise — but it can have two
        // things to say, and the first version returned the file refusal and made the other
        // unreachable for the whole life of the hold: an agent that wedged with its socket open kept
        // painting a claim as fresh, and D43 promises the panel goes stale where a person can read
        // that it has. They are joined rather than one chosen, because they say different things.
        if let Some(holder) = &self.holder {
            let mut said: Vec<String> = Vec::new();
            if holder.file_refused {
                said.push(format!(
                    "the file is not being read: \"{}\" is publishing here",
                    holder.source
                ));
            }
            if self.problem.is_none()
                && let Some(shown) = &self.last_good
                && shown.is_stale_at(now)
            {
                said.push(panel::no_news(shown, now));
            }
            return (!said.is_empty()).then(|| said.join(" · "));
        }

        // **The file that does not exist yet is the exception, and it goes first for that.** The rest
        // is ordered by what has to be said: first the read failures, which are the most recent thing
        // known, and only if there is none is the clock looked at.
        match (&self.problem, &self.last_good) {
            (Some(problem), _) if panel::is_a_missing_file(problem) => self
                .last_good
                .as_ref()
                .map(|_| "the panel file is no longer there".to_owned()),
            (Some(problem), Some(_)) => Some(format!("what is shown is not the latest: {problem}")),
            (Some(problem), None) => Some(format!("no panel: {problem}")),
            (None, Some(shown)) if shown.is_stale_at(now) => Some(panel::no_news(shown, now)),
            (None, _) => None,
        }
    }
}
