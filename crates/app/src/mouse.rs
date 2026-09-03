//! Which bytes each mouse gesture sends to the program.
//!
//! It is a pure function from a gesture to the bytes a terminal would send, sibling of [`keyboard`]. It
//! writes nowhere: whoever sends them is the loop, and where it sends them through is the bridge.
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

use eframe::egui::{Modifiers, PointerButton};
use term_core::MouseReporting;

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
