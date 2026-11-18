//! A press on a console's search box is the box's (**D61**).

use super::*;

/// Takes this pass's first press that fell on a search box painted in the previous pass, and
/// does what that part of the box does.
///
/// **The press is claimed the way a header control's is**: held while the button stays down, so
/// the release and any drag after it are nobody else's, and returned before the console loop —
/// which would otherwise start a selection from it or report it to a program that asked for the
/// mouse. Taking it out of the queue alone, as the notices do, still lets that loop focus, clear
/// the selection and start a drag from it.
///
/// **Any button**: what lies under the box is out of sight, so neither a menu nor a paste may
/// come from a press on it. Only the primary button acts.
///
/// A press anywhere else is left to whatever it was aimed at; [`fold_the_lists`] has already folded
/// the options lists it did not land on.
pub(super) fn intercept(window: &mut Window, ui: &egui::Ui) -> bool {
    if !ui.is_enabled() {
        return false;
    }
    let frame = ui.ctx().cumulative_frame_nr();
    for button in Window::collect_the_buttons(ui) {
        if !button.pressed {
            continue;
        }
        let Some((name, target)) = box_under(window, frame, button.at) else {
            continue;
        };
        let down = ui
            .ctx()
            .input(|input| input.pointer.button_down(button.button));
        window
            .header_input
            .claim_press(down.then_some(button.button), frame);
        window.active_mut().focus = name;
        window.drag_in_progress = None;
        if button.button == egui::PointerButton::Primary
            && let Some(step) = target.step()
            && let Some(pane) = window.pane_called(name)
        {
            crate::search::press(&mut pane.search, &mut pane.screen, step);
        }
        ui.ctx().request_repaint();
        return true;
    }
    false
}

/// Folds every unfolded options list this pass's presses did not land on, as a menu closes under
/// a click outside it — **before the header controls and the consoles' own routing take the
/// press**, and whatever order the consoles are walked in.
pub(super) fn fold_the_lists(window: &mut Window, ui: &egui::Ui) {
    if !ui.is_enabled() {
        return;
    }
    let frame = ui.ctx().cumulative_frame_nr();
    for button in Window::collect_the_buttons(ui) {
        if !button.pressed {
            continue;
        }
        let kept = box_under(window, frame, button.at).map(|(name, _)| name);
        for pane in window.active_mut().panes_mut() {
            if Some(pane.name) != kept && pane.search.open.as_ref().is_some_and(|open| open.list())
            {
                crate::search::press(
                    &mut pane.search,
                    &mut pane.screen,
                    crate::search::Step::FoldList,
                );
            }
        }
    }
}

/// The console whose box, painted in the pass before `frame`, lies under `at`, and its part there.
fn box_under(
    window: &mut Window,
    frame: u64,
    at: egui::Pos2,
) -> Option<(usize, crate::search::floating::Target)> {
    window
        .active_mut()
        .panes_mut()
        .into_iter()
        .find_map(|pane| {
            let target = pane
                .search
                .open
                .as_ref()?
                .painted
                .as_ref()?
                .hit(frame, at)?;
            Some((pane.name, target))
        })
}
