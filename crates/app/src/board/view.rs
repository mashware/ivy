//! A native, scrollable kanban; each painted card retains the console's stable identity.
use super::{
    cards,
    layout::Slot,
    model::{Entry, Section},
    observation::{self, COLUMNS, Column},
    roadmap,
};
use crate::{Window, agent_signals::View};
use eframe::egui::{self, Rect, Vec2};
use std::collections::HashMap;
use std::time::{Instant, SystemTime};

#[derive(Default)]
pub(super) struct ViewState {
    pub(super) cards: std::collections::HashMap<usize, cards::Reader>,
    pub(super) roadmaps: roadmap::Reader,
    pub(super) child_hits: Vec<(Rect, cards::Command)>,
    pub(super) child_pressed: Option<(Rect, cards::Command)>,
    pub(super) link_to_open: Option<String>,
    pub(super) painted: Vec<(usize, Rect)>,
    pub(super) pressed: Option<(usize, Rect)>,
}

impl ViewState {
    pub(super) fn apply_card_command(
        &mut self,
        command: &cards::Command,
        timing: &mut crate::measure::Board,
    ) {
        let pane = match command {
            cards::Command::Body(pane, _)
            | cards::Command::Expand(pane)
            | cards::Command::Compact(pane) => *pane,
        };
        if !self.cards.contains_key(&pane) {
            return;
        }
        match command {
            cards::Command::Body(_, crate::panel::body::Action::Link(target)) => {
                self.link_to_open = Some(target.clone())
            }
            cards::Command::Body(_, crate::panel::body::Action::Fold(..)) => {
                self.cards.get_mut(&pane).unwrap().apply(command);
                timing.folded();
            }
            cards::Command::Expand(_) => {
                self.cards.get_mut(&pane).unwrap().apply(command);
                timing.expanded();
            }
            cards::Command::Compact(_) => self.cards.get_mut(&pane).unwrap().apply(command),
        }
    }
}

#[derive(Clone)]
struct Geometry {
    viewport: Rect,
    offset: Vec2,
    next_offset: Vec2,
    cards: Vec<(usize, Rect)>,
}

fn reveal_offset(geometry: &Geometry, pane: usize, offset: Vec2) -> Vec2 {
    let Some((_, previous)) = geometry.cards.iter().find(|(id, _)| *id == pane) else {
        return offset;
    };
    let rect = previous.translate(geometry.offset - offset);
    let mut result = offset;
    for axis in 0..2 {
        if rect.min[axis] < geometry.viewport.min[axis] {
            result[axis] += rect.min[axis] - geometry.viewport.min[axis];
        } else if rect.max[axis] > geometry.viewport.max[axis] {
            result[axis] += rect.max[axis] - geometry.viewport.max[axis];
        }
    }
    result.max(Vec2::ZERO)
}

fn column(entry: &Entry, signals: &HashMap<usize, View>, now: SystemTime) -> Column {
    observation::classify(
        entry.snapshot.document.as_deref(),
        signals.get(&entry.pane).copied(),
        now,
    )
}

pub(super) fn paint(
    window: &mut Window,
    parent: &mut egui::Ui,
    area: Rect,
    sections: &[Section],
    signals: &HashMap<usize, View>,
    now: SystemTime,
) {
    // ScrollArea resolves wheel ownership through registered Areas, not painted layer IDs.
    // Keep the Board's foreground layer in that registry as well as in the paint list.
    egui::Area::new(egui::Id::new("board-layer"))
        .order(egui::Order::Foreground)
        .fixed_pos(area.min)
        .movable(false)
        .constrain(false)
        .show(parent.ctx(), |registered| {
            registered.allocate_rect(area, egui::Sense::hover());
            paint_contents(window, registered, area, sections, signals, now);
        });
}

fn paint_contents(
    window: &mut Window,
    parent: &mut egui::Ui,
    area: Rect,
    sections: &[Section],
    signals: &HashMap<usize, View>,
    now: SystemTime,
) {
    let theme = super::style::theme(window.theme);
    let mut ui = egui::Ui::new(
        parent.ctx().clone(),
        egui::Id::new("board-screen"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("board-layer"),
            ))
            .max_rect(area),
    );
    ui.set_clip_rect(area);
    super::style::apply(&mut ui, &theme);
    ui.painter().rect_filled(area, 0.0, theme.background());
    let total = sections.iter().map(|s| s.entries.len()).sum::<usize>();
    if sections.is_empty() {
        egui::Frame::new()
            .fill(theme.divider())
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(&mut ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Board");
                    ui.weak("0 consoles · 0 projects");
                });
            });
    }
    if sections
        .iter()
        .flat_map(|s| &s.entries)
        .all(|e| e.snapshot.document.is_none())
    {
        ui.weak("No task state has been published yet");
    }
    window.board.surface.painted.clear();
    window.board.surface.child_hits.clear();
    window.board.order.clear();
    let reveal = std::mem::take(&mut window.board.scroll_selected);
    let canvas_width = ui.available_width().max(1360.0);
    let widths = super::layout::column_widths(canvas_width - 16.0, 8.0);
    let scroll_id = ui.make_persistent_id("board-list");
    let geometry_id = scroll_id.with("geometry");
    let previous = ui.ctx().data(|data| data.get_temp::<Geometry>(geometry_id));
    let mut offset = previous.as_ref().map_or(Vec2::ZERO, |g| g.next_offset);
    if reveal && let (Some(geometry), Some(pane)) = (&previous, window.board.selected) {
        offset = reveal_offset(geometry, pane, offset);
    }
    let mut geometry = Geometry {
        viewport: Rect::NOTHING,
        offset: Vec2::ZERO,
        next_offset: Vec2::ZERO,
        cards: Vec::new(),
    };
    let output = egui::ScrollArea::both()
        .animated(false)
        .scroll_offset(offset)
        .id_salt("board-list")
        .auto_shrink([false, false])
        .show(&mut ui, |ui| {
            ui.set_min_width(canvas_width);
            let mut row_base = 0;
            for (section_index, section) in sections.iter().enumerate() {
                let section_top = ui.next_widget_position().y;
                let started = Instant::now();
                if let Some(pane) = roadmap::paint_project_with_signals(
                    ui,
                    section,
                    &mut window.board.surface.roadmaps,
                    window.board.roadmap_window,
                    &theme,
                    signals,
                    now,
                    (section_index == 0).then_some((total, sections.len())),
                ) {
                    window.board.activate = Some(pane);
                }
                window.board.timing.plan_paint(started.elapsed());
                if section.entries.is_empty() {
                    ui.separator();
                    continue;
                }
                let context_top = if roadmap::project_open(&window.board.surface.roadmaps, section)
                {
                    ui.next_widget_position().y
                } else {
                    section_top
                };
                let counts = COLUMNS.map(|c| {
                    section
                        .entries
                        .iter()
                        .filter(|e| column(e, signals, now) == c)
                        .count()
                });
                paint_columns(ui, canvas_width, &widths, counts, &theme);
                let mut longest = 0;
                egui::Frame::new().inner_margin(8).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        for (index, c) in COLUMNS.iter().enumerate() {
                            let mut entries: Vec<_> = section
                                .entries
                                .iter()
                                .filter(|e| column(e, signals, now) == *c)
                                .collect();
                            if *c == Column::Wait {
                                entries.sort_by_key(|e| window.board.observations.since(e.pane));
                            }
                            longest = longest.max(entries.len());
                            ui.allocate_ui_with_layout(
                                egui::vec2(widths[index], 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_min_width(widths[index]);
                                    if entries.is_empty() {
                                        ui.weak("—");
                                    }
                                    for (row, entry) in entries.into_iter().enumerate() {
                                        let signal = signals.get(&entry.pane).copied();
                                        let panel_wait = entry
                                            .snapshot
                                            .document
                                            .as_ref()
                                            .and_then(|p| p.attention)
                                            == Some(panel_contract::Attention::Wait)
                                            && !signal.is_some_and(|s| s.busy);
                                        let backed = if *c == Column::Wait && !panel_wait {
                                            signal.is_some_and(|s| s.backed)
                                        } else {
                                            entry.snapshot.attention.is_none_or(|a| a.backed)
                                        };
                                        let since = window.board.observations.since(entry.pane);
                                        let branch = window
                                            .board
                                            .branches
                                            .get(entry.pane)
                                            .map(str::to_owned);
                                        let selected = window.board.selected == Some(entry.pane);
                                        let show_selection =
                                            selected && window.board.original != Some(entry.pane);
                                        let board = &mut window.board;
                                        let reader =
                                            board.surface.cards.entry(entry.pane).or_default();
                                        let rect = cards::paint(
                                            ui,
                                            entry,
                                            *c,
                                            backed,
                                            since,
                                            branch.as_deref(),
                                            show_selection,
                                            reader,
                                            &mut window.discoverer,
                                            &theme,
                                            &mut board.timing,
                                            &mut board.surface.child_hits,
                                        );
                                        board.order.push(Slot {
                                            pane: entry.pane,
                                            row: row_base + row,
                                            column: index,
                                        });
                                        if rect.intersects(ui.clip_rect()) {
                                            board
                                                .surface
                                                .painted
                                                .push((entry.pane, rect.intersect(ui.clip_rect())));
                                            board.timing.painted(1);
                                        }
                                        let mut context = rect;
                                        if row == 0 {
                                            context.min.y = context_top;
                                        }
                                        geometry.cards.push((entry.pane, context));
                                        if selected
                                            && reveal
                                            && previous.as_ref().is_none_or(|g| {
                                                !g.cards.iter().any(|(pane, _)| *pane == entry.pane)
                                            })
                                        {
                                            ui.scroll_to_rect(context, None);
                                        }
                                        if let Some(delay) =
                                            cards::age_refresh(since, Instant::now())
                                        {
                                            ui.ctx().request_repaint_after(delay);
                                        }
                                        ui.add_space(8.0);
                                    }
                                },
                            );
                        }
                    });
                });
                row_base += longest.max(1);
                ui.separator();
            }
        });
    geometry.viewport = output.inner_rect;
    geometry.offset = offset;
    geometry.next_offset = output.state.offset;
    ui.ctx()
        .data_mut(|data| data.insert_temp(geometry_id, geometry));
    window
        .board
        .order
        .sort_by_key(|slot| (slot.row, slot.column));
}

fn paint_columns(
    ui: &mut egui::Ui,
    canvas: f32,
    widths: &[f32; 5],
    counts: [usize; 5],
    theme: &crate::theme::Theme,
) {
    let (header, _) = ui.allocate_exact_size(egui::vec2(canvas, 34.0), egui::Sense::hover());
    let mut left = header.left() + 8.0;
    for (index, column) in COLUMNS.iter().enumerate() {
        let ink = cards::color(*column, theme);
        let icon = Rect::from_center_size(
            egui::pos2(left + 6.0, header.center().y),
            egui::vec2(12.0, 16.0),
        );
        super::style::column_mark(ui.painter(), icon, *column, ink);
        ui.painter().text(
            egui::pos2(left + 18.0, header.center().y),
            egui::Align2::LEFT_CENTER,
            column.name().to_uppercase(),
            egui::FontId::monospace(12.0),
            ink,
        );
        let count = counts[index].to_string();
        let width = 14.0 + count.len() as f32 * 7.0;
        let rect = Rect::from_center_size(
            egui::pos2(left + widths[index] - width / 2.0, header.center().y),
            egui::vec2(width, 18.0),
        );
        ui.painter().rect_filled(rect, 9.0, theme.divider());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            count,
            egui::FontId::monospace(11.0),
            if counts[index] > 0 {
                ink
            } else {
                super::style::dim(theme)
            },
        );
        left += widths[index] + 8.0;
    }
    ui.painter().line_segment(
        [header.left_bottom(), header.right_bottom()],
        egui::Stroke::new(1.0, theme.divider()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_headers_have_local_right_aligned_count_badges_and_monospace_labels() {
        let ctx = egui::Context::default();
        let widths = super::super::layout::column_widths(1344.0, 8.0);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint_columns(
                ui,
                1360.0,
                &widths,
                [2, 1, 0, 2, 0],
                &crate::theme::Theme::default(),
            )
        });
        let mut left = 8.0;
        for ((column, width), expected) in COLUMNS.iter().zip(widths).zip(["2", "1", "0", "2", "0"])
        {
            let label = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == column.name().to_uppercase() => {
                        Some(t)
                    }
                    _ => None,
                })
                .expect("uppercase state header");
            assert!(
                (label.pos.x - (left + 18.0)).abs() < 1.0,
                "state label not in its column"
            );
            assert!(
                label
                    .galley
                    .job
                    .sections
                    .iter()
                    .all(|s| s.format.font_id == egui::FontId::monospace(12.0))
            );
            let count = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t)
                        if t.galley.text() == expected
                            && t.pos.x > left + width - 30.0
                            && t.pos.x < left + width =>
                    {
                        Some(t)
                    }
                    _ => None,
                })
                .expect("right aligned local count");
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect.height()==18.0 && r.rect.contains(count.pos) )));
            left += width + 8.0;
        }
        output.drop_without_applying_deltas();
    }
}
