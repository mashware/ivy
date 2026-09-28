//! The mouse, from the frame's queue to its effect — **D23's table, made code**.
//!
//! **Whose each gesture is, and then the handing over.** What lives here reads the pointer queue,
//! works out who a click, a drag or a turn of the wheel belongs to — the window, the selection, or
//! the program running inside a console — and then carries that out: composing the bytes a program
//! is sent, extending a selection, dropping a pane where the veil showed it would land. **D23** is
//! the record that fixes the order of who keeps each gesture, and a decision worth a decision record
//! is worth a module.
//!
//! **The deciding and the doing are in one module, and the first cut of this file got that wrong.**
//! It kept only the router and left the six methods that act on its verdict behind in `lib.rs`,
//! under a header claiming «it decides; it does not do». An idiom audit took the claim apart: the
//! router itself reports to the program and seeds the report's counters, so the line was not one a
//! reader could reconstruct — and it showed in the only place that cannot be argued with. **With the
//! doers outside, eighteen items and fields of this file had to be crate-visible; with them inside,
//! two.** A seam that forces a module to open itself is the wrong seam, and the visibility surface
//! is how you measure that without taking anybody's word for it.
//!
//! **Two `impl` blocks and not one, because D28 caps a block at 300 production lines** and together
//! they are over four hundred. That is a real cost of the ceiling, said here rather than hidden: the
//! split is by decide-against-do, the same line this header draws, so a reader who knows which half
//! they want knows which block to open. Both blocks are walked and both are held.
//!
//! **What did not stay**: `WheelOwner` came here with the other gesture types and was never
//! mentioned again — the function that decides it is `Pane`'s, not the window's — so it went back
//! beside it. `pane_called` is a lookup by name with nothing pointer-shaped about it, and went back
//! beside `active_mut`. Both were found by the same audit. And **dragging a divider of the tree**
//! left for [`crate::tree`], where it is a method of the tab whose weights it shares out: the
//! delivery before this one had already sent the panel's border to the module that owns panels, and
//! a gesture over a piece of chrome belongs with that piece. What stays here is deciding **whose** a
//! gesture is, and the gestures that belong to no single piece.
//!
//! It came out of `lib.rs` under **D28**.

use crate::painter::CellMetrics;
use crate::tiling::{Orientation, Side, can_split, half, side_of};
use crate::tree::Node;
use crate::{Window, mouse};
use eframe::egui;
use std::time::{Duration, Instant, SystemTime};
use term_core::Cell;

pub(crate) mod links;

/// What the mouse's pass over the consoles leaves for the rest of the frame.
///
/// **Two things and not one because the second cannot be deduced from the first**: when somebody
/// drops, the tree changes and the target goes out on the same frame, so a `None` does not tell
/// «there is no drag» from «it has just been dropped».
pub(crate) struct MouseOutcome {
    /// Over which console —its slot in the tiling— and by which side it would be dropped, so that
    /// it can be painted.
    pub(crate) target: Option<(usize, Side)>,
    /// Whether the layout was reordered in this pass, and therefore the rectangles from before are
    /// no longer valid.
    pub(crate) the_tree_changed: bool,
}

/// What the wheel has asked for in a frame, **before converting it to whole movement**.
///
/// **Raw and not already in [`term_core::Scroll`], and that is where the defect the review found was.** A
/// trackpad sends **points**, and a brush of it is a few: divided by the height of a cell they give
/// **less than one line**. Converting right here to an integer, that fraction was truncated to
/// zero, the remainder was thrown away and the event was consumed all the same — that is,
/// **dragging two fingers slowly never moved the scrollback**, and turning fast up to one line per
/// frame was lost, always against the movement. Whoever looks at it has no way of noticing except
/// by trying to turn very hard.
///
/// It comes out of here raw so that [`crate::Pane::handle_wheel`] adds it up, which is the one that keeps
/// one frame's remainder for the next.
///
/// **Two named fields and not a pair of numbers**, for the same reason as the rest of this file:
/// two `f32` in a row get swapped in the call without the compiler saying anything, and here one is
/// worth a line and the other a whole screen.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub(crate) struct WheelInput {
    pub(crate) lines: f32,
    pub(crate) pages: f32,
}

/// What the drag that is in progress right now means.
///
/// **It is decided on pressing and is not re-read**, which is what **D20** calls the latch. Both
/// gestures come in through the same button and the only thing that separates them is the chord;
/// re-reading it on every frame would make releasing `Ctrl+Shift` halfway through a drag change
/// what the gesture means **with the button still pressed**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragIntent {
    /// Marking text: the left button on its own inside a console.
    Select,
    /// Taking the pane somewhere else: the **D15** gesture, now with `Ctrl+Shift`.
    MovePane,
    /// Telling the program inside, which has asked for it.
    ToTheProgram,
}

/// Whether the press this gesture was born from landed on its pane's strip.
///
/// **A type and not a `bool`, and the reason is the call it appears in.** Beside it travels «the
/// program asked for the mouse», also a yes-or-no: two adjacent booleans are swapped in a call
/// without the compiler saying a word, and swapping these two turns a press on the strip of a
/// console running `vim` into a mouse report — the console moves nowhere and the program receives a
/// click it should never have seen. It is the same move [`crate::header::Headers`] makes one file
/// away, and the one [`WheelInput`] made for its pair of numbers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct OnTheStrip(pub(crate) bool);

impl DragIntent {
    /// What pressing with that chord over a console whose program wants —or does not want— the
    /// mouse means.
    ///
    /// **The chord comes from the press event itself**, not from the frame's state: that one
    /// describes the **end of the batch of events** and not the instant of the click. It is the
    /// same source the pasting already drinks from, and the same trap that cost a case there.
    ///
    /// **The order of the three branches is the contract, not a writing convenience.** Moving the
    /// pane always wins: it belongs to the window and no program can take it away. Then, **«Shift»
    /// forces the selection** even if the program claims the mouse — without that exit, inside
    /// `vim` or `htop` there would be **no** way to get a line off the screen, and it is what every
    /// terminal does. Only then is it looked at whether the program asked for it.
    ///
    /// **The strip is a second door into the same branch, not a fourth intent.** Everything that
    /// hangs off `MovePane` — the identity the drag is held by, the half previewed before dropping,
    /// the tree transformed by value, D20's latch — is earned once; a door of its own would have to
    /// earn every one of them again. What the strip adds is that the gesture can be **discovered**:
    /// `Ctrl+Shift` is a chord nobody is told about, and a band you can grab says by itself that a
    /// console can be moved.
    ///
    /// **`on_the_strip` is where the button went down, never where the pointer is now.** egui gives
    /// a drag as started on the frame the pointer has already moved, which by then may be anywhere;
    /// reading the position there would move a pane whose strip was never pressed, and leave one
    /// whose strip was pressed marking text instead.
    fn from_the_press(
        chord: egui::Modifiers,
        on_the_strip: OnTheStrip,
        the_program_wants_it: bool,
    ) -> Self {
        if (chord.ctrl && chord.shift) || on_the_strip.0 {
            Self::MovePane
        } else if the_program_wants_it && !chord.shift {
            Self::ToTheProgram
        } else {
            Self::Select
        }
    }
}

/// Whether a press at that point fell on that pane's strip.
///
/// **`None` is not `false` in disguise**: a pane with no strip — a measured run — is a pane no press
/// can fall on the strip of, and a window that has not seen a press yet has no position to compare.
/// Both answer no, and neither one invents a point. Where the strip ends is
/// [`crate::panel::pane::PaneChrome::strip_holds`]' to say, not this one's.
///
/// **It is not a method of the window**, for the reason [`Window::collect_the_wheel`] gives one
/// screen below: it reads nothing from it, and a receiver promises a dependency that does not exist.
fn the_press_fell_on(
    chrome: Option<&crate::panel::pane::PaneChrome>,
    at: Option<egui::Pos2>,
) -> OnTheStrip {
    OnTheStrip(match (chrome, at) {
        (Some(chrome), Some(at)) => chrome.strip_holds(at),
        _ => false,
    })
}

/// A button that was pressed or released on this frame, with where and with which chord.
///
/// **They are collected from the queue instead of read from each console's response**, and it is
/// not a preference: egui's response says `clicked()` **on releasing**, so a program waiting to
/// know when it was pressed would know it late and would never know which button it was released
/// with. The queue brings the two instants separately, which is exactly what the report has to
/// count.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ButtonPress {
    at: egui::Pos2,
    button: egui::PointerButton,
    pressed: bool,
    chord: egui::Modifiers,
}

/// What the console takes away from having sent the turn to the program.
///
/// **Both are worth it and that is why they travel together**: the cost is noted in the mouse's
/// series, and whether the turn was a real report decides whether it counts in the number the
/// batches read.
pub(crate) struct WheelOutcome {
    pub(crate) cost: Duration,
    /// Whether what went out was a mouse report and not the arrow keys.
    pub(crate) was_a_report: bool,
}

impl Window {
    /// Attends to the mouse over the consoles: the click that gives focus, the bare drag that marks
    /// text, and the drag with `Ctrl+Shift` that moves them from one place to another (**D20**).
    ///
    /// It lives apart from the painting so it can be checked with a test `Ui`, without opening a
    /// window.
    ///
    /// **It returns where it would be dropped, instead of painting it here.** What is seen is the
    /// painting's business, and this function has to be able to run in a test that paints nothing.
    /// It is a **place in the tiling and not a name**, unlike the focus: it is not kept from one
    /// frame to the next —it is computed and painted in the same one— and what has to be painted is
    /// a rectangle, which is looked up by its place.
    ///
    /// **It keeps no state between frames, and that is not an oversight:** egui holds the drag
    /// itself and keeps the pointer's position **also on the frame in which it is released** — it
    /// does so on purpose, with `res.drag_stopped()` inside its own «is being interacted with»
    /// condition (`context.rs:1532`, with its comment). A field of our own holding the grabbed
    /// console would be a second copy of something that is already stored, and the two could
    /// disagree.
    pub(crate) fn handle_the_mouse(
        &mut self,
        ui: &egui::Ui,
        rects: &[egui::Rect],
        metrics: &CellMetrics,
    ) -> MouseOutcome {
        // **The stopwatch lives here and not in the caller**, unlike the one for walking the
        // tiling. The reason is that this way the property can be proved: with the stopwatch
        // outside, a test that calls this function does not touch the counter, and the only way to
        // check it would be to time the whole pass from outside — where this gesture costs the same
        // as the scheduler's noise and no test tells the two apart. Checked by mutating it: with
        // the stopwatch in the caller, removing it leaves the whole suite green.
        //
        // **And the work lives in another function on purpose.** With the body in here, moving the
        // first clock reading up a few lines —leaving out the tree walk, the allocations and the N
        // calls to `interact`— would publish almost the whole gesture as framework cost **without
        // any test falling**: a counter greater than zero is satisfied by any two consecutive
        // readings. By splitting it, the timed region is the whole call and shrinking it stops
        // being moving a line.
        let started = Instant::now();
        let of_the_mouse = self.handle_the_mouse_without_claiming_it(ui, rects, metrics);
        self.timed_separately += started.elapsed();
        of_the_mouse
    }

    /// The work of [`Self::handle_the_mouse`], without the stopwatch. It is not called from
    /// anywhere else.
    fn handle_the_mouse_without_claiming_it(
        &mut self,
        ui: &egui::Ui,
        rects: &[egui::Rect],
        metrics: &CellMetrics,
    ) -> MouseOutcome {
        // **A single vector, not two.** Walking the tree noting down names allocates once; asking
        // for `panes()` and mapping over it would allocate another, and this path is walked on
        // **every** frame, gesture or no gesture.
        let mut names = Vec::new();
        self.active().root.walk(&mut |pane| names.push(pane.name));

        // **One reading for every console rectangle this pass resolves**, for the reason
        // [`Pane::tile_with_its_panel`] gives: how much of a pane its console takes depends on
        // whether its panel is still there, and two readings a few microseconds apart can answer
        // that differently — which would resolve two gestures of the same frame against rectangles
        // 36 columns apart. It does not claim to be the only clock in the pass: answering a
        // standing question takes one of its own, and works from unrefreshed state on purpose.
        let now = SystemTime::now();
        // **Read once, beside the clock and for the same reason.** Both say how a pane's rectangle
        // is shared out, and a pass that resolved two gestures against two different answers would
        // measure them against rectangles a row apart.
        let headers = self.headers;

        let mut target = None;
        let mut the_tree_changed = false;

        // **The wheel goes to the console under the pointer, not to the focused one.** It is what
        // any terminal does, and the opposite comes as a surprise daily: you look at one console,
        // you turn the wheel, and another one would move.
        //
        // **It is collected just once and before the loop**: inside it would be read for each pane
        // and the scroll would be applied as many times as there are consoles. And it is
        // **consumed**, so that no later reader of the queue applies the same scroll again — ours,
        // or an egui that replayed a frame's events on its repeated pass, which 0.36 does not do.
        // Unlike the keyboard's keys, egui itself does not read it again (see `collect_the_wheel`).
        let wheel = Self::collect_the_wheel(ui, metrics);

        // **Which chord it was pressed with, from the event itself and not from the state.** See
        // [`DragIntent::from_the_press`] and [`Self::chord_of_the_last_press`]. It is looked at before
        // the loop because the answer belongs to the window, not to each console: only one of them
        // receives the press.
        if let Some((at, chord)) = Self::the_primary_press(ui) {
            // **Both are written from one reading of one event, in one statement.** Kept as two
            // separate reads, a frame could note the chord of this press beside the position of the
            // previous one, and the gesture would be decided from a point nobody pressed.
            self.chord_of_the_last_press = chord;
            self.where_the_last_press_fell = Some(at);
        }

        // **Presses and releases are routed here, before the loop**, and not inside with each
        // console's response: egui gives `clicked()` **on release**, so the program would find out
        // about the press late and never about which button was released. See [`ButtonPress`].
        //
        // **And just once per frame, not per pass.** egui 0.36 hands a frame's events to its first
        // pass only, so this count is defence: these events **cannot be consumed** —the press is
        // what egui needs to start the drag—, and an egui that replayed them would hand the program
        // **two presses where the user gave one**, a double click nobody made; the tripwire is
        // `egui_hands_a_frame_s_events_to_its_first_pass_only`. `cumulative_frame_nr` goes up once
        // per frame; `cumulative_pass_nr` goes up per pass and would filter nothing.
        // **A press aimed at a standing question is answered before the press is routed**, because
        // below it is handed to whichever program asked for the mouse: dismissing a notice must not
        // also send a button press into somebody's `vim`.
        //
        // **The queue is collected once and handed on already filtered**, and that shape is a
        // review's: passing «which press to skip» and re-deriving it by value meant comparing events
        // that are not identities — two presses at the same point in one queue matched each other —
        // and it collected the queue twice. What goes to the routing is the rest of it, releases
        // included: dropping the whole queue was the first shape of this and it left a program
        // believing a button was still down for ever, stealing the next release.
        let queue = Self::collect_the_buttons(ui);
        let queue = self.answer_a_clicked_question(ui, rects, metrics, queue);
        let link_was_released = links::has_primary(&queue, false);

        // **Which console the strip's button was pressed on is worked out here, under the same
        // once-per-frame guard as the routing below, and acted on after the walk.** The guard is not
        // shared for tidiness: the press is **not consumed** — egui needs it to decide who is being
        // dragged — so an egui that replayed it on a repeated pass (0.36 does not; the tripwire is
        // `egui_hands_a_frame_s_events_to_its_first_pass_only`) would not deliver a keystroke twice,
        // it would kill a **second console with one click**. With three consoles in a row and the
        // rightmost one's button pressed, a second reading finds the siblings refilled over the
        // whole area and the new rightmost strip's button on the same abscissa: the same press
        // closes that one too. Two panes hide it, as the survivor of the first close has no button.
        let frame_nr = ui.ctx().cumulative_frame_nr();
        self.a_link_gesture_ended(frame_nr);
        let mut asked_to_close = None;
        if self.last_frame_with_buttons != Some(frame_nr) {
            self.last_frame_with_buttons = Some(frame_nr);
            let queue = self.open_link_presses(metrics, frame_nr, queue);
            asked_to_close = self.whose_button_was_pressed(rects, metrics, now, &queue);
            self.route_the_buttons(rects, &names, metrics, queue, now);
        }
        // **Where the button went down, which is where a selection starts.** egui keeps it while
        // the button is still pressed, so it is read on whatever frame is needed.
        let press_origin = ui.ctx().input(|state| state.pointer.press_origin());

        // **The rectangles and the names are walked paired up, not indexing one with the other.**
        // They are two lists that come out of the same tree and always have the same length;
        // pairing them makes the mismatch impossible instead of defending against it with a guard
        // that in the application can never fire.
        for (which_pane, (rect, &name)) in rects.iter().zip(&names).enumerate() {
            // The gesture is detected with `interact` and not by looking at the pointer's position
            // bare, because that way it respects layers and painting order.
            //
            // **`Sense::CLICK | Sense::DRAG`, and not `Sense::click_and_drag()`**: the second
            // includes `FOCUSABLE`, that is, the widget **would ask for egui's keyboard focus** and
            // would compete with the routing the application does —which is geographic and not per
            // widget—. It is the same trap that already cost one case with the click and another
            // with the border, and here there is no constant that avoids it in one go:
            // `Sense::CLICK_AND_DRAG` does not exist.
            //
            // **The identifier carries the console's NAME, not its place**, and here is the
            // difference with the divider next door. egui holds the drag between frames by the
            // identifier, so with the place inside, anything that renumbers the consoles **while
            // the button is still pressed** makes the in-flight drag start pointing at another one:
            // changing tab (`Ctrl+Shift+←/→`), splitting or closing, all of which are applied at
            // the beginning of this very pass. Changing tab with a console grabbed got as far as
            // reordering **the destination tab**, which is exactly what this milestone leaves out.
            // With the name, the identifier travels with the console: if it disappears or changes
            // tab, nobody claims it and the drag dies by itself.
            let id = ui.id().with(("pane", name));
            let response = ui.interact(*rect, id, egui::Sense::CLICK | egui::Sense::DRAG);

            // **Grabbing gives focus too.** The gesture starts by pressing on the console, and not
            // giving it focus would leave the keyboard on one that is no longer the one being
            // touched. `clicked()` does not cover this case: when the pointer moves far enough,
            // egui decides it is a drag and the click never gets to exist.
            // **Except when the press was spent on that console's close button.** Pressing and
            // releasing inside one frame — a quick click, or any frame this window took a tenth of a
            // second over — makes egui report the click here as well, so the console about to leave
            // would take the keyboard on its way out and hand it to its neighbour when it goes. The
            // person typing in a third console loses their place to a gesture aimed at neither.
            let link_gesture = self.a_link_gesture_holds(name);
            if (response.clicked() || response.drag_started())
                && asked_to_close != Some(name)
                && !link_gesture
            {
                self.active_mut().focus = name;
            }

            // **A bare click throws the selection away, and it goes up here on purpose.** Further
            // down there is a guard that skips the console when it is not being dragged —it is
            // there so that no button other than the primary moves anything—, and a click **is not
            // a drag**: put after it, it would never run. Without this there is no way to stop
            // having something selected other than waiting for the program to clear the screen.
            if response.clicked()
                && !link_gesture
                && let Some(pane) = self.pane_called(name)
            {
                pane.screen.clear_selection();
            }

            // **Only the primary button moves consoles.** `dragged()` and `drag_stopped()` do not
            // look at which button it is —egui notes any press down as a candidate for dragging—,
            // and in a terminal that is not theoretical: the middle button is X11's paste and the
            // right one is the selection in almost all of them. A drag with either of the two would
            // swap two consoles' places without anyone asking for it.
            if !response.dragged_by(egui::PointerButton::Primary) && !response.drag_stopped() {
                continue;
            }

            // **How this pane's rectangle is shared out, resolved once for the whole gesture.**
            // The strip decides what the gesture means, and what is left of it is measured against
            // the console's rectangle — the one the grid was measured from, see
            // [`Pane::chrome_within`]. Moving the pane asks for neither: what is being moved is the
            // pane, rectangle and all.
            //
            // **It is an `Option` and not an early exit, and that is the whole point of the
            // binding.** Leaving here when the console is gone would also skip the release's own
            // reset at the foot of each branch, and the gesture would stay latched with the button
            // already up — a path nobody can reach today, which is exactly the kind that gets
            // written and then becomes reachable.
            let chrome = self
                .pane_called(name)
                .map(|pane| pane.chrome_within(*rect, metrics, now, headers));
            let console = chrome.as_ref().map(|chrome| chrome.console());

            // **The meaning of the gesture is fixed here, at the start, and is not looked at
            // again.** See [`DragIntent`]. With no chord in the queue —which is what happens when
            // egui takes the drag as started one frame after the press— the one for a bare button
            // is taken, which is selecting.
            // **Nothing here defends against the press that closed a console, and the design asked
            // for one.** Its reason was that egui cannot consume a pointer button event, so a press
            // on the close button would arm this latch as well — the button sits inside the strip,
            // which is the door into moving the pane. Measured instead of argued: egui does not
            // decide a press is a drag in the frame the press arrives in (it compares against the
            // origin it has only just recorded, which is what this file's own drag helper documents
            // having checked the hard way), and by the next frame the console has been closed at the
            // foot of this pass and nobody interacts with its widget again. A guard here fires in no
            // frame that exists; it was written, mutated away, and no test moved.
            if response.drag_started() {
                self.drag_in_progress = (!link_gesture).then(|| {
                    DragIntent::from_the_press(
                        self.chord_of_the_last_press,
                        the_press_fell_on(chrome.as_ref(), self.where_the_last_press_fell),
                        self.the_press_went_to_the_program,
                    )
                });
            }

            if self.drag_in_progress == Some(DragIntent::Select) {
                if let Some(console) = console {
                    self.select_while_dragging(&response, name, console, metrics, press_origin);
                }
                if response.drag_stopped() {
                    self.drag_in_progress = None;
                }
                continue;
            }

            if self.drag_in_progress == Some(DragIntent::ToTheProgram) {
                // **On the frame in which it is released no movement is reported, and that was a
                // defect.** The release is routed before this loop, so the program first received
                // the `MouseReport::Release` and **afterwards** a `MouseReport::Drag` of the same
                // cell: a movement with the button already up, which for anyone keeping mouse state
                // is a contradiction.
                if response.drag_stopped() {
                    self.drag_in_progress = None;
                } else if let Some(console) = console {
                    self.drag_for_the_program(&response, name, console, metrics);
                }
                continue;
            }

            // **Only a gesture that IS the move reaches the tree, and saying so is not a
            // formality.** This branch used to run for anything that was not selecting and was not
            // the program's — `MovePane` **and** «no intent at all». The second is reachable: a
            // press that starts outside the console's grid cancels the selection by clearing the
            // intent, and from the next frame on the gesture fell through to here and **moved the
            // pane**. It was found by mutating the strip's door: taking the door out left the test
            // for it green, because the press on the strip was cancelling a selection and then
            // being moved by this branch — with the chord nobody held.
            if self.drag_in_progress != Some(DragIntent::MovePane) {
                if response.drag_stopped() {
                    self.drag_in_progress = None;
                }
                continue;
            }

            // With no pointer there is no destination: the mouse has left the window. The gesture
            // ends up with no effect and the tree is not touched, which is what is expected of
            // cancelling halfway.
            let over = response.interact_pointer_pos().and_then(|pointer| {
                let at = rects
                    .iter()
                    .position(|rect| rect.contains(pointer))
                    .filter(|&at| at != which_pane)?;
                let side = side_of(rects[at], pointer);
                // **A destination where it would not fit is not a destination.** The veil says «if
                // you drop, it goes here», and without this check it would say so also where
                // dropping is going to be rejected: the user sees the half marked, drops, and
                // nothing happens — the warning only comes out on the error output, which nobody is
                // watching. It is the **same rule** dropping applies, asked earlier so as not to
                // promise what is not going to be kept.
                can_split(rects[at], side.cut_along(), metrics, self.headers).then_some((at, side))
            });

            // **The button is required to have been RELEASED, and the drag ending is not enough.**
            // egui aborts the drag when Escape is pressed and that also raises `drag_stopped()`,
            // with the button still pressed and the pointer's position intact: without this
            // distinction, having second thoughts halfway through the maneuver —grab, pass over
            // another one, press Escape— **completed the exchange** instead of cancelling it.
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                if let Some((at, side)) = over
                    && let Some(&other) = names.get(at)
                {
                    the_tree_changed |= self.drop_a_pane(name, other, side, rects[at], metrics);
                }
            } else if !response.drag_stopped() {
                target = over;
            }

            if response.drag_stopped() {
                self.drag_in_progress = None;
            }
        }

        // **After the loop, and not inside**: inside, `self` is borrowed by the tree walk.
        if let Some((pointer, requested, chord)) = wheel
            && let Some(at) = rects.iter().position(|rect| rect.contains(pointer))
            && let Some(&name) = names.get(at)
            && let Some(rect) = rects.get(at).copied()
            && let Some(pane) = self
                .active_mut()
                .panes_mut()
                .into_iter()
                .find(|pane| pane.name == name)
            // **The cell underneath the pointer, clamped and not rejected.** The turn can land on
            // the panel's strip, and there what the program expects to know is that the wheel
            // turned: losing it would leave the wheel dead in the console's right-hand columns.
            && let Some(to_the_program) = {
                let console = pane.console_within(rect, metrics, now, headers);
                let cell = pane.cell_to_extend_to(console, pointer, &metrics.clone());
                pane.handle_wheel(requested, chord, cell)
            }
        {
            // **The mouse's outbound leg is recorded in ITS console's measurement**, not in the
            // window's: the report is published per pane and a shared series would not say in which
            // one the gesture happened.
            pane.timings.record_mouse_outbound(to_the_program.cost);
            if to_the_program.was_a_report {
                self.one_more_reported(true);
            }
        }

        // **The console the button was pressed on is closed here, with the walk over and the wheel
        // already spent.** What forces the delay is not a borrow — the walk above transforms the
        // tree inline when somebody drops a console — but the rectangles: `rects` and the names
        // beside them were computed before this pass, and closing reabsorbs a pane's place among its
        // siblings, so from that instant every one of them describes a console that has moved.
        // Whatever still reads them in this pass has to read them whole.
        //
        // **By name, and not by moving the focus onto it and closing «the focused one».** That was
        // the first shape and the design named it; both are wrong for the same reason. A button is
        // pressed on the console it belongs to, which is very often **not** the one being typed in:
        // forcing the focus across first made the closing hand the keyboard to the neighbour of the
        // console that left, so pressing a third pane's button took the keyboard off the pane
        // somebody was working in. The by-name door moves the focus **only if the console that
        // leaves was holding it**. And the cost is not added here on purpose: this whole pass is
        // already inside [`Self::handle_the_mouse`]'s stopwatch, and the focused door adds its own —
        // which from in here would subtract the same microseconds from the frame twice (**D13**).
        //
        // Its refusal of a tab's only console cannot be reached from here — that console carries no
        // button — and it is reported rather than swallowed, like every other impossible branch in
        // this file.
        if let Some(name) = asked_to_close {
            if let Err(why) = self.close_a_pane_called(name, ui.ctx()) {
                eprintln!("the button could not close its console: {why}");
            }
            // **And the caller is told**, or it would paint this frame over the tiling of the
            // console that has just left — the same thing dropping one pane onto another already
            // says here.
            the_tree_changed = true;
        }

        self.a_link_gesture_was_released(link_was_released, frame_nr);

        MouseOutcome {
            target,
            the_tree_changed,
        }
    }

    /// Routes this frame's presses and releases among the consoles that received them.
    ///
    /// **It also notes down whether the press was for a program that asked for the mouse**, which
    /// is what the drag's latch needs one frame later. See [`Self::the_press_went_to_the_program`].
    fn route_the_buttons(
        &mut self,
        rects: &[egui::Rect],
        names: &[usize],
        metrics: &CellMetrics,
        queue: Vec<ButtonPress>,
        now: SystemTime,
    ) {
        let headers = self.headers;
        for button in queue {
            // **A release goes to the console that kept ITS press, not to the one underneath the
            // pointer.** Releasing over another console, over the panel's strip or outside the
            // window is what normally happens at the end of a drag; resolving it by the pointer,
            // the console that received the press is left believing the button pressed **for ever**
            // —`htop` keeps stretching its column— and the one next door receives a release for a
            // press it never saw.
            let of_a_live_press = (!button.pressed)
                .then(|| {
                    self.presses_for_the_program
                        .iter()
                        .position(|&(_, which)| which == button.button)
                })
                .flatten();
            if let Some(which) = of_a_live_press {
                let (name, _) = self.presses_for_the_program.remove(which);
                self.release_for_the_program(name, button, rects, names, metrics, now);
                continue;
            }

            let Some(at) = rects.iter().position(|rect| rect.contains(button.at)) else {
                continue;
            };
            let (Some(&name), Some(rect)) = (names.get(at), rects.get(at)) else {
                continue;
            };

            // A release that gets this far had no live press: its gesture was not the program's, or
            // it was already closed. It is not reported, which is what avoids the orphan release.
            if !button.pressed {
                continue;
            }

            // **The chord for moving the pane wins before anything else is looked at**: it belongs
            // to the window, and letting a program keep it would make it impossible to rearrange
            // consoles inside `vim`.
            // **The pane is shared out once for this press, and both questions below read that one
            // answer.** Asking twice is not only two walks: composing a panel's warning is inside
            // that call, so a press would lay out the same sentence twice — and, worse, «where does
            // the strip end» would be answered in two places a later delivery can move apart.
            let chrome = self
                .pane_called(name)
                .map(|pane| pane.chrome_within(*rect, metrics, now, headers));

            // **The strip is asked about here too, and through the same function.** A press on it
            // belongs to the window whatever the program below has claimed; with two functions
            // answering «whose is this press», the two would drift apart on the first delivery that
            // touched one of them.
            let on_the_strip = OnTheStrip(
                chrome
                    .as_ref()
                    .is_some_and(|chrome| chrome.strip_holds(button.at)),
            );
            let for_the_program =
                DragIntent::from_the_press(button.chord, on_the_strip, self.wants_the_mouse(name))
                    == DragIntent::ToTheProgram;

            // **The cell is resolved before anything is noted down, and that was a defect.** The
            // status panel's columns are inside the pane's rectangle but are not the console's: a
            // press there has no cell to report. Noting it down anyway, the drag that started over
            // the panel of a console whose program asks for the mouse sent that program a gesture
            // that **started outside it** as soon as the pointer came in.
            let over_the_grid =
                chrome
                    .as_ref()
                    .map(|chrome| chrome.console())
                    .and_then(|console| {
                        self.pane_called(name)
                            .and_then(|pane| pane.cell_under(console, button.at, metrics))
                    });
            let of_the_program = for_the_program && over_the_grid.is_some();

            if button.button == egui::PointerButton::Primary {
                self.the_press_went_to_the_program = of_the_program;
            }
            let Some(cell) = over_the_grid.filter(|_| of_the_program) else {
                continue;
            };

            self.presses_for_the_program.push((name, button.button));
            // **The drag's first report is seeded here.** Without this, the next frame reports
            // again the cell the press has just reported: one extra write to the pseudoterminal per
            // gesture, which is exactly what that field exists to avoid.
            self.last_cell_reported = Some((name, cell.row, cell.col));
            self.report_the_mouse(
                name,
                mouse::MouseReport::Press(button.button),
                cell,
                button.chord,
            );
        }
    }

    /// The presses and releases in this frame's queue.
    ///
    /// **It does not consume them**, just like [`Self::the_primary_press`]: the press is what egui
    /// needs in order to decide who is being dragged, and taking it away would leave the gesture
    /// never starting.
    fn collect_the_buttons(ui: &egui::Ui) -> Vec<ButtonPress> {
        ui.ctx().input(|state| {
            state
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers,
                    } => Some(ButtonPress {
                        at: *pos,
                        button: *button,
                        pressed: *pressed,
                        chord: *modifiers,
                    }),
                    _ => None,
                })
                .collect()
        })
    }

    /// Where the primary button was pressed on this frame and with which chord, if it was pressed
    /// at all.
    ///
    /// **From the event itself and not from the frame's state.** The state describes the **end of
    /// the batch of events**: if the press arrives in the same batch and afterwards the notice that
    /// `Ctrl` was released, reading it would give «no chord» for a press that did carry one. It is
    /// the same source the paste already drinks from, and the same trap that cost a case over
    /// there.
    ///
    /// **It does not consume the event**, unlike the wheel and the keyboard: the press is what egui
    /// needs in order to start the drag, and taking it away would leave the gesture never starting.
    /// Here it is only looked at.
    fn the_primary_press(ui: &egui::Ui) -> Option<(egui::Pos2, egui::Modifiers)> {
        ui.ctx().input(|state| {
            state.events.iter().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } => Some((*pos, *modifiers)),
                _ => None,
            })
        })
    }

    /// Collects what the wheel asks for on this frame, **raw**.
    ///
    /// **egui's three units, not two.** Besides lines and points there are **pages**, and letting
    /// that one fall into a wildcard would give a silently wrong jump — the kind that does not
    /// fail, it just misbehaves. They are returned separately instead of converting them all to
    /// lines because a page **is decided by the console's grid**, which here is not known yet.
    ///
    /// **As a fraction and not in whole lines**: the one that converts them is the pane, which is
    /// the one that keeps the remainder. See [`WheelInput`], where the defect that forced splitting
    /// it that way is.
    ///
    /// It also returns **where** the pointer was, which is what decides whose turn it is, and it
    /// returns something **even when the wheel asks for nothing**: the event exists and is consumed
    /// so that nothing reading the queue after it applies the same scroll again. egui itself is not
    /// what that stops — it turns the wheel into its scroll deltas at the start of the pass (egui
    /// 0.36.1, `input_state/mod.rs:417-469`), before this runs.
    ///
    /// **It is not a method of the window**: it reads nothing from it, and a receiver promises a
    /// dependency that does not exist.
    fn collect_the_wheel(
        ui: &egui::Ui,
        metrics: &CellMetrics,
    ) -> Option<(egui::Pos2, WheelInput, egui::Modifiers)> {
        ui.ctx().input_mut(|state| {
            let pointer = state.pointer.hover_pos()?;
            let mut requested = WheelInput::default();
            let mut there_was_one = false;
            // **The chord comes from the turn's own event**, for the same reason as the press's:
            // the frame's state describes the end of the batch of events, and here it decides who
            // keeps the wheel.
            let mut chord = egui::Modifiers::NONE;

            state.events.retain(|event| {
                let egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                // **The chord is the FIRST turn of the batch's, not the last one's.** The batch
                // adds up everything the wheel asked for in the frame, so keeping the last one let
                // releasing «Shift» halfway through a wheel stroke hand the program **the part that
                // was turned with the key held down too** — and that key is precisely the way out
                // that guarantees the scrollback can always be looked at. It is the same criterion
                // as the press: the gesture is fixed at the start.
                if !there_was_one {
                    chord = *modifiers;
                }
                there_was_one = true;
                match unit {
                    egui::MouseWheelUnit::Line => requested.lines += delta.y,
                    // The touchpad sends points: they are converted with the height of a cell,
                    // which is what a line measures on screen.
                    egui::MouseWheelUnit::Point => requested.lines += delta.y / metrics.height(),
                    egui::MouseWheelUnit::Page => requested.pages += delta.y,
                }
                false
            });

            there_was_one.then_some((pointer, requested, chord))
        })
    }
}

/// **The second block, and the reason it is a second block rather than a second file.**
///
/// Everything above decides whose a gesture is. Everything here is the handing over: composing the
/// bytes a program is sent, extending a selection, dropping a pane where the veil showed it would
/// land. **They belong in one module** — a reader following a click from the queue to its effect
/// should not change file halfway, and the visibility says so too: with the doers outside, nine
/// items and fields of this file had to be crate-visible for them, and with the doers inside
/// almost none does.
///
/// **Two blocks and not one because D28 caps an `impl` at 300 production lines** and together they
/// are over four hundred. That is a real cost of the ceiling and it is written here rather than
/// hidden: the split is by decide-against-do, which is the same line the module header draws, so a
/// reader who knows which half they want knows which block to open.
impl Window {
    // **These two live in this block and not in the one above, and the seam is the file's own**:
    // both of them *do* something — one throws a standing question away, the other writes a mark
    // the painter reads. It is also what keeps the first block under D28's 300 lines, which is the
    // cost this file's header already declares.

    /// Which console's close button this frame's bare press hit, if it hit one.
    ///
    /// **It notes the name and closes nothing**, which is the whole shape of this. The caller closes
    /// with the walk over, for the reason written at that call site: the rectangles of this pass
    /// describe a tiling that closing immediately makes false.
    ///
    /// **And it does not take the press out of the queue, unlike the doors below it.** The notices
    /// and the button under the question have to: their boxes are drawn **inside the console**,
    /// where the routing would go on to report the press to a program that asked for the mouse. This one lands on the strip, where
    /// the routing already declines it — `from_the_press` answers `MovePane` for anything on the
    /// strip, so the press is never the program's and the latch is set to «not the program's» by the
    /// routing itself. (The panel's buttons take theirs out for a different reason again: the press
    /// has already been spent writing to a program, and leaving it would let a second door spend it
    /// twice.) Removing it here would be a second mechanism for a case that has none, and the
    /// delivery that cut the strip had already recorded the premise this rests on.
    ///
    /// **Only a completely bare press**, the same rule [`Self::answer_a_clicked_question`] applies
    /// and for a stronger reason: the chord that moves a console is held down over the very strip
    /// this button lives in, so a press carrying it is somebody else's gesture — and the thing this
    /// one does cannot be undone.
    ///
    /// **The position comes from the queue's own event**, never from egui's latest pointer position:
    /// that one is where the pointer got to, and a frame that batches a press with a move after it —
    /// a flicked click, or any frame this window took a tenth of a second over — would close a
    /// console nobody aimed at. The module documents that trap twice already.
    ///
    /// **The strip is asked first and the button second**, both through the functions that already
    /// own those answers: a press on the line the strip and the console share belongs to the console
    /// ([`crate::panel::pane::PaneChrome::strip_holds`]), and where the button sits is
    /// [`crate::header::button_within`]'s to say — the same one the painting draws inside.
    fn whose_button_was_pressed(
        &self,
        rects: &[egui::Rect],
        metrics: &CellMetrics,
        now: SystemTime,
        queue: &[ButtonPress],
    ) -> Option<usize> {
        let bare = queue.iter().copied().find(|button| {
            button.pressed
                && button.button == egui::PointerButton::Primary
                && button.chord.is_none()
        })?;
        let headers = self.headers;
        // **One rectangle per console of this tab**, which is the question
        // [`crate::header::Button::in_a_tab_of`] answers: on a tab of one, no strip carries a
        // button at all and this walk cannot find one.
        let button = crate::header::Button::in_a_tab_of(rects);

        self.active()
            .panes()
            .into_iter()
            .zip(rects)
            .find_map(|(pane, rect)| {
                let chrome = pane.chrome_within(*rect, metrics, now, headers);
                let hit = chrome.strip_holds(bare.at)
                    && chrome.header().is_some_and(|strip| {
                        crate::header::button_within(strip, metrics, button)
                            .is_some_and(|target| target.holds(bare.at))
                    });
                hit.then_some(pane.name)
            })
    }

    /// Answers the standing question a press was aimed at, and says which press was spent on it.
    ///
    /// **Read off the frame's button queue and not off the pointer's state.** What egui calls the
    /// interact position is its *latest* one, which is not where the button went down: one batch carrying the
    /// press and a later move —a flicked click, or any frame that batches, and this window can
    /// freeze for a tenth of a second— would test the wrong point, answer a question nobody aimed
    /// at, and let the real press through to the program. The queue's own events carry the position
    /// they happened at. The module documents that trap twice already.
    ///
    /// **Only a completely bare press answers**, which is stricter than the conditions
    /// [`DragIntent::from_the_press`] decides a drag by, and deliberately so: a press carrying anything
    /// at all is somebody else's gesture and answering it would throw a held paste away in silence,
    /// which is the failure this feature exists to stop. The sharpest case is the chord that moves a
    /// console — it would move a pane *and* lose the paste — and the escape hatch that takes the
    /// selection back is the other one this must not eat, though that one moves nothing.
    ///
    /// It runs **before** the routing and returns the press it consumed, so the routing can leave
    /// that one out instead of dropping the whole queue.
    fn answer_a_clicked_question(
        &mut self,
        ui: &egui::Ui,
        rects: &[egui::Rect],
        metrics: &CellMetrics,
        queue: Vec<ButtonPress>,
    ) -> Vec<ButtonPress> {
        // **The press is found by its place in the queue, not by its value.** Two presses of the
        // same button at the same point in one batch are different events and comparing their
        // fields cannot tell them apart: the version that did threw both away, which leaves a
        // release with no press — the mirror of the defect the routing below guards against.
        let Some(which) = queue.iter().position(|button| {
            button.pressed
                && button.button == egui::PointerButton::Primary
                && button.chord.is_none()
        }) else {
            return queue;
        };
        let bare = queue[which];
        let theme = self.theme;
        let painter = ui.painter();
        let now = SystemTime::now();
        // Read before the walk borrows the panes, and for the reason the clock beside it is: the
        // notices are drawn inside the console, so the box a press is measured against has to be
        // cut the same way the painting cut it.
        let headers = self.headers;
        // **One walk over the panes, paired with their rectangles.** Asking `pane_called` inside the
        // loop allocates a vector of every pane once per pane, on every frame, and nothing declares
        // that cost — which the frame's subtraction would publish as the framework's.
        for (pane, rect) in self.active_mut().panes_mut().into_iter().zip(rects) {
            // **The panel's buttons are answered first, and they cost no geometry at all.** What the
            // painting recorded is where each button really ended up, in the window's own
            // coordinates, so this asks a question about a point instead of measuring a rectangle —
            // and it asks it of the panel that drew, which is what makes a press on pane A's strip
            // write into pane A's program wherever the keyboard happens to be.
            if let Some(action) = pane.panel_state.button_at(bare.at) {
                let bytes = crate::keyboard::what_a_button_sends(action.send());
                match pane.pty.write(&bytes) {
                    // **Counted only when the bytes went**, and the report's line says «pressed»
                    // meaning «reached a program». Counting a failed write would publish a number
                    // that is false in the direction that matters: a run where every write failed
                    // would validate and claim presses that reached nothing.
                    Ok(()) => {
                        self.buttons.pressed += 1;
                        // **For the bench, a press that landed IS this gesture's effect.** The
                        // generic counter means «the tree accepted a drop» or «a width changed», and
                        // a button press does neither — so without this the batch injects hundreds
                        // of gestures, every one of them arriving, and the script invalidates the
                        // run for having changed nothing.
                        if let Some(injected_drags) = self.injected_drags.as_mut() {
                            injected_drags.with_effect += 1;
                        }
                    }
                    // Reported and not recorded, like the three call sites this one joins: a send
                    // that failed is not a fast outbound trip and would skew the series that judges
                    // one.
                    Err(failure) => {
                        eprintln!("the button could not write to the program: {failure}");
                    }
                }
                let mut rest = queue;
                rest.remove(which);
                self.the_press_went_to_the_program = false;
                return rest;
            }

            // **A few reads and no allocation**, which is the shape this loop already had for one
            // notice: a console with nothing to say must not make the frame pay for the thing that
            // is almost never there.
            let silence = crate::painter::notices::what_the_silence_is(pane);
            if pane.held.is_none() && silence.is_none() {
                continue;
            }
            // **The console's rectangle, and this pass works it out without having refreshed the
            // panel — which a review raised and which is left as it is, on purpose.** On the one
            // frame a producer publishes a panel for the first time, the box measured here is the
            // one the *previous* frame drew: as wide as the pane, because there was no panel then
            // either. That is exactly what the person pressing could see, so the press is answered
            // where they aimed it. A guard of «the press must resolve to a cell of the console» was
            // tried here and removed: it reads the same unrefreshed state, so it would have said
            // yes in the very case it was written for — a guard whose comment promised more than it
            // did.
            let console = pane.console_within(*rect, metrics, now, headers);
            // **Every box in one pass**, because each is measured against where the one above it
            // ends and asking for them separately lays the question's text out twice per quiet pane
            // — work this pass would not be declaring (**D13**).
            let boxes = crate::painter::notices::the_boxes(painter, pane, &theme, metrics, console);
            let on = |target: Option<egui::Rect>| target.is_some_and(|it| it.contains(bare.at));
            // **The button under the question sends it**, composed against the mode the program is
            // in now (**D31**). Its place is decided by nothing that dismissing «stopped receiving»
            // changes, which is what keeps a second press on that sentence from landing on it.
            if on(boxes.send) {
                pane.send_the_held_paste();
            } else if on(boxes.question) {
                pane.held = None;
            // **The second notice, and a press on it discards nothing.** It takes the sentence off
            // the glass; how the reading ended is what the pseudoterminal recorded and stays
            // readable, and the console's last output goes back to being markable, which is what
            // somebody clicking there most likely wanted.
            } else if on(boxes.silence) {
                // **Only the reason on screen is dismissed**, and it is asked of the same function
                // the painting chose the sentence by. Dismissing both was the first shape and a
                // review refuted it: with the output ended *and* the queue refusing, only the
                // ended-output sentence is shown, and a press on it silenced a refusal nobody had
                // seen — the failure D31 exists against.
                match silence {
                    Some(crate::painter::notices::Silence::WentQuiet) => {
                        pane.pty_output.dismiss_the_silence(pane.pty.ending());
                    }
                    Some(crate::painter::notices::Silence::Refused) => {
                        pane.pty_output.dismiss_the_refusal(pane.pty.backlog());
                    }
                    None => {}
                }
            } else {
                continue;
            }
            let mut rest = queue;
            rest.remove(which);
            // **And that press is recorded as not having gone to a program as a mouse gesture** — the
            // button's paste did go, but as the person's input, not as a report. The latch this field
            // feeds decides, one frame later, what the drag beginning at this press means; leaving
            // the previous press's answer there made it decide by a gesture that is over.
            self.the_press_went_to_the_program = false;
            return rest;
        }
        queue
    }

    /// Tells the program that the button it saw go down was released.
    ///
    /// **The cell is clamped to the grid instead of being rejected**, unlike in the press and for
    /// the same reason the selection does it: releasing past the edge is the normal ending of a
    /// drag, and what the program needs to know is that the button went up — not to lose it because
    /// the pointer left.
    fn release_for_the_program(
        &mut self,
        name: usize,
        button: ButtonPress,
        rects: &[egui::Rect],
        names: &[usize],
        metrics: &CellMetrics,
        now: SystemTime,
    ) {
        let Some(its_index) = names.iter().position(|&which| which == name) else {
            // The console of the press is gone —closed halfway through the gesture—: there is
            // nobody to tell.
            return;
        };
        let Some(rect) = rects.get(its_index).copied() else {
            return;
        };
        let headers = self.headers;
        let Some(cell) = self.pane_called(name).map(|pane| {
            let console = pane.console_within(rect, metrics, now, headers);
            pane.clamped_cell(console, button.at, metrics)
        }) else {
            return;
        };
        self.last_cell_reported = None;
        self.report_the_mouse(
            name,
            mouse::MouseReport::Release(button.button),
            cell,
            button.chord,
        );
    }

    /// Sends the report and notes it down in that console's measurement and in the batch's.
    fn report_the_mouse(
        &mut self,
        name: usize,
        gesture: mouse::MouseReport,
        cell: Cell,
        chord: egui::Modifiers,
    ) {
        if let Some(pane) = self.pane_called(name)
            && let Some(cost) = pane.send_mouse_to_the_program(gesture, cell, chord)
        {
            pane.timings.record_mouse_outbound(cost);
            self.one_more_reported(matches!(gesture, mouse::MouseReport::Press(_)));
        }
    }

    /// Notes down in the batch's report that a report went out towards the program.
    ///
    /// **The effect is counted ONCE PER GESTURE, not per report**, and that distinction charged for
    /// a bad reading. The effects counter answers «how many of the injected gestures changed
    /// something», and in every other batch it comes out less than or equal to the gestures one;
    /// counting each report —one mouse gesture is the press, its movements and the release— the
    /// mouse batch published **three times as many effects as gestures** and its column stopped
    /// being comparable with the rest. It counts the press, which is what opens the gesture, and
    /// the wheel turn, which **is** a whole gesture in itself.
    fn one_more_reported(&mut self, counts_as_effect: bool) {
        if let Some(injected_drags) = self.injected_drags.as_mut() {
            injected_drags.one_more_reported();
            if counts_as_effect {
                injected_drags.one_more_with_effect();
            }
        }
    }

    /// Whether that console's program has asked for the mouse.
    fn wants_the_mouse(&mut self, name: usize) -> bool {
        self.pane_called(name)
            .is_some_and(|pane| pane.screen.mouse_reporting().wants_mouse())
    }

    /// Tells the program that the pointer is travelling over cells with the button pressed.
    ///
    /// **Only when the cell changes.** A slow drag sends dozens of frames over the same cell, and
    /// the repeated report tells the program nothing new: what it expects is to find out that it
    /// has moved on to another one.
    fn drag_for_the_program(
        &mut self,
        response: &egui::Response,
        name: usize,
        console: egui::Rect,
        metrics: &CellMetrics,
    ) {
        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };
        let button = egui::PointerButton::Primary;
        // **It is clamped to the grid instead of being rejected, just as the selection does.**
        // Dragging past the edge and carrying on is the usual gesture: with the strict variant,
        // taking the pointer out of the console left the program's drag **frozen** on the last cell
        // that did fall inside —an `htop` bar that stops following the hand— until it came back in.
        // `xterm` clamps.
        let Some(cell) = self
            .pane_called(name)
            .map(|pane| pane.clamped_cell(console, pointer, metrics))
        else {
            return;
        };
        if self.last_cell_reported == Some((name, cell.row, cell.col)) {
            return;
        }
        self.last_cell_reported = Some((name, cell.row, cell.col));
        // **The chord is the press's, not «none».** With a fixed `Modifiers::NONE`, a key held down
        // during the drag travelled in the press and in the release but **in none of the reports in
        // between**: for a program that tells a drag with a chord apart —block selection in an
        // editor— the key was released and pressed again by itself halfway through the gesture.
        let chord = self.chord_of_the_last_press;
        self.report_the_mouse(name, mouse::MouseReport::Drag(button), cell, chord);
    }

    /// Takes that console's selection to where the pointer is.
    ///
    /// **Starting and extending are told apart by `drag_started()` and not by whether there already
    /// was a selection.** With the second, dragging over a console that already had something
    /// selected would extend that old selection instead of starting a new one from where it was
    /// pressed.
    fn select_while_dragging(
        &mut self,
        response: &egui::Response,
        name: usize,
        console: egui::Rect,
        metrics: &CellMetrics,
        press_origin: Option<egui::Pos2>,
    ) {
        let just_started = response.drag_started();
        // **At the start what counts is WHERE IT WAS PRESSED, not where the pointer is.** egui
        // takes the drag as started on the frame in which the pointer **has already moved**, so
        // asking for it there returns the end of the first stretch: starting the selection with
        // that point opens and closes it in the same cell, and nothing is selected. Learned the
        // hard way — the selection test came out with nothing.
        let point = if just_started {
            press_origin
        } else {
            response.interact_pointer_pos()
        };
        let Some(pointer) = point else {
            return;
        };
        let Some(pane) = self.pane_called(name) else {
            return;
        };

        if just_started {
            // **Starting over the panel starts nothing, and with that the whole gesture is
            // cancelled.** The columns left over from the grid are its own and its handle's, and
            // whoever presses there is not pointing at text.
            //
            // **Cancelling the gesture —and not just skipping this frame— is what fixes the defect
            // the review found.** Without it, the following frames kept extending: the anchor of a
            // **previous** selection survived and its far end jumped to the pointer, so pressing on
            // the panel and dragging changed only what was selected and `Ctrl+Shift+C` copied a
            // text nobody had pointed at.
            match pane.cell_under(console, pointer, metrics) {
                Some(cell) => pane.screen.start_selection(cell),
                None => self.drag_in_progress = None,
            }
            return;
        }

        // Extending does **not** reject: dragging past the edge and carrying on selecting is the
        // usual gesture.
        let cell = pane.cell_to_extend_to(console, pointer, metrics);
        pane.screen.extend_selection(cell);
        let it_selected = pane.screen.has_selection();

        // **And it is noted down whether something really ended up selected, for a batch's
        // report.** See [`measure::Drags::marked`]: it is what tells «the gesture was injected»
        // apart from «the gesture selected», and what lets a batch for ANOTHER gesture invalidate
        // itself if its own ended up selecting.
        if it_selected && let Some(injected_drags) = self.injected_drags.as_mut() {
            injected_drags.one_more_selected();
            // **And it counts as an effect too, because for the selecting gesture THIS is its
            // effect.** The effects counter says «the gesture really changed something», and what
            // changes when selecting is what is selected. Without this the selecting batch would
            // publish **zero effects** —the reading that the gesture did nothing— and the script
            // would invalidate it, being right for the wrong reason.
            injected_drags.one_more_with_effect();
        }
    }

    /// Leaves one console beside another, if it fits.
    ///
    /// **The geometric floor is checked here and not inside the tree**, because this is where the
    /// rectangles are known. It is exactly what the splitting chord already does, and for the same
    /// reason: receiving a console raises the destination branch's children by one, just as
    /// splitting it does.
    pub(crate) fn drop_a_pane(
        &mut self,
        moved: usize,
        target: usize,
        side: Side,
        its_area: egui::Rect,
        metrics: &CellMetrics,
    ) -> bool {
        // **The same rule the gesture already used to decide whether to show the veil, and it is
        // not redundant.** There it serves not to promise; here it serves to make the operation
        // safe whoever calls it — the gesture is the only caller today, but the guard does not
        // depend on it staying that way.
        if !can_split(its_area, side.cut_along(), metrics, self.headers) {
            // It is said out loud for the same reason as when splitting: the gesture has finished
            // and nothing has happened, and without this line it reads as if the drag were broken.
            eprintln!(
                "it is not dropped there: the target console cannot give both of them their minimum. Make \
                 the window bigger or drop it somewhere else"
            );
            return false;
        }

        // **The tree is taken out, transformed and put back, whether it goes well or badly.** The
        // gap is filled with an empty branch in the meantime —the same placeholder `make_a_branch`
        // already uses, and for the same reason: *something* is needed there in order to take the
        // tree away without cloning it, and it lives for as long as these two lines take—. Both
        // arms of the `match` return a whole tree, which is what makes it impossible to end up
        // halfway.
        let placeholder = Node::Branch {
            orientation: Orientation::Columns,
            children: Vec::new(),
        };
        let tree = std::mem::replace(&mut self.active_mut().root, placeholder);
        let (tree, it_moved) = tree.move_pane(moved, target, side);
        self.active_mut().root = tree;

        // **The reordering is noted down here, which is where it is known to have happened**, and
        // not in whoever paints. What a batch's report has to declare is not how many gestures were
        // injected but how many **changed the layout**: sending the drop event does not mean the
        // tree accepts it, and a batch where none of them takes would measure the gesture just as
        // much as one with no gestures at all. See [`InjectedDrags::with_effect`].
        //
        // And it lives in this function and not in the painting because of what this project has on
        // record about tests: tying it where the painting found out, the only possible test called
        // the counter by hand and would not have caught production stopping calling it.
        //
        // **The `it_moved` here has no scenario today that fires it, and it stays on purpose.**
        // Checked by mutating it: removing it leaves the suite green, because the two rejections
        // come out before this line —the geometric floor's with its own `return` a few lines above,
        // and dropping onto itself filtered out by whoever attends to the mouse—, so everything
        // that reaches here has moved. What it protects is not a scenario: it is **the number the
        // annex publishes**, and publishing too much is exactly the defect this count comes to
        // correct. It is the same deal the project already accepted for picking the console back up
        // should inserting fail.
        if it_moved && let Some(injected_drags) = self.injected_drags.as_mut() {
            injected_drags.one_more_with_effect();
        }
        it_moved
    }

    /// Marks, while dragging, which console it would be dropped on.
    ///
    /// **It is painted on top of the consoles and not underneath**: what is underneath is whatever
    /// the program is writing, and a mark covered by a screenful of text marks nothing.
    ///
    /// **Nothing is painted on the grabbed one, and it is not an oversight**: grabbing gives it
    /// focus, so it already carries its highlight. Putting another border on it would be drawing
    /// the same thing twice with two different thicknesses.
    ///
    /// **It declares its cost from inside**, for the same reason as [`Self::handle_the_mouse`]:
    /// leaving it in the caller, no test catches its removal —checked by mutating it— because from
    /// outside this work costs the same as the noise between two clock readings.
    ///
    /// **It receives the rectangle and not the place in the list.** With the place it would have to
    /// be looked up, and with the lookup a guard in case it is not there — for a number that comes
    /// out of walking that very list.
    pub(crate) fn paint_the_drag(&mut self, ui: &egui::Ui, target: egui::Rect, side: Side) {
        let started = Instant::now();
        ui.painter()
            .rect_filled(half(target, side), 0.0, self.theme.drop_target());
        self.timed_separately += started.elapsed();
    }
}
