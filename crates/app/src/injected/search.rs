//! The bench's search gesture: open a search on the focused console, type a query the load
//! writes on every line, step back through its matches, close it, and go back to the bottom.
//! **D61**.
//!
//! **A free function and not a method**, unlike its sibling for links: `impl InjectedDrags` is over
//! the type ceiling **D28** records, and a method here would grow it by its whole body.

use eframe::egui;

use super::InjectedDrags;

/// What the gesture types: the load writes it on every line, so every press finds a match and
/// every visible row carries one to mark — the heaviest marking a real query asks for.
const QUERY: &str = "module";

/// How many presses of `Enter` each gesture makes before closing the search.
const PRESSES: usize = 8;

/// One step of the gesture per frame: the chord, the query, the presses, `Escape` and `Shift+End`.
///
/// **The chord is the factory one on purpose**: every measured run has the settings file vetoed, so
/// the keymap a batch runs with is always the factory keymap.
///
/// **Each on a frame of its own**, for the reason the other gestures give: a hand does not press a
/// chord, type and search in one batch.
pub(super) fn search_the_focused(drags: &mut InjectedDrags, frame_nr: u64) -> Vec<egui::Event> {
    drags.last_frame = Some(frame_nr);
    let step = drags.step;
    drags.step += 1;
    let chord = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
    match step {
        0 => [
            vec![egui::Event::ModifiersChanged(chord)],
            key(egui::Key::F, chord).to_vec(),
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        ]
        .concat(),
        1 => vec![egui::Event::Text(QUERY.to_owned())],
        press if press < 2 + PRESSES => key(egui::Key::Enter, egui::Modifiers::NONE).to_vec(),
        closing if closing == 2 + PRESSES => key(egui::Key::Escape, egui::Modifiers::NONE).to_vec(),
        // **Back to where the program writes**, as a person does after finding what they wanted:
        // left scrolled into the history, the console would spend the rest of the run painting
        // old rows its control never paints, and that difference would be published as the search's.
        _ => {
            drags.step = 0;
            drags.completed += 1;
            [
                vec![egui::Event::ModifiersChanged(egui::Modifiers::SHIFT)],
                key(egui::Key::End, egui::Modifiers::SHIFT).to_vec(),
                vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
            ]
            .concat()
        }
    }
}

/// A key pressed **and released**, in one batch.
///
/// **The release is not decoration.** egui keeps the set of keys held down and marks a press of a
/// key already in it as a repeat, whatever the event says — and the search's chord ignores repeats,
/// as every chord that opens or closes something does. Measured: without releases the first gesture
/// opened the search and every later one sent its query and its presses to the console's program.
fn key(key: egui::Key, modifiers: egui::Modifiers) -> [egui::Event; 2] {
    let state = |pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    [state(true), state(false)]
}
