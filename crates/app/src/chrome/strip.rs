//! Where the title bar's pieces fall: one geometry for its painting, its input claim and the tabs.
//!
//! The tabs live inside the title bar (D96), so the bar's own claim on the pointer and the tabs'
//! clicks share one row. Both read their rectangles from here, which is what keeps a press on a tab
//! from being swallowed by the bar and a press on the bar from landing on a tab.

use eframe::egui::{self, Rect};

/// The width of one title-bar button.
pub(super) const BUTTON: f32 = 34.0;
/// The left group: two split buttons, a separator's gap and the logo's slot.
const LEFT_GROUP: f32 = 3.0 * BUTTON + 14.0;
/// Space kept free between the last tab and the right-hand buttons, so the window can always be
/// dragged by its title bar however many tabs there are; a compact bar gives the tabs all but a
/// sliver of it.
pub(crate) const DRAG_MIN: f32 = 48.0;
const COMPACT_DRAG_MIN: f32 = 16.0;

/// Where the platform's own window buttons leave room on the left.
pub(super) fn left_offset() -> f32 {
    if cfg!(target_os = "macos") { 78.0 } else { 6.0 }
}

/// How many of the right-hand buttons this platform paints: macOS draws its own three.
pub(super) fn right_buttons() -> usize {
    if cfg!(target_os = "macos") { 3 } else { 6 }
}

/// The title bar's partition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Strip {
    pub(crate) row: Rect,
    pub(crate) compact: bool,
    /// Where the left group ends.
    pub(crate) left_end: f32,
    /// The logo's slot, after the split buttons; absent when the bar is compact.
    pub(crate) logo: Option<Rect>,
    /// The band the tabs and the add button sit in, on the bar's bottom edge.
    pub(crate) tabs: Rect,
    /// Where the right-hand buttons start.
    pub(crate) right_start: f32,
}

/// The partition of a title bar `bar_height` tall at the top of `area`, with tabs `tab_height`
/// tall; `compact_reserve` is what a compact bar keeps on the right.
pub(crate) fn within(area: Rect, bar_height: f32, tab_height: f32, compact_reserve: f32) -> Strip {
    let row = Rect::from_min_max(
        area.min,
        egui::pos2(area.right(), (area.top() + bar_height).min(area.bottom())),
    );
    let compact = row.width() < 300.0;
    let left_end = row.left() + left_offset() + if compact { 0.0 } else { LEFT_GROUP };
    let right_start = row.right()
        - if compact {
            compact_reserve
        } else {
            6.0 + right_buttons() as f32 * BUTTON
        };
    let logo = (!compact).then(|| {
        Rect::from_min_size(
            egui::pos2(left_end - BUTTON, row.top()),
            egui::vec2(BUTTON, row.height()),
        )
    });
    let top = (row.bottom() - tab_height).max(row.top());
    let drag = if compact { COMPACT_DRAG_MIN } else { DRAG_MIN };
    let right = (right_start - drag).max(left_end);
    let tabs = Rect::from_min_max(
        egui::pos2(left_end.min(row.right()), top),
        egui::pos2(right.min(row.right()), row.bottom()),
    );
    Strip {
        row,
        compact,
        left_end,
        logo,
        tabs,
        right_start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_sit_on_the_bars_bottom_edge_between_the_logo_and_a_draggable_gap() {
        let area = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 800.0));
        let strip = within(area, 40.0, 34.0, 40.0);
        assert_eq!(strip.row.height(), 40.0);
        assert_eq!(strip.tabs.height(), 34.0);
        assert_eq!(strip.tabs.bottom(), 40.0);
        let logo = strip.logo.expect("a wide bar has its logo");
        assert!(logo.right() <= strip.tabs.left());
        assert_eq!(logo.left(), left_offset() + 2.0 * BUTTON + 14.0);
        assert_eq!(strip.right_start - strip.tabs.right(), DRAG_MIN);
        for width in [240.0, 120.0, 30.0, 0.0] {
            let area = Rect::from_min_size(egui::pos2(5.0, 7.0), egui::vec2(width, 800.0));
            let strip = within(area, 40.0, 34.0, 40.0);
            assert!(strip.compact);
            assert!(strip.logo.is_none());
            assert!(strip.tabs.width() >= 0.0, "{width}");
            assert!(strip.row.contains_rect(strip.tabs), "{width}");
        }
    }
}
