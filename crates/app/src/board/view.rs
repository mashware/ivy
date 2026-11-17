//! A native, scrollable kanban; each painted card retains the console's stable identity.
use super::{
    cards,
    layout::Slot,
    model::{Entry, Section},
    observation::{self, COLUMNS, Column},
};
use crate::{Window, agent_signals::View};
use eframe::egui::{self, Rect, Vec2};
use std::collections::HashMap;
use std::time::{Instant, SystemTime};

#[derive(Default)]
pub(super) struct ViewState {
    pub(super) cards: std::collections::HashMap<usize, cards::Reader>,
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
            cards::Command::Bar(_) => return,
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
            cards::Command::Bar(_) => {}
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
    totals: super::bar::Totals,
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
            paint_contents(window, registered, area, sections, signals, now, totals);
        });
}

fn paint_contents(
    window: &mut Window,
    parent: &mut egui::Ui,
    area: Rect,
    sections: &[Section],
    signals: &HashMap<usize, View>,
    now: SystemTime,
    totals: super::bar::Totals,
) {
    let theme = super::style::theme(window.theme);
    let roles = crate::ui::style::roles(parent.ctx(), &window.theme);
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
    ui.painter().rect_filled(area, 0.0, roles.base);
    window.board.surface.child_hits.clear();
    super::bar::paint(
        &mut ui,
        area.width(),
        totals,
        window.board.arrangement,
        &window.board.filter,
        &roles,
        &mut window.board.surface.child_hits,
    );
    if sections.is_empty() && window.board.filter.is_active() {
        ui.weak(
            "No console matches this filter. Clear the field or choose All to see every console.",
        );
    } else if sections
        .iter()
        .flat_map(|s| &s.entries)
        .all(|e| e.snapshot.document.is_none())
    {
        ui.weak("No task state has been published yet");
    }
    window.board.surface.painted.clear();
    window.board.order.clear();
    let reveal = std::mem::take(&mut window.board.scroll_selected);
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
    let output = egui::ScrollArea::vertical()
        .animated(false)
        .scroll_offset(offset)
        .id_salt("board-list")
        .auto_shrink([false, false])
        .max_height((ui.available_height() - HINT_HEIGHT).max(0.0))
        .show(&mut ui, |ui| {
            let canvas_width = ui.available_width().max(1.0);
            let stacked = canvas_width < 1360.0;
            let widths = super::layout::column_widths(canvas_width - 16.0, 8.0);
            let mut row_base = 0;
            for section in sections {
                let mut claims = super::milestones::Claims::default();
                for entry in &section.entries {
                    match column(entry, signals, now) {
                        Column::Wait => claims.wait += 1,
                        Column::Block => claims.block += 1,
                        Column::Done => claims.done += 1,
                        Column::Working | Column::Idle => {}
                    }
                }
                if window.board.arrangement == super::filter::Arrangement::ByMilestone {
                    let started = Instant::now();
                    let base = row_base;
                    // Cards are timed by their own spans (`content_layout`, `content_paint`), and
                    // «By state» paints them outside the plan's span: subtract them here too, or
                    // their cost is published twice and once under the plan's name (D13).
                    let mut cards = std::time::Duration::ZERO;
                    let mut paint_card =
                        |ui: &mut egui::Ui, entry: &Entry, row: usize, index: usize, width: f32| {
                            let card_started = Instant::now();
                            let context_top = ui.next_widget_position().y;
                            paint_entries(
                                window,
                                ui,
                                vec![entry],
                                column(entry, signals, now),
                                index,
                                base + row,
                                context_top,
                                reveal,
                                previous.as_ref(),
                                &mut geometry,
                                signals,
                                width,
                                &theme,
                            );
                            cards += card_started.elapsed();
                        };
                    let plan = section.roadmap.as_ref().and_then(|r| r.plan.clone());
                    // The bar's 16-point gutter, so columns and cards never touch the window's edges.
                    row_base += egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(super::bar::GUTTER, 4))
                        .show(ui, |ui| {
                            super::milestones::project_header(ui, section, claims, true, &roles);
                            match &plan {
                                Some(plan) => super::milestones::paint_plan(
                                    ui,
                                    section,
                                    plan,
                                    &roles,
                                    &mut paint_card,
                                ),
                                None => {
                                    super::milestones::paint_loose(ui, section, &mut paint_card)
                                }
                            }
                        })
                        .inner;
                    super::milestones::rule(ui, &roles);
                    window
                        .board
                        .timing
                        .plan_paint(started.elapsed().saturating_sub(cards));
                    continue;
                }
                // «By state» is about consoles: a project known only by its roadmap lives in
                // «By milestone», which lays the whole plan out.
                if section.entries.is_empty() {
                    continue;
                }
                let context_top = ui.next_widget_position().y;
                // No plan is painted here, so nothing goes under the plan's paint span (D13).
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(super::bar::GUTTER, 4))
                    .show(ui, |ui| {
                        super::milestones::project_header(ui, section, claims, false, &roles);
                    });
                if stacked {
                    for c in COLUMNS {
                        let mut entries: Vec<_> = section
                            .entries
                            .iter()
                            .filter(|entry| column(entry, signals, now) == c)
                            .collect();
                        if c == Column::Wait {
                            entries
                                .sort_by_key(|entry| window.board.observations.since(entry.pane));
                        }
                        let entry_count = entries.len();
                        let context_top = ui.next_widget_position().y;
                        paint_stacked_header(ui, c, entries.len(), canvas_width, &theme, &roles);
                        egui::Frame::new().inner_margin(8).show(ui, |ui| {
                            let card_width = ui.available_width().max(1.0);
                            if entries.is_empty() {
                                ui.weak("—");
                            }
                            paint_entries(
                                window,
                                ui,
                                entries,
                                c,
                                0,
                                row_base,
                                context_top,
                                reveal,
                                previous.as_ref(),
                                &mut geometry,
                                signals,
                                card_width,
                                &theme,
                            );
                        });
                        row_base += entry_count;
                    }
                } else {
                    let counts = COLUMNS.map(|c| {
                        section
                            .entries
                            .iter()
                            .filter(|entry| column(entry, signals, now) == c)
                            .count()
                    });
                    paint_columns(ui, canvas_width, &widths, counts, &theme, &roles);
                    let mut longest = 0;
                    egui::Frame::new().inner_margin(8).show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            for (index, c) in COLUMNS.iter().enumerate() {
                                let mut entries: Vec<_> = section
                                    .entries
                                    .iter()
                                    .filter(|entry| column(entry, signals, now) == *c)
                                    .collect();
                                if *c == Column::Wait {
                                    entries.sort_by_key(|entry| {
                                        window.board.observations.since(entry.pane)
                                    });
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
                                        paint_entries(
                                            window,
                                            ui,
                                            entries,
                                            *c,
                                            index,
                                            row_base,
                                            context_top,
                                            reveal,
                                            previous.as_ref(),
                                            &mut geometry,
                                            signals,
                                            widths[index],
                                            &theme,
                                        );
                                    },
                                );
                            }
                        });
                    });
                    row_base += longest.max(1);
                }
                ui.separator();
            }
        });
    paint_hint(&mut ui, window.board.remember_error.as_deref(), &roles);
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

const HINT_HEIGHT: f32 = 26.0;
pub(crate) const HINT: &str = "Enter go to console  ·  Arrows move between cards  ·  Esc close";

/// The keys, named once at the foot of the Board instead of in every card.
fn paint_hint(ui: &mut egui::Ui, refused: Option<&str>, roles: &crate::theme::Roles) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), HINT_HEIGHT),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 0.0, roles.chrome);
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, roles.line),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 16.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        HINT,
        egui::FontId::proportional(11.5),
        roles.tx3,
    );
    if let Some(reason) = refused {
        ui.painter().text(
            egui::pos2(rect.right() - 16.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            format!(
                "This arrangement will not be remembered: {}",
                super::cards::clean(reason)
            ),
            egui::FontId::proportional(11.5),
            roles.wait,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_entries(
    window: &mut Window,
    ui: &mut egui::Ui,
    entries: Vec<&Entry>,
    column: Column,
    column_index: usize,
    row_base: usize,
    context_top: f32,
    reveal: bool,
    previous: Option<&Geometry>,
    geometry: &mut Geometry,
    signals: &HashMap<usize, View>,
    width: f32,
    theme: &crate::theme::Theme,
) {
    ui.set_min_width(width);
    let roles = crate::ui::style::roles(ui.ctx(), &window.theme);
    for (row, entry) in entries.into_iter().enumerate() {
        let signal = signals.get(&entry.pane).copied();
        let panel_wait = entry
            .snapshot
            .document
            .as_ref()
            .and_then(|panel| panel.attention)
            == Some(panel_contract::Attention::Wait)
            && !signal.is_some_and(|view| view.busy);
        let backed = if column == Column::Wait && !panel_wait {
            signal.is_some_and(|view| view.backed)
        } else {
            entry
                .snapshot
                .attention
                .is_none_or(|attention| attention.backed)
        };
        let since = window.board.observations.since(entry.pane);
        let branch = window.board.branches.get(entry.pane).map(str::to_owned);
        let selected = window.board.selected == Some(entry.pane);
        let show_selection = selected && window.board.original != Some(entry.pane);
        let board = &mut window.board;
        let reader = board.surface.cards.entry(entry.pane).or_default();
        let rect = cards::paint(
            ui,
            entry,
            column,
            backed,
            since,
            branch.as_deref(),
            show_selection,
            reader,
            &mut window.discoverer,
            theme,
            &roles,
            &mut board.timing,
            &mut board.surface.child_hits,
        );
        board.order.push(Slot {
            pane: entry.pane,
            row: row_base + row,
            column: column_index,
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
            && previous.is_none_or(|old| !old.cards.iter().any(|(pane, _)| *pane == entry.pane))
        {
            ui.scroll_to_rect(context, None);
        }
        if let Some(delay) = cards::age_refresh(since, Instant::now()) {
            ui.ctx().request_repaint_after(delay);
        }
        ui.add_space(8.0);
    }
}

fn paint_stacked_header(
    ui: &mut egui::Ui,
    column: Column,
    count: usize,
    width: f32,
    theme: &crate::theme::Theme,
    roles: &crate::theme::Roles,
) {
    let (header, _) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::hover());
    let ink = cards::color(column, theme);
    let icon = Rect::from_center_size(
        egui::pos2(header.left() + 14.0, header.center().y),
        egui::vec2(12.0, 16.0),
    );
    super::style::column_mark(ui.painter(), icon, column, ink);
    ui.painter().text(
        egui::pos2(header.left() + 26.0, header.center().y),
        egui::Align2::LEFT_CENTER,
        column.name().to_uppercase(),
        egui::FontId::monospace(12.0),
        ink,
    );
    let badge = Rect::from_center_size(
        egui::pos2(header.right() - 18.0, header.center().y),
        egui::vec2(28.0, 18.0),
    );
    ui.painter().rect_filled(badge, 9.0, roles.raised);
    ui.painter().text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        count.to_string(),
        egui::FontId::monospace(11.0),
        if count > 0 { ink } else { roles.tx3 },
    );
    ui.painter().line_segment(
        [header.left_bottom(), header.right_bottom()],
        egui::Stroke::new(1.0, roles.line),
    );
}

fn paint_columns(
    ui: &mut egui::Ui,
    canvas: f32,
    widths: &[f32; 5],
    counts: [usize; 5],
    theme: &crate::theme::Theme,
    roles: &crate::theme::Roles,
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
        ui.painter().rect_filled(rect, 9.0, roles.raised);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            count,
            egui::FontId::monospace(11.0),
            if counts[index] > 0 { ink } else { roles.tx3 },
        );
        left += widths[index] + 8.0;
    }
    ui.painter().line_segment(
        [header.left_bottom(), header.right_bottom()],
        egui::Stroke::new(1.0, roles.line),
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
                &crate::theme::Roles::of(&crate::theme::Theme::default()),
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
            let raised = crate::theme::Roles::of(&crate::theme::Theme::default()).raised;
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect.height()==18.0 && r.rect.contains(count.pos) && r.fill == raised)), "count badge on the kit's raised surface");
            left += width + 8.0;
        }
        output.drop_without_applying_deltas();
    }
}
