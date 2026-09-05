//! Which bytes each key sends to the program.
//!
//! It is a pure function from a window event to the bytes a terminal would send. It writes
//! nowhere: the one that sends them is the loop, and where it sends them through is the bridge.
//!
//! **Where this milestone stops, and why.** Alt, the function keys and the numeric
//! keypad are left out. None of them is needed to type commands in a shell or to move around an editor,
//! which is what the milestone has to be able to demonstrate; and the three of them drag along a decision that is not
//! the one to make here —Alt prepends an escape to what the key sends, and the other two change their bytes
//! depending on the mode the program is in—. Adding them without being able to see them working would be writing
//! tables blind. What is left out returns silence, never an approximate byte: a wrong
//! byte is understood by the program inside as a different key, and that is worse than receiving nothing.

use eframe::egui::{Event, Key, Modifiers};
use std::borrow::Cow;

/// The bytes that correspond to an event, or nothing if that event does not translate.
pub fn bytes(event: &Event) -> Option<Cow<'static, [u8]>> {
    match event {
        // The text already comes resolved by the system: accents, dead keys and input methods
        // included. Translating it from the physical key instead of from here would be badly redoing
        // the desktop's work.
        Event::Text(text) if !text.is_empty() => Some(Cow::Owned(text.as_bytes().to_vec())),

        // **These three keys do not arrive as keys.** The layer that translates the operating
        // system's events looks at Ctrl+C, Ctrl+X and Ctrl+V *before* emitting anything and turns them
        // into copy, cut and paste, without emitting the key event. On a normal desktop that is what is
        // expected; in a terminal, Ctrl+C is the way to interrupt a hung program, so
        // they have to be caught through this door or nothing gets interrupted.
        //
        // **What this costs, said plainly:** as long as the clipboard does not exist, in this window
        // you cannot copy. These events do not carry the modifiers, so from here there is no
        // way to tell Ctrl+C from Ctrl+Shift+C —which is how you copy in a terminal—, and
        // both end up interrupting. Whoever brings the clipboard will have to decide it earlier,
        // in the loop, where the modifier state is indeed available.
        Event::Copy => Some(Cow::Borrowed(&[3])),
        Event::Cut => Some(Cow::Borrowed(&[24])),
        // **The two shortcuts arrive here, and through this door they are indistinguishable.** The desktop emits
        // the same `Paste` for Ctrl+V and for Ctrl+Shift+V, with no modifiers. The one that decides which
        // is which is the loop, which reconstructs the modifiers by walking the events in order and
        // calls [`paste`] when appropriate; what reaches here is the case without Shift.
        //
        // And that case **does not paste**: Ctrl+V is worth in a terminal what the control byte is worth, which is
        // «the next thing, literal» for a shell and the visual block for vim. Stealing it to paste
        // would be removing two things that are used daily. That a program inside decides that for it
        // that byte means paste —Claude does— is its own business, and it keeps working that way.
        Event::Paste(_) => Some(Cow::Borrowed(&[22])),

        // Releasing a key sends nothing. The repeat of holding it down does: it is what makes
        // holding an arrow down move the cursor.
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => key_bytes(*key, *modifiers),

        _ => None,
    }
}

/// What is sent to the program when text is really pasted.
///
/// `bracketed` is what the program inside has requested: with it, the text goes surrounded by the two
/// sequences that say «this comes pasted», and a shell that understands them leaves it written instead of
/// executing it line by line.
///
/// **Line breaks come out as carriage return**, just like the Enter key and for the same
/// reason: it is what a terminal sends. Pasting «a⏎b» has to do exactly what typing a,
/// Enter, b does. Here there is only `\n` to convert, because the desktop already normalizes `\r\n` before
/// delivering it.
///
/// **All control characters except the tab and line breaks are removed from the text**,
/// and both halves of that sentence are needed.
///
/// The first reason is the wrapper. A clipboard copied from anywhere can bring its
/// closing sequence hidden; if it slipped through, it would close the mark on its own and whatever came
/// behind would arrive as if the user had typed it — **executing without Enter**, which is exactly
/// what the mark exists to prevent. The terminal engine filters nothing: it was checked, and there is not
/// a single mention of these sequences in the whole crate.
///
/// **And removing only that sequence is not enough, checked with an attack that worked.** Search-and-delete
/// makes one pass over the original and does not look again at what it leaves: with `ESC[2` + the closing + `01~`,
/// the only occurrence is in the middle, and on deleting it **the pieces on the sides join and form a
/// new closing** that no one sees any more.
///
/// **The second reason is worse, and the wrapper protects from it not at all.** Control characters
/// are processed by **the kernel's line discipline**, which sees the bytes *before* the
/// program: a `0x03` in the clipboard sends an interrupt to the foreground process, a
/// `0x1a` suspends the shell and leaves the pane looking dead, a `0x1c` triggers a dump, and
/// a `0x04` closes the input — and if the two consoles get closed, the whole window. Marking the
/// paste changes none of that, because the wrapper is a matter for the application inside and the kernel
/// acts first. It is for this that `xterm` and `VTE` have filtered paste's controls for years.
///
/// **What it costs, said plainly:** a text with color sequences loses its escapes and the rest
/// stays as plain text — pasting a colored dump leaves `[32mok[0m` on the line. It is the same
/// that `xterm` does with its default behavior, and it is accepted for the same reason: from here there is no
/// way to tell the control the user wanted from the one slipped in, and a terminal whose
/// paste can execute or suspend is worse than one that pastes as plain text.
///
/// It is filtered **also when it is not wrapped**: the mode is consulted when composing and the program may
/// have changed it right after; and the kernel's danger does not depend on the wrapper.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    const OPENS: &[u8] = b"\x1b[200~";
    const CLOSES: &[u8] = b"\x1b[201~";

    // It is composed in one pass and with the reservation made: the clipboard can be megabytes and this
    // runs on the thread that paints.
    let mut body = Vec::with_capacity(text.len() + OPENS.len() + CLOSES.len());
    if bracketed {
        body.extend_from_slice(OPENS);
    }
    let before = body.len();

    // **How a line break comes out depends on whether there is a mark, and slipping up here shows on screen.**
    //
    // *Without a mark*, the program cannot tell the pasted from the typed, so the key has to be
    // imitated: Enter sends **carriage return**, and pasting «a, break, b» has to do the same as
    // typing a, Enter, b.
    //
    // *With a mark*, the text goes **as is**: the program knows it is a paste and it decides what to do
    // with the breaks —a modern shell leaves the two lines written without executing either—. Sending it
    // there a carriage return is sending it **a control character inside a text**, and what it does
    // is insert it literally: a box appears in the middle of the command and `echo` prints it. Checked on
    // screen, which is how it was discovered.
    let newline = if bracketed { b'\n' } else { b'\r' };

    for byte in text.bytes() {
        match byte {
            // The desktop already normalizes the pairs before delivering it, so a lone carriage return that
            // arrives here came alone and is worth the same as a break.
            b'\n' | b'\r' => body.push(newline),
            b'\t' => body.push(b'\t'),
            0x00..=0x1f | 0x7f => {}
            other => body.push(other),
        }
    }

    // With nothing to send not even the mark is sent: an empty wrapper is noise that the program
    // inside has to interpret for nothing.
    if body.len() == before {
        return Vec::new();
    }

    if bracketed {
        body.extend_from_slice(CLOSES);
    }
    body
}

fn key_bytes(key: Key, modifiers: Modifiers) -> Option<Cow<'static, [u8]>> {
    // Alt and the macOS command key are left out of the milestone, and «out» means silence: Alt
    // prepends an escape, so sending the bare byte would not be an approximation, it would be a different
    // key. Shift does pass, because it does not change the control code — it is what `xterm` does.
    if modifiers.alt || modifiers.mac_cmd {
        return None;
    }
    if modifiers.ctrl
        && let Some(control_byte) = with_ctrl(key)
    {
        return Some(Cow::Owned(vec![control_byte]));
    }
    // If Ctrl accompanies something that is not a letter, it falls to the usual table instead of going silent:
    // Ctrl+Enter sends carriage return just like Enter, and staying mute would be a dead key.
    let bytes_out: &'static [u8] = match key {
        // Carriage return, not line break: it is what a terminal sends, and what the program
        // inside translates as suits it.
        Key::Enter => b"\r",
        // With Shift, Tab is a different key: it backtabs instead of advancing, and it has
        // its own sequence. Sending the usual tab would move the focus the opposite of how the
        // program asked.
        Key::Tab if modifiers.shift => b"\x1b[Z",
        Key::Tab => b"\t",
        // Modern terminals send delete, not backspace. With the other byte, the shell deletes
        // but many editors do not.
        Key::Backspace => &[0x7F],
        Key::Escape => &[0x1B],
        // The four arrows in normal mode. A program can request application mode, in which
        // they change form; that is out of this milestone and is written above.
        Key::ArrowUp => b"\x1b[A",
        Key::ArrowDown => b"\x1b[B",
        Key::ArrowRight => b"\x1b[C",
        Key::ArrowLeft => b"\x1b[D",
        _ => return None,
    };
    Some(Cow::Borrowed(bytes_out))
}

/// Ctrl with a letter gives the control codes from 1 to 26.
///
/// It is the usual rule —the letter minus sixty-four— and it is the one that makes Ctrl+D close the
/// input and Ctrl+Z suspend.
///
/// The twenty-six are written one by one, and not taken from the key's name, even though that name
/// happens to be the letter itself today: that text is meant to be shown to a person in a
/// shortcut menu, it is not a contract. If it changes tomorrow, a control byte would change with it without
/// anything failing to compile.
fn with_ctrl(key: Key) -> Option<u8> {
    let control_byte = match key {
        Key::A => 1,
        Key::B => 2,
        Key::C => 3,
        Key::D => 4,
        Key::E => 5,
        Key::F => 6,
        Key::G => 7,
        Key::H => 8,
        Key::I => 9,
        Key::J => 10,
        Key::K => 11,
        Key::L => 12,
        Key::M => 13,
        Key::N => 14,
        Key::O => 15,
        Key::P => 16,
        Key::Q => 17,
        Key::R => 18,
        Key::S => 19,
        Key::T => 20,
        Key::U => 21,
        Key::V => 22,
        Key::W => 23,
        Key::X => 24,
        Key::Y => 25,
        Key::Z => 26,
        _ => return None,
    };
    Some(control_byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord_of(key: Key, modifiers: Modifiers) -> Option<Vec<u8>> {
        bytes(&Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .map(|b| b.into_owned())
    }

    fn press(key: Key) -> Option<Vec<u8>> {
        chord_of(key, Modifiers::NONE)
    }

    fn press_with_ctrl(key: Key) -> Option<Vec<u8>> {
        chord_of(key, Modifiers::CTRL)
    }

    fn text(s: &str) -> Option<Vec<u8>> {
        bytes(&Event::Text(s.into())).map(|b| b.into_owned())
    }

    #[test]
    fn the_text_travels_as_is_in_utf8() {
        assert_eq!(text("ls"), Some(b"ls".to_vec()));
        // A character that is not ASCII takes several bytes and has to come out whole.
        assert_eq!(text("ñ"), Some(vec![0xC3, 0xB1]), "the ñ is two bytes");
    }

    #[test]
    fn the_empty_text_is_nothing_to_send_and_not_an_empty_list() {
        // Returning something empty would force the caller to tell «there is nothing to send» from «it
        // does not translate», which are the same decision for it.
        assert_eq!(text(""), None);
    }

    #[test]
    fn the_four_basic_control_keys() {
        // Carriage return, not line break.
        assert_eq!(press(Key::Enter), Some(vec![0x0D]));
        assert_eq!(press(Key::Tab), Some(vec![0x09]));
        // Delete, not backspace: with 0x08 the shell deletes but many editors do not.
        assert_eq!(press(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(press(Key::Escape), Some(vec![0x1B]));
    }

    #[test]
    fn the_arrows_go_as_an_escape_sequence() {
        assert_eq!(press(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        assert_eq!(press(Key::ArrowDown), Some(b"\x1b[B".to_vec()));
        assert_eq!(press(Key::ArrowRight), Some(b"\x1b[C".to_vec()));
        assert_eq!(press(Key::ArrowLeft), Some(b"\x1b[D".to_vec()));
    }

    #[test]
    fn the_twenty_six_letters_land_in_their_place_and_none_strays() {
        let letters = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        for (i, key) in letters.iter().enumerate() {
            let expected = (i + 1) as u8;
            assert_eq!(
                press_with_ctrl(*key),
                Some(vec![expected]),
                "the letter at position {i} does not give its control code"
            );
        }
        // The two used daily, named so what is at stake reads clearly.
        assert_eq!(press_with_ctrl(Key::D), Some(vec![4]), "closes the input");
        assert_eq!(press_with_ctrl(Key::Z), Some(vec![26]), "suspends");
    }

    #[test]
    fn interrupt_cut_and_paste_arrive_through_their_own_door() {
        // The layer below keeps Ctrl+C, Ctrl+X and Ctrl+V and turns them into copy, cut
        // and paste before emitting any key. If this case disappeared, **Ctrl+C would stop
        // interrupting** and no key test would notice.
        assert_eq!(
            bytes(&Event::Copy).map(|b| b.into_owned()),
            Some(vec![3]),
            "Ctrl+C has to keep interrupting"
        );
        assert_eq!(bytes(&Event::Cut).map(|b| b.into_owned()), Some(vec![24]));
        assert_eq!(
            bytes(&Event::Paste("whatever there was".into())).map(|b| b.into_owned()),
            Some(vec![22]),
            "real pasting is for another milestone; here it is worth what it is worth in a terminal"
        );
    }

    #[test]
    fn a_letter_key_on_its_own_sends_nothing() {
        // The layer below emits **two** events for each normal keypress: the key and the text
        // it produces. If the key table also answered the letters, every character the
        // user typed would arrive doubled at the program inside. The text is the one that
        // rules, and the key has to stay silent.
        assert_eq!(press(Key::A), None);
        assert_eq!(press(Key::Z), None);
        assert_eq!(press(Key::Space), None);
        assert_eq!(press(Key::Num0), None);
        // And with shift the same: the «A» also travels as text.
        assert_eq!(chord_of(Key::A, Modifiers::SHIFT), None);
    }

    #[test]
    fn the_ctrl_of_linux_and_windows_also_travels_in_command() {
        // Outside macOS, Ctrl arrives marked in both places at once. Confusing that `command`
        // with the macOS command key **would leave Linux and Windows without any Ctrl**.
        //
        // It is tested with D and not with C, X or V: those three never arrive as a key because
        // the layer below turns them first into copy, cut and paste.
        let linux_ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::D, linux_ctrl), Some(vec![4]));
    }

    #[test]
    fn the_macos_command_key_sends_no_control_bytes() {
        // On macOS ⌘ arrives without `ctrl` and with `mac_cmd`. If it were treated as any Ctrl,
        // ⌘+Q would ask to quit the application and would also send the byte that hangs the terminal's
        // output.
        let macos_command = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::Q, macos_command), None);
        assert_eq!(chord_of(Key::Enter, macos_command), None);
    }

    #[test]
    fn shift_with_enter_is_still_carriage_return() {
        // Shift alone does not go through the control-code table, so it needs its own
        // test: if it were filtered the way Alt is filtered, **Shift+Enter would be a dead key**.
        assert_eq!(chord_of(Key::Enter, Modifiers::SHIFT), Some(vec![0x0D]));
    }

    #[test]
    fn shift_with_tab_backtabs_instead_of_advancing() {
        // It is the only key whose meaning Shift changes: with it the program
        // moves the focus back instead of forward. If this sent the usual tab,
        // **moving backward through a form would go forward**.
        assert_eq!(
            chord_of(Key::Tab, Modifiers::SHIFT),
            Some(b"\x1b[Z".to_vec())
        );
        assert_eq!(
            press(Key::Tab),
            Some(vec![0x09]),
            "and without Shift, it advances"
        );
    }

    #[test]
    fn the_byte_is_decided_by_the_logical_key_and_not_the_physical_one() {
        // The layer below sends both: the one the keyboard layout says and the physical
        // position. In a layout that is not the English one they do not match, and heeding the
        // position would send the control byte **of a different letter**.
        let in_another_layout = Event::Key {
            key: Key::D,
            physical_key: Some(Key::H),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        };
        assert_eq!(
            bytes(&in_another_layout).map(|b| b.into_owned()),
            Some(vec![4]),
            "closes the input, does not delete"
        );
    }

    #[test]
    fn shift_does_not_change_the_control_code() {
        // It is what a real terminal does: Ctrl+Shift+C sends the same as Ctrl+C.
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::D, ctrl_shift), Some(vec![4]));
    }

    #[test]
    fn alt_does_not_send_an_approximate_byte() {
        // Alt prepends an escape to what the key sends. Sending the bare byte would not be an
        // approximation: the program inside would understand it as a different keypress.
        let ctrl_alt = Modifiers {
            ctrl: true,
            alt: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::D, ctrl_alt), None);
        assert_eq!(chord_of(Key::Enter, Modifiers::ALT), None);
    }

    #[test]
    fn ctrl_with_a_key_that_is_not_a_letter_falls_to_the_usual_table() {
        // Staying mute here would turn Ctrl+Enter into a dead key, when what a
        // terminal sends is exactly the same as without Ctrl.
        assert_eq!(press_with_ctrl(Key::Enter), Some(vec![0x0D]));
        assert_eq!(press_with_ctrl(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(press_with_ctrl(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        // And what is in neither of the two tables still sends nothing.
        assert_eq!(press_with_ctrl(Key::F1), None);
    }

    #[test]
    fn releasing_a_key_sends_nothing() {
        let release = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(bytes(&release).is_none());
    }

    #[test]
    fn holding_a_key_down_keeps_sending() {
        // The repeat is what makes holding an arrow down move the cursor. Filtering it
        // would break something that shows immediately.
        let repeated = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            bytes(&repeated).map(|b| b.into_owned()),
            Some(b"\x1b[B".to_vec())
        );
    }

    #[test]
    fn the_keys_this_milestone_leaves_out_do_not_invent_bytes() {
        // They are out on purpose, and what matters is that it shows as silence and not as a wrong
        // byte that the program inside would interpret as something else.
        assert_eq!(press(Key::F1), None);
        assert_eq!(press(Key::Home), None);
        assert_eq!(press(Key::PageUp), None);
        assert_eq!(press(Key::Delete), None);
    }

    #[test]
    fn without_a_mark_the_breaks_imitate_the_enter_key() {
        // Without a mark the program does not tell the pasted from the typed, so the key has to be
        // imitated: Enter sends carriage return. Sending 0x0A would be a different key.
        assert_eq!(paste("ls\n", false), b"ls\r");
        assert_eq!(paste("a\nb", false), b"a\rb");
        assert_eq!(paste("no breaks", false), b"no breaks");
    }

    #[test]
    fn with_a_mark_the_breaks_go_as_is_and_not_as_carriage_return() {
        // **This was discovered by looking at the screen, not a test.** With a mark, a carriage return is not
        // «the Enter key»: it is a control character inside a text that the program is going to insert
        // literally. Pasting two commands gave «echo one ▯ echo two» with a box in the middle, and on pressing
        // Enter the shell printed the box. The break has to go as a break and let the program decide.
        assert_eq!(
            paste("echo one\necho two", true),
            b"\x1b[200~echo one\necho two\x1b[201~"
        );
        // A lone carriage return in the text is worth the same as a break, here too.
        assert_eq!(paste("a\rb", true), b"\x1b[200~a\nb\x1b[201~");
    }

    #[test]
    fn with_the_program_requesting_it_the_text_goes_marked_as_pasted() {
        assert_eq!(paste("ls\n", true), b"\x1b[200~ls\n\x1b[201~");
    }

    #[test]
    fn the_text_cannot_close_its_own_mark() {
        // **It is the edge that turns pasting into executing without Enter.** A clipboard copied from
        // anywhere can bring the closing sequence hidden; if it slipped through, it would close the
        // mark on its own and whatever came behind would arrive as typed by the user. The
        // terminal engine filters nothing — it was checked—, so if this does not do it, no one does.
        //
        // **The second case is an attack that worked** and that an adversarial reviewer found: with
        // the previous filtering —search and delete the sequence— the pieces on the sides joined on
        // removing the middle one and formed a new closing that no one looked at any more. That is why it now asserts
        // over the invariant and not over a case: **in the body there is not a single escape**.
        let poisons = [
            "innocent\x1b[201~; curl something | sh\n",
            // The overlap: `ESC[2` + the closing + `01~`.
            "\x1b[2\x1b[201~01~; touch /tmp/PWNED\n",
        ];

        for poison in poisons {
            let bracketed = String::from_utf8(paste(poison, true)).expect("it is text");
            let body = bracketed
                .strip_prefix("\x1b[200~")
                .and_then(|s| s.strip_suffix("\x1b[201~"))
                .unwrap_or_else(|| panic!("the mark does not wrap what it should: {bracketed:?}"));
            assert!(
                !body.bytes().any(|b| b < 0x20 && b != b'\t' && b != b'\n'),
                "a control character stayed inside the mark: {body:?}"
            );

            // And without marking either: the mode is consulted when composing and can change right after.
            let stripped = String::from_utf8(paste(poison, false)).expect("it is text");
            assert!(
                !stripped
                    .bytes()
                    .any(|b| b < 0x20 && b != b'\t' && b != b'\r'),
                "a control character got through unmarked: {stripped:?}"
            );
        }
    }

    #[test]
    fn the_clipboard_controls_do_not_reach_the_kernel() {
        // **The wrapper does not protect from this and that is why the filter is needed.** Control
        // characters are processed by the kernel's line discipline, which sees the bytes before the
        // program: 0x03 interrupts, 0x1a suspends the shell, 0x1c triggers a dump and 0x04 closes the
        // input. A hostile clipboard carries them without their being seen when pasting them.
        let hostile = "echo hello\x03\x1a\x1c\x04 and continues\n";

        for bracketed in [true, false] {
            let bytes_out = paste(hostile, bracketed);
            let body = if bracketed {
                &bytes_out[6..bytes_out.len() - 6]
            } else {
                &bytes_out[..]
            };
            for byte in body {
                assert!(
                    *byte >= 0x20 || *byte == b'\t' || *byte == b'\n' || *byte == b'\r',
                    "a control reached the program: {byte:#04x} in {body:?}"
                );
            }
        }
    }

    #[test]
    fn the_tab_and_the_breaks_survive_the_filter() {
        // They are the three controls a pasted text really carries, and removing them would turn pasting
        // a table or a script into pasting a single run-on line.
        assert_eq!(paste("a\tb\nc", false), b"a\tb\rc");
        assert_eq!(paste("a\tb\nc", true), b"\x1b[200~a\tb\nc\x1b[201~");
        // A lone carriage return is worth the same as a break: the desktop already normalized the pairs.
        assert_eq!(paste("a\rb", false), b"a\rb");
    }

    #[test]
    fn pasting_nothing_sends_nothing_not_even_an_empty_mark() {
        assert!(paste("", true).is_empty());
        assert!(paste("", false).is_empty());
        // A text that was only controls comes to nothing, and it does not send the empty mark either.
        assert!(paste("\x1b\x1b\x1b", true).is_empty());
        assert!(paste("\x03\x04\x1a", true).is_empty());

        // **But what remains after removing the escape IS sent, and that is correct.** `[201~` without
        // its escape in front is plain text: it is written on the line, it closes nothing. Dropping it would be
        // eating characters the user pasted on purpose.
        let left = String::from_utf8(paste("\x1b[201~", true)).expect("it is text");
        assert_eq!(left, "\x1b[200~[201~\x1b[201~");
    }

    #[test]
    fn an_event_that_is_not_from_the_keyboard_is_ignored() {
        assert!(bytes(&Event::WindowFocused(true)).is_none());
    }
}

// ── The application's shortcuts: which chord does what ───────────────────────────────────────────

/// What the application does when someone presses a chord of its own.
///
/// **Eleven, and they are the ones that exist today.** It is not an open list on purpose: each one has its condition and
/// its effect written where they happen, and an action with no effect would be a name that can be
/// configured and does nothing.
///
/// **Copy and paste are not here, and it is not an oversight.** Their chords **do not arrive as keys**: the
/// desktop resolves them earlier and they arrive as their own events, with no chord to reassign or to hand back
/// to the program inside. Paste has been verified since F2.6; copy was uncovered by a review of
/// this milestone, **by running it**, and along the way it found that until then copy did not copy: it interrupted the
/// program.
///
/// Of copy only the attention can be **turned off**, with `key.copy = none`; paste has no key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    SplitRight,
    SplitDown,
    ClosePane,
    NewTab,
    CloseTab,
    PreviousTab,
    NextTab,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
}

impl Action {
    /// How it is written in the configuration file.
    pub fn as_written(self) -> &'static str {
        match self {
            Self::SplitRight => "split-right",
            Self::SplitDown => "split-down",
            Self::ClosePane => "close-pane",
            Self::NewTab => "new-tab",
            Self::CloseTab => "close-tab",
            Self::PreviousTab => "previous-tab",
            Self::NextTab => "next-tab",
            Self::ScrollPageUp => "scroll-page-up",
            Self::ScrollPageDown => "scroll-page-down",
            Self::ScrollToTop => "scroll-to-top",
            Self::ScrollToBottom => "scroll-to-bottom",
        }
    }

    /// The one written this way, if any.
    pub fn from_written(text: &str) -> Option<Self> {
        ALL.iter()
            .copied()
            .find(|action| action.as_written() == text)
    }
}

/// The eleven, to iterate over them.
pub const ALL: [Action; 11] = [
    Action::SplitRight,
    Action::SplitDown,
    Action::ClosePane,
    Action::NewTab,
    Action::CloseTab,
    Action::PreviousTab,
    Action::NextTab,
    Action::ScrollPageUp,
    Action::ScrollPageDown,
    Action::ScrollToTop,
    Action::ScrollToBottom,
];

/// A chord: the modifier keys needed, and the key.
///
/// **`Copy` and comparable by value**, because it is the key looked up in the table once per
/// keyboard event — the hottest path this project has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: eframe::egui::Key,
}

impl Chord {
    /// The chord written this way, if possible.
    ///
    /// The parts are separated by `+`, in any order and regardless of case: `ctrl+shift+e`,
    /// `Shift+Prior`, `alt+f4`.
    pub fn from_written(text: &str) -> Option<Self> {
        let mut chord = Self {
            ctrl: false,
            shift: false,
            alt: false,
            key: eframe::egui::Key::Space,
        };
        let mut key = None;
        for part in text.split('+') {
            let part = part.trim();
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" => chord.alt = true,
                _ => {
                    // **A single key per chord.** Two would be a chord that cannot be pressed, and
                    // accepting it would leave the user expecting it to work.
                    if key.is_some() {
                        return None;
                    }
                    key = Some(key_named(part)?);
                }
            }
        }
        chord.key = key?;
        // **The copy and paste chords cannot be claimed, and accepting them would be promising something that
        // does not happen.** The desktop resolves them **before** they arrive: they do not appear as keys, they arrive
        // already turned into a copy or a paste, and **without saying whether there was Shift**. A
        // `key.split-right = ctrl+shift+c` would be accepted, would never work, and whoever wrote it would
        // have no way to know why.
        if chord.ctrl && matches!(chord.key, eframe::egui::Key::C | eframe::egui::Key::V) {
            return None;
        }
        // **A chord with no modifier key stays with the program inside.** Allowing it would be
        // letting whoever configures take away the `a` from typing without anything warning them.
        (chord.ctrl || chord.shift || chord.alt).then_some(chord)
    }
}

/// The key written this way, if it exists.
///
/// **`Key::from_name` is case-sensitive and whoever writes a configuration file is not**: `F4` yes and
/// `f4` no, `PageUp` yes and `pageup` no. Checked by running it. So it is tried as is, and if not, with
/// the initial capitalized.
///
/// **And there are aliases.** `prior` and `next` are what the traditional terminals call them —and what
/// this project's document called them before anyone checked they do not exist—; `pgup`, `pgdn` and `esc` are the
/// abbreviations written on the key itself.
fn key_named(name: &str) -> Option<eframe::egui::Key> {
    use eframe::egui::Key;
    let alias = match name.to_ascii_lowercase().as_str() {
        "prior" | "pgup" => Some(Key::PageUp),
        "next" | "pgdn" => Some(Key::PageDown),
        "esc" => Some(Key::Escape),
        // **The arrows are also written with the arrow, and that is how the project's document publishes them.**
        // Not accepting them was a real mismatch between what was promised and what the code understood, caught
        // by contrasting the document against the source. **The symbol has no language**, so it survives the
        // milestone: what is withdrawn is the Spanish name, not the drawing of the key.
        //
        // **And only the symbol: `up`, `down`, `left` and `right` are NOT here because they are not needed.** egui
        // calls these four keys `Up`, `Down`, `Left` and `Right`, so the fallback below already
        // resolves them by their name. Measured before removing them: without these four arms the whole suite stays
        // green, including the cases that test them. An alias that repeats what the fallback already does reads
        // as a decision and it is not one.
        "↑" => Some(Key::ArrowUp),
        "↓" => Some(Key::ArrowDown),
        "←" => Some(Key::ArrowLeft),
        "→" => Some(Key::ArrowRight),
        _ => None,
    };
    if alias.is_some() {
        return alias;
    }
    // **It searches among ALL ignoring case, and not by flipping the name around.** Capitalizing the
    // initial fixes `f4` but not `pageup`, which is really called `PageUp` with two. Iterating the
    // list is once per line of the file, not per frame.
    Key::ALL
        .iter()
        .copied()
        .find(|key| key.name().eq_ignore_ascii_case(name))
}

/// Which chord does each thing.
///
/// **It is the single source of which chord belongs to the application**, and that is the reason it exists. Before
/// they were **two parallel lists** —one said whether the chord was ours and the other what it did— and misaligning them already
/// left an arrow «in the queue never reaching the program», with its comment written in place. With
/// eleven configurable chords that risk multiplies by eleven.
///
/// **What it does NOT unify is each action's condition**: that creating does not count repeated, or that switching
/// tabs is only ours with more than one open, still hangs off the action that needs it. They are
/// real and have to survive the chord changing.
#[derive(Debug, Clone)]
pub struct Keymap {
    /// Chord → action pairs. **A vector and not a map**: they are eleven at most, and iterating eleven pairs of
    /// booleans and an integer is faster than reducing the chord to a number. It is looked up once per keyboard
    /// event, which is the project's hottest path.
    table: Vec<(Chord, Action)>,
}

impl Default for Keymap {
    /// The factory ones: the same usual thirteen minus paste, which does not arrive as a key.
    fn default() -> Self {
        use eframe::egui::Key;
        let with_the_chord = |key| Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        };
        // **The scrollback goes with plain «Shift», and it is not a capricious exception.** Without it,
        // `PageUp` is the program's —a pager uses it— and taking it away would break `less` inside this
        // window.
        let scrollback = |key| Chord {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        };
        Self {
            table: vec![
                (with_the_chord(Key::D), Action::SplitRight),
                (with_the_chord(Key::B), Action::SplitDown),
                (with_the_chord(Key::X), Action::ClosePane),
                (with_the_chord(Key::T), Action::NewTab),
                (with_the_chord(Key::W), Action::CloseTab),
                (with_the_chord(Key::ArrowLeft), Action::PreviousTab),
                (with_the_chord(Key::ArrowRight), Action::NextTab),
                (scrollback(Key::PageUp), Action::ScrollPageUp),
                (scrollback(Key::PageDown), Action::ScrollPageDown),
                (scrollback(Key::Home), Action::ScrollToTop),
                (scrollback(Key::End), Action::ScrollToBottom),
            ],
        }
    }
}

impl Keymap {
    /// Which action that chord does, if any.
    ///
    /// **A chord that is not in the table is NOT the application's**, and its bytes go down to the program
    /// inside. It is the half that makes unbinding a shortcut worth something.
    pub fn action_for(&self, chord: Chord) -> Option<Action> {
        self.table
            .iter()
            .find(|(chord_of, _)| *chord_of == chord)
            .map(|(_, action)| *action)
    }

    /// Changes an action's chord. `None` unbinds it.
    ///
    /// Returns the action left **without** its chord by the clash, if any: two actions cannot
    /// share a chord, because then pressing would not know which of the two to do.
    pub fn bind(&mut self, action: Action, chord: Option<Chord>) -> Option<Action> {
        self.table.retain(|(_, chord_of)| *chord_of != action);
        let chord = chord?;
        // **The last one read wins, and which one lost is said.** It is what makes pasting one set of shortcuts
        // on top of another do what is expected; keeping quiet would leave someone half an hour changing a line
        // that another further down is stepping on.
        let displaced = self
            .table
            .iter()
            .find(|(chord_of, _)| *chord_of == chord)
            .map(|(_, action)| *action);
        self.table.retain(|(chord_of, _)| *chord_of != chord);
        self.table.push((chord, action));
        displaced
    }
}

#[cfg(test)]
mod keymap_tests {
    use super::*;
    use eframe::egui::Key;

    #[test]
    fn the_document_publishes_exactly_the_shortcuts_the_code_has() {
        // **It is the highest-yielding reviewer in this project, written as a test.** Contrasting the published
        // document against the source found nine mismatches in a single PR, none visible in the diff.
        // Here it already caught two: the document promised «shift+prior», which the code rejected, and published the
        // arrows with their symbol, which it did not understand either.
        //
        // **The table below is copied from the document by hand and on purpose.** Generating it from the code would measure
        // that the code is consistent with itself; what has to be tied down is that what is published is true.
        let published = [
            ("split-right", "ctrl+shift+d", Action::SplitRight),
            ("split-down", "ctrl+shift+b", Action::SplitDown),
            ("close-pane", "ctrl+shift+x", Action::ClosePane),
            ("new-tab", "ctrl+shift+t", Action::NewTab),
            ("close-tab", "ctrl+shift+w", Action::CloseTab),
            ("previous-tab", "ctrl+shift+←", Action::PreviousTab),
            ("next-tab", "ctrl+shift+→", Action::NextTab),
            ("scroll-page-up", "shift+pageup", Action::ScrollPageUp),
            ("scroll-page-down", "shift+pagedown", Action::ScrollPageDown),
            ("scroll-to-top", "shift+home", Action::ScrollToTop),
            ("scroll-to-bottom", "shift+end", Action::ScrollToBottom),
        ];
        assert_eq!(
            published.len(),
            ALL.len(),
            "the document publishes {} shortcuts and the code has {}",
            published.len(),
            ALL.len()
        );

        let keymap = Keymap::default();
        for (name, written, action) in published {
            assert_eq!(
                Action::from_written(name),
                Some(action),
                "the document publishes the name «{name}» and the code does not understand it"
            );
            let chord = Chord::from_written(written).unwrap_or_else(|| {
                panic!("the document publishes «{written}» and the code does not understand it")
            });
            assert_eq!(
                keymap.action_for(chord),
                Some(action),
                "the document says «{name}» is «{written}» by default, and it is not"
            );
        }
    }

    #[test]
    fn the_eleven_factory_ones_are_the_same_as_always() {
        // **It is what makes this change break no one.** The factory chords are the ones the
        // terminal had before they could be configured, and a table born with others would
        // change them for everyone all at once without anyone asking.
        let keymap = Keymap::default();
        let with_the_chord = |key| Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        };
        let scrollback = |key| Chord {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        };

        let factory = [
            (with_the_chord(Key::D), Action::SplitRight),
            (with_the_chord(Key::B), Action::SplitDown),
            (with_the_chord(Key::X), Action::ClosePane),
            (with_the_chord(Key::T), Action::NewTab),
            (with_the_chord(Key::W), Action::CloseTab),
            (with_the_chord(Key::ArrowLeft), Action::PreviousTab),
            (with_the_chord(Key::ArrowRight), Action::NextTab),
            (scrollback(Key::PageUp), Action::ScrollPageUp),
            (scrollback(Key::PageDown), Action::ScrollPageDown),
            (scrollback(Key::Home), Action::ScrollToTop),
            (scrollback(Key::End), Action::ScrollToBottom),
        ];

        for (chord, expected) in factory {
            assert_eq!(
                keymap.action_for(chord),
                Some(expected),
                "the factory chord of «{}» changed",
                expected.as_written()
            );
        }
        // **Against the pairs just exercised, not against a number written separately.** A literal
        // here goes stale as soon as the list changes, and what has to be tied down is that **no action is
        // left without a factory chord**: a new action with no pair in the list above would never
        // fire and nothing would say so.
        assert_eq!(
            ALL.len(),
            factory.len(),
            "there are actions without a factory chord, or factory chords of actions that no longer exist"
        );
    }

    #[test]
    fn a_chord_that_no_one_claims_is_not_the_applications() {
        // **It is the half that makes unbinding a shortcut worth something**: what is not in the table goes down to the
        // program inside. Without it, `none` would leave the chord in a limbo where it neither does anything nor
        // reaches `vim`.
        let keymap = Keymap::default();
        assert_eq!(
            keymap.action_for(Chord {
                ctrl: true,
                shift: true,
                alt: false,
                key: Key::Z
            }),
            None,
            "a chord no one configured came out as the application's"
        );
    }

    #[test]
    fn unbinding_an_action_gives_the_chord_back_to_the_program() {
        let mut keymap = Keymap::default();
        let its_own = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::D,
        };
        assert_eq!(keymap.action_for(its_own), Some(Action::SplitRight));

        assert_eq!(
            keymap.bind(Action::SplitRight, None),
            None,
            "unbinding steps on no one"
        );
        assert_eq!(
            keymap.action_for(its_own),
            None,
            "the unbound chord is still the application's, so it does not reach the program"
        );
    }

    #[test]
    fn changing_a_chord_frees_the_old_one() {
        // **The two halves of the criterion**: the new one does its thing, and the old one stops doing it. Without the
        // second, reassigning would add chords instead of moving them.
        let mut keymap = Keymap::default();
        let old = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::D,
        };
        let new = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::E,
        };

        keymap.bind(Action::SplitRight, Some(new));

        assert_eq!(
            keymap.action_for(new),
            Some(Action::SplitRight),
            "the new chord does nothing"
        );
        assert_eq!(
            keymap.action_for(old),
            None,
            "the old chord still splits: reassigning added instead of moving, and the program never receives it"
        );
    }

    #[test]
    fn two_actions_do_not_keep_the_same_chord_and_which_one_loses_is_said() {
        // **The last one wins, and which one lost is said.** Keeping quiet would leave someone half an hour changing a
        // line that another further down is stepping on; and leaving both with the same chord would make pressing
        // not know which of the two to do.
        let mut keymap = Keymap::default();
        let of_closing = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::X,
        };

        let displaced = keymap.bind(Action::SplitRight, Some(of_closing));

        assert_eq!(
            displaced,
            Some(Action::ClosePane),
            "it did not say which action was left without a chord"
        );
        assert_eq!(
            keymap.action_for(of_closing),
            Some(Action::SplitRight),
            "the last one did not win"
        );
    }

    #[test]
    fn a_chord_is_written_the_way_people_write_it() {
        for text in [
            "ctrl+shift+e",
            "Ctrl+Shift+E",
            "SHIFT+ctrl+e",
            "  ctrl + shift + e  ",
        ] {
            assert_eq!(
                Chord::from_written(text),
                Some(Chord {
                    ctrl: true,
                    shift: true,
                    alt: false,
                    key: Key::E
                }),
                "«{text}» was not understood"
            );
        }
        // **And the modifier key is now written only in English.** D27 brings into the milestone what whoever uses the
        // terminal sees and decides to leave no alias table; without this assertion, a branch that kept admitting
        // the old spelling goes unnoticed and brings back through the back door the debt the decision
        // saved.
        for text in ["ctrl+mayusculas+e", "ctrl+mayúsculas+e", "ctrl+mayus+e"] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» is still accepted: there is an alias table where D27 says there is none"
            );
        }
    }

    #[test]
    fn the_keys_are_written_the_way_people_write_them() {
        // **`Key::from_name` is case-sensitive and whoever types a file is not.** Checked by running it:
        // `F4` yes and `f4` no, `PageUp` yes and `pageup` no. Without this, half the chords someone
        // would write by hand would be rejected without it being understood why.
        for (text, expected) in [
            ("ctrl+f4", Key::F4),
            ("ctrl+F4", Key::F4),
            ("shift+pageup", Key::PageUp),
            ("shift+home", Key::Home),
            ("ctrl+space", Key::Space),
            ("ctrl+delete", Key::Delete),
        ] {
            assert_eq!(
                Chord::from_written(text).map(|a| a.key),
                Some(expected),
                "«{text}» was not understood"
            );
        }

        // **And the aliases.** `prior` and `next` are what the traditional terminals call them — and what
        // this project's document called them before anyone checked they do not exist. The arrows
        // are also written with the arrow, which is how the document publishes them and it has no language.
        for (text, expected) in [
            ("shift+prior", Key::PageUp),
            ("shift+next", Key::PageDown),
            ("shift+pgup", Key::PageUp),
            ("shift+pgdn", Key::PageDown),
            ("ctrl+esc", Key::Escape),
            ("ctrl+left", Key::ArrowLeft),
            ("ctrl+up", Key::ArrowUp),
            // **The four arrows by their symbol, and the four because they are the only alias in this
            // `match` that the fallback CANNOT resolve**: egui calls them `Up`, `Down`, `Left` and
            // `Right`, so their names resolve on their own and the drawing does not. Two of them nothing
            // exercised while the document published them.
            ("ctrl+←", Key::ArrowLeft),
            ("ctrl+→", Key::ArrowRight),
            ("ctrl+↑", Key::ArrowUp),
            ("ctrl+↓", Key::ArrowDown),
        ] {
            assert_eq!(
                Chord::from_written(text).map(|a| a.key),
                Some(expected),
                "«{text}» was not understood"
            );
        }

        // **And the Spanish key names are no longer valid**, for the same reason as the modifier key above:
        // without asserting it, half of an alias table that D27 forbids can survive without anything failing.
        for text in [
            "shift+repág",
            "ctrl+supr",
            "shift+inicio",
            "shift+fin",
            "ctrl+izquierda",
            "ctrl+espacio",
        ] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» is still accepted: there is an alias table where D27 says there is none"
            );
        }
    }

    #[test]
    fn the_copy_and_paste_chords_cannot_be_claimed() {
        // **Accepting them would be promising something that does not happen**: the desktop resolves them before they
        // arrive, so those chords never appear as keys. Whoever wrote them would see a shortcut
        // that does nothing and would have no way to know why.
        for text in [
            "ctrl+shift+v",
            "ctrl+v",
            "ctrl+alt+v",
            "ctrl+shift+c",
            "ctrl+c",
            "ctrl+alt+c",
        ] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» was accepted, and it would never arrive"
            );
        }
        // But without Ctrl they are keys like any other.
        for text in ["shift+v", "shift+c", "alt+c"] {
            assert!(
                Chord::from_written(text).is_some(),
                "«{text}» was rejected, and that one does arrive"
            );
        }
    }

    #[test]
    fn a_chord_without_a_modifier_key_is_not_allowed() {
        // **Allowing it would let whoever configures take a letter away from typing** without anything
        // warning them: `key.copy = c` would make the `c` never reach the program.
        for text in ["c", "e", "prior"] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» was accepted as a chord, and it would take that key down with it"
            );
        }
        // And what is no known key either.
        for text in ["ctrl+shift+", "ctrl+shift+noexiste", "ctrl+shift+e+f", ""] {
            assert_eq!(Chord::from_written(text), None, "«{text}» was accepted");
        }
    }

    #[test]
    fn the_actions_are_written_the_way_the_document_publishes_them() {
        for action in ALL {
            let written = action.as_written();
            assert_eq!(Action::from_written(written), Some(action));
        }
        assert_eq!(
            Action::from_written("paste"),
            None,
            "paste is not a reassignable shortcut"
        );
        assert_eq!(Action::from_written("made-up"), None);

        // **And the old names are no longer valid**, which is the half D27 decides and nothing else tied down: without this,
        // leaving the old list beside the new one no test notices.
        for old in [
            "partir-al-lado",
            "cerrar-division",
            "pestaña-nueva",
            "pestana-nueva",
        ] {
            assert_eq!(
                Action::from_written(old),
                None,
                "«{old}» is still accepted: there is an alias table where D27 says there is none"
            );
        }
    }

    /// **The eleven names live twice**: here and in the configuration document, written by hand
    /// in both places. Nothing compared them, so renaming one left a
    /// configuration file published with a key the application rejects — and no test noticed.
    ///
    /// The document is read at compile time instead of repeating the list: a test that
    /// copied the eleven names fixes itself when someone translates them on both sides and
    /// forgets the document, which is exactly the failure it comes to catch.
    ///
    /// **And it is the other direction of [`the_document_publishes_exactly_the_shortcuts_the_code_has`],
    /// not a duplicate of it.** That one copies the document's table by hand **on purpose** —its
    /// comment says so— and with that catches **the code** deviating from what is published. But being a copy,
    /// a change in the document without touching the code leaves it green: measured. This one reads the document
    /// for real, so it catches that half. Both are needed, and that is why neither is redundant.
    /// **The eleven names live twice**: here and in the configuration document, written by hand
    /// in both places. Nothing compared them, so renaming one left a
    /// configuration file published with a key the application rejects — and no test noticed.
    ///
    /// The document is read at compile time instead of repeating the list: a test that
    /// copied the eleven names fixes itself when someone translates them on both sides and
    /// forgets the document, which is exactly the failure it comes to catch.
    ///
    /// **And it is the other direction of [`the_document_publishes_exactly_the_shortcuts_the_code_has`],
    /// not a duplicate of it.** That one copies the document's table by hand **on purpose** —its
    /// comment says so— and with that catches **the code** deviating from what is published. But being a copy,
    /// a change in the document without touching the code leaves it green: measured. This one reads the document
    /// for real, so it catches that half. Both are needed, and that is why neither is redundant.
    /// **Each chord the document publishes is really interpreted.**
    ///
    /// It is the gap that remained between the other two guards: those tie down the eleven **names** and the key
    /// table, and leave out the prose —the key spellings, the list of modifier keys and the
    /// «By default» column—, which is where this project's historical mismatch came from: the document promised
    /// `shift+prior` and the code rejected it, and no one saw it until a person contrasted the two
    /// files by hand. That chord is one of the fourteen this test walks.
    ///
    /// **A chord is recognized by its shape and not by where it is**, so this guard anchors on no
    /// phrase. That is what carried it through the delivery that translated this document: three of
    /// its sibling guards hooked on Spanish headings and column names, and all three came back red
    /// the day the document turned English. This one did not notice, which is the point.
    #[test]
    fn each_chord_the_document_publishes_the_code_understands() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        // **The example blocks are removed before splitting on the backticks**, and not for
        // fun: their fences are three backticks, so splitting over the whole document
        // returns the block's body as if it were a token. The examples already have their own guard,
        // that passes them through the file's parser.
        let mut inside = false;
        let prose: String = DOCUMENT
            .lines()
            .filter(|line| {
                if line.starts_with("```") {
                    inside = !inside;
                    return false;
                }
                !inside
            })
            .collect::<Vec<_>>()
            .join("\n");
        let published: Vec<&str> = prose
            .split('`')
            .skip(1)
            .step_by(2)
            // A chord is a `+` **between two parts**, and that shape is what distinguishes it from the plain `+`
            // with which the document explains how one is written.
            .filter(|token| token.contains('+') && token.split('+').all(|part| !part.is_empty()))
            // **And the case distinguishes the two things the document writes with `+`.** What is in
            // lowercase is a line of the file —the examples and the «By default» column—; what carries a
            // capital names **the key that is pressed** (`Ctrl+Shift+C`), which is prose and not
            // syntax. The two the document declares impossible to assign fall in the second group,
            // and that they are rejected is already asserted by [`the_copy_and_paste_chords_cannot_be_claimed`].
            .filter(|token| **token == token.to_lowercase())
            .collect();
        // Without a floor, a document that stopped writing chords in backticks would leave this
        // test passing by being empty. Today it publishes fourteen.
        assert_eq!(
            published.len(),
            14,
            "the document publishes {} chords in lowercase: either it changed shape, or this walk stopped \
             finding them and the test is not looking at anything — {published:?}",
            published.len()
        );

        for chord in published {
            assert!(
                Chord::from_written(chord).is_some(),
                "the document publishes the chord «{chord}» and the code rejects it: whoever copies it \
                 would write a line that does nothing"
            );
        }
    }

    #[test]
    fn each_action_is_published_in_the_configuration_document() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        let unpublished: Vec<&str> = ALL
            .iter()
            .map(|action| action.as_written())
            .filter(|name| !DOCUMENT.contains(&format!("`{name}`")))
            .collect();

        assert!(
            unpublished.is_empty(),
            "these actions can be configured and the document does not publish them: {unpublished:?}. \
             Whoever reads the document would not know they exist, and whoever follows it would write a key that \
             the application rejects"
        );
    }

    /// **The shortcut table is read out of the document, row by row, and confronted with the code.**
    ///
    /// It is the half the other three leave open, and both of its directions were measured green
    /// before it was written:
    ///
    /// - [`the_document_publishes_exactly_the_shortcuts_the_code_has`] copies the table **by hand and
    ///   on purpose**, so its `published.len() == ALL.len()` counts the copy in this file and never
    ///   the document. Adding a twelfth row — `zoom-pane`, a name the code does not have — left the
    ///   whole suite green.
    /// - [`each_action_is_published_in_the_configuration_document`] goes the other way, code to
    ///   document, so a name the document invents is nothing to it.
    /// - [`each_chord_the_document_publishes_the_code_understands`] asks only whether a chord
    ///   **parses**, never which action it belongs to. Changing the «Out of the box» cell of
    ///   `split-right` from `ctrl+shift+d` to `ctrl+shift+g` also left the suite green, and that cell
    ///   is the one somebody reads to find out what their terminal does today.
    ///
    /// **The names and the chords are paired by position inside the row**, because two of the rows
    /// carry two of each — `previous-tab` · `next-tab` — and pairing them any other way would be
    /// asserting on a set instead of on the mapping, which is the whole thing being published.
    #[test]
    fn the_document_publishes_the_shortcut_table_the_code_really_has() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        // Every backticked token of a cell, in the order the cell writes them.
        fn backticked(cell: &str) -> Vec<&str> {
            cell.split('`').skip(1).step_by(2).collect()
        }

        let mut published: Vec<(&str, &str)> = Vec::new();
        let rows = DOCUMENT
            .lines()
            .skip_while(|line| !line.starts_with("| Name | What it does | Out of the box |"))
            .skip(2)
            .take_while(|line| line.starts_with('|'));
        for row in rows {
            let cells: Vec<&str> = row.split('|').collect();
            let names = backticked(cells.get(1).unwrap_or(&""));
            let chords = backticked(cells.get(3).unwrap_or(&""));
            assert_eq!(
                names.len(),
                chords.len(),
                "the row «{row}» publishes {} names and {} chords: whoever reads it cannot tell which \
                 chord is which shortcut's",
                names.len(),
                chords.len()
            );
            published.extend(names.into_iter().zip(chords));
        }

        // **The floor is an equality against the code and not a count written here**, which is what
        // makes this the guard the hand copy cannot be: a row invented in the document fails it, and
        // so does a shortcut the code grows and the table does not.
        assert_eq!(
            published.len(),
            ALL.len(),
            "the document's shortcut table publishes {} shortcuts and the code has {}: either the \
             document names one the application does not, or this walk stopped finding the table — \
             {published:?}",
            published.len(),
            ALL.len()
        );

        let keymap = Keymap::default();
        for (name, written) in published {
            let action = Action::from_written(name).unwrap_or_else(|| {
                panic!(
                    "the document publishes the shortcut «{name}» and the code does not understand \
                     it: whoever follows the document writes «key.{name} = …» and the terminal \
                     ignores the line"
                )
            });
            let chord = Chord::from_written(written).unwrap_or_else(|| {
                panic!("the document publishes «{written}» for «{name}» and the code rejects it")
            });
            assert_eq!(
                keymap.action_for(chord),
                Some(action),
                "the document says «{name}» comes as «{written}» out of the box, and pressing that \
                 chord does {:?}",
                keymap.action_for(chord)
            );
        }
    }
}
