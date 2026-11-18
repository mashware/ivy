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
/// A press anywhere else **folds an unfolded options list and goes on to whatever it was aimed
/// at**, as a menu closes under a click outside it.
pub(super) fn intercept(window: &mut Window, ui: &egui::Ui) -> bool {
    if !ui.is_enabled() {
        return false;
    }
    let frame = ui.ctx().cumulative_frame_nr();
    for button in Window::collect_the_buttons(ui) {
        if !button.pressed {
            continue;
        }
        let mut aimed = None;
        for pane in window.active_mut().panes_mut() {
            let Some(open) = pane.search.open.as_ref() else {
                continue;
            };
            match open.painted.as_ref().and_then(|box_| box_.hit(frame, button.at)) {
                Some(target) => {
                    aimed = Some((pane.name, target));
                    break;
                }
                None if open.list => {
                    crate::search::press(
                        &mut pane.search,
                        &mut pane.screen,
                        crate::search::Step::FoldList,
                    );
                }
                None => {}
            }
        }
        let Some((name, target)) = aimed else {
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
