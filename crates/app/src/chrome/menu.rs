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
    // A floating menu of the kit: raised, a control border, the hovered row on `hover`.
    let roles = crate::ui::style::roles(&ctx, &window.theme);
    let (bg, fg, hover, border, rule) =
        (roles.raised, roles.tx, roles.hover, roles.line2, roles.line);
    let mut action = None;
    let menu_entries: Vec<_> = [
        ("Check for updates", Action::Updates(true), None, false),
        ("What's new", Action::Updates(false), None, false),
        ("Preferences", Action::Preferences, None, true),
        ("Keyboard shortcuts", Action::Shortcuts, None, false),
        ("AI usage", Action::Usage, None, false),
        (
            "Files",
            Action::Tool(crate::sidebar::Tool::Files),
            None,
            true,
        ),
        (
            "File",
            Action::Tool(crate::sidebar::Tool::Document),
            None,
            false,
        ),
        (
            "Board",
            Action::Tool(crate::sidebar::Tool::Board),
            Some(crate::keyboard::Action::Board),
            false,
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
    ]
    .into_iter()
    .filter(|entry| {
        // A compact bar has no room for the tools' and the window's own buttons; its menu does.
        compact
            || !matches!(
                entry.1,
                Action::Tool(_) | Action::Minimize | Action::Maximize | Action::Close
            )
    })
    .collect();
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
        window.chrome.menu_selected = window
            .chrome
            .menu_selected
            .filter(|index| *index < menu_entries.len());
        let mut moved = false;
        let mut activate = false;
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Key {
                    key, pressed: true, ..
                } = event
                {
                    match key {
                        egui::Key::ArrowDown | egui::Key::ArrowUp => {
                            let count = menu_entries.len();
                            window.chrome.menu_selected =
                                Some(match (window.chrome.menu_selected, key) {
                                    (Some(index), egui::Key::ArrowDown) => (index + 1) % count,
                                    (Some(index), _) => (index + count - 1) % count,
                                    (None, egui::Key::ArrowDown) => 0,
                                    (None, _) => count - 1,
                                });
                            moved = true;
                        }
                        egui::Key::Enter => activate = true,
                        _ => {}
                    }
                }
            }
        });
        if let Some(index) = window.chrome.menu_selected {
            if moved {
                let top = 6.0
                    + index as f32 * 30.0
                    + menu_entries[..=index]
                        .iter()
                        .filter(|entry| entry.3)
                        .count() as f32
                        * 9.0;
                if top < window.chrome.menu_scroll {
                    window.chrome.menu_scroll = top;
                } else if top + 30.0 > window.chrome.menu_scroll + menu_rect.height() {
                    window.chrome.menu_scroll = top + 30.0 - menu_rect.height();
                }
                window.chrome.menu_scroll = window.chrome.menu_scroll.clamp(0.0, scroll_max);
            }
            if activate {
                action = Some(menu_entries[index].1);
            }
        }

        let id = egui::Id::new("integrated-menu");
        let layer = egui::LayerId::new(egui::Order::Foreground, id);
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(menu_rect.min)
            .movable(false)
            .constrain(false)
            .show(&ctx, |registered| {
                registered.allocate_rect(menu_rect, Sense::hover());
            });
        if window.board.is_open() {
            ctx.set_sublayer(
                egui::LayerId::new(egui::Order::Foreground, egui::Id::new("board-layer")),
                layer,
            );
        }
        // Geometry is known; paint even on the Area's invisible measurement pass.
        let mut menu_ui = egui::Ui::new(
            ctx.clone(),
            id.with("rows"),
            egui::UiBuilder::new().layer_id(layer).max_rect(menu_rect),
        );
        menu_ui.set_clip_rect(menu_rect);
        let ui = &menu_ui;
        let painter = ui.painter();
        let radius = crate::theme::metrics::RADIUS_FLOATING;
        // The rows' painter is clipped to the menu; the shadow falls outside it.
        ctx.layer_painter(layer)
            .add(roles.shadow.as_shape(menu_rect, radius));
        painter.rect_filled(menu_rect, radius, bg);
        painter.rect_stroke(
            menu_rect,
            radius,
            Stroke::new(1.0, border),
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
                border,
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
                    Stroke::new(1.0, rule),
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
            if response.hovered() && ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
                window.chrome.menu_selected = Some(index);
            }
            if response.hovered() || window.chrome.menu_selected == Some(index) {
                painter.rect_filled(
                    item.shrink2(egui::vec2(4.0, 0.0)),
                    crate::theme::metrics::RADIUS_CONTROL,
                    hover,
                );
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
                    roles.tx3,
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
