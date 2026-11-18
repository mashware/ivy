//! Board presentation follows the reference without changing the console or sidebar theme.
use crate::theme::Theme;
use eframe::egui::{self, Color32};

pub(super) fn theme(base: Theme) -> Theme {
    let foreground = base
        .color(term_core::TermColor::Indexed(15))
        .gamma_multiply(0.85);
    let mut colors = crate::settings::Colors {
        background: Some(
            base.panel_background()
                .lerp_to_gamma(base.background(), 0.125),
        ),
        panel: Some(base.background()),
        foreground: Some(foreground),
        divider: Some(base.chrome_outline()),
        ..Default::default()
    };
    colors.palette[15] = Some(foreground);
    base.with_colors(&colors)
}

pub(super) fn dim(theme: &Theme) -> Color32 {
    theme.foreground().gamma_multiply(0.5)
}

pub(super) fn apply(ui: &mut egui::Ui, theme: &Theme) {
    crate::settings::editor::view::style(ui, *theme);
    let style = ui.style_mut();
    for (role, size) in [
        (egui::TextStyle::Body, 12.0),
        (egui::TextStyle::Button, 12.0),
        (egui::TextStyle::Small, 11.0),
        (egui::TextStyle::Monospace, 12.0),
        (egui::TextStyle::Heading, 14.0),
    ] {
        style
            .text_styles
            .insert(role, egui::FontId::monospace(size));
    }
    style.spacing.item_spacing = egui::vec2(8.0, 2.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    style.spacing.interact_size.y = 22.0;
    style.visuals.weak_text_color = Some(dim(theme));
    style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    style.visuals.widgets.noninteractive.bg_stroke.color = theme.divider();
    style.visuals.widgets.inactive.bg_stroke.color = theme.divider();
}

pub(super) fn column_mark(
    painter: &egui::Painter,
    rect: egui::Rect,
    column: super::observation::Column,
    ink: Color32,
) {
    use super::observation::Column;
    if column == Column::Done {
        painter.rect_filled(
            egui::Rect::from_center_size(rect.center(), egui::vec2(6.0, 6.0)),
            0.0,
            ink,
        );
    } else {
        let mark = match column {
            Column::Wait => panel_contract::Mark::Wait,
            Column::Block => panel_contract::Mark::Block,
            Column::Working => panel_contract::Mark::Current,
            Column::Idle => panel_contract::Mark::Pending,
            Column::Done => unreachable!(),
        };
        crate::panel::body::paint_mark(painter, rect, mark, ink, egui::FontId::monospace(12.0));
    }
}

pub(super) fn branch(ui: &mut egui::Ui, rect: egui::Rect, name: &str, theme: &Theme) {
    let ink = dim(theme);
    let font = egui::FontId::monospace(11.0);
    let mut job =
        egui::text::LayoutJob::simple(name.to_owned(), font, ink, (rect.width() - 20.0).max(0.0));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    crate::ui::icons::paint(
        ui.painter(),
        egui::pos2(rect.left() + 7.0, rect.center().y),
        crate::ui::icons::Icon::Branch,
        13.0,
        ink,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 18.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
    ui.interact(rect, ui.id().with("branch"), egui::Sense::hover())
        .on_hover_text(name);
}

pub(super) fn console_navigation(ui: &mut egui::Ui, width: f32, theme: &Theme) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
    // The arrow inks about seven tenths of its square: the square keeps it the 8 points the drawn
    // one reached, smaller in a narrower column.
    let size = (width * 0.85).min(11.0);
    crate::ui::icons::paint(
        ui.painter(),
        rect.center(),
        crate::ui::icons::Icon::ArrowUpRight,
        size,
        theme.focus(),
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Go to console");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colliding_board_controls_paint_final_fills_in_both_channels() {
        for source in [
            "panel=#262b2d\n",
            "background=#000000\nforeground=#757575\n",
        ] {
            let (settings, _) = crate::settings::interpret(source).unwrap();
            let board = theme(Theme::default().with_colors(&settings.colors));
            let roles = crate::theme::Roles::of(&board);
            let ctx = egui::Context::default();
            let mut at = egui::Pos2::ZERO;
            for step in 0..3 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        events: if step > 0 {
                            vec![egui::Event::PointerMoved(at)]
                        } else {
                            vec![]
                        },
                        ..Default::default()
                    },
                    |ui| {
                        apply(ui, &board);
                        ui.style_mut().animation_time = 0.0;
                        assert_eq!(ui.visuals().widgets.hovered.bg_fill, roles.hover);
                        assert_eq!(ui.visuals().widgets.hovered.weak_bg_fill, roles.hover);
                        assert_eq!(ui.visuals().selection.bg_fill, roles.sel);
                        at = ui.button("Board action").rect.center();
                        let _ = ui.selectable_label(true, "Chosen row");
                    },
                );
                if step == 2 {
                    assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == roles.hover && r.rect.contains(at))));
                    assert!(
                        output.shapes.iter().any(
                            |s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == roles.sel)
                        )
                    );
                }
                output.drop_without_applying_deltas();
            }
        }
    }
}
