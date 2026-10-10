//! **The gestures the measurement bench injects into the window**, and the only thing here that
//! production ever calls is [`InjectedDrags::step`].
//!
//! It came out of `lib.rs` with **D28**, which is the entry that gave this crate a ceiling per
//! module. It is the first piece to leave because it is the easiest to argue: it is instrumentation
//! for the bench, it reaches into the window through one method, and nothing but the bench asks for
//! it. **Not into `measure.rs`, which is the module whose name matches it**: at the time that module
//! had no room under its ceiling, so the obvious home was ruled out before the move was made. **The
//! number this line used to state is deliberately gone** — `measure.rs` has since been split and the
//! figure was stale twice over, which is what a comment asserting a count always becomes.
//!
//! What it does not own is the *measuring*: the spans and the guards that read them stay in
//! `measure.rs`, and **publishing the report left for [`crate::report`]** with the delivery that took
//! it out of `lib.rs` — this line said `measure.rs` until then, and it was the design of that
//! delivery that spotted it rather than a reader. This is the half that pretends to be a hand.

use crate::{
    CELLS_THE_DIVIDER_GESTURE_CROSSES, CELLS_THE_GESTURE_MARKS, CELLS_THE_PANEL_GESTURE_CROSSES,
    DIVIDER_THICKNESS, FRAMES_DRAGGING, GESTURE_VARIABLE, MOVE_CHORD,
};
use eframe::egui;

mod links;
mod search;

/// How many drag gestures to inject, from what the variable said.
///
/// **Zero when there is no variable, and that is what makes this not exist for whoever uses the
/// application**: with no gestures asked for not a single event is injected and the new path does
/// not get to run. A value that is not understood is ignored just as if it were not there, as in
/// [`crate::how_many_tabs`] and [`crate::window_size`]: whoever measures finds out on reading the drags the
/// report declares.
///
/// **It receives the value and does not read it**, for the same reason as [`crate::how_many_tabs`]:
/// `std::env::set_var` has a contract over all the environment and this suite reads it plenty from
/// several threads.
pub(crate) fn how_many_drags(value: Option<&str>) -> usize {
    value.and_then(|value| value.parse().ok()).unwrap_or(0)
}

/// What the environment variable says, already resolved.
pub(crate) fn drags_at_startup() -> usize {
    how_many_drags(std::env::var(crate::DRAGS_VARIABLE).ok().as_deref())
}

/// The drag gestures the measurement batch does to itself. See [`crate::DRAGS_VARIABLE`].
///
/// **It alternates the side it drops by, and the reason is NOT geometry** — that was checked by
/// mutating it. With two consoles of the same weight, always dropping by the same side leaves
/// **exactly the same two rectangles**: removing one of two collapses the branch the other is left
/// in and inserting it again tiles it in halves once more. What alternating achieves is that the
/// gesture **does not always repeat the same movement**, which is what a real gesture does.
///
/// **And beware of what alternating does NOT achieve, which is what this comment claimed before the
/// review.** The gesture grabs **by slot** —slot zero and slot one— and not by name, and out of
/// that comes something that is not visible reading the code: **half the gestures end up dropping
/// the console onto itself and are discarded**. Probed with the instrumentation put on the drop
/// path, not reasoned:
///
/// | Gesture | Console it drags | Its slot | Pointer on dropping | Target |
/// |---|---|---|---|---|
/// | odd | the one in slot 0 | 0 | inside slot 1 | valid → reorders |
/// | even | **the same as before**, now in slot 1 | 1 | inside slot 1 | **itself → discarded** |
///
/// And it shows in the report of any batch: **1,750 gestures injected, 875 reordered**, exactly
/// half. Dropping a console onto itself is a legitimate case of the gesture —it is tested apart— so
/// the batch measures a mixture of the two, and that is why the report publishes **both numbers**:
/// without the one for the reorderings, a batch where none of them took would read the same as this
/// one.
///
/// **What geometry does decide is how many consoles there are**, and that is why the batch with
/// drags is of two: with more, inserting splits the target's weight and removing shares it out
/// among the ones that are left, so the weight concentrates gesture by gesture. Tied down by
/// `the_cycle_of_two_gestures_returns_to_the_starting_shape_only_with_two_consoles`.
pub(crate) struct InjectedDrags {
    /// The ones the batch asked for, so they can be declared next to the ones that were done.
    pub(crate) asked_for: usize,
    /// Which frame of the gesture in progress it is on: 0 presses, the next ones drag, the last one
    /// releases.
    pub(crate) step: usize,
    /// Gestures **injected whole**: the ones that got to send their release event. **How many are
    /// left is derived from here** instead of keeping its own count: the review caught that a
    /// `remaining` field was always worth `asked_for − completed` —the two writes were stuck
    /// together— and two sources of the same datum are an invariant somebody will break.
    pub(crate) completed: usize,
    /// How many times the gesture **actually changed something**: drops the tree accepted, or
    /// frames on which a drag moved a panel's width.
    ///
    /// **It can be greater than the injected ones with the panel gesture**, and it is not a
    /// miscount: a single drag crosses several cell boundaries and each crossing changes the width.
    /// That is why the report publishes the two figures **side by side and not as a fraction** —
    /// the first version printed «423 of 211», which is a number impossible to read properly.
    ///
    /// **It is not the same as the injected ones, and confusing them lets an empty batch through.**
    /// Sending the release event does not mean the layout changes: the geometric floor rejects the
    /// drop when the target console cannot give both of them their minimum, and then `drop_a_pane`
    /// returns `false`, says so on the error output and **the tree is not touched**. With a narrow
    /// window —`WINDOW_SIZE=500x400`, for instance— that happens on **every** drop of the batch:
    /// the run does not carry a single reordering inside, and counting only the injected ones the
    /// report would declare «1,720 done» and the script would pass it as good. The annex would then
    /// publish that the gesture costs nothing, which is exactly the wrongly labelled number the
    /// project's hard rule goes after.
    ///
    /// **A drop that leaves the order as it was counts too**, and that is the right thing: the tree
    /// is rebuilt all the same —remove, insert and redo the tiling— and that is the work the batch
    /// measures. What does not count is the one that **does not happen**.
    pub(crate) with_effect: usize,
    /// On how many **frames** the gesture left text marked. See [`crate::measure::Drags::marked`].
    pub(crate) selected: usize,
    /// How many mouse reports went out to the program. See [`crate::measure::Drags::reported`].
    pub(crate) reported: usize,
    /// Which gesture is injected. See [`InjectedGesture`].
    pub(crate) which: InjectedGesture,
    /// Where the previous pass drew the first button of **whichever pane painted last**, if it drew
    /// one.
    ///
    /// **It comes from the painting and not from geometry of its own**, for the reason the whole
    /// mechanism has: a button's place is decided by wrapping, by how many buttons there are and by
    /// what fits, so computing it here would be a second derivation that can disagree with the one
    /// on screen — and the batch would then press somewhere no button is and publish «pressing a
    /// button costs nothing». It is the previous frame's, exactly like the panel's border, because
    /// this hook runs before the interface pass.
    pub(crate) a_button_at: Option<egui::Pos2>,
    /// Painted points for plain, OSC 8 and panel links, in that order.
    pub(crate) links_at: [Option<egui::Pos2>; 3],
    /// The frame whose painting `links_at` comes from, so the next frame's starts it afresh.
    pub(crate) links_painted_in: Option<u64>,
    /// What a cell of the font measures, for the panel gesture: its travel is counted in cells.
    ///
    /// **And it arrives empty on purpose, because at startup it cannot be known.** egui **has no
    /// fonts until the first pass** —`Context::fonts_mut` panics with «No fonts available until
    /// first call to Context::run()»— and the first version of this asked for it in the window's
    /// constructor: **the batch died before opening**, and the first real run exposed it. The
    /// painting fills it in, which is where the metric exists, and until then the panel gesture
    /// sends nothing.
    pub(crate) cell: Option<f32>,
    /// Whether the gesture in progress goes one way or the other. For the console one, whether it
    /// drops by the target's right or by its left; for the panel one, whether it narrows or widens
    /// it.
    pub(crate) on_the_right: bool,
    /// Where the tree's divider was when this gesture grabbed it, and **nothing** outside a gesture
    /// in progress.
    ///
    /// **The travel is counted from here and not from where the divider is now, and that is what
    /// makes the there and back cancel out.** Asking the divider to place itself where the pointer
    /// is leaves it with its left edge there, that is, with its center half a thickness beyond:
    /// grabbing it by the center gives it that half thickness **in the same direction** wherever
    /// the gesture goes. Measured with the cycle test: four points per gesture, which over a run of
    /// a hundred gestures move the tiling from 50 % to 73 % and leave one console stuck to its
    /// minimum — two regimes under the same label.
    pub(crate) grabbed_at: Option<f32>,
    /// The last frame on which it advanced, and **nothing** while it has not advanced on any.
    ///
    /// **The raw input hook can run more than once on one frame number**, and this count is what
    /// holds the gesture to one step. eframe calls it before egui's passes, so a repeated pass never
    /// reaches it — but it also calls it on every logic-only tick while the window is hidden
    /// (eframe 0.36.1, `native/epi_integration.rs:323-362`), and those ticks do not advance
    /// `cumulative_frame_nr`. A second call on the same frame would send the «button pressed» of the
    /// same gesture twice, and the drag egui holds between frames would be left half done.
    pub(crate) last_frame: Option<u64>,
}

impl InjectedDrags {
    /// How many the batch asked for and of what kind, and nothing if it asked for none.
    pub(crate) fn new(how_many: usize, which: InjectedGesture) -> Option<Self> {
        (how_many > 0).then_some(Self {
            a_button_at: None,
            links_at: [None; 3],
            links_painted_in: None,
            asked_for: how_many,
            step: 0,
            completed: 0,
            with_effect: 0,
            selected: 0,
            reported: 0,
            which,
            cell: None,
            grabbed_at: None,
            on_the_right: true,
            last_frame: None,
        })
    }

    /// Notes that the gesture left text marked. See [`Self::selected`].
    pub(crate) fn one_more_selected(&mut self) {
        self.selected += 1;
    }

    /// Notes that a mouse report went out to the program. See [`Self::reported`].
    pub(crate) fn one_more_reported(&mut self) {
        self.reported += 1;
    }

    /// Notes that a gesture actually had an effect. See [`Self::with_effect`].
    pub(crate) fn one_more_with_effect(&mut self) {
        self.with_effect += 1;
    }

    /// Tells it how much a cell measures, from where the metric already exists. See [`Self::cell`].
    pub(crate) fn already_knows_the_cell(&mut self, cell: f32) {
        self.cell = Some(cell);
    }

    /// The events this frame is due, and it advances the gesture.
    ///
    /// **At least two consoles are needed**: with one there is nowhere to drop, and the gesture
    /// does not invent a target. The empty list is returned instead of skipping the step so that a
    /// badly asked batch —drags with a single pane— declares **zero gestures done** and the script
    /// can invalidate it, instead of publishing spans nobody would know carry no drag inside.
    pub(crate) fn events(
        &mut self,
        painted: &[(egui::Rect, Option<f32>)],
        frame_nr: u64,
    ) -> Vec<egui::Event> {
        if self.completed == self.asked_for || self.last_frame == Some(frame_nr) {
            return Vec::new();
        }
        match self.which {
            InjectedGesture::MovePane => self.move_a_pane(painted, frame_nr),
            InjectedGesture::ResizePanel => self.narrow_a_panel(painted, frame_nr),
            InjectedGesture::MoveDivider => self.move_a_divider(painted, frame_nr),
            InjectedGesture::Select | InjectedGesture::MouseToProgram => {
                self.select_a_stretch(painted, frame_nr)
            }
            InjectedGesture::PressAButton => self.press_a_button(frame_nr),
            InjectedGesture::ActivateLinks => self.activate_a_link(frame_nr),
            InjectedGesture::Search => search::search_the_focused(self, frame_nr),
        }
    }

    /// A bare press on a button, and its release on the next frame.
    ///
    /// **Two frames and not one**, so the press and the release do not arrive in the same batch: a
    /// window that saw both at once would still route the press, but the batch would then be
    /// measuring a shape no hand produces.
    ///
    /// **Nothing is emitted until the painting has recorded a button**, and the empty list is
    /// returned **without advancing the gesture** — so a run where nothing was seeded declares
    /// **zero gestures** and the script invalidates it, instead of publishing thousands of frames
    /// with nothing inside under this label. It is the same discipline as the panel's border.
    fn press_a_button(&mut self, frame_nr: u64) -> Vec<egui::Event> {
        let Some(at) = self.a_button_at else {
            return Vec::new();
        };
        self.last_frame = Some(frame_nr);
        let step = self.step;
        self.step += 1;

        // The approach on its own frame, for the reason the header gives about every other gesture:
        // a pointer that arrives and presses in the same batch is not what a hand does.
        if step == 0 {
            return vec![egui::Event::PointerMoved(at)];
        }
        let pressed = step == 1;
        if !pressed {
            self.step = 0;
            self.completed += 1;
        }
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    /// The gesture of marking text: grab inside the console and drag without stopping.
    ///
    /// **Only over the FIRST console, and the second is the control within the same run** — just
    /// like the batch of the panel's border. Both dump the same and only one receives the gesture,
    /// so the difference between their spans is what marking costs, measured against a control that
    /// shared machine, frame and minute.
    ///
    /// **Without a chord on purpose**: the primary button on its own inside a console is what
    /// **D20** defines as marking. One chord more and this batch would be measuring the gesture of
    /// moving the pane.
    ///
    /// **It alternates how far it reaches** so that the marked stretch grows and shrinks instead of
    /// staying still: a selection that does not change goes over nothing again, and the batch would
    /// be measuring the gesture **without its work**.
    pub(crate) fn select_a_stretch(
        &mut self,
        painted: &[(egui::Rect, Option<f32>)],
        frame_nr: u64,
    ) -> Vec<egui::Event> {
        let Some((pane_area, _)) = painted.first().copied() else {
            return Vec::new();
        };
        // With no cell yet there is no travel to compute: it is the first frame, before the metric
        // exists. Empty is returned **without advancing the gesture**, so as not to count one that
        // did not happen.
        let Some(cell) = self.cell else {
            return Vec::new();
        };
        self.last_frame = Some(frame_nr);

        // **It grabs inside the console and away from its edges.** The right-hand one is taken by
        // the panel's handle and the one beyond by the tree's divider: pressing near either of the
        // two would measure another gesture under this label.
        let mid_y = pane_area.center().y;
        let press_at = egui::pos2(pane_area.left() + cell, mid_y);
        let travel = CELLS_THE_GESTURE_MARKS * cell;
        let drag_to = if self.on_the_right {
            press_at.x + travel
        } else {
            press_at.x + travel / 2.0
        };

        let step = self.step;
        self.step += 1;
        // The approach goes apart from the press for the same reason as in the border one: the tie.
        if step == 0 {
            return vec![egui::Event::PointerMoved(press_at)];
        }
        if step == 1 {
            return vec![egui::Event::PointerButton {
                pos: press_at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }];
        }
        if step <= FRAMES_DRAGGING + 1 {
            return vec![egui::Event::PointerMoved(egui::pos2(drag_to, mid_y))];
        }

        self.step = 0;
        self.completed += 1;
        self.on_the_right = !self.on_the_right;
        vec![egui::Event::PointerButton {
            pos: egui::pos2(drag_to, mid_y),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    /// The gesture of the divider that tiles two consoles: grab it and take it one way and the
    /// other.
    ///
    /// **The divider is derived from the two rectangles, it is not tiled again.** The gap that is
    /// left between the right edge of one console and the left edge of the next **is** the divider:
    /// [`crate::tiling::tile`] builds it right there and with that thickness. Tiling a second time would be a copy
    /// of the computation that decides where each console is, and the two could disagree precisely
    /// in the batch that measures.
    ///
    /// **Only with two consoles side by side**, which is the batch's layout. With any other one the
    /// empty list is returned instead of inventing a divider: the batch declares **zero gestures**
    /// and the script can invalidate it, rather than publishing spans nobody would know go empty.
    ///
    /// **It alternates the side, and with that the layout returns to the starting one every two
    /// gestures.** A gesture that only pushed one way would leave a console at its minimum after a
    /// few gestures and would be measuring a different regime —one wide console and one on the
    /// floor— for almost the whole run.
    ///
    /// **The pointer is brought close on one frame and pressed on the NEXT, and it is not
    /// cosmetic.** It is the only one of the three gates where the grab point is **disputed**: the
    /// divider measures two points and both consoles touch it, so all three enter egui's grace
    /// radius and the one that wins the tie is the last registered — the consoles, which are
    /// attended to afterwards. With the approach and the press on the same frame, a console won the
    /// tie: the gesture **dragged it**, the drop replaced it, and the tiling went back to half and
    /// half. The batch would have published the F2.3 gesture under this one's label, which is the
    /// same wrongly labelled number this project goes after in the report. Probed, not reasoned: a
    /// human drag —which arrives, stops and presses— always wins, and the injected one only won the
    /// first of the run.
    pub(crate) fn move_a_divider(
        &mut self,
        painted: &[(egui::Rect, Option<f32>)],
        frame_nr: u64,
    ) -> Vec<egui::Event> {
        let (Some((left_pane, _)), Some((right_pane, _))) =
            (painted.first().copied(), painted.get(1).copied())
        else {
            return Vec::new();
        };
        // Side by side, which is the only thing this gesture knows how to grab. Split horizontally
        // the gap above does not exist and the point would fall inside a console, that is, the
        // gesture would drag **the console** and the batch would measure the neighboring gesture
        // under this label.
        if right_pane.left() < left_pane.right() {
            return Vec::new();
        }
        // With no cell yet there is no travel to compute: it is the first frame, before the metric
        // exists. Empty is returned **without advancing the gesture**, so as not to count one that
        // did not happen.
        let Some(cell) = self.cell else {
            return Vec::new();
        };
        self.last_frame = Some(frame_nr);

        // **The center of the line, which is the only point that wins the tie.** By its left edge
        // the tie goes to the console next to it —checked: the gesture moved nothing— because that
        // edge *is* its right edge and the one that wins a tie is the last registered.
        let center_x = (left_pane.right() + right_pane.left()) / 2.0;
        // Halfway up both of them, which is where the line is longest to grab.
        let mid_y = left_pane.center().y;
        // **A travel in cells and not in a fraction of the width**, for the same reason as in the
        // panel gesture: what has to be crossed are cell boundaries, because that is where the grid
        // of the two consoles really changes and where their text is reflowed.
        let travel = CELLS_THE_DIVIDER_GESTURE_CROSSES * cell;

        let step = self.step;
        self.step += 1;
        // The approach, alone. Why it goes on its own frame, in the header.
        if step == 0 {
            return vec![egui::Event::PointerMoved(egui::pos2(center_x, mid_y))];
        }
        if step == 1 {
            // Where this gesture comes from, noted **before** the press moves anything. See
            // [`Self::grabbed_at`].
            self.grabbed_at = Some(center_x);
            return vec![egui::Event::PointerButton {
                pos: egui::pos2(center_x, mid_y),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }];
        }
        // **Every drag frame aims at an absolute place counted from the origin**, and not at «where
        // it is now plus the travel». Both forms move the same per frame; the difference is that
        // this one does not accumulate the half thickness the divider takes every time it chases
        // the pointer, and that is why the way back lands exactly where the way out started.
        // **Without a fallback value on purpose.** The press of step 1 leaves it set and the drag
        // steps are only reached after it, so an `unwrap_or` here would protect from nothing: it
        // would cover a broken invariant instead of telling it, and the batch would publish a
        // travel measured from a place that is not the one that was grabbed.
        let grabbed_at = self
            .grabbed_at
            .expect("a drag only gets here after the press, and the press is what records where it starts from");
        let frames_so_far = (step - 1).min(FRAMES_DRAGGING) as f32;
        let walked = if self.on_the_right {
            frames_so_far * travel
        } else {
            -frames_so_far * travel
        };
        // And it aims half a thickness **before** the place it wants, because the divider plants
        // itself with its left edge where the pointer is: its center ends up half a thickness
        // beyond.
        let drag_to = grabbed_at + walked - DIVIDER_THICKNESS / 2.0;
        if step <= FRAMES_DRAGGING + 1 {
            return vec![egui::Event::PointerMoved(egui::pos2(drag_to, mid_y))];
        }

        self.step = 0;
        self.completed += 1;
        self.grabbed_at = None;
        self.on_the_right = !self.on_the_right;
        vec![egui::Event::PointerButton {
            pos: egui::pos2(drag_to, mid_y),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    /// The gesture of the panel's border: grab it and take it in and out, without stopping.
    ///
    /// **It alternates the side on purpose, and with that the width returns to where it was every
    /// two gestures.** A batch that only narrowed would end up with the panel at its minimum after
    /// a few gestures and would be measuring a different regime —a tiny panel, a wide console— for
    /// almost the whole run. Alternating, the layout that is measured is the starting one.
    ///
    /// **It never folds**, and that is deliberate: folding leaves the pane with no panel border, so
    /// the next gesture would have nowhere to grab by this route and the batch would stay still
    /// halfway.
    pub(crate) fn narrow_a_panel(
        &mut self,
        painted: &[(egui::Rect, Option<f32>)],
        frame_nr: u64,
    ) -> Vec<egui::Event> {
        // **A pane with the panel in view is needed.** Without it the empty list is returned
        // instead of skipping the step, so that the batch declares **zero gestures** and the script
        // can invalidate it — just as when console drags are asked for with a single pane.
        let Some((pane, divider)) = painted.first().copied() else {
            return Vec::new();
        };
        let Some(divider) = divider else {
            // **If the border disappears halfway through a gesture, it releases and closes.**
            // Without this the gesture stayed stopped forever with the button pressed —it never
            // sent the release, it never advanced the step— and the batch went on publishing
            // thousands of frames with no gesture inside under its label. The script only
            // invalidates when there was **none**, so a run that hangs after three gestures was
            // published as good. It happens as soon as the panel falls below the handle: a smaller
            // window, or four panes.
            if self.step == 0 {
                return Vec::new();
            }
            self.step = 0;
            self.completed += 1;
            self.on_the_right = !self.on_the_right;
            self.last_frame = Some(frame_nr);
            return vec![egui::Event::PointerButton {
                pos: egui::pos2(pane.right(), pane.center().y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }];
        };
        // With no cell yet there is no travel to compute: it is the first frame, before the metric
        // exists. Empty is returned **without advancing the gesture**, so as not to count one that
        // did not happen.
        let Some(cell) = self.cell else {
            return Vec::new();
        };
        self.last_frame = Some(frame_nr);

        // Halfway up, which is where the handle is widest to grab and where it competes with
        // nothing.
        let mid_y = pane.center().y;
        // **A travel in cells and not in a fraction of the width.** What the gesture has to cross
        // are cell boundaries, because that is where the panel's width really changes and where its
        // layout is redone; measured as a fraction, the same batch in another window would cross a
        // different number of them.
        let travel = CELLS_THE_PANEL_GESTURE_CROSSES * cell;
        let drag_to = if self.on_the_right {
            divider + travel
        } else {
            divider - travel
        };

        let step = self.step;
        self.step += 1;
        if step == 0 {
            return vec![
                egui::Event::PointerMoved(egui::pos2(divider, mid_y)),
                egui::Event::PointerButton {
                    pos: egui::pos2(divider, mid_y),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
        }
        if step <= FRAMES_DRAGGING {
            return vec![egui::Event::PointerMoved(egui::pos2(drag_to, mid_y))];
        }

        self.step = 0;
        self.completed += 1;
        self.on_the_right = !self.on_the_right;
        vec![egui::Event::PointerButton {
            pos: egui::pos2(drag_to, mid_y),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    pub(crate) fn move_a_pane(
        &mut self,
        painted: &[(egui::Rect, Option<f32>)],
        frame_nr: u64,
    ) -> Vec<egui::Event> {
        let (Some((grabbed_at, _)), Some((target, _))) =
            (painted.first().copied(), painted.get(1).copied())
        else {
            return Vec::new();
        };
        let (grabbed_at, target) = (&grabbed_at, &target);
        self.last_frame = Some(frame_nr);

        // **Near the edge and halfway up, so that the side the gesture picks is the horizontal
        // one.** The choice compares the distance to each edge **in proportion to the side it
        // belongs to**, so a point halfway up has the top and bottom edges half a screen of
        // proportional distance away and the side ones a tenth: it comes out left or right, never
        // top or bottom.
        let near_the_edge = if self.on_the_right {
            egui::pos2(target.right() - target.width() * 0.1, target.center().y)
        } else {
            egui::pos2(target.left() + target.width() * 0.1, target.center().y)
        };

        let step = self.step;
        self.step += 1;
        if step == 0 {
            // **It grabs by the center of the console, and that has to be watched since the panel
            // has a border of its own.** Its handle is at `36 · cell` from the pane's right edge,
            // so with the reference window and two panes it stays some sixteen points from the
            // center — plenty. But if the center fell inside the handle, the grab would go to the
            // panel: the console's drag would not start, no drop would reorder anything and **the
            // whole batch would be published as invalid without one line pointing at the panel**.
            // Whoever changes the factory width, the font size or the window size has to do this
            // sum again.
            return vec![
                egui::Event::PointerMoved(grabbed_at.center()),
                egui::Event::PointerButton {
                    pos: grabbed_at.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    // **With the moving chord, and without it this batch would measure something
                    // else.** Since **D20** the primary button on its own inside a console **marks
                    // text**: an injected gesture with no chord would go down the selection path
                    // and publish its cost under the label «moving consoles», with the report
                    // declaring gestures all the same.
                    modifiers: MOVE_CHORD,
                },
            ];
        }
        if step < FRAMES_DRAGGING {
            // **The middle movements stay INSIDE the origin, and not halfway.** Measuring a drag is
            // two states and not one: in flight **without** a valid target —over the console being
            // dragged, where there is no veil to paint— and over another one, where there is. At
            // the halfway point between two adjacent consoles the point already falls inside the
            // one next door, so the state without a veil would never be measured. 90 % of the width
            // leaves the point inside and far enough from the center to pass the threshold at which
            // egui takes a drag as started.
            //
            // **All the middle steps go to the same point, without interpolating.** The first
            // version spread them along the way, and that forced two guards —a `max(1)` against a
            // division by zero and a `min(1.0)` clip— for a case that **cannot happen**: the drag
            // frames are a constant of this file. The over-engineering audit threw them out, and
            // the still point is still a drag: what egui needs is for the pointer to have moved
            // from where it was pressed, not for it to move on every frame.
            return vec![egui::Event::PointerMoved(egui::pos2(
                grabbed_at.left() + grabbed_at.width() * 0.9,
                grabbed_at.center().y,
            ))];
        }
        if step == FRAMES_DRAGGING {
            // And the last movement already falls in the half of the target it is going to enter
            // by: it is the frame that paints the veil, and the one that leaves the pointer where
            // the drop of the next step expects it.
            return vec![egui::Event::PointerMoved(near_the_edge)];
        }

        self.step = 0;
        self.completed += 1;
        self.on_the_right = !self.on_the_right;
        vec![egui::Event::PointerButton {
            pos: near_the_edge,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }
}

/// Which gesture is injected when a batch asks for drags.
///
/// **The second one came with F2.4**, because there are two dividers that get dragged and they do
/// not cost the same: the one that tiles consoles —which reorders the tree on dropping— and the
/// panel's, which only changes the width of one pane and redoes its layout on crossing each cell.
///
/// **And the third with the scrollback**, because the two previous ones leave out the milestone's
/// worst case: the panel one changes the width of **one** console and the console one does not
/// cross cells while it flies. The tree's divider is the only one that changes the width of **two**
/// at once, and reflowing the text is paid per console. The later ones came with the milestones
/// their variants name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InjectedGesture {
    /// Grab a console and drop it onto another: the F2.3 gesture.
    MovePane,
    /// Grab the panel's border and take it in and out: the F2.4 one.
    ResizePanel,
    /// Grab the divider that tiles two consoles and take it one way and the other: the F2.2 one,
    /// and the only one that changes the width of both at once.
    MoveDivider,
    /// Mark text inside a console: the left button on its own, which is what **D20** decides.
    Select,
    /// **A bare press on a button of the first pane's panel.** The only injected gesture that does
    /// not travel: what it costs is the walk that decides which button a point reaches and the
    /// composing of what that button sends, and a travel would add a selection's cost under this
    /// label.
    PressAButton,
    /// Activate plain, OSC 8 and panel links in rotation without launching a browser.
    ActivateLinks,
    /// **The same travel as marking, over a console whose program asked for the mouse.** What
    /// changes is not the gesture but where it goes: instead of extending a selection, a report
    /// goes out for every cell it crosses. That is why it reuses the injector of [`Self::Select`]
    /// instead of having one of its own — a different travel would make the two batches impossible
    /// to compare, which is just what the gesture/control pair exists to allow.
    MouseToProgram,
    /// Open a search on the focused console, type a query and step through its matches (**D61**).
    /// The only gesture that types: every other one is the mouse's.
    Search,
}

impl InjectedGesture {
    /// What it is called in the report.
    ///
    /// **It goes to the report because the gestures do not cost the same**: the console one
    /// reorders the tree on dropping, the panel one redoes the layout of one on crossing each cell,
    /// and the one of the tree's divider reflows the text of two. Two batches of different kinds
    /// are not compared, and without this word in the header nobody would know which one they are
    /// reading.
    pub(crate) fn as_published(self) -> &'static str {
        match self {
            Self::MovePane => "moving consoles",
            Self::ResizePanel => "narrowing the panel",
            Self::MoveDivider => "moving the tree divider",
            Self::Select => "marking text",
            Self::MouseToProgram => "handing the mouse to the program",
            Self::PressAButton => "pressing a button on the panel",
            Self::ActivateLinks => "opening links",
            Self::Search => "searching the scrollback",
        }
    }
}

/// Which gesture the batch asks for, by the word the match below reads — `panel` for the panel's
/// border, `divider` for the one that tiles consoles, and one word for each of the others —
/// anything else the console one.
///
/// **What is not understood falls into the console one**, just like the other measurement
/// variables: aborting the startup over a badly written word would be worse, and the report
/// publishes which one was injected, so whoever measures finds out by comparing what they asked for
/// with what comes out.
pub(crate) fn gesture_of(value: Option<&str>) -> InjectedGesture {
    match value.map(str::trim) {
        Some("panel") => InjectedGesture::ResizePanel,
        Some("divider") => InjectedGesture::MoveDivider,
        Some("mark") => InjectedGesture::Select,
        Some("mouse") => InjectedGesture::MouseToProgram,
        Some("button") => InjectedGesture::PressAButton,
        Some("link") => InjectedGesture::ActivateLinks,
        Some("search") => InjectedGesture::Search,
        _ => InjectedGesture::MovePane,
    }
}

/// What the environment variable says, already resolved.
pub(crate) fn gesture_at_startup() -> InjectedGesture {
    gesture_of(std::env::var(GESTURE_VARIABLE).ok().as_deref())
}
