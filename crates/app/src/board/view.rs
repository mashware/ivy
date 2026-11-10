//! Sections of fixed-size cards. Producer actions and links have no interactive widgets here.
use super::{
    input,
    layout::{self, CARD_PADDING, CARD_SLOTS, Layout},
    model::{self, Entry, Section},
};
use crate::{Window, theme::Theme};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, text::LayoutJob};

const MARGIN: f32 = 16.0;

pub(super) fn paint(window: &mut Window, parent: &mut egui::Ui, area: Rect, sections: &[Section]) {
    let theme = window.theme;
    // An independent foreground UI stays enabled when the covered console UI is disabled.
    let mut ui = egui::Ui::new(
        parent.ctx().clone(),
        egui::Id::new("board-screen"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("board-layer"),
            ))
            .max_rect(area.shrink2(egui::vec2(MARGIN, 0.0))),
    );
    ui.set_clip_rect(area);
    crate::settings::editor::view::style(&mut ui, theme);
    ui.painter().rect_filled(area, 0.0, theme.background());
    let mut close = false;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.heading("Board");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = ui.button("Close").clicked();
        });
    });
    let entries = || sections.iter().flat_map(|s| &s.entries);
    if entries().all(|e| e.snapshot.document.is_none()) {
        ui.add(egui::Label::new("No task state has been published yet").truncate());
    }
    ui.separator();
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let line = ui.fonts_mut(|f| f.row_height(&font)).max(1.0) + 3.0;
    let counts: Vec<usize> = sections.iter().map(|s| s.entries.len()).collect();
    let width = (ui.available_width() - ui.spacing().scroll.bar_width - 4.0).max(1.0);
    let grid = layout::layout(&counts, width, line);
    window.board.grid = grid.cards.iter().map(|c| (c.row, c.column)).collect();
    let selected = window.board.selected;
    let scroll_selected = std::mem::take(&mut window.board.scroll_selected);
    window.board.painted.clear();
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("board-list")
        .auto_shrink([false, false]);
    if scroll_selected
        && let Some(card) = entries()
            .position(|e| Some(e.pane) == selected)
            .and_then(|index| grid.cards.get(index))
    {
        scroll = scroll.vertical_scroll_offset(card.rect.top());
    }
    scroll.show_viewport(&mut ui, |ui, viewport| {
        let origin = ui.max_rect().min.to_vec2();
        ui.allocate_rect(
            Rect::from_min_size(origin.to_pos2(), egui::vec2(width, grid.height)),
            egui::Sense::hover(),
        );
        let visible = viewport.translate(origin);
        let clip = ui.clip_rect();
        let mut index = 0;
        for (section, header) in sections.iter().zip(&grid.headers) {
            let header = header.translate(origin);
            if header.intersects(visible) {
                paint_header(ui, header, section, &theme, &font);
            }
            for entry in &section.entries {
                let rect = card_at(&grid, index).translate(origin);
                index += 1;
                if !rect.intersects(visible) {
                    continue;
                }
                window.board.timing.painted(1);
                window
                    .board
                    .painted
                    .push((entry.pane, rect.intersect(clip)));
                paint_card(
                    ui,
                    rect,
                    entry,
                    section.ink,
                    selected == Some(entry.pane),
                    &theme,
                    &font,
                    line,
                );
            }
        }
    });
    if close {
        input::close(window, parent.ctx());
    }
}

fn card_at(grid: &Layout, index: usize) -> Rect {
    grid.cards
        .get(index)
        .map_or(Rect::NOTHING, |card| card.rect)
}

fn paint_header(ui: &egui::Ui, rect: Rect, section: &Section, theme: &Theme, font: &FontId) {
    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
    let y = rect.center().y;
    let mut x = rect.left();
    if let Some(ink) = section.ink {
        painter.rect_filled(
            Rect::from_center_size(egui::pos2(x + 5.0, y), egui::vec2(10.0, 10.0)),
            2.0,
            ink,
        );
        x += 18.0;
    }
    let consoles = match section.entries.len() {
        1 => "1 console".to_owned(),
        n => format!("{n} consoles"),
    };
    let name = text(
        ui,
        &section.name,
        rect.right() - x,
        font,
        theme.foreground(),
    );
    let after = x + name.size().x + 12.0;
    painter.galley(
        egui::pos2(x, y - name.size().y / 2.0),
        name,
        theme.foreground(),
    );
    let count = text(ui, &consoles, rect.right() - after, font, muted(theme));
    painter.galley(
        egui::pos2(after, y - count.size().y / 2.0),
        count,
        muted(theme),
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "one card's paint inputs, read once per frame"
)]
fn paint_card(
    ui: &egui::Ui,
    rect: Rect,
    entry: &Entry,
    ink: Option<Color32>,
    selected: bool,
    theme: &Theme,
    font: &FontId,
    line: f32,
) {
    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
    painter.rect_filled(rect, 4.0, theme.panel_background());
    let (stroke, color) = if selected {
        (1.5, theme.focus())
    } else {
        (1.0, theme.divider())
    };
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(stroke, color),
        egui::StrokeKind::Inside,
    );
    if let Some(ink) = ink {
        painter.rect_filled(
            Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
            0.0,
            ink,
        );
    }
    let inner = rect.shrink(CARD_PADDING);
    let mut slots = (0..CARD_SLOTS).map(|n| {
        Rect::from_min_size(
            inner.min + egui::vec2(4.0, n as f32 * line),
            egui::vec2(inner.width() - 4.0, line),
        )
    });
    let put = |content: &str, color: Color32, indent: f32, slot: Rect| {
        let slot = Rect::from_min_max(
            slot.min + egui::vec2(indent.min(slot.width()), 0.0),
            slot.max,
        );
        let galley = text(ui, content, slot.width(), font, color);
        painter.galley(
            Pos2::new(slot.left(), slot.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
    };
    let name = format!("{} · {}", entry.console_label, entry.tab_label);
    if let Some(slot) = slots.next() {
        put(&name, theme.foreground(), 0.0, slot);
    }
    if let Some(status) = model::status(&entry.snapshot)
        && let Some(slot) = slots.next()
    {
        let color = entry
            .snapshot
            .attention
            .map_or(muted(theme), |a| theme.attention_color(a.class, a.backed));
        put(&status, color, 0.0, slot);
    }
    let document = entry.snapshot.document.as_deref();
    if let Some(phase) = document.and_then(|d| d.phase.as_deref())
        && let Some(slot) = slots.next()
    {
        put(phase, muted(theme), 0.0, slot);
    }
    if let Some(warning) = &entry.snapshot.warning
        && let Some(slot) = slots.next()
    {
        put(
            warning,
            theme.panel_color(None, Some(panel_contract::Style::Warn)),
            0.0,
            slot,
        );
    }
    for body in document.into_iter().flat_map(model::excerpt) {
        let Some(slot) = slots.next() else { break };
        let mark = body
            .mark
            .map(crate::panel::summary::body_symbol)
            .map(|m| format!("{m} "))
            .unwrap_or_default();
        let reference = body
            .reference
            .as_ref()
            .map(|s| format!("{} ", bounded(s, slot.width(), font.size)))
            .unwrap_or_default();
        put(
            &format!("{mark}{reference}{}", body.text),
            theme.panel_color(body.mark, body.style),
            body.indent as f32 * font.size * 0.6,
            slot,
        );
    }
}

fn muted(theme: &Theme) -> Color32 {
    theme.foreground().gamma_multiply(0.65)
}

/// One left-aligned line cut with «…» at the slot's width; never wraps, so no text grows a card.
fn text(
    ui: &egui::Ui,
    content: &str,
    width: f32,
    font: &FontId,
    color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job =
        LayoutJob::simple_singleline(bounded(content, width, font.size), font.clone(), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(0.0));
    ui.fonts_mut(|fonts| fonts.layout_job(job))
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
