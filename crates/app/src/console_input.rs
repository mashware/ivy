//! Console keyboard routing, separate from window rendering.
use crate::{Window, WindowRequest, header, keyboard};
use eframe::egui;
use std::time::Instant;

/// Sends what was typed in this pass to **one single** console, the one that has the focus.
///
/// It lives apart from the painting for the same reason as [`Window::handle_the_mouse`]: sending
/// it to both would write the same key into two programs, which is exactly what «without
/// stepping on each other» forbids, and inside the interface pass that decision cannot be
/// checked without opening a window.
///
/// **On a pass somebody typed in, the pass is timed whole and subtracted from the frame
/// (D13).** It used to be a declared hole — «in the batches nobody types» — and the
/// scrollback's search is the batch that types: a press of `Enter` walks the history right
/// here, so leaving it untimed would publish that walk as the interface framework's cost. The
/// press is also published on its own line of the report (D61); only this whole pass is
/// subtracted, so it is never subtracted twice.
///
/// **A pass nobody typed in declares nothing, and that is the hole that is left, said rather
/// than hidden.** It clones the keymap and walks an event list with no press, text or paste in
/// it — key releases and modifier changes, at most — and declaring it on every frame would hand
/// every other declaring path's test a non-zero it did not earn. «Typed» is the blinking cursor's
/// question (`keyboard::somebody_typed`): a press, a text or a paste. A copy on its own is not
/// in it, and stays in the hole. The cursor's anchor is read before the clock starts.
pub(crate) fn send_keys(window: &mut Window, ctx: &egui::Context) -> Vec<WindowRequest> {
    if header::menu::keys(window, ctx) {
        return Vec::new();
    }
    // **The phase goes back to the start whenever a key is pressed**, so the cursor is lit the
    // instant you type instead of at whatever point of the cycle you caught it — and typing
    // continuously never leaves it half-drawn. **It is asked BEFORE the keys are handed over**:
    // sending them consumes the frame's events, so read afterwards this always sees an empty
    // list and the anchor never moves. Measured — the test for it failed exactly that way.
    let typed = ctx.input(keyboard::somebody_typed);
    if typed {
        window.blink.saw_a_key();
    }
    let before = Instant::now();
    let started = window.frame_started;
    // The seed is taken out and put back because the pane and the field are two borrows of the
    // same owner, and the compiler does not let both be held at once.
    let mut modifiers = window.modifiers;
    let tabs_open = window.tabs.len();
    // **The table is cloned, and it is cheap: at most one pair per `keyboard::Action`.**
    // The borrow is the same problem as the modifiers above —the pane and the field are two
    // borrows of the same owner— and here it is solved the same way, without leaving the table
    // somewhere nobody would look for it.
    let keymap = window.keymap.clone();
    let to_copy = window.copy_enabled;
    let tab = window.active_mut();
    // **The focused one is looked up by its name**, and if it does not turn up nobody is typed
    // to: writing into «the first one there is» would send the keys to a program the user is
    // not looking at, which is worse than losing them.
    let at = tab.where_the_focus_is();
    let requests = at
        .map(|at| {
            tab.panes_mut()[at].send_keys(ctx, started, &mut modifiers, tabs_open, &keymap, to_copy)
        })
        .unwrap_or_default();
    // Declared on a pass somebody typed in, and only then: see this function's docblock.
    window.timed_separately += before.elapsed() * u32::from(typed);
    window.modifiers = modifiers;
    requests
}

impl Window {
    pub(crate) fn send_keys_to_the_focused(&mut self, ctx: &egui::Context) -> Vec<WindowRequest> {
        send_keys(self, ctx)
    }
}
