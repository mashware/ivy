//! A native, scrollable kanban; each painted card retains the console's stable identity.
use super::{
    cards, input,
    layout::Slot,
    model::{Entry, Section},
    observation::{self, COLUMNS, Column},
    roadmap,
};
use crate::Window;
use eframe::egui::{self, Rect, Vec2};
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

fn column(window: &Window, entry: &Entry, now: SystemTime) -> Column {
    let signal = window
        .all_panes()
        .find(|p| p.name == entry.pane)
        .and_then(|p| p.signals.view(Instant::now()));
    observation::classify(entry.snapshot.document.as_deref(), signal, now)
}

pub(super) fn paint(window: &mut Window, parent: &mut egui::Ui, area: Rect, sections: &[Section]) {
    let theme = window.theme;
    let now = SystemTime::now();
    let mut ui = egui::Ui::new(
        parent.ctx().clone(),
        egui::Id::new("board-screen"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("board-layer"),
            ))
            .max_rect(area.shrink(12.0)),
    );
    ui.set_clip_rect(area);
    crate::settings::editor::view::style(&mut ui, theme);
    ui.painter().rect_filled(area, 0.0, theme.background());
    let total = sections.iter().map(|s| s.entries.len()).sum::<usize>();
    let mut close = false;
    ui.horizontal(|ui| {
        ui.heading("Board");
        ui.weak(format!("{total} consoles · {} projects", sections.len()));
        roadmap::paint_controls(ui, sections, &mut window.board.surface.roadmaps);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = ui.button("Close").clicked();
        });
    });
    if sections
        .iter()
        .flat_map(|s| &s.entries)
        .all(|e| e.snapshot.document.is_none())
    {
        ui.weak("No task state has been published yet");
    }
    ui.separator();
    window.board.surface.painted.clear();
    window.board.surface.child_hits.clear();
    window.board.order.clear();
    let reveal = std::mem::take(&mut window.board.scroll_selected);
    let canvas_width = ui.available_width().max(1360.0);
    let widths = super::layout::column_widths(canvas_width, ui.spacing().item_spacing.x);
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
            ui.horizontal_top(|ui| {
                for (index, c) in COLUMNS.iter().enumerate() {
                    let count = sections
                        .iter()
                        .flat_map(|s| &s.entries)
                        .filter(|e| column(window, e, now) == *c)
                        .count();
                    ui.allocate_ui_with_layout(
                        egui::vec2(widths[index], 28.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.colored_label(
                                cards::color(*c, &theme),
                                format!("{} {}  {count}", c.symbol(), c.name()),
                            );
                        },
                    );
                }
            });
            let mut row_base = 0;
            for section in sections {
                let section_top = ui.next_widget_position().y;
                let started = Instant::now();
                if let Some(pane) = roadmap::paint_project(
                    ui,
                    section,
                    &mut window.board.surface.roadmaps,
                    window.board.roadmap_window,
                    &theme,
                ) {
                    window.board.activate = Some(pane);
                }
                window.board.timing.plan_paint(started.elapsed());
                let mut longest = 0;
                ui.horizontal_top(|ui| {
                    for (index, c) in COLUMNS.iter().enumerate() {
                        let mut entries: Vec<_> = section
                            .entries
                            .iter()
                            .filter(|e| column(window, e, now) == *c)
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
                                    let signal = window
                                        .all_panes()
                                        .find(|p| p.name == entry.pane)
                                        .and_then(|p| p.signals.view(Instant::now()));
                                    let panel_wait =
                                        entry.snapshot.document.as_ref().and_then(|p| p.attention)
                                            == Some(panel_contract::Attention::Wait)
                                            && !signal.is_some_and(|s| s.busy);
                                    let backed = if *c == Column::Wait && !panel_wait {
                                        signal.is_some_and(|s| s.backed)
                                    } else {
                                        entry.snapshot.attention.is_none_or(|a| a.backed)
                                    };
                                    let since = window.board.observations.since(entry.pane);
                                    let branch =
                                        window.board.branches.get(entry.pane).map(str::to_owned);
                                    let selected = window.board.selected == Some(entry.pane);
                                    let board = &mut window.board;
                                    let reader = board.surface.cards.entry(entry.pane).or_default();
                                    let rect = cards::paint(
                                        ui,
                                        entry,
                                        *c,
                                        backed,
                                        since,
                                        branch.as_deref(),
                                        selected,
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
                                        context.min.y = section_top;
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
                                    if let Some(delay) = cards::age_refresh(since, Instant::now()) {
                                        ui.ctx().request_repaint_after(delay);
                                    }
                                    ui.add_space(8.0);
                                }
                            },
                        );
                    }
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
    if close {
        input::close(window, parent.ctx());
    }
}
