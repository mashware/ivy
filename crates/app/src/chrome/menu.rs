//! The global menu scrolls within the window instead of hiding commands below its edge.
use super::{Action, shortcut_label};
use crate::Window;
use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Vec2};

pub(super) fn show(
    window: &mut Window,
    ui: &mut egui::Ui,
    row: Rect,
    area: Rect,
    compact: bool,
) -> (Rect, Option<Action>) {
    let ctx = ui.ctx().clone();
    let bg = window.theme.panel_background();
    let fg = window.theme.foreground();
    let hover = window.theme.divider();
    let mut action = None;
    let menu_entries = [
        ("Check for updates", Action::Updates(true), None, false),
        ("What's new", Action::Updates(false), None, false),
        (
            "New tab",
            Action::NewTab,
            Some(crate::keyboard::Action::NewTab),
            false,
        ),
        (
            "Split right",
            Action::SplitRight,
            Some(crate::keyboard::Action::SplitRight),
            true,
        ),
        (
            "Split down",
            Action::SplitDown,
            Some(crate::keyboard::Action::SplitDown),
            false,
        ),
        ("Toggle information panel", Action::TogglePanel, None, false),
        ("Preferences", Action::Preferences, None, true),
        ("Keyboard shortcuts", Action::Shortcuts, None, false),
        ("Markdown viewer", Action::Viewer(true), None, true),
        ("Git diff viewer", Action::Viewer(false), None, false),
        (
            "Board",
            Action::Board,
            Some(crate::keyboard::Action::Board),
            true,
        ),
        ("Connect agents", Action::Connect, None, true),
        (
            "Search console",
            Action::Search,
            Some(crate::keyboard::Action::Search),
            true,
        ),
        ("Minimize window", Action::Minimize, None, true),
        ("Maximize window", Action::Maximize, None, false),
        ("Close window", Action::Close, None, false),
    ];
    let menu_entries = &menu_entries[..if compact { 16 } else { 12 }];
    let menu_height = 12.0
        + menu_entries.len() as f32 * 30.0
        + menu_entries.iter().filter(|entry| entry.3).count() as f32 * 9.0;
    let menu_rect = Rect::from_min_size(
        egui::pos2((row.right() - 250.0).max(row.left()), row.bottom()),
        Vec2::new(
            240.0_f32.min(row.width()),
            menu_height.min((area.bottom() - row.bottom()).max(0.0)),
        ),
    );
    if window.chrome.menu && !window.settings_editor.is_open() {
        let scroll_max = (menu_height - menu_rect.height()).max(0.0);
        let wheel = ctx.input(|input| {
            if input
                .pointer
                .hover_pos()
                .is_some_and(|p| menu_rect.contains(p))
            {
                input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::MouseWheel { unit, delta, .. } => Some(
                            delta.y
                                * match unit {
                                    egui::MouseWheelUnit::Point => 1.0,
                                    egui::MouseWheelUnit::Line => 30.0,
                                    egui::MouseWheelUnit::Page => menu_rect.height(),
                                },
                        ),
                        _ => None,
                    })
                    .sum::<f32>()
            } else {
                0.0
            }
        });
        window.chrome.menu_scroll = (window.chrome.menu_scroll - wheel).clamp(0.0, scroll_max);
        // An Area's first pass is invisible while egui measures it. The menu's geometry is
        // already known, so draw on a foreground layer directly and interact before the pane UI.
        let painter = ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("integrated-menu"),
            ))
            .with_clip_rect(menu_rect);
        painter.rect_filled(menu_rect, 8.0, bg);
        painter.rect_stroke(
            menu_rect,
            8.0,
            Stroke::new(1.0, hover),
            egui::StrokeKind::Inside,
        );
        if scroll_max > 0.0 {
            let track = Rect::from_min_max(
                menu_rect.right_top() + egui::vec2(-4.0, 6.0),
                menu_rect.right_bottom() - egui::vec2(1.0, 6.0),
            );
            let height = track.height() * menu_rect.height() / menu_height;
            let top =
                track.top() + (track.height() - height) * window.chrome.menu_scroll / scroll_max;
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(track.left(), top),
                    egui::vec2(track.width(), height),
                ),
                2.0,
                hover,
            );
        }
        let mut top = menu_rect.top() + 6.0 - window.chrome.menu_scroll;
        for (index, &(label, cmd, shortcut, separator)) in menu_entries.iter().enumerate() {
            if separator {
                let y = top + 4.0;
                painter.line_segment(
                    [
                        egui::pos2(menu_rect.left() + 6.0, y),
                        egui::pos2(menu_rect.right() - 6.0, y),
                    ],
                    Stroke::new(1.0, hover),
                );
                top += 9.0;
            }
            let item = Rect::from_min_max(
                egui::pos2(menu_rect.left(), top),
                egui::pos2(menu_rect.right(), top + 30.0),
            );
            if !item.intersects(menu_rect) {
                top += 30.0;
                continue;
            }
            let response = ui.interact(
                item.intersect(menu_rect),
                ui.id().with(("menu-item", index)),
                Sense::CLICK,
            );
            if response.hovered() {
                painter.rect_filled(item, 3.0, hover);
            }
            painter.text(
                item.left_center() + egui::vec2(10.0, 0.0),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(12.0),
                fg,
            );
            if let Some(hint) = shortcut
                .and_then(|a| window.keymap.chord_for(a))
                .map(shortcut_label)
            {
                painter.text(
                    item.right_center() - egui::vec2(9.0, 0.0),
                    Align2::RIGHT_CENTER,
                    hint,
                    FontId::proportional(10.0),
                    hover,
                );
            }
            if response.clicked() && menu_rect.contains_rect(item) {
                action = Some(cmd);
            }
            top += 30.0;
        }
    }
    (menu_rect, action)
}
