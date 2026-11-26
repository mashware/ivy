use super::{
    CardPainter,
    projection::{Class, Projection},
    sections::{self, Group},
    state::{Identity, Key, Reader, Target},
    strip,
};
use crate::{
    board::{
        filter::Filter,
        model::{Entry, Section},
        observation::Column,
    },
    theme::Roles,
};
use eframe::egui::{self, Rect, Stroke, pos2, vec2};
use panel_contract::plan::Plan;

struct ColumnSpec {
    index: Option<usize>,
    group: usize,
    full: bool,
    collapsed: bool,
    card: bool,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::board) fn paint(
    ui: &mut egui::Ui,
    section: &Section,
    plan: &Plan,
    roles: &Roles,
    reader: &mut Reader,
    filter: &Filter,
    column: &impl Fn(&Entry) -> Column,
    paint_card: &mut CardPainter,
) -> usize {
    let project = super::project_key(section);
    let placed = super::place(&section.entries, plan);
    let projection = Projection::new(plan, &placed, column);
    let classes: Vec<_> = projection
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| (strip::key(&project, plan, i).milestone, *c))
        .collect();
    reader.reconcile(&project, &classes);
    for diagnostic in reader.new_diagnostics(&project, plan, &projection.diagnostics) {
        eprintln!("Board: {diagnostic}");
    }
    strip::paint(ui, &project, plan, &projection, reader, roles);
    let p = reader.project(&project);
    let show_all = p.show_all;
    let mut specs = Vec::new();
    for (i, class) in projection.classes.iter().enumerate() {
        let id = &classes[i].0;
        let opened = p.opened.contains(id);
        let group = if class.active() {
            Some(0)
        } else if *class == Class::Ready {
            Some(1)
        } else if opened {
            Some(3)
        } else {
            None
        };
        let visible = show_all
            || (group.is_some() && (!filter.claiming_only || matches!(class, Class::Claim(_))));
        if visible {
            specs.push(ColumnSpec {
                index: Some(i),
                group: if show_all { 0 } else { group.unwrap_or(3) },
                full: show_all || p.full.contains(id),
                collapsed: !show_all && *class == Class::Ready && !p.full.contains(id),
                card: !placed.milestones[i].loose.is_empty()
                    || placed.milestones[i].items.iter().any(|x| !x.is_empty()),
            });
        }
    }
    let off_visible = if filter.claiming_only && !show_all {
        placed
            .off
            .iter()
            .any(|e| crate::board::filter::claims(column(e)))
    } else {
        !placed.off.is_empty() || !plan.outside.is_empty()
    };
    if off_visible {
        specs.push(ColumnSpec {
            index: None,
            group: if show_all { 0 } else { 2 },
            full: show_all || p.full.contains(&Identity::Outside),
            collapsed: false,
            card: !placed.off.is_empty(),
        });
    }
    if !show_all {
        specs.sort_by_key(|s| s.group);
    }
    reader.dividers.retain(|d| d.project != project);
    if !show_all {
        let populated: Vec<_> = specs.iter().map(|s| Group::ALL[s.group]).collect();
        let p = reader.project(&project);
        if p.sections
            .maximized
            .is_some_and(|g| !populated.contains(&g))
        {
            p.sections.maximized = None;
        }
        specs.retain(|s| p.sections.shows(Group::ALL[s.group]));
        sections::recovery(ui, &project, reader, roles);
    }
    if projection.classes.iter().all(|c| *c == Class::Done) && !show_all {
        ui.label(
            egui::RichText::new("All milestones done · Show all to review")
                .size(12.0)
                .color(roles.tx2),
        );
    }
    if plan.milestones.is_empty() {
        ui.weak("No roadmap items");
    }
    if specs.is_empty() {
        return 1;
    }
    let separators = specs
        .windows(2)
        .filter(|s| s[0].group != s[1].group)
        .count();
    let sizes: Vec<_> = specs.iter().map(|s| (s.collapsed, s.card)).collect();
    let widths = if show_all {
        super::projection::widths(ui.available_width(), &sizes, separators)
    } else {
        let columns: Vec<_> = specs
            .iter()
            .map(|s| (s.group, s.collapsed, s.card))
            .collect();
        sections::widths(
            ui.available_width(),
            &columns,
            &reader.project(&project).sections,
        )
    };
    let mut group_widths = [0.0; 4];
    let mut group_minima = [0.0; 4];
    let mut group_counts = [0; 4];
    for (spec, width) in specs.iter().zip(&widths) {
        if group_counts[spec.group] > 0 {
            group_widths[spec.group] += 20.0;
            group_minima[spec.group] += 20.0;
        }
        group_counts[spec.group] += 1;
        group_widths[spec.group] += width;
        group_minima[spec.group] += if spec.collapsed { 190.0 } else { 240.0 };
    }
    let maximized = reader.project(&project).sections.maximized.is_some();
    let total = widths.iter().sum::<f32>()
        + 20.0 * specs.len().saturating_sub(1) as f32
        + 4.0 * separators as f32;
    let viewport_width = ui.available_width();
    let reveal = reader.reveal.clone().filter(|k| k.project == project);
    let mut rows = 1;
    let output = egui::ScrollArea::horizontal()
        .id_salt(("milestone-columns", &project))
        .animated(false)
        .auto_shrink([false, true])
        .max_width(viewport_width)
        .show(ui, |ui| {
            ui.set_min_width(total);
            let mut boundaries = Vec::new();
            let mut captions = Vec::new();
            let row = ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (n, (spec, width)) in specs.iter().zip(&widths).enumerate() {
                    if n > 0 {
                        if specs[n - 1].group != spec.group {
                            let at = ui.next_widget_position();
                            boundaries.push((at.x + 12.0, specs[n - 1].group, spec.group));
                            ui.add_space(24.0);
                        } else {
                            ui.add_space(20.0);
                        }
                    }
                    let start = ui.next_widget_position();
                    let bounds =
                        Rect::from_min_max(start, pos2(start.x + width, ui.clip_rect().bottom()));
                    let k = spec
                        .index
                        .map(|i| strip::key(&project, plan, i))
                        .unwrap_or_else(|| Key {
                            project: project.clone(),
                            milestone: Identity::Outside,
                        });
                    if !show_all && (n == 0 || specs[n - 1].group != spec.group) {
                        captions.push((
                            Rect::from_min_size(start, vec2(group_widths[spec.group], 24.0)),
                            spec.group,
                        ));
                    }
                    let response = ui.allocate_ui_with_layout(
                        vec2(*width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(*width);
                            ui.set_clip_rect(bounds.intersect(ui.clip_rect()));
                            ui.allocate_exact_size(vec2(*width, 26.0), egui::Sense::hover());
                            let mut row = 0;
                            if let Some(i) = spec.index {
                                let m = &plan.milestones[i];
                                let class = projection.classes[i];
                                super::details::header(
                                    ui,
                                    m,
                                    plan,
                                    class,
                                    &projection.missing[i],
                                    &k,
                                    spec.full,
                                    spec.collapsed,
                                    spec.group == 3 && !show_all,
                                    reader,
                                    roles,
                                );
                                for entry in &placed.milestones[i].loose {
                                    card(
                                        ui, entry, &k, n, &mut row, *width, reader, filter, column,
                                        roles, paint_card,
                                    );
                                }
                                let attached: Vec<_> = placed.milestones[i]
                                    .items
                                    .iter()
                                    .map(|x| !x.is_empty())
                                    .collect();
                                let visible = super::projection::visible(
                                    m,
                                    &attached,
                                    spec.full,
                                    spec.collapsed,
                                );
                                for &item in &visible {
                                    let claim = super::projection::claim(
                                        placed.milestones[i].items[item].iter().map(|e| column(e)),
                                    );
                                    if spec.full {
                                        super::details::item_row(
                                            ui,
                                            &m.items[item].text,
                                            m.items[item].mark,
                                            claim,
                                            &k,
                                            item,
                                            reader,
                                            roles,
                                        );
                                    }
                                    for entry in &placed.milestones[i].items[item] {
                                        card(
                                            ui, entry, &k, n, &mut row, *width, reader, filter,
                                            column, roles, paint_card,
                                        );
                                    }
                                }
                            } else {
                                super::details::off_header(
                                    ui,
                                    placed.off.len() + plan.outside.len(),
                                    &k,
                                    spec.full,
                                    reader,
                                    roles,
                                );
                                for entry in &placed.off {
                                    card(
                                        ui, entry, &k, n, &mut row, *width, reader, filter, column,
                                        roles, paint_card,
                                    );
                                }
                                let visible = super::projection::visible_items(
                                    &plan.outside,
                                    &[],
                                    spec.full,
                                    false,
                                );
                                for &i in &visible {
                                    let item = &plan.outside[i];
                                    super::details::item_row(
                                        ui, &item.text, item.mark, None, &k, i, reader, roles,
                                    );
                                }
                            }
                            rows = rows.max(row + 1);
                        },
                    );
                    if reveal.as_ref() == Some(&k) {
                        ui.scroll_to_rect(response.response.rect, Some(egui::Align::Center));
                    }
                }
            });
            for (rect, group) in captions {
                sections::caption(
                    ui,
                    rect,
                    &project,
                    Group::ALL[group],
                    maximized,
                    reader,
                    roles,
                );
            }
            for (x, left, right) in boundaries {
                let height = row.response.rect.height();
                let line =
                    Rect::from_min_size(pos2(x - 4.0, row.response.rect.top()), vec2(8.0, height));
                let hit = line.intersect(ui.clip_rect());
                let hovered = ui.rect_contains_pointer(hit);
                ui.painter().vline(
                    x,
                    line.y_range(),
                    Stroke::new(
                        if hovered { 2.0 } else { 1.0 },
                        if hovered { roles.accent } else { roles.line },
                    ),
                );
                if !maximized && !show_all {
                    // ScrollArea must see the same drag owner as Board, including on touch devices.
                    ui.interact(
                        line,
                        ui.id().with(("section-divider", &project, left, right)),
                        egui::Sense::drag(),
                    );
                    if hovered || reader.resizing() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    reader.dividers.push(sections::Divider {
                        rect: hit,
                        project: project.clone(),
                        groups: [Group::ALL[left], Group::ALL[right]],
                        widths: [group_widths[left], group_widths[right]],
                        minima: [group_minima[left], group_minima[right]],
                    });
                }
            }
        });
    if reveal.is_some() {
        reader.reveal = None;
    }
    // The strip never moves; only this row receives the horizontal cut-edge veil.
    if total > viewport_width && output.state.offset.x + viewport_width < total - 1.0 {
        let rect = Rect::from_min_size(
            output.inner_rect.right_top() - vec2(6.0, 0.0),
            vec2(6.0, output.inner_rect.height()),
        );
        ui.painter()
            .rect_filled(rect, 0, roles.base.gamma_multiply(0.65));
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn card(
    ui: &mut egui::Ui,
    entry: &Entry,
    _key: &Key,
    col: usize,
    row: &mut usize,
    width: f32,
    reader: &mut Reader,
    filter: &Filter,
    column: &impl Fn(&Entry) -> Column,
    roles: &Roles,
    paint: &mut CardPainter,
) {
    if !crate::board::filter::accepts(entry, filter, column(entry)) {
        return;
    }
    ui.add_space(2.0);
    let response = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(20.0);
        ui.allocate_ui_with_layout(
            vec2((width - 20.0).max(1.0), 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width((width - 20.0).max(1.0));
                paint(ui, entry, *row, col, (width - 20.0).max(1.0));
            },
        )
    });
    let mut rect = response.response.rect;
    rect.min.x += 20.0;
    if reader.reveal_pane == Some(entry.pane)
        || reader.reveal_focus == Some(Target::Card(entry.pane))
    {
        ui.scroll_to_rect(rect, None);
        reader.reveal_pane = None;
        reader.reveal_focus = None;
    }
    reader.targets.push((Target::Card(entry.pane), rect));
    if reader.focus == Some(Target::Card(entry.pane)) {
        ui.painter().rect_stroke(
            rect,
            8,
            Stroke::new(1.0, roles.accent),
            egui::StrokeKind::Inside,
        );
    }
    *row += 1;
    ui.add_space(6.0);
}
