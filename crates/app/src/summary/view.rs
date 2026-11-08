//! Fixed-height visible rows. Producer actions and links have no interactive widgets here.
use super::{
    input,
    model::{self, Entry},
};
use crate::Window;
use eframe::egui::{self, Rect, RichText};

pub(super) fn paint(window: &mut Window, parent: &mut egui::Ui, area: Rect, rows: &[Entry]) {
    let theme = window.theme;
    // An independent foreground UI stays enabled when the covered console UI is disabled.
    let mut ui = egui::Ui::new(
        parent.ctx().clone(),
        egui::Id::new("overview-screen"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("overview-layer"),
            ))
            .max_rect(area),
    );
    ui.set_clip_rect(area);
    crate::settings::editor::view::style(&mut ui, theme);
    ui.painter().rect_filled(area, 0.0, theme.background());
    let mut close = false;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.heading("Overview");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = ui.button("Close").clicked();
        });
    });
    if rows.iter().all(|r| r.snapshot.document.is_none()) {
        ui.add(egui::Label::new("No task state has been published yet").truncate());
    }
    ui.separator();
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let line_height = ui.fonts_mut(|f| f.row_height(&font)).max(1.0) + 3.0;
    let row_height = line_height * 6.0 + 12.0;
    let selected = window.summary.selected;
    let scroll_selected = std::mem::take(&mut window.summary.scroll_selected);
    window.summary.painted.clear();
    let selected_index = rows.iter().position(|r| Some(r.pane) == selected);
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("overview-list")
        .auto_shrink([false, false]);
    if scroll_selected && let Some(index) = selected_index {
        scroll = scroll
            .vertical_scroll_offset(index as f32 * (row_height + ui.spacing().item_spacing.y));
    }
    scroll.show_rows(&mut ui, row_height, rows.len(), |ui, visible| {
        for index in visible {
            let entry = &rows[index];
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), row_height),
                egui::Sense::hover(),
            );
            window.summary.timing.painted(1);
            let clipped = rect.intersect(ui.clip_rect());
            window.summary.painted.push((entry.pane, clipped));
            if Some(entry.pane) == selected {
                ui.painter()
                    .rect_filled(rect, 3.0, theme.panel_background());
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    egui::Stroke::new(1.0, theme.focus()),
                    egui::StrokeKind::Inside,
                );
            }
            let inner = rect.shrink2(egui::vec2(8.0, 6.0));
            let slot = |n: usize| {
                Rect::from_min_size(
                    inner.min + egui::vec2(0.0, n as f32 * line_height),
                    egui::vec2(inner.width(), line_height),
                )
            };
            let names = format!("{} · {}", entry.console_label, entry.tab_label);
            one_line(ui, slot(0), &names, theme.foreground(), &font, true);
            let mut status = model::status(&entry.snapshot);
            if let Some(phase) = entry
                .snapshot
                .document
                .as_ref()
                .and_then(|p| p.phase.as_deref())
            {
                status.push_str(" · ");
                status.push_str(&bounded(phase, inner.width(), font.size));
            }
            let color = entry
                .snapshot
                .attention
                .map(|a| theme.attention_color(a.class, a.backed))
                .unwrap_or(theme.foreground());
            one_line(ui, slot(1), &status, color, &font, false);
            if let Some(warning) = &entry.snapshot.warning {
                let response = one_line(
                    ui,
                    slot(2),
                    warning,
                    theme.panel_color(None, Some(panel_contract::Style::Warn)),
                    &font,
                    false,
                );
                response.on_hover_text(warning);
            }
            if let Some(document) = &entry.snapshot.document {
                for (n, line) in model::excerpt(document).enumerate() {
                    let target = slot(3 + n);
                    let indent =
                        (line.indent as f32 * font.size * 0.6).min(target.width().max(0.0));
                    let target =
                        Rect::from_min_max(target.min + egui::vec2(indent, 0.0), target.max);
                    let mark = line
                        .mark
                        .map(crate::panel::summary::body_symbol)
                        .map(|m| format!("{m} "))
                        .unwrap_or_default();
                    let reference = line
                        .reference
                        .as_ref()
                        .map(|s| format!("{} ", bounded(s, target.width(), font.size)))
                        .unwrap_or_default();
                    let text = format!(
                        "{mark}{reference}{}",
                        bounded(&line.text, target.width(), font.size)
                    );
                    one_line(
                        ui,
                        target,
                        &text,
                        theme.panel_color(line.mark, line.style),
                        &font,
                        line.style == Some(panel_contract::Style::Title),
                    );
                }
            }
        }
    });
    if close {
        input::close(window, parent.ctx());
    }
}

fn one_line(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    color: egui::Color32,
    font: &egui::FontId,
    strong: bool,
) -> egui::Response {
    let mut text = RichText::new(bounded(text, rect.width(), font.size))
        .font(font.clone())
        .color(color);
    if strong {
        text = text.strong();
    }
    // Text slots must not move the fixed-height row cursor.
    let mut slot = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    slot.set_clip_rect(ui.clip_rect().intersect(rect));
    slot.put(rect, egui::Label::new(text).truncate().selectable(false))
}

/// Bound layout before egui sees producer metadata; a legal document may fill 256 KiB.
fn bounded(text: &str, width: f32, size: f32) -> String {
    let budget = ((width.max(0.0) / size.max(1.0)) * 8.0) as usize + 16;
    let mut chars = text.chars();
    let mut shown: String = chars
        .by_ref()
        .take(budget)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if chars.next().is_some() {
        shown.push('…');
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enormous_metadata_is_bounded_before_layout_and_never_grows_rows() {
        let result = bounded(&"界\n".repeat(100_000), 100.0, 14.0);
        assert!(result.chars().count() < 80);
        assert!(!result.contains('\n'));
        assert!(result.ends_with('…'));
    }
}
