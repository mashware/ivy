//! How a window comes into being: with the tabs it is asked for, or with the ones it left behind.
//!
//! **Three methods and one subject.** Building a window is not what a window *does* — it is what
//! somebody does to one — and it reads as its own vocabulary: how many tabs, which consoles, what
//! the session file said and what happens when it says nothing understandable. Everything about
//! running is in [`crate`]; everything about starting is here.
//!
//! It came out of the window's core under **D28**, which gives an `impl` block a ceiling of 300
//! production lines and records what that file is at so the number can only go down. **It went out
//! ahead of the delivery that needed the room** — the milestone that gives the panel buttons — which
//! is the order that entry's own prose asks for.

use crate::painter::CellMetrics;
use crate::*;
use eframe::egui;

impl Pane {
    pub(crate) fn new_counting_reads(
        ctx: &egui::Context,
        name: usize,
        reads: control::ReadActivity,
    ) -> term_core::Result<Self> {
        Self::launching_counting_reads(ctx, &the_users_shell(), name, reads)
    }

    #[cfg(test)]
    pub(crate) fn launching(
        ctx: &egui::Context,
        program: &str,
        name: usize,
    ) -> term_core::Result<Self> {
        Self::launching_counting_reads(ctx, program, name, control::ReadActivity::default())
    }

    pub(crate) fn launching_counting_reads(
        ctx: &egui::Context,
        program: &str,
        name: usize,
        reads: control::ReadActivity,
    ) -> term_core::Result<Self> {
        Self::with_panel_counting_reads(
            ctx,
            program,
            &[],
            panel_or_none(PanelDir::create()),
            name,
            reads,
        )
    }

    pub(crate) fn launching_with_args_counting_reads(
        ctx: &egui::Context,
        program: &str,
        args: &[String],
        name: usize,
        reads: control::ReadActivity,
    ) -> term_core::Result<Self> {
        Self::with_panel_counting_reads(
            ctx,
            program,
            args,
            panel_or_none(PanelDir::create()),
            name,
            reads,
        )
    }

    #[cfg(test)]
    pub(crate) fn with_panel(
        ctx: &egui::Context,
        program: &str,
        args: &[String],
        panel: Option<PanelDir>,
        name: usize,
    ) -> term_core::Result<Self> {
        Self::with_panel_counting_reads(
            ctx,
            program,
            args,
            panel,
            name,
            control::ReadActivity::default(),
        )
    }

    fn with_panel_counting_reads(
        ctx: &egui::Context,
        program: &str,
        args: &[String],
        panel: Option<PanelDir>,
        name: usize,
        reads: control::ReadActivity,
    ) -> term_core::Result<Self> {
        let identity = identity_of(panel.as_ref());
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let (pty, output) = match Pty::spawn_with_env(program, &borrowed, INITIAL_GRID, &identity) {
            Ok(spawned) => spawned,
            Err(failure) => {
                eprintln!(
                    "'{program}' could not be launched ({failure}); opening {FALLBACK_SHELL}"
                );
                Pty::spawn_with_env(FALLBACK_SHELL, &[], INITIAL_GRID, &identity)?
            }
        };

        let waker = ctx.clone();
        let pty_output = PtyOutput::new(output, move || waker.request_repaint());
        let (panel_state, control) = panel::pane::state_and_control(ctx, panel.as_ref(), reads);

        Ok(Self {
            name,
            pty,
            pty_output,
            panel,
            control,
            panel_state,
            to_reread: Flag::raised(),
            screen: Screen::new(INITIAL_GRID),
            painted_console: None,
            received: Vec::new(),
            keystrokes: Vec::new(),
            held: None,
            measured: INITIAL_GRID,
            timings: Timings::default(),
            pending_measurement: None,
            panel_reread_unpainted: std::time::Duration::ZERO,
            control_unpainted: std::time::Duration::ZERO,
            reflowed: std::time::Duration::ZERO,
            pending_wheel: WheelInput::default(),
            panel_width: panel::DEFAULT_WIDTH,
            folded: false,
            taken_over: None,
        })
    }
}

impl Window {
    /// A window with as many tabs as it is asked for.
    ///
    /// **How many there are is told from outside and is not read from the environment here**, even
    /// though the only place that asks for more than one today is [`run`] reading
    /// [`TABS_VARIABLE`]. Reading it inside would let a variable exported in the session change
    /// what the tests that call here open —they run in the same process and at the same time—, and
    /// a test saying «the window opens with a single tab» would start failing depending on which
    /// other one happened to be running at that instant.
    ///
    /// **Asking for zero tabs is the caller's error, and it is not corrected here.** The floor is
    /// set by [`how_many_tabs`], which is where the only value we do not control comes in; a second
    /// guard here would only silently cover up a call that cannot happen. The honest thing would be
    /// a type that did not admit zero —`NonZeroUsize`—, and it is not part of this change because
    /// it forces a wrapper in every test that opens a window.
    pub(crate) fn with_tabs(
        ctx: &egui::Context,
        how_many: usize,
        layout: OpeningLayout,
        size: Option<egui::Vec2>,
    ) -> term_core::Result<Self> {
        // **If any one fails none is opened**, and the `?` over the `collect` is what achieves it:
        // the same criterion as `panel_or_none`. Half a dozen tabs of which one is half-built is
        // worse than not starting up at all, because the failure is discovered on reaching it.
        let per_tab = layout.how_many_panes();
        let control = control::WindowActivity::default();
        let reads = control.read_activity();
        let tabs: Vec<Tab> = (0..how_many)
            .map(|name| Tab::new_counting_reads(ctx, name, name * per_tab, &layout, reads.clone()))
            .collect::<term_core::Result<_>>()?;

        Self::with_these_tabs(ctx, tabs, size, control)
    }

    /// The window the application opens: factory-fresh, or with whatever was left saved.
    ///
    /// **And if restoring fails, it opens factory-fresh instead of not opening.** It is this
    /// format's per-document tolerance taken all the way: what can fail here is opening N consoles
    /// in one go —a descriptor, a process—, and against that «the usual window» is a much better
    /// answer than an application that does not start. Atomicity comes from the step below: if one
    /// leaf cannot be opened, **none is installed**, so half a layout never arrives here.
    pub(crate) fn opening(
        ctx: &egui::Context,
        with_what: OpenWith,
        program: &str,
        size: Option<egui::Vec2>,
        persistence: session::Persistence,
    ) -> term_core::Result<Self> {
        // **The program is received, and the branch that restores is the one that uses it.** The
        // factory branch opens the user's shell as it always has, just like the three dozen tests
        // that call it: passing it all the way down there would mean touching the signature of a
        // constructor with thirty-three calls for the one path this PR does not introduce. It is
        // said because the previous comment claimed that **no** branch read the environment, and
        // that was false precisely for the factory one — the two reviews caught it.
        let mut do_not_overwrite = None;
        let opened = match with_what {
            OpenWith::Factory { tabs, layout } => Self::with_tabs(ctx, tabs, layout, size),
            OpenWith::Restored(saved) => {
                let panes = saved.leaves();
                let control = control::WindowActivity::default();
                // **Opening them is what is timed, not reading the file, and the difference
                // matters.** What grows with the size of the layout are the `fork`s and `exec`s
                // here: reading a four-leaf document and a two-hundred-leaf one cost almost the
                // same. What reading cost travels with the persistence, which already knows how
                // long its own load took.
                let before = Instant::now();
                match Self::materialize_counting_reads(
                    ctx,
                    &saved,
                    program,
                    control.read_activity(),
                ) {
                    Ok(tabs) => {
                        Self::with_these_tabs(ctx, tabs, size, control).map(|mut window| {
                            // **Only when they really were opened.** If materializing fails it
                            // opens factory-fresh, and a factory window declaring «restored» would
                            // be the disguise this line exists to remove.
                            //
                            // **And the stopwatch closes HERE, with the window already built, not
                            // when the processes exist.** Behind `materialize` there are two jobs
                            // that **grow with the number of consoles** —arming each directory's
                            // watcher and describing the whole tree for the first mirror—, and
                            // stopping it earlier left them out of a number that claims to measure
                            // exactly that.
                            window.restoring = Some(measure::Restoring {
                                consoles: panes,
                                read: persistence.load_time(),
                                opening: before.elapsed(),
                            });
                            window
                        })
                    }
                    Err(failure) => {
                        eprintln!(
                            "the saved layout of {panes} consoles could not be opened ({failure}); \
                             opening the factory one, and what is saved is left alone"
                        );
                        // **And this session does not step on the file, which is what avoids losing
                        // the layout for ever.** What is left on screen is a factory window; with
                        // automatic saving on, the user's first change would write it over their
                        // twenty consoles. Before this milestone the file survived a failed
                        // start-up and the next one recovered it: this preserves that property.
                        do_not_overwrite = Some("the layout that was saved could not be opened");
                        Self::with_tabs(ctx, 1, OpeningLayout::DEFAULT, size)
                    }
                }
            }
        };

        // **The one that loaded at start-up stays with the window, and this was a debt with a
        // name.** The veto of a newer version lives in it: building a second one mid-session would
        // lose that veto and would overwrite the layout of a newer binary, which is the exact
        // damage the format version exists to avoid. It is installed here, after building, instead
        // of being pushed down through the three constructors: the factory one is called by
        // thirty-three tests that save nothing.
        opened.map(|mut window| {
            window.persistence = persistence;
            if let Some(reason) = do_not_overwrite {
                window.persistence.forbid_overwrite(reason);
            }
            window
        })
    }

    /// The rest of the window, which is the same whether it was restored or not.
    ///
    /// **It exists so that opening factory-fresh and restoring share everything that is not the tab
    /// list.** The counters, the watcher and the stopwatches come **from the built list** and not
    /// from what anyone might have asked for: with two paths arming this state on their own, the
    /// second would forget one of them.
    pub(crate) fn with_these_tabs(
        ctx: &egui::Context,
        tabs: Vec<Tab>,
        size: Option<egui::Vec2>,
        control: control::WindowActivity,
    ) -> term_core::Result<Self> {
        let next_name = tabs.len();
        // **Counted from the list and not from the parameter**, for the same reason explained below
        // for the tab name: they are two sources for the same datum, and with two you can take the
        // wrong one.
        let next_pane_name = tabs.iter().map(|tab| tab.panes().len()).sum();

        let mut window = Self {
            // **Over the panes of every tab, not just the first.** A background tab left unwatched
            // would not reread its panel until someone looked at it, which is exactly the opposite
            // of what this milestone promises.
            watcher: Self::watch(ctx, tabs.iter().flat_map(|tab| tab.panes().into_iter())),
            tabs,
            active: 0,
            modifiers: egui::Modifiers::NONE,
            discoverer: fonts::Discoverer::default(),
            discovery: measure::Discovery::default(),
            color: measure::Rasterizing::default(),
            theme: Theme::default(),
            keymap: keyboard::Keymap::default(),
            copy_enabled: true,
            blink: alarms::BlinkState::default(),
            font_size: DEFAULT_FONT_SIZE,
            rest_of_frame: RestOfFrame::default(),
            frame_started: Instant::now(),
            last_closed_frame: 0,
            timed_separately: std::time::Duration::ZERO,
            tab_bar_this_frame: std::time::Duration::ZERO,
            tab_bar_cost: measure::TabBar::default(),
            strips: crate::report::Strips::default(),
            buttons: measure::Buttons::default(),
            links: measure::Links::default(),
            desktop: crate::links::Desktop::System,
            control,
            reflow: measure::Reflow::default(),
            persistence: session::Persistence::at(None),
            restoring: None,
            reading_settings: measure::ReadingSettings::default(),
            chosen_font: measure::ChoosingTheFont::default(),
            last_seen: None,
            mirror_this_frame: std::time::Duration::ZERO,
            saving: measure::Saving::default(),
            // **No input at all, and whoever measures is the one who puts it there.** It used to be
            // one more parameter of this constructor, and the review threw it out with an argument
            // the tests themselves already proved: nobody but a batch sets it, and in exchange the
            // signature ended in two consecutive `Option`s filled with two bare `None`s in more
            // than thirty calls — two slots that do not say which is which.
            injected_drags: None,
            spent_injecting: std::time::Duration::ZERO,
            // **Painted unless whoever opens the window says otherwise**, which is what
            // [`crate::run`] does for a measurement batch: a constructor that decided it by reading
            // the environment would make every test depend on what the session that launched it
            // exported.
            headers: crate::header::Headers::PAINTED,
            chord_of_the_last_press: egui::Modifiers::NONE,
            where_the_last_press_fell: None,
            the_press_went_to_the_program: false,
            presses_for_the_program: Vec::new(),
            last_cell_reported: None,
            last_frame_with_buttons: None,
            link_press_in_progress: None,
            drag_in_progress: None,
            last_painted: Vec::new(),
            // **From the length of the list and not from the `panes` parameter, even though here
            // they are worth the same.** They are two sources for the same datum, and with two you
            // can take the wrong one: putting a zero or the wrong parameter makes the next tab that
            // opens **be called the same as one that already exists**, and the report publish two
            // blocks of numbers under the same label. Checked by mutating it, and no test catches
            // it: this constructor launches real consoles reading `SHELL`. Since restoring exists
            // **a test does catch it**: the one that opens with two saved tabs asserts both
            // counters. Deriving it from the list is what leaves nothing to confuse.
            next_name,
            next_pane_name,
            some_closed_by_hand: false,
            // Until the first frame there is no window to measure. A zero is what is really known,
            // and the report is only published once some console got to interpret something — that
            // is, after several frames.
            window_size: egui::Vec2::ZERO,
            requested_size: size.map(|how_much| RequestedSize {
                how_much,
                attempts: FRAMES_INSISTING_ON_SIZE,
            }),
            opened_by_agents: control::OpenedByAgents::default(),
        };

        // **The window is described once when it is born, and with that the deadline does not arm
        // itself.** Without this, the first frame's mirror differs from «nothing» and every session
        // would write the file half a second after opening, without the user having arranged
        // anything. The last thing **written** is left as nothing on purpose: what is on disk may
        // not be exactly this —a width below the floor comes back clipped—, so the first real
        // change rewrites the whole file and brings it up to date.
        window.last_seen = Some(window.describe());

        Ok(window)
    }
}

/// What a keyboard chord asks of the window, when what it asks is not the console's.
///
/// **It comes out through [`Pane::send_keys`]'s return value and not through an output parameter**,
/// because it is output only: that function's `&mut Modifiers` is a parameter because it **is also
/// input** —the chord can start on one frame and be used on the next—, and a request drags nothing
/// from the previous pass.
///
/// **It lives here and not in the crate's core** because applying one is what somebody asks a window
/// to do, which is this module's subject. It came out under **D28** ahead of the delivery that gives
/// an agent the same gestures over the channel, which is what pays the lines the core spends back.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum WindowRequest {
    /// Change tab, **as many as the number says**: negative backwards, positive forwards.
    ///
    /// **It carries a number and not a direction because clicking a card jumps wherever**: from the
    /// first to the third is two. Each key press, on the other hand, is worth exactly one — they do
    /// not compose with each other, they are applied in order, and why it is like that is in
    /// [`Pane::send_keys`].
    Go(isize),
    /// Open a new tab, with its two consoles, and leave it in front.
    NewTab,
    /// Close the tab being looked at. If it is the only one, what is closed is the window.
    CloseTab,
    /// Split in two the pane that has the focus, cutting in that direction.
    Split(Orientation),
    /// Close the pane that has the focus, and let its siblings share out its place.
    ///
    /// **It carries the suffix, and so do its two sisters above.** Since both things can be closed,
    /// a bare «close» next to a «close the pane» would be told apart only by what **one of the two
    /// does not say**, and getting it wrong kills the programs of a whole tab instead of one. For
    /// the same reason [`WindowRequest::NewTab`] says what it opens: if some day something that is
    /// not a tab can be opened, the name that did not say so would become the ambiguous one of the
    /// two.
    ClosePane,
}

impl Window {
    /// Does what the chord asked for, and **asks for a frame**.
    ///
    /// The second is not a precaution: today the only ones that ask for a repaint are the pending
    /// queue, the blind watcher's fallback and the panels' alarm clock. With two quiet tabs —two
    /// agents waiting for an answer, which is when this is used— none of the three has anything to
    /// ask for, so without this line the window would go to sleep **showing the previous tab** and
    /// the chord would look as though it had not worked. `whole_window` and `metrics` are where the
    /// geometry **splitting** needs comes from: how big the focused pane is has to be known in
    /// order to be able to refuse when it does not fit. They are passed instead of stored because
    /// the area belongs to the interface pass and this can run several times inside one —the same
    /// batch of events can bring several chords—, and **the bar appears or disappears along the
    /// way**: creating the second tab takes height away from the consoles at that very instant, so
    /// the area is recomputed one request at a time and not once for all of them.
    pub(crate) fn apply(
        &mut self,
        request: WindowRequest,
        ctx: &egui::Context,
        whole_window: egui::Rect,
        metrics: &CellMetrics,
    ) {
        match request {
            // The wrap-around comes from `rem_euclid`, which unlike the usual remainder **does not
            // return negatives**: from the first one backwards you reach the last, with no separate
            // branch for that case.
            WindowRequest::Go(delta) => {
                let tabs = self.tabs.len() as isize;
                self.active = (self.active as isize + delta).rem_euclid(tabs) as usize;
            }
            WindowRequest::NewTab => self.create_a_tab(ctx),
            WindowRequest::CloseTab => self.close_the_active_tab(ctx),
            WindowRequest::Split(cut_along) => {
                self.split_the_focused(cut_along, ctx, whole_window, metrics)
            }
            WindowRequest::ClosePane => self.close_the_focused(ctx),
        }

        ctx.request_repaint();
    }
}
