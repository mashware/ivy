use super::*;

pub(super) fn intercept(
    window: &mut Window,
    ui: &egui::Ui,
    rects: &[egui::Rect],
    metrics: &CellMetrics,
    names: &[usize],
    now: SystemTime,
) -> bool {
    if !ui.is_enabled() {
        return false;
    }
    if !ui.input(|input| {
        input.pointer.button_pressed(egui::PointerButton::Secondary)
            || (cfg!(target_os = "linux")
                && input.pointer.button_pressed(egui::PointerButton::Middle))
    }) {
        return false;
    }
    let queue = Window::collect_the_buttons(ui);
    for button in &queue {
        if !button.pressed || button.chord.ctrl || button.chord.alt || button.chord.mac_cmd {
            continue;
        }
        let menu = button.button == egui::PointerButton::Secondary;
        let primary = cfg!(target_os = "linux") && button.button == egui::PointerButton::Middle;
        if !menu && !primary {
            continue;
        }
        let Some(name) = window
            .active()
            .panes()
            .into_iter()
            .zip(rects)
            .find_map(|(pane, rect)| {
                let console = pane
                    .chrome_within(*rect, metrics, now, window.headers)
                    .console();
                console.contains(button.at).then_some(pane.name)
            })
        else {
            continue;
        };
        if window.wants_the_mouse(name) && !button.chord.shift {
            continue;
        }
        let frame = ui.ctx().cumulative_frame_nr();
        window.header_input.hold_context(button.button, frame);
        window.active_mut().focus = name;
        window.drag_in_progress = None;
        if menu {
            window
                .pane_view
                .open_menu(window.active().name, name, names.len() > 1, button.at);
        } else {
            window
                .clipboard
                .request(name, crate::clipboard::Source::Primary, Instant::now());
        }
        ui.ctx().request_repaint();
        return true;
    }
    false
}

pub(super) fn select_while_dragging(
    window: &mut Window,
    response: &egui::Response,
    name: usize,
    console: egui::Rect,
    metrics: &CellMetrics,
    origin: Option<egui::Pos2>,
) {
    window.select_while_dragging(response, name, console, metrics, origin);
    if response.drag_stopped()
        && let Some(text) = window
            .pane_called(name)
            .and_then(|pane| pane.screen.selected_text())
    {
        window.clipboard.publish_primary(text);
    }
}
