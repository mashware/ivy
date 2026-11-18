//! Where the title bar's pieces fall: one geometry for its painting, its input claim and the tabs.
//!
//! The tabs live inside the title bar (D96), so the bar's own claim on the pointer and the tabs'
//! clicks share one row. Both read their rectangles from here, which is what keeps a press on a tab
//! from being swallowed by the bar and a press on the bar from landing on a tab.

use eframe::egui::{self, Rect};

/// The width of one title-bar button (`Window.dc.html`).
pub(super) const BUTTON: f32 = 28.0;
/// The space between two buttons of a group.
pub(super) const GAP: f32 = 2.0;
/// The width of one of the window's own buttons.
pub(super) const WINDOW_BUTTON: f32 = 32.0;
/// A separator line with its margins.
pub(super) const SEPARATOR: f32 = 13.0;
/// The Board's labelled button.
pub(super) const BOARD: f32 = 78.0;
/// The logo's slot at the bar's left end, and the gap that sets Files a little apart from it.
const LOGO: f32 = 24.0;
const LOGO_GAP: f32 = 10.0;
/// The left group: a margin, the logo, its gap, Files, and the space before the tabs.
pub(super) const LEFT_GROUP: f32 = 6.0 + LOGO + LOGO_GAP + BUTTON + 14.0;
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

/// Whether this bar draws the window's own buttons: macOS draws its three itself.
pub(super) fn window_buttons() -> bool {
    !cfg!(target_os = "macos")
}

/// What the right-hand groups and the margin take from a full bar: the Board, the two splits,
/// Markdown, connect and the menu, then the window's buttons. Search has no button here: each
/// console carries its own in its header (D61).
fn right_reserve() -> f32 {
    RIGHT_MARGIN
        + if window_buttons() {
            3.0 * WINDOW_BUTTON + SEPARATOR
        } else {
            0.0
        }
        + 3.0 * BUTTON
        + 2.0 * GAP
        + SEPARATOR
        + 2.0 * BUTTON
        + GAP
        + 6.0
        + BOARD
}

/// Every button of a full bar, in the kit's order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slots {
    pub(crate) logo: Rect,
    pub(crate) files: Rect,
    pub(crate) board: Rect,
    pub(crate) split_right: Rect,
    pub(crate) split_down: Rect,
    pub(crate) markdown: Rect,
    pub(crate) connect: Rect,
    pub(crate) menu: Rect,
    /// Minimize, maximize and close, where this bar draws them.
    pub(crate) window: Option<[Rect; 3]>,
    /// The x of each separator line.
    pub(crate) separators: [f32; 2],
}

/// The full bar's buttons over `row`.
pub(crate) fn slots(row: Rect) -> Slots {
    let top = row.top() + ((row.height() - BUTTON) / 2.0).max(0.0);
    let height = BUTTON.min(row.height());
    let at =
        |x: f32, width: f32| Rect::from_min_size(egui::pos2(x, top), egui::vec2(width, height));
    let left = row.left() + left_offset() + 6.0;
    let logo = at(left, LOGO);
    let files = at(logo.right() + LOGO_GAP, BUTTON);
    let mut x = row.right() - RIGHT_MARGIN;
    let window = window_buttons().then(|| {
        let close = at(x - WINDOW_BUTTON, WINDOW_BUTTON);
        let maximize = at(close.left() - WINDOW_BUTTON, WINDOW_BUTTON);
        let minimize = at(maximize.left() - WINDOW_BUTTON, WINDOW_BUTTON);
        x = minimize.left() - SEPARATOR;
        [minimize, maximize, close]
    });
    let late = x + SEPARATOR / 2.0;
    let menu = at(x - BUTTON, BUTTON);
    let connect = at(menu.left() - GAP - BUTTON, BUTTON);
    let markdown = at(connect.left() - GAP - BUTTON, BUTTON);
    let early = markdown.left() - SEPARATOR / 2.0;
    let split_down = at(markdown.left() - SEPARATOR - BUTTON, BUTTON);
    let split_right = at(split_down.left() - GAP - BUTTON, BUTTON);
    let board = Rect::from_min_size(
        egui::pos2(split_right.left() - 6.0 - BOARD, top + 1.0),
        egui::vec2(BOARD, (height - 2.0).max(0.0)),
    );
    Slots {
        logo,
        files,
        board,
        split_right,
        split_down,
        markdown,
        connect,
        menu,
        window,
        separators: [early, if window.is_some() { late } else { f32::NAN }],
    }
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
    /// The logo's slot at the bar's left end; absent when the bar is compact.
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
    let logo = (!compact).then(|| slots(row).logo);
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
        assert_eq!(logo.left(), left_offset() + 6.0, "the logo opens the bar");
        assert_eq!(strip.right_start - strip.tabs.right(), DRAG_MIN);
        for width in [240.0, 120.0, 30.0, 0.0] {
            let area = Rect::from_min_size(egui::pos2(5.0, 7.0), egui::vec2(width, 800.0));
            let strip = within(area, 40.0, 34.0, |_| 40.0);
            assert!(strip.compact());
            assert!(strip.tabs.width() >= 0.0, "{width}");
            assert!(strip.row.contains_rect(strip.tabs), "{width}");
        }
    }

    /// The kit's order, with the owner's change of 17 November 2026: the logo first, Files a
    /// little apart from it; then, right of the tabs, the Board, the two splits, Markdown,
    /// connect and the menu, and the window's own buttons.
    #[test]
    fn the_bar_lays_its_buttons_out_in_the_kits_order() {
        let row = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 40.0));
        let s = slots(row);
        assert!(
            s.files.left() - s.logo.right() >= 8.0,
            "Files stands apart from the logo"
        );
        let strip = within(
            Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0)),
            40.0,
            34.0,
            |_| 40.0,
        );
        assert!(s.files.right() <= strip.tabs.left());
        assert!(strip.right_start <= s.board.left());
        let order = [
            s.board,
            s.split_right,
            s.split_down,
            s.markdown,
            s.connect,
            s.menu,
        ];
        for pair in order.windows(2) {
            assert!(pair[0].right() <= pair[1].left(), "{pair:?}");
        }
        assert!(s.separators[0] > s.split_down.right() && s.separators[0] < s.markdown.left());
        if let Some([minimize, maximize, close]) = s.window {
            assert!(s.menu.right() < s.separators[1] && s.separators[1] < minimize.left());
            assert!(minimize.right() <= maximize.left() && maximize.right() <= close.left());
            assert_eq!(close.right(), row.right() - RIGHT_MARGIN);
        } else {
            assert_eq!(s.menu.right(), row.right() - RIGHT_MARGIN);
        }
    }

    /// Above the old 300-point threshold the full bar used to leave the tabs a band of zero below
    /// about 380 points, and one too narrow for one card ([`crate::tab_bar::MIN_BAND`]) up to about
    /// 456 — 600 since the bar carries the kit's full set, Files, Markdown and a labelled Board
    /// included (D72, D96). At every width one tab keeps
    /// some label and an add target, and a full bar's right-hand buttons all fit after the left
    /// group, so the band starts where the drawn buttons leave it.
    #[test]
    fn one_tab_keeps_its_label_and_add_target_at_every_narrow_width() {
        for width in 300..=640 {
            let width = width as f32;
            let area = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, 800.0));
            let strip = within(area, 40.0, 34.0, |_| 74.0);
            let card = crate::tab_bar::title_card(strip.tabs, 0, 1);
            let (label, _, _) = crate::tab_bar::title_label_dots_summary(card, 1, false);
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
                assert!(slots(strip.row).board.left() >= strip.left_end, "{width}");
            }
        }
        // Both sides of the boundary are reached by the sweep, and the boundary is the one D96
        // publishes, 30 points lower since the search button left the bar (D61): 570 points where
        // the window draws its own buttons, 533 on macOS.
        assert!(is_compact(300.0));
        assert!(!is_compact(640.0));
        let boundary = if cfg!(target_os = "macos") {
            533.0
        } else {
            570.0
        };
        assert!(is_compact(boundary - 1.0), "{boundary}");
        assert!(!is_compact(boundary), "{boundary}");
    }
}
