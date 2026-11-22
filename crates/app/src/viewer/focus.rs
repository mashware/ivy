//! Returning the keyboard from a drawer to a visible console.

use eframe::egui;

pub(super) fn return_to_console(
    window: &mut crate::Window,
    ctx: &egui::Context,
    area: egui::Rect,
    metrics: &crate::painter::CellMetrics,
) -> bool {
    let at = ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } if modifiers.is_none() && area.contains(*pos) => Some(*pos),
            _ => None,
        })
    });
    let Some(at) = at else { return false };
    let tiling = crate::pane_view::view_tiling(window, area, metrics);
    let now = std::time::SystemTime::now();
    let target = window
        .active()
        .panes()
        .into_iter()
        .zip(&tiling.parts)
        .find_map(|(pane, rect)| {
            pane.console_within(*rect, metrics, now, window.headers)
                .contains(at)
                .then_some(pane.name)
        });
    let Some(target) = target else { return false };
    window.active_mut().focus = target;
    ctx.memory_mut(|m| {
        if let Some(editor) = m.focused() {
            m.surrender_focus(editor);
        }
    });
    true
}
