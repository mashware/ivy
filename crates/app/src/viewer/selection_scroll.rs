use eframe::egui;

pub(super) fn while_dragging(
    ui: &egui::Ui,
    visible: egui::Rect,
    viewport: egui::Rect,
    content_height: f32,
    pointer: egui::Pos2,
) {
    // ScrollArea ignores wheel input while a different widget owns the drag.
    let wheel = ui.input_mut(|input| std::mem::take(&mut input.smooth_scroll_delta.y));
    let distance = pointer.y - pointer.y.clamp(visible.top(), visible.bottom());
    let dt = ui.input(|input| input.stable_dt.min(0.05));
    let edge = distance.clamp(-80.0, 80.0) * 8.0 * dt;
    let delta = if wheel == 0.0 { -edge } else { wheel };
    if (delta > 0.0 && viewport.min.y > 0.0) || (delta < 0.0 && viewport.max.y < content_height) {
        ui.scroll_with_delta_animation(
            egui::vec2(0.0, delta),
            egui::style::ScrollAnimation::none(),
        );
        ui.ctx().request_repaint();
    }
}
