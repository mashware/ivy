//! One tab-strip partition for painting, input and every pane-size decision.
use super::{CellMetrics, TAB_BAR_MARGIN, set_the_tab_bar_aside};
use crate::Window;
use eframe::egui;

pub(crate) struct Geometry {
    pub cards: Option<egui::Rect>,
    pub gear: Option<egui::Rect>,
    pub consoles: egui::Rect,
    pub dock: Option<egui::Rect>,
    pub rail: egui::Rect,
    pub divider: Option<egui::Rect>,
}

impl Geometry {
    pub fn within(area: egui::Rect, tabs: usize, metrics: &CellMetrics, settings: bool) -> Self {
        if !settings {
            let (cards, consoles) = set_the_tab_bar_aside(area, tabs, metrics);
            return Self {
                cards,
                gear: None,
                dock: None,
                rail: egui::Rect::NOTHING,
                divider: None,
                consoles,
            };
        }
        let height = (metrics.height() + 2.0 * TAB_BAR_MARGIN)
            .max(32.0)
            .min(area.height());
        let (strip, consoles) = area.split_top_bottom_at_y(area.top() + height);
        let width = 38.0_f32.min(strip.width());
        let (cards, gear) = strip.split_left_right_at_x(strip.right() - width);
        Self {
            cards: (cards.width() > 0.0 && tabs > 0).then_some(cards),
            gear: Some(gear),
            dock: None,
            rail: egui::Rect::NOTHING,
            divider: None,
            consoles,
        }
    }
}

/// Shared window partition without adding layout policy to the window's behaviour.
fn raw_geometry(window: &Window, area: egui::Rect, metrics: &CellMetrics) -> Geometry {
    if window.chrome.enabled() {
        let top = (area.top() + crate::chrome::HEIGHT).min(area.bottom());
        let (_, rest) = area.split_top_bottom_at_y(top);
        let bottom = (rest.top() + window.chrome.title_sizes.tab_height()).min(rest.bottom());
        let (cards, consoles) = rest.split_top_bottom_at_y(bottom);
        return Geometry {
            cards: Some(cards),
            gear: Some(egui::Rect::NOTHING),
            dock: None,
            rail: egui::Rect::NOTHING,
            divider: None,
            consoles,
        };
    }
    Geometry::within(
        area,
        window.tabs.len(),
        metrics,
        window.settings_editor.chrome_visible(),
    )
}

pub(crate) fn geometry(window: &Window, area: egui::Rect, metrics: &CellMetrics) -> Geometry {
    let mut geometry = raw_geometry(window, area, metrics);
    let dock = crate::sidebar::partition(window, geometry.consoles, metrics);
    geometry.consoles = dock.consoles;
    geometry.dock = dock.dock;
    geometry.rail = dock.rail;
    geometry.divider = dock.divider;
    geometry
}

/// Frame layout work has its own named span and is discounted exactly once.
pub(crate) fn partition(
    window: &mut Window,
    ui: &mut egui::Ui,
    metrics: &CellMetrics,
    header_owns_pointer: bool,
    dim_consoles: bool,
) -> (Option<egui::Rect>, egui::Rect) {
    let started = std::time::Instant::now();
    let area = ui.max_rect();
    let geometry = raw_geometry(window, area, metrics);
    window.settings_editor.record_geometry(started.elapsed());
    window.timed_separately += window.settings_editor.take_cost();
    if let Some(cards) = geometry.cards {
        super::handle_clicks(window, ui, cards, metrics, header_owns_pointer);
    }
    if header_owns_pointer {
        crate::header::menu::block_pointer(ui, dim_consoles);
    }
    // Tab selection may change both strip visibility and the active tree's console floor.
    let started = std::time::Instant::now();
    let geometry = raw_geometry(window, area, metrics);
    window.settings_editor.record_geometry(started.elapsed());
    window.timed_separately += window.settings_editor.take_cost();
    let started = std::time::Instant::now();
    let dock = crate::sidebar::partition(window, geometry.consoles, metrics);
    window.sidebar.last_dock = dock.dock;
    window.sidebar.record(started.elapsed());
    window.timed_separately += window.sidebar.take_cost();
    (geometry.cards, dock.consoles)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metrics(ctx: &egui::Context) -> CellMetrics {
        CellMetrics::measure(ctx, egui::FontId::monospace(14.0))
    }
    #[test]
    fn single_multiple_and_tiny_windows_partition_the_same_strip() {
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |ui| {
            let metrics = metrics(ui.ctx());
            for tabs in [1, 2, 12] {
                for size in [
                    egui::vec2(900.0, 600.0),
                    egui::vec2(100.0, 20.0),
                    egui::vec2(9.0, 3.0),
                ] {
                    let area = egui::Rect::from_min_size(egui::pos2(7.0, 11.0), size);
                    let geometry = Geometry::within(area, tabs, &metrics, true);
                    let gear = geometry.gear.expect("gear exists with every tab count");
                    assert_eq!(gear.right(), area.right());
                    assert_eq!(gear.top(), area.top());
                    assert_eq!(gear.bottom(), geometry.consoles.top());
                    assert!(area.contains_rect(gear));
                    assert!(area.contains_rect(geometry.consoles));
                    if let Some(cards) = geometry.cards {
                        assert_eq!(cards.right(), gear.left());
                        assert!(cards.right() <= gear.left());
                        assert_eq!(cards.bottom(), geometry.consoles.top());
                    }
                }
            }
        })
        .drop_without_applying_deltas();
    }
    #[test]
    fn historical_mode_preserves_the_exact_old_partition() {
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |ui| {
            let metrics = metrics(ui.ctx());
            let area = egui::Rect::from_min_size(egui::pos2(7.0, 11.0), egui::vec2(900.0, 600.0));
            for tabs in [1, 2, 12] {
                let actual = Geometry::within(area, tabs, &metrics, false);
                let (cards, consoles) = set_the_tab_bar_aside(area, tabs, &metrics);
                assert_eq!(actual.cards, cards);
                assert_eq!(actual.consoles, consoles);
                assert!(actual.gear.is_none());
            }
        })
        .drop_without_applying_deltas();
    }
}
