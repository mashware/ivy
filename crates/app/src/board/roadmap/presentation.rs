//! Reference roadmap layout, independent of producer storage and authority.
use super::*;

pub(super) fn paint_project_contents(
    ui: &mut egui::Ui,
    section: &Section,
    reader: &mut Reader,
    window: (usize, usize),
    surface: &Surface<'_>,
) -> Option<usize> {
    let theme = surface.theme;
    let key = project_key(section);
    let mut selected = None;
    let resolved = section.roadmap.as_ref();
    let plan = resolved.and_then(|r| r.plan.as_deref());
    let fallback = fallback(section);
    let mut is_open = reader.projects.get(&key).copied().unwrap_or(false);
    let total: usize = plan.map_or(0, |p| p.milestones.iter().map(|m| m.items.len()).sum());
    let completed: usize = plan.map_or(0, |p| p.milestones.iter().map(|m| progress(m).0).sum());
    let positions: Vec<_> = section
        .entries
        .iter()
        .map(|e| {
            plan.map_or(Placement::Off, |p| {
                placement(e.snapshot.document.as_deref(), p)
            })
        })
        .collect();
    let attached: Vec<_> = plan
        .map(|p| {
            (0..p.milestones.len())
                .map(|m| {
                    positions.iter().any(
                        |p| matches!(p, Placement::Item(n, _) | Placement::Milestone(n) if *n == m),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let active_ids: Vec<_> = plan
        .map(|p| {
            p.milestones
                .iter()
                .enumerate()
                .filter(|(i, m)| started(m, attached[*i]))
                .filter_map(|(_, m)| m.id.as_deref())
                .collect()
        })
        .unwrap_or_default();
    egui::Frame::new()
        .fill(theme.divider())
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let offset = (ui.clip_rect().left() - ui.max_rect().left()).max(0.0);
                ui.add_space(offset);
                ui.label(
                    egui::RichText::new(crate::board::cards::clean(section.name()))
                        .size(13.0)
                        .strong(),
                );
                ui.weak(format!("{} consoles", section.entries.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if resolved.is_some() || !fallback.is_empty() {
                        let notice = resolved.map_or_else(
                            || "Producer section headings; no plan published".into(),
                            |r| match &r.error {
                                Some(e) => format!("{}\n{e}", r.source),
                                None => r.source.clone(),
                            },
                        );
                        if crate::board::style::disclosure(ui, is_open, theme)
                            .on_hover_text(crate::board::cards::clean(&notice))
                            .clicked()
                        {
                            is_open = !is_open;
                            reader.projects.insert(key.clone(), is_open);
                        }
                        if let Some(plan) = plan {
                            if !active_ids.is_empty() {
                                ui.weak(format!(
                                    "· now {}",
                                    crate::board::cards::clean(&active_ids.join(", "))
                                ));
                            }
                            ui.weak(format!("{completed}/{total}"));
                            crate::board::style::progress(ui, completed, total, 70.0, theme);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(crate::board::cards::clean(
                                        plan.title.as_deref().unwrap_or("Roadmap"),
                                    ))
                                    .color(crate::board::style::dim(theme)),
                                )
                                .truncate()
                                .halign(egui::Align::Max),
                            );
                        } else {
                            ui.weak(format!("no plan · {} blocks", fallback.len()));
                        }
                    }
                });
            });
        });
    if !is_open {
        return selected;
    }
    egui::Frame::new().inner_margin(egui::Margin::symmetric(12,10)).show(ui, |ui| {
    if let Some(error) = resolved.and_then(|r| r.error.as_deref()) {
        ui.colored_label(
            theme.color(term_core::TermColor::Indexed(1)),
            crate::board::cards::clean(error),
        );
    }
    let Some(plan) = plan else {
        ui.horizontal_wrapped(|ui| {
            for (title, line) in fallback {
                egui::Frame::new()
                    .stroke(egui::Stroke::new(1.0, theme.divider()))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.weak(format!("{} ·", crate::board::cards::clean(title)));
                            ui.strong(crate::board::cards::clean(&line.text));
                            if let Some(summary) = &line.summary {
                                ui.weak(crate::board::cards::clean(summary));
                            }
                        });
                    });
            }
        });
        ui.small("No plan published: these are the block headers of this project's consoles, in no particular order.");
        return selected;
    };
    let prefix = plan
        .milestones
        .iter()
        .enumerate()
        .take_while(|(i, m)| done(m) && !attached[*i])
        .count();
    let ms_done = plan.milestones.iter().filter(|m| done(m)).count();
    ui.horizontal(|ui| {
        if prefix > 0 {
            let ids: Vec<_> = plan.milestones[..prefix]
                .iter()
                .filter_map(|m| m.id.as_deref())
                .collect();
            let n: usize = plan.milestones[..prefix]
                .iter()
                .map(|m| m.items.len())
                .sum();
            crate::board::style::done_summary(ui,&format!("{} · {n} deliverables",crate::board::cards::clean(&ids.join(" · "))),theme);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.weak(format!(
                "{ms_done}/{} milestones · {completed}/{total} deliverables",
                plan.milestones.len()
            ));
        });
    });
    let range = super::window(plan, window.0, window.1);
    let all = reader.all_milestones.contains(&key);
    let detailed: Vec<_> = plan
        .milestones
        .iter()
        .enumerate()
        .filter(|(i, m)| {
            (all || *i >= prefix)
                && (all
                    || started(m, attached[*i])
                    || reader.milestones.contains(&milestone_key(&key, m, *i)))
        })
        .map(|(i, _)| i)
        .collect();
    let detailed_set: HashSet<_> = detailed.iter().copied().collect();
    let max_columns = ((ui.available_width() + 8.0) / 448.0).floor().max(1.0) as usize;
    for row in detailed.chunks(max_columns) {
        let width = ((ui.available_width() - 8.0 * (row.len().saturating_sub(1)) as f32)
            / row.len() as f32)
            .max(1.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for &mi in row {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        paint_milestone(
                            ui,
                            section,
                            plan,
                            mi,
                            &positions,
                            &key,
                            reader,
                            &mut selected,
                            surface,
                        );
                    },
                );
            }
        });
        ui.add_space(8.0);
    }
    ui.horizontal_wrapped(|ui| {
        for (mi, m) in plan.milestones.iter().enumerate() {
            if detailed_set.contains(&mi) || (!all && (mi < prefix || !range.contains(&mi))) {
                continue;
            }
            let (d, n) = progress(m);
            let deps = if m.depends_on.is_empty() {
                String::new()
            } else {
                format!(" · after {}", crate::board::cards::clean(&m.depends_on.join(", ")))
            };
            let text = format!(
                "{}{} {} {d}/{n}{deps}",
                if done(m) { "✓ " } else { "" },
                crate::board::cards::clean(m.id.as_deref().unwrap_or("")),
                crate::board::cards::clean(&m.title)
            );
            let mk = milestone_key(&key, m, mi);
            let id = egui::Id::new(("board-roadmap-later", ui.ctx().viewport_id(), &mk));
            if ui
                .scope_builder(egui::UiBuilder::new().id(id), |ui| {
                    crate::board::style::tag(
                        ui,
                        text,
                        if done(m) {
                            theme.focus()
                        } else {
                            theme.foreground()
                        },
                        theme,
                    )
                    .clicked()
                })
                .inner
            {
                reader.milestones.insert(mk);
            }
        }
    });
    if (prefix > 0 || range.len() < plan.milestones.len() || all)
        && ui
            .small_button(if all {
                "Show less milestones"
            } else {
                "Show all milestones"
            })
            .clicked()
    {
        if all {
            reader.all_milestones.remove(&key);
        } else {
            reader.all_milestones.insert(key.clone());
        }
    }
    if section
        .entries
        .iter()
        .zip(&positions)
        .any(|(e, p)| e.snapshot.document.is_some() && *p == Placement::Off)
    {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Off the plan:");
            chips(
                ui,
                section,
                &positions,
                &Placement::Off,
                &mut selected,
                surface,
            );
        });
    }
    if !plan.outside.is_empty() {
        let pending = plan
            .outside
            .iter()
            .filter(|i| i.mark != Some(Mark::Done))
            .count();
        ui.horizontal_wrapped(|ui| {
            ui.weak(format!("Outside the code · {pending} pending:"));
            for item in &plan.outside {
                let text=crate::board::cards::clean(&item.text);
                if item.mark==Some(Mark::Done) {
                    crate::board::style::completed_tag(ui,&text,crate::board::style::dim(theme),theme);
                } else {
                    crate::board::style::tag(ui,text,theme.foreground(),theme);
                }

            }
        });
    }
    if let Some(source) = &plan.source {
        ui.small(format!("From {}", crate::board::cards::clean(source)));
    }
    selected
    }).inner
}

#[allow(clippy::too_many_arguments)]
fn paint_milestone(
    ui: &mut egui::Ui,
    section: &Section,
    plan: &Plan,
    mi: usize,
    positions: &[Placement],
    key: &str,
    reader: &mut Reader,
    selected: &mut Option<usize>,
    surface: &Surface<'_>,
) {
    let theme = surface.theme;
    let m = &plan.milestones[mi];
    let mk = milestone_key(key, m, mi);
    let id = egui::Id::new(("board-roadmap-milestone", ui.ctx().viewport_id(), &mk));
    ui.scope_builder(egui::UiBuilder::new().id(id), |ui| {
        egui::Frame::new()
            .fill(theme.panel_background())
            .stroke(egui::Stroke::new(1.0, theme.focus()))
            .corner_radius(8)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let (d, n) = progress(m);
                ui.horizontal(|ui| {
                    if let Some(id) = &m.id {
                        ui.strong(crate::board::cards::clean(id));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(format!("{d}/{n}"));
                        crate::board::style::progress(ui, d, n, 60.0, theme);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(crate::board::cards::clean(&m.title)).strong(),
                            )
                            .truncate()
                            .halign(egui::Align::Min),
                        )
                        .on_hover_text(if m.depends_on.is_empty() {
                            crate::board::cards::clean(&m.title)
                        } else {
                            format!(
                                "{}\nDepends on: {}",
                                crate::board::cards::clean(&m.title),
                                crate::board::cards::clean(&m.depends_on.join(", "))
                            )
                        });
                    });
                });
                if let Some(done_when) = &m.done_when {
                    ui.small(format!(
                        "Done when: {}",
                        crate::board::cards::clean(done_when)
                    ));
                }
                let attached: Vec<_> = (0..m.items.len())
                    .map(|i| positions.contains(&Placement::Item(mi, i)))
                    .collect();
                let expanded = reader.milestones.contains(&mk);
                let visible = visible_items(m, &attached, expanded);
                if !expanded && d > 0 {
                    crate::board::style::done_summary(ui, &format!("{d} done"), theme);
                }
                for &i in &visible {
                    let item = &m.items[i];
                    let on: Vec<_> = section
                        .entries
                        .iter()
                        .zip(positions)
                        .filter(|(_, p)| **p == Placement::Item(mi, i))
                        .map(|(e, _)| e)
                        .collect();
                    let badge_width: f32 = on
                        .iter()
                        .map(|entry| {
                            let title = crate::board::cards::clean(
                                crate::board::cards::title(entry)
                                    .split(" · ")
                                    .next()
                                    .unwrap_or(""),
                            );
                            ui.painter()
                                .layout_no_wrap(
                                    title,
                                    egui::FontId::monospace(11.0),
                                    theme.foreground(),
                                )
                                .size()
                                .x
                                + 26.0
                        })
                        .sum();
                    let reference = item.reference.as_deref().map(crate::board::cards::clean);
                    let ref_width = reference.as_ref().map_or(0.0, |reference| {
                        ui.painter()
                            .layout_no_wrap(
                                reference.clone(),
                                egui::FontId::monospace(12.0),
                                theme.foreground(),
                            )
                            .size()
                            .x
                    });
                    let gaps = 6.0 * (1 + on.len() + usize::from(reference.is_some())) as f32;
                    let text_width =
                        (ui.available_width() - 12.0 - ref_width - badge_width - gaps).max(16.0);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.spacing_mut().interact_size.y = 16.0;
                        let mark_color = match item.mark {
                            Some(Mark::Done | Mark::Current) => theme.focus(),
                            Some(Mark::Wait | Mark::Block) => crate::board::cards::color(
                                crate::board::observation::Column::Wait,
                                theme,
                            ),
                            _ => crate::board::style::dim(theme),
                        };
                        let (mark_rect, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 16.0), egui::Sense::hover());
                        crate::panel::body::paint_mark(
                            ui.painter(),
                            mark_rect,
                            item.mark.unwrap_or(Mark::Pending),
                            mark_color,
                            egui::FontId::monospace(12.0),
                        );
                        ui.add_sized(
                            egui::vec2(text_width, 0.0),
                            egui::Label::new(
                                egui::RichText::new(crate::board::cards::clean(&item.text)).color(
                                    if item.mark == Some(Mark::Done) {
                                        crate::board::style::dim(theme)
                                    } else {
                                        theme.foreground()
                                    },
                                ),
                            )
                            .wrap()
                            .halign(egui::Align::Min),
                        );
                        if let Some(reference) = &reference {
                            ui.weak(reference);
                        }
                        for entry in on {
                            chip(ui, entry, selected, surface);
                        }
                    });
                }

                if (visible.len() < m.items.len() || expanded)
                    && ui
                        .add(
                            egui::Label::new(
                                egui::RichText::new(if expanded {
                                    "Show less".into()
                                } else {
                                    format!("Show all {}", m.items.len())
                                })
                                .size(11.0)
                                .color(theme.focus()),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .clicked()
                {
                    if expanded {
                        reader.milestones.remove(&mk);
                    } else {
                        reader.milestones.insert(mk);
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    chips(
                        ui,
                        section,
                        positions,
                        &Placement::Milestone(mi),
                        selected,
                        surface,
                    )
                });
            });
    });
}
