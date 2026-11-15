//! Where the title bar's pieces fall: one geometry for its painting, its input claim and the tabs.
//!
//! The tabs live inside the title bar (D96), so the bar's own claim on the pointer and the tabs'
//! clicks share one row. Both read their rectangles from here, which is what keeps a press on a tab
//! from being swallowed by the bar and a press on the bar from landing on a tab.

use eframe::egui::{self, Rect};

/// The width of one title-bar button.
pub(super) const BUTTON: f32 = 34.0;
/// The left group: two split buttons, a separator's gap and the logo's slot.
pub(super) const LEFT_GROUP: f32 = 3.0 * BUTTON + 14.0;
/// The margin between the right-hand buttons and the window's edge.
pub(super) const RIGHT_MARGIN: f32 = 6.0;
/// The menu button a compact bar keeps at its right edge.
pub(super) const COMPACT_MENU: f32 = 40.0;
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

/// What the right-hand buttons and their margin take from a full bar.
fn right_reserve() -> f32 {
    RIGHT_MARGIN + right_buttons() as f32 * BUTTON
}

/// Whether a bar `width` points wide is compact: a full bar there would leave its tab band without
/// room for one readable tab and the add button, and a band of zero hides every tab.
pub(crate) fn is_compact(width: f32) -> bool {
    width - left_offset() - LEFT_GROUP - right_reserve() - DRAG_MIN < crate::tab_bar::MIN_BAND
}

/// The title bar's partition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Strip {
    pub(crate) row: Rect,
    /// Where the left group ends.
    pub(crate) left_end: f32,
    /// The logo's slot, after the split buttons; absent when the bar is compact.
    pub(crate) logo: Option<Rect>,
    /// The band the tabs and the add button sit in, on the bar's bottom edge.
    pub(crate) tabs: Rect,
    /// Where the right-hand buttons start.
    pub(crate) right_start: f32,
}

impl Strip {
    /// A compact bar is the one without the left group, so the logo's absence is the one fact.
    pub(crate) fn compact(&self) -> bool {
        self.logo.is_none()
    }
}

/// The partition of a title bar `bar_height` tall at the top of `area`, with tabs `tab_height`
/// tall; `compact_reserve` says what a compact bar keeps on the right of its row.
pub(crate) fn within(
    area: Rect,
    bar_height: f32,
    tab_height: f32,
    compact_reserve: impl FnOnce(Rect) -> f32,
) -> Strip {
    let row = Rect::from_min_max(
        area.min,
        egui::pos2(area.right(), (area.top() + bar_height).min(area.bottom())),
    );
    let compact = is_compact(row.width());
    let left_end = row.left() + left_offset() + if compact { 0.0 } else { LEFT_GROUP };
    let right_start = row.right()
        - if compact {
            compact_reserve(row)
        } else {
            right_reserve()
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
        let strip = within(area, 40.0, 34.0, |_| 40.0);
        assert_eq!(strip.row.height(), 40.0);
        assert_eq!(strip.tabs.height(), 34.0);
        assert_eq!(strip.tabs.bottom(), 40.0);
        let logo = strip.logo.expect("a wide bar has its logo");
        assert!(logo.right() <= strip.tabs.left());
        assert_eq!(logo.left(), left_offset() + 2.0 * BUTTON + 14.0);
        assert_eq!(strip.right_start - strip.tabs.right(), DRAG_MIN);
        for width in [240.0, 120.0, 30.0, 0.0] {
            let area = Rect::from_min_size(egui::pos2(5.0, 7.0), egui::vec2(width, 800.0));
            let strip = within(area, 40.0, 34.0, |_| 40.0);
            assert!(strip.compact());
            assert!(strip.tabs.width() >= 0.0, "{width}");
            assert!(strip.row.contains_rect(strip.tabs), "{width}");
        }
    }

    /// Above the old 300-point threshold the full bar used to leave the tabs a band of zero below
    /// about 380 points, and one too narrow for a pill up to about 456. At every width one tab keeps
    /// its pill whole, some label and an add target, and a full bar's right-hand buttons all fit
    /// after the left group, so the band starts where the drawn buttons leave it.
    #[test]
    fn one_tab_keeps_its_pill_label_and_add_target_at_every_narrow_width() {
        for width in 300..=520 {
            let width = width as f32;
            let area = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, 800.0));
            let strip = within(area, 40.0, 34.0, |_| 74.0);
            let card = crate::tab_bar::title_card(strip.tabs, 0, 1);
            let (pill, label) = crate::tab_bar::title_pill_and_label(card, 1, false);
            assert_eq!(
                pill.width(),
                crate::ui::claim_pill::SIZE,
                "{width}: {card:?}"
            );
            assert!(label.width() >= 12.0, "{width}: {label:?}");
            assert!(
                crate::tab_bar::add_target(strip.tabs, 1).is_some(),
                "{width}: no add target in {:?}",
                strip.tabs
            );
            if !strip.compact() {
                let leftmost = width - right_reserve();
                assert!(leftmost >= strip.left_end, "{width}");
                assert_eq!(strip.right_start, leftmost);
            }
        }
        // Both sides of the boundary are reached by the sweep, and the boundary is the one D96
        // publishes: 456 points where the window draws its own buttons, 426 on macOS.
        assert!(is_compact(300.0));
        assert!(!is_compact(520.0));
        let boundary = if cfg!(target_os = "macos") {
            426.0
        } else {
            456.0
        };
        assert!(is_compact(boundary - 1.0), "{boundary}");
        assert!(!is_compact(boundary), "{boundary}");
    }
}
