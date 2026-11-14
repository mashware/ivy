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
    style.visuals.widgets.noninteractive.bg_stroke.color = theme.divider();
    style.visuals.widgets.inactive.bg_stroke.color = theme.divider();
}

pub(super) fn progress(ui: &mut egui::Ui, done: usize, total: usize, width: f32, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 4.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, theme.divider());
    if total > 0 && done > 0 {
        let mut fill = rect;
        fill.max.x = fill.min.x + (width * done as f32 / total as f32).round();
        ui.painter().rect_filled(fill, 2.0, theme.focus());
    }
}

pub(super) fn tag(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    ink: Color32,
    theme: &Theme,
) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text.into())
                .monospace()
                .size(11.0)
                .color(ink),
        )
        .fill(theme.panel_background())
        .stroke(egui::Stroke::new(1.0, theme.divider()))
        .corner_radius(6.0),
    )
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

pub(super) fn console_chip(
    ui: &mut egui::Ui,
    column: super::observation::Column,
    title: &str,
    theme: &Theme,
) -> egui::Response {
    let ink = super::cards::color(column, theme);
    let frame = egui::Frame::new()
        .fill(theme.panel_background())
        .stroke(egui::Stroke::new(1.0, ink))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.spacing_mut().interact_size.y = 14.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
                column_mark(ui.painter(), rect, column, ink);
                ui.label(egui::RichText::new(title).size(11.0).color(ink));
            });
        });
    ui.interact(
        frame.response.rect,
        ui.id().with("console-chip"),
        egui::Sense::click(),
    )
}

pub(super) fn disclosure(ui: &mut egui::Ui, open: bool, theme: &Theme) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(theme.panel_background())
        .stroke(egui::Stroke::new(1.0, theme.divider()))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.spacing_mut().interact_size.y = 14.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
                chevron(ui.painter(), rect, open, theme.foreground());
                ui.label("Roadmap");
            });
        });
    ui.interact(
        frame.response.rect,
        ui.id().with("roadmap-disclosure"),
        egui::Sense::click(),
    )
}

pub(super) fn chevron(painter: &egui::Painter, rect: egui::Rect, open: bool, ink: Color32) {
    crate::panel::body::paint_chevron(painter, rect, open, ink);
}

pub(super) fn branch(ui: &mut egui::Ui, rect: egui::Rect, name: &str, theme: &Theme) {
    let x = rect.left() + 4.0;
    let y = rect.center().y;
    let ink = dim(theme);
    let stroke = egui::Stroke::new(1.0, ink);
    ui.painter()
        .line_segment([egui::pos2(x, y - 4.0), egui::pos2(x, y + 4.0)], stroke);
    ui.painter().line_segment(
        [egui::pos2(x, y + 2.0), egui::pos2(x + 5.0, y - 2.0)],
        stroke,
    );
    for point in [
        egui::pos2(x, y - 5.0),
        egui::pos2(x, y + 5.0),
        egui::pos2(x + 6.0, y - 3.0),
    ] {
        ui.painter().circle_stroke(point, 1.5, stroke);
    }
    let mut text = rect;
    text.min.x += 18.0;
    ui.put(
        text,
        egui::Label::new(egui::RichText::new(name).size(11.0).color(ink))
            .truncate()
            .halign(egui::Align::Min),
    );
}

pub(super) fn go(ui: &mut egui::Ui, theme: &Theme) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(12.0, 16.0), egui::Sense::hover());
    let c = rect.center();
    let stroke = egui::Stroke::new(1.0, dim(theme));
    for (a, b) in [
        (egui::vec2(-3.0, 3.0), egui::vec2(3.0, -3.0)),
        (egui::vec2(-3.0, 0.0), egui::vec2(-3.0, 3.0)),
        (egui::vec2(0.0, 3.0), egui::vec2(-3.0, 3.0)),
        (egui::vec2(0.0, -3.0), egui::vec2(3.0, -3.0)),
        (egui::vec2(3.0, 0.0), egui::vec2(3.0, -3.0)),
    ] {
        ui.painter().line_segment([c + a, c + b], stroke);
    }
    response.on_hover_text("Go to this console");
}

pub(super) fn done_summary(ui: &mut egui::Ui, text: &str, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().interact_size.y = 16.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 16.0), egui::Sense::hover());
        crate::panel::body::paint_mark(
            ui.painter(),
            rect,
            panel_contract::Mark::Done,
            theme.focus(),
            egui::FontId::monospace(12.0),
        );
        ui.colored_label(theme.focus(), text);
    });
}

pub(super) fn completed_tag(
    ui: &mut egui::Ui,
    text: &str,
    ink: Color32,
    theme: &Theme,
) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(theme.panel_background())
        .stroke(egui::Stroke::new(1.0, theme.divider()))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.spacing_mut().interact_size.y = 14.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
                crate::panel::body::paint_mark(
                    ui.painter(),
                    rect,
                    panel_contract::Mark::Done,
                    ink,
                    egui::FontId::monospace(11.0),
                );
                ui.label(egui::RichText::new(text).size(11.0).color(ink));
            });
        });
    ui.interact(
        frame.response.rect,
        ui.id().with("completed-tag"),
        egui::Sense::click(),
    )
}
