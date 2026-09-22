//! What a pane does with the wheel and with a mouse gesture, and which bytes each one sends.
//!
//! **Two halves, and the seam between them is the point.** [`encode`] and everything under it is a
//! pure function from a gesture to the bytes a terminal would send, sibling of [`keyboard`]: it
//! writes nowhere. The `impl Pane` block at the end is the other half — which cell a pointer falls
//! in, who keeps the gesture, what the scrollback does with it and what finally reaches the
//! program — and that one **does** write.
//! It came out of the window's core under **D28**, which gives a module a ceiling and records what
//! that file is at so the number can only go down.
//!
//! **This encoding does not come from the library.** The VT engine knows how to say whether the program
//! wants the mouse —those are its flags—, but assembling the report was the Alacritty binary's job, not
//! its crate's. So the table belongs to this project and that is why it is written here with its reasoning.
//!
//! **Where this milestone stops, and why.** The UTF-8 coordinate encoding is left out. The old one puts
//! each number in one byte with 32 added, so it **saturates at column 223**, and what `xterm` does
//! without that mode is exactly that: stop reporting. Writing that table without being able to see it
//! working would be guessing, and this project already declared that same cutoff for the function keys.
//! What is left out **returns silence, never an approximate byte**: wrong coordinates the program reads
//! as a click somewhere else, and that is worse than receiving nothing.
//!
//! [`keyboard`]: crate::keyboard

use crate::{Cell, Scroll, WheelInput, WheelOutcome, WheelOwner};
use eframe::egui;
use eframe::egui::{Modifiers, PointerButton};
use std::time::{Duration, Instant};
use term_core::{Half, MouseReporting};

use crate::painter::CellMetrics;

/// The highest column or row that the old encoding can report.
///
/// Each number travels in **one byte** with 32 added, so the largest that fits is `255 - 32`. Past that
/// point nothing is reported, which is what `xterm` does without the UTF-8 encoding.
const LEGACY_ENCODING_CEILING: u16 = 223;

/// What the mouse did, in the vocabulary of the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseReport {
    /// A button was pressed.
    Press(PointerButton),
    /// A button was released.
    Release(PointerButton),
    /// The pointer moved with a button held down.
    Drag(PointerButton),
    /// The wheel was turned toward the old.
    WheelUp,
    /// The wheel was turned toward the new.
    WheelDown,
}

/// The cell where the gesture happened, counted **from one** as the report requires.
///
/// It is built with [`from_zero`](CellPos::from_zero) so the conversion happens in a single place: the
/// rest of the program counts rows and columns from zero, and mixing the two origins leaves a report
/// shifted by one cell that nobody notices until a menu opens the entry next door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPos {
    column: u16,
    row: u16,
}

impl CellPos {
    /// The cell that whoever paints calls `(row, column)`, counting from zero.
    pub fn from_zero(row: u16, column: u16) -> Self {
        Self {
            column: column + 1,
            row: row + 1,
        }
    }
}

/// The bytes that correspond to that gesture, or nothing if there is nothing to send.
///
/// Returns nothing when the program **has not asked** for what was going to be reported —a drag to one
/// who only asked for the clicks— and when the coordinates do not fit in the encoding that applies.
pub fn encode(
    report: MouseReport,
    at: CellPos,
    reporting: MouseReporting,
    chord: Modifiers,
) -> Option<Vec<u8>> {
    if !wants_this(report, reporting) {
        return None;
    }
    let button = button_number(report)? + chord_bits(chord);
    if reporting.extended {
        return Some(extended_form(report, button, at));
    }
    legacy_form(report, button, at)
}

/// Whether the program has asked to be told about **this class** of gesture.
///
/// **Asking for the clicks is not asking for the drag**, and they are distinct flags precisely because a
/// program that only wants to know where a click lands does not want a report for every cell the pointer
/// travels with the button held down.
fn wants_this(report: MouseReport, reporting: MouseReporting) -> bool {
    match report {
        // **The wheel goes with the buttons.** A program that asks for the clicks receives the turns: it
        // is what makes the wheel move `htop`'s list without asking for anything else.
        MouseReport::Press(_)
        | MouseReport::Release(_)
        | MouseReport::WheelUp
        | MouseReport::WheelDown => reporting.wants_mouse(),
        MouseReport::Drag(_) => reporting.drag || reporting.motion,
    }
}

/// The number the button travels with, before adding the chord to it.
///
/// **Releasing travels as the button that was released only in the extended encoding.** The old one has
/// no way to say which one was released and uses a number of its own for «something was released», which
/// is the 3.
fn button_number(report: MouseReport) -> Option<u8> {
    let number_of = |button| match button {
        PointerButton::Primary => Some(0),
        PointerButton::Middle => Some(1),
        PointerButton::Secondary => Some(2),
        // The side buttons have no number in this encoding. Silence, not an approximate number: the
        // program would read a click that nobody made.
        _ => None,
    };
    match report {
        MouseReport::Press(button) | MouseReport::Release(button) => number_of(button),
        // **Dragging carries 32 added**: it is the «this is movement» bit, which is what tells apart
        // travelling a cell with the button held down from pressing it there again.
        MouseReport::Drag(button) => Some(number_of(button)? + 32),
        MouseReport::WheelUp => Some(64),
        MouseReport::WheelDown => Some(65),
    }
}

/// What the chord adds to the button's number.
///
/// **«Shift» does not appear, and it is on purpose**: in this window that key forces the selection,
/// so a gesture with it held down never reaches here. Putting it in anyway would be writing a dead branch
/// that on top of that suggests the opposite of what the window does.
fn chord_bits(chord: Modifiers) -> u8 {
    let mut bits = 0;
    if chord.alt {
        bits += 8;
    }
    if chord.ctrl {
        bits += 16;
    }
    bits
}

/// The report in the extended encoding: numbers as text and a letter that says whether it was pressed or released.
fn extended_form(report: MouseReport, button: u8, at: CellPos) -> Vec<u8> {
    let final_byte = if matches!(report, MouseReport::Release(_)) {
        'm'
    } else {
        'M'
    };
    format!("\x1b[<{button};{};{}{final_byte}", at.column, at.row).into_bytes()
}

/// The report in the old encoding: three bytes with 32 added to each number.
///
/// Returns nothing past column or row 223: see [`LEGACY_ENCODING_CEILING`].
fn legacy_form(report: MouseReport, button: u8, at: CellPos) -> Option<Vec<u8>> {
    if at.column > LEGACY_ENCODING_CEILING || at.row > LEGACY_ENCODING_CEILING {
        return None;
    }
    // **Releasing does not say which button was released**: this encoding has no room for it and uses 3
    // for «something was released». Whoever wants to know which one asks for the extended.
    let button = if matches!(report, MouseReport::Release(_)) {
        3 + (button & !0b11)
    } else {
        button
    };
    Some(vec![
        0x1b,
        b'[',
        b'M',
        32 + button,
        32 + at.column as u8,
        32 + at.row as u8,
    ])
}

/// What a pane does with the wheel and with a mouse gesture, before the bytes above are composed.
///
/// **The same vocabulary as the rest of this module**: who keeps the gesture, what the scrollback
/// does with it and what finally reaches the program. It came out of the window's core under
/// **D28**, which records what that file is at so the number can only go down.
impl crate::Pane {
    /// Which cell of this console that point falls in, **clamped to what the console really takes
    /// up**.
    ///
    /// **What arrives is the console's rectangle, not the pane's** ([`Self::console_within`]): the
    /// corner it counts from has to be the corner the grid was measured from, or the two disagree
    /// the moment anything is taken off the pane — a panel from the side, a header from the top.
    ///
    /// **And the clamping is still not a convenience: it is what stops anything being selected over
    /// the panel.** A cell divides the rectangle a whole number of times only by luck, so the
    /// leftover strip on the right belongs to no column and a point there is not pointing at text.
    ///
    /// Returns **nothing when the point falls outside the console's grid**, which is what whoever
    /// starts a selection has to respect; whoever extends one clamps, because dragging beyond the
    /// border is the normal gesture of selecting to the end.
    pub(crate) fn cell_under(
        &self,
        console: egui::Rect,
        point: egui::Pos2,
        metrics: &CellMetrics,
    ) -> Option<Cell> {
        let offset = point - console.min;
        let column = offset.x / metrics.width();
        let row = offset.y / metrics.height();
        (column >= 0.0
            && row >= 0.0
            && column < f32::from(self.measured.cols)
            && row < f32::from(self.measured.rows))
        .then(|| Cell {
            row: row as u16,
            col: column as u16,
            half: Half::from_fraction(column.fract()),
        })
    }

    /// The same arithmetic, but **rejecting nothing**: it is the one used by whoever is already
    /// dragging.
    ///
    /// **Dragging out of the console and going on marking is the age-old gesture**, so here a point
    /// from outside is not an error: it is somebody wanting to reach the end of the line.
    ///
    /// **And it does not clamp, because clamping here would change nothing.** The engine takes the
    /// point to its grid on its own when converting it into a range —read in its source, not
    /// assumed— and the cast to `u16` saturates instead of wrapping around. A clamp of our own
    /// would be a mechanism with no scenario: checked by removing it, no test changes its outcome.
    ///
    /// **What this function does decide is which HALF of the cell is being talked about, and there
    /// is a case there that cost a defect.** Past the right border the half has to be the **right**
    /// one: the engine **excludes** the last cell when the selection ends on its left, so giving it
    /// «left» lost **the last character of the line** — right in the gesture that exists to grab it
    /// whole.
    pub(crate) fn cell_to_extend_to(
        &self,
        console: egui::Rect,
        point: egui::Pos2,
        metrics: &CellMetrics,
    ) -> Cell {
        let offset = point - console.min;
        let last_column = f32::from(self.measured.cols.saturating_sub(1));
        let column = offset.x / metrics.width();
        let row = offset.y / metrics.height();
        let half = if column >= last_column {
            Half::Right
        } else {
            Half::from_fraction(column.fract())
        };
        Cell {
            row: row as u16,
            col: column as u16,
            half,
        }
    }

    /// The cell under the point, **taken to the grid** if the point went outside it.
    ///
    /// **Clamping here and not letting it through, the other way round from extending a
    /// selection.** There the point is handed to the engine, which takes it to its grid on its own;
    /// here what is done with the cell is **to tell the program about it**, and a column its grid
    /// does not have is an invented cell. It happens as soon as a drag ends outside the console
    /// where it began, which is the normal end of a gesture.
    pub(crate) fn clamped_cell(
        &self,
        console: egui::Rect,
        point: egui::Pos2,
        metrics: &CellMetrics,
    ) -> Cell {
        let released_at = self.cell_to_extend_to(console, point, metrics);
        Cell {
            row: released_at.row.min(self.measured.rows.saturating_sub(1)),
            col: released_at.col.min(self.measured.cols.saturating_sub(1)),
            half: released_at.half,
        }
    }

    /// Moves the scrollback by what the wheel has asked for, **keeping what does not add up to a
    /// line**.
    ///
    /// **The remainder accumulates from one frame to the next, and that is where the defect was.**
    /// A touchpad sends points: every brush is worth a fraction of a line, and truncating without
    /// keeping the remainder left the scrollback **still forever** with two fingers dragging
    /// slowly. With a notched wheel it showed less —every notch is a whole line— but it lost the
    /// fraction all the same, always against the movement.
    ///
    /// **It truncates towards zero and the remainder keeps its sign**, so changing direction
    /// halfway does not drag debt from the previous direction: the fraction that was left is spent
    /// against the new movement, which is what the hand does.
    pub(crate) fn handle_wheel(
        &mut self,
        requested: WheelInput,
        chord: egui::Modifiers,
        at: Cell,
    ) -> Option<WheelOutcome> {
        self.pending_wheel.lines += requested.lines;
        self.pending_wheel.pages += requested.pages;

        let lines = self.pending_wheel.lines.trunc();
        let pages = self.pending_wheel.pages.trunc();
        self.pending_wheel.lines -= lines;
        self.pending_wheel.pages -= pages;

        match self.who_takes_the_wheel(chord) {
            WheelOwner::Scrollback => {
                // **Only what moves something.** A `Lines(0)` is not harmless for free: it is an
                // order to the engine that does nothing, and publishing it would make whoever reads
                // the code believe the frame scrolled.
                if lines != 0.0 {
                    self.screen.scroll_by(Scroll::Lines(lines as i32));
                }
                if pages != 0.0 {
                    self.screen.scroll_by(Scroll::Pages(pages as i32));
                }
                None
            }
            owner => {
                // **Pages are turned into lines before going out, and are not sent separately.** A
                // program does not understand «a page»: what it expects is as many turns as lines,
                // and the page is measured by the grid it has in front of it. Without this
                // arithmetic, a touchpad that sends pages would leave `less` still.
                let rows = f32::from(self.screen.size().rows);
                let how_many = (lines + pages * rows) as i32;
                (how_many != 0)
                    .then(|| self.send_wheel(owner, how_many, at))
                    .flatten()
            }
        }
    }

    /// Who gets the turn, in the order **D23** sets.
    ///
    /// **«Shift» first and always.** It is the way out that guarantees going up the scrollback can
    /// be done in any console, whatever the program inside is watching; without it, inside a pager
    /// there would be no way to see what went off the top **of the console**, which is a different
    /// thing from what the pager shows.
    fn who_takes_the_wheel(&self, chord: egui::Modifiers) -> WheelOwner {
        if chord.shift {
            WheelOwner::Scrollback
        } else if self.screen.mouse_reporting().wants_mouse() {
            WheelOwner::Program
        } else if self.screen.wheel_acts_as_arrows() {
            WheelOwner::ArrowKeys
        } else {
            WheelOwner::Scrollback
        }
    }

    /// Sends the program as many turns as lines, and returns what it cost to reach the
    /// pseudoterminal.
    ///
    /// **One send and not one per line.** The bytes are put together and go out at once: the
    /// pseudoterminal is one system call per write, and with a touchpad a single gesture brings
    /// dozens of lines.
    fn send_wheel(&mut self, owner: WheelOwner, how_many: i32, at: Cell) -> Option<WheelOutcome> {
        let started = Instant::now();
        let to_the_top = how_many > 0;
        let mut bytes = Vec::new();
        for _ in 0..how_many.unsigned_abs() {
            match owner {
                WheelOwner::ArrowKeys => {
                    // **The same table as the keys, and it inherits its declared cut**: application
                    // mode —the one where the arrows change shape— is left out of the milestone,
                    // and that is written in the keyboard module. Writing it here in another way
                    // would leave two tables contradicting each other.
                    bytes.extend_from_slice(if to_the_top { b"\x1b[A" } else { b"\x1b[B" });
                }
                _ => {
                    let gesture = if to_the_top {
                        MouseReport::WheelUp
                    } else {
                        MouseReport::WheelDown
                    };
                    // **The turn is reported on the cell under the pointer, not on the corner.** A
                    // program with two scrolling areas —`tmux` with two panes, `vim` with two
                    // windows— decides **which** one moves by the cell the report brings: with 1;1
                    // fixed, the wheel always moved the top-left one, wherever the pointer was
                    // looking.
                    if let Some(report) = encode(
                        gesture,
                        CellPos::from_zero(at.row, at.col),
                        self.screen.mouse_reporting(),
                        egui::Modifiers::NONE,
                    ) {
                        bytes.extend_from_slice(&report);
                    }
                }
            }
        }
        if bytes.is_empty() {
            return None;
        }
        if let Err(failure) = self.pty.write(&bytes) {
            eprintln!("the wheel turn could not be sent to the program: {failure}");
            // **A send that failed is not a fast outbound trip**, just as on the keyboard's path
            // and on the click's: recording it would measure the failure and publish it as the
            // gesture's normal cost.
            return None;
        }
        // The person turned the wheel into the program, so the console is theirs again.
        self.hand_it_back_to_the_person();
        Some(WheelOutcome {
            cost: started.elapsed(),
            // **Arrow keys are not a mouse report, and counting them as one invalidated good
            // batches.** What goes out along that path are the same keys the keyboard sends;
            // publishing them under «N mouse reports» put foreign samples into that series and
            // tripped the guard that demands zero reports in every batch that is not the mouse one
            // — with a `less` scrolling, a correct run was marked invalid.
            was_a_report: owner == WheelOwner::Program,
        })
    }

    /// Sends the program what the mouse has just done over this console.
    ///
    /// Returns what it cost to **hand the gesture over**, or nothing if there was nothing to send —
    /// which is the normal case: the program only receives what it has asked for.
    ///
    /// **It used to be «to reach the pseudoterminal», and it is not any more.** Since the write is
    /// handed to a thread of its own, this stopwatch stops at the queue: the byte reaches the
    /// program afterwards, on a thread nobody times. D23 published this span as 5.1–5.3 µs per
    /// gesture when it still included the write; what it covers now is a shorter stretch under the
    /// same name.
    pub(crate) fn send_mouse_to_the_program(
        &mut self,
        gesture: MouseReport,
        cell: Cell,
        chord: egui::Modifiers,
    ) -> Option<Duration> {
        let started = Instant::now();
        let bytes = encode(
            gesture,
            CellPos::from_zero(cell.row, cell.col),
            self.screen.mouse_reporting(),
            chord,
        )?;
        if let Err(failure) = self.pty.write(&bytes) {
            // A gesture lost in silence inside an editor reads as the window being hung, just like
            // a lost key.
            eprintln!("the mouse could not be sent to the program: {failure}");
            // A send that failed is not a fast outbound trip: it is not recorded, or the number
            // would measure the failure.
            return None;
        }
        // A mouse report is the person's input reaching the console, so the console is theirs again.
        self.hand_it_back_to_the_person();
        Some(started.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program that asked for the clicks and the drag, in the old encoding.
    fn like_htop() -> MouseReporting {
        MouseReporting {
            buttons: true,
            drag: true,
            ..MouseReporting::default()
        }
    }

    /// The same, but asking for the extended encoding.
    fn with_the_extended() -> MouseReporting {
        MouseReporting {
            extended: true,
            ..like_htop()
        }
    }

    #[test]
    fn a_program_that_does_not_ask_for_the_mouse_receives_nothing() {
        // The everyday console. Without this, every click inside a shell would put garbage on the
        // command line.
        assert_eq!(
            encode(
                MouseReport::Press(PointerButton::Primary),
                CellPos::from_zero(0, 0),
                MouseReporting::default(),
                Modifiers::NONE,
            ),
            None
        );
    }

    #[test]
    fn the_report_cell_starts_at_one() {
        // **The top-left corner is 1;1 and not 0;0.** Whoever paints counts from zero, the report from
        // one, and mixing the two origins leaves everything shifted by one cell: a menu opens the entry
        // next door and a bar is grabbed by the wrong spot.
        let bytes = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        )
        .expect("the program asked for the clicks");

        assert_eq!(bytes, b"\x1b[<0;1;1M");
    }

    #[test]
    fn releasing_and_pressing_are_told_apart_in_the_extended_encoding() {
        // **The final letter is the only thing that separates them**, and it is what an editor uses to
        // know you finished dragging the selection. Always sending the same one leaves the gesture open
        // forever from the program's point of view.
        let press = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(4, 9),
            with_the_extended(),
            Modifiers::NONE,
        );
        let release = encode(
            MouseReport::Release(PointerButton::Primary),
            CellPos::from_zero(4, 9),
            with_the_extended(),
            Modifiers::NONE,
        );

        assert_eq!(press.as_deref(), Some(&b"\x1b[<0;10;5M"[..]));
        assert_eq!(release.as_deref(), Some(&b"\x1b[<0;10;5m"[..]));
    }

    #[test]
    fn the_old_encoding_does_not_say_which_button_was_released() {
        // It has no room to say it: it uses a number of its own for «something was released». Sending the
        // button's would make the program read a new click of the middle button right as the right one is
        // released.
        let bytes = encode(
            MouseReport::Release(PointerButton::Secondary),
            CellPos::from_zero(0, 0),
            like_htop(),
            Modifiers::NONE,
        )
        .expect("the program asked for the clicks");

        assert_eq!(bytes, vec![0x1b, b'[', b'M', 32 + 3, 33, 33]);
    }

    #[test]
    fn the_old_encoding_goes_silent_past_column_223() {
        // **Silence, not an approximate byte.** Each number travels in one byte with 32 added, so 224
        // does not fit: saturating it at 223 would tell the program a click landed somewhere it did not,
        // and starting over from 0 would be worse still.
        let inside = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 222),
            like_htop(),
            Modifiers::NONE,
        );
        let outside = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 223),
            like_htop(),
            Modifiers::NONE,
        );

        assert!(inside.is_some(), "column 223 does fit and it went silent");
        assert_eq!(outside, None, "column 224 does not fit and was sent anyway");
    }

    #[test]
    fn the_extended_encoding_does_not_have_that_ceiling() {
        // It is the whole reason the extended one exists: its numbers travel as text, so a truly wide
        // window keeps reporting where the old one no longer can.
        let bytes = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 399),
            with_the_extended(),
            Modifiers::NONE,
        )
        .expect("the extended one does not saturate");

        assert_eq!(bytes, b"\x1b[<0;400;1M");
    }

    #[test]
    fn asking_for_the_clicks_is_not_asking_for_the_drag() {
        // **They are distinct flags because they mean distinct things.** A program that only wants to
        // know where a click lands would otherwise receive a report for every cell the pointer travels
        // with the button held down — dozens per gesture, and none of them asked for.
        let clicks_only = MouseReporting {
            buttons: true,
            ..MouseReporting::default()
        };

        assert_eq!(
            encode(
                MouseReport::Drag(PointerButton::Primary),
                CellPos::from_zero(1, 1),
                clicks_only,
                Modifiers::NONE,
            ),
            None
        );
        assert!(
            encode(
                MouseReport::Press(PointerButton::Primary),
                CellPos::from_zero(1, 1),
                clicks_only,
                Modifiers::NONE,
            )
            .is_some(),
            "asked for the clicks and did not receive its own"
        );
    }

    #[test]
    fn dragging_carries_the_movement_bit() {
        // The 32 added are what tells apart «I am travelling cells with the button held down» from «I
        // have pressed here again». Without them, dragging an `htop` bar reads as a burst of clicks.
        let bytes = encode(
            MouseReport::Drag(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        )
        .expect("asked for the drag");

        assert_eq!(bytes, b"\x1b[<32;1;1M");
    }

    #[test]
    fn the_wheel_travels_with_its_two_numbers() {
        // 64 and 65, and they are what makes `htop`'s list scroll up and down. Swapping them leaves the
        // wheel running backwards, a defect that shows daily and breaks nothing else.
        let wheel_up = encode(
            MouseReport::WheelUp,
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        );
        let wheel_down = encode(
            MouseReport::WheelDown,
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        );

        assert_eq!(wheel_up.as_deref(), Some(&b"\x1b[<64;1;1M"[..]));
        assert_eq!(wheel_down.as_deref(), Some(&b"\x1b[<65;1;1M"[..]));
    }

    #[test]
    fn the_chord_travels_added_to_the_button() {
        // `Ctrl` adds 16 and `Alt` 8, which is how the program tells a normal click from one with a chord
        // —open in another tab, for instance—. **«Shift» does not appear on purpose**: in this
        // window that key forces the selection, so a gesture with it does not reach here.
        let with_ctrl = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::CTRL,
        );
        let with_alt = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::ALT,
        );

        assert_eq!(with_ctrl.as_deref(), Some(&b"\x1b[<16;1;1M"[..]));
        assert_eq!(with_alt.as_deref(), Some(&b"\x1b[<8;1;1M"[..]));
    }

    #[test]
    fn the_side_buttons_send_nothing() {
        // They have no number in this encoding. Giving them a borrowed one would be telling the program a
        // click of a button that nobody touched.
        assert_eq!(
            encode(
                MouseReport::Press(PointerButton::Extra1),
                CellPos::from_zero(0, 0),
                with_the_extended(),
                Modifiers::NONE,
            ),
            None
        );
    }
}
