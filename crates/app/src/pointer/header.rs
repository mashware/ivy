//! Header gestures leave releases for earlier program presses routable.

use super::*;

pub(super) fn intercept(
    window: &mut Window,
    ui: &egui::Ui,
    rects: &[egui::Rect],
    metrics: &CellMetrics,
    names: &[usize],
    now: SystemTime,
) -> Option<MouseOutcome> {
    let outcome = crate::header::actions::pointer(window, ui, rects, metrics)?;
    let releases = Window::collect_the_buttons(ui)
        .into_iter()
        .filter(|button| !button.pressed)
        .collect();
    window.route_the_buttons(rects, names, metrics, releases, now);
    Some(outcome)
}

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
/// **Only a completely bare press**, the same rule [`Window::answer_a_clicked_question`] applies
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
pub(super) fn whose_button_was_pressed(
    window: &Window,
    rects: &[egui::Rect],
    metrics: &CellMetrics,
    now: SystemTime,
    queue: &[ButtonPress],
) -> Option<usize> {
    if window.chrome.enabled() {
        return None;
    }
    let bare = queue.iter().copied().find(|button| {
        button.pressed && button.button == egui::PointerButton::Primary && button.chord.is_none()
    })?;
    let headers = window.headers;
    // **One rectangle per console of this tab**, which is the question
    // [`crate::header::Button::in_a_tab_of`] answers: on a tab of one, no strip carries a
    // button at all and this walk cannot find one.
    let button = crate::header::Button::in_a_tab_of(rects);

    window
        .active()
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
