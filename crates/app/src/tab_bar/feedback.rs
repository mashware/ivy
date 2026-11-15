//! Response snapshots belong to this frame; animation memory belongs to egui.
use eframe::egui;

#[derive(Clone, Copy)]
struct State {
    pass: u64,
    hovered: bool,
    held: bool,
}

pub(super) fn id(ui: &egui::Ui, name: usize, role: &str) -> egui::Id {
    ui.id().with(("tab-target", name, role))
}

pub(super) fn interact(ui: &egui::Ui, rect: egui::Rect, id: egui::Id) -> egui::Response {
    let response = ui
        .interact(rect, id, egui::Sense::CLICK)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let state = State {
        pass: ui.ctx().cumulative_pass_nr(),
        hovered: response.hovered(),
        held: response.hovered()
            && response.is_pointer_button_down_on()
            && ui.input(|input| input.pointer.primary_down()),
    };
    ui.ctx()
        .data_mut(|data| data.insert_temp(id.with("feedback"), state));
    response
}

/// How far the hover has faded in, and whether the target is held, for this pass.
pub(super) fn hover_and_held(ui: &egui::Ui, id: egui::Id) -> (f32, bool) {
    let state = ui
        .ctx()
        .data(|data| data.get_temp::<State>(id.with("feedback")))
        .filter(|state| state.pass == ui.ctx().cumulative_pass_nr());
    let hovered = state.is_some_and(|s| s.hovered);
    let hover =
        ui.ctx()
            .animate_value_with_time(id.with("hover"), if hovered { 1.0 } else { 0.0 }, 0.120);
    (hover, state.is_some_and(|s| s.held))
}

pub(super) fn paint(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    id: egui::Id,
    foreground: egui::Color32,
    selected: bool,
) {
    let (hover, held) = hover_and_held(ui, id);
    let active = ui.ctx().animate_value_with_time(
        id.with("selected"),
        if selected { 1.0 } else { 0.0 },
        0.120,
    );
    let alpha = if held {
        0.20
    } else {
        hover * 0.10 + active * 0.025
    };
    if alpha > 0.0 {
        painter.rect_filled(rect, 4.0, foreground.gamma_multiply(alpha));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_press_and_selection_settle_without_idle_repaints() {
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(100.0, 35.0));
        let frame = |time, events, selected| {
            ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let id = id(ui, 42, "body");
                    interact(ui, rect, id);
                    paint(ui, ui.painter(), rect, id, egui::Color32::WHITE, selected);
                },
            )
        };
        let fill = |out: &egui::FullOutput| {
            out.shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.rect == rect => Some(r.fill),
                    _ => None,
                })
                .next_back()
        };
        frame(0.0, vec![], false).drop_without_applying_deltas();
        let hovered = frame(0.01, vec![egui::Event::PointerMoved(rect.center())], false);
        assert_eq!(
            hovered.platform_output.cursor_icon,
            egui::CursorIcon::PointingHand
        );
        let hover = frame(0.13, vec![], false);
        assert!(fill(&hover).is_some());
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let held = frame(0.14, vec![button(rect.center(), true)], false);
        assert_ne!(fill(&held), fill(&hover));
        let settled = frame(0.30, vec![], false);
        assert_eq!(fill(&settled), fill(&held));
        let idle = frame(0.50, vec![], false);
        assert!(
            idle.viewport_output[&egui::ViewportId::ROOT].repaint_delay
                > std::time::Duration::from_secs(1)
        );
        let outside = egui::pos2(200.0, 200.0);
        let cancelled = frame(0.51, vec![egui::Event::PointerMoved(outside)], false);
        assert_ne!(fill(&cancelled), fill(&held));
        frame(0.52, vec![button(outside, false)], true).drop_without_applying_deltas();
        frame(0.65, vec![], true).drop_without_applying_deltas();
        let selected = frame(0.80, vec![], true);
        assert!(fill(&selected).is_some());
        assert!(
            selected.viewport_output[&egui::ViewportId::ROOT].repaint_delay
                > std::time::Duration::from_secs(1)
        );
    }
}
