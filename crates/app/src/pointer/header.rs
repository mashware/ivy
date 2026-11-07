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
