use super::*;

struct Row {
    entry: Entry,
    depth: usize,
    message: Option<String>,
}

fn paint_entry_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: EntryKind,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.0, color);
    match kind {
        EntryKind::Directory => {
            let top = rect.min + egui::vec2(2.0, 4.0);
            let points = vec![
                top,
                top + egui::vec2(6.0, 0.0),
                top + egui::vec2(8.0, 2.0),
                top + egui::vec2(14.0, 2.0),
                top + egui::vec2(14.0, 11.0),
                top + egui::vec2(0.0, 11.0),
                top,
            ];
            painter.add(egui::Shape::line(points, stroke));
        }
        _ => {
            let paper =
                egui::Rect::from_min_size(rect.min + egui::vec2(3.0, 1.0), egui::vec2(10.0, 14.0));
            painter.rect_stroke(paper, 1.0, stroke, egui::StrokeKind::Inside);
            for y in [6.0, 9.0, 12.0] {
                painter.line_segment(
                    [
                        paper.min + egui::vec2(2.0, y - 1.0),
                        paper.min + egui::vec2(8.0, y - 1.0),
                    ],
                    stroke,
                );
            }
        }
    }
}

fn refresh_button(ui: &mut egui::Ui, available: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    let response = response
        .on_hover_text("Refresh")
        .on_hover_cursor(if available {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::NotAllowed
        });
    let visuals = ui.style().interact(&response);
    if available && (response.hovered() || response.is_pointer_button_down_on()) {
        ui.painter().rect_filled(rect, 3.0, visuals.bg_fill);
    }
    let color = if available {
        visuals.fg_stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let stroke = egui::Stroke::new(1.4, color);
    let center = rect.center();
    let points = (0..=20)
        .map(|i| {
            let angle = (45.0 + i as f32 * 14.0).to_radians();
            center + egui::vec2(angle.cos(), angle.sin()) * 6.5
        })
        .collect();
    ui.painter().add(egui::Shape::line(points, stroke));
    let tip = center + egui::vec2(6.5, -3.5);
    ui.painter().add(egui::Shape::line(
        vec![
            tip + egui::vec2(-4.0, 0.0),
            tip,
            tip + egui::vec2(0.0, -4.0),
        ],
        stroke,
    ));
    available && response.clicked()
}

fn rows(state: &mut Explorer, pane: usize) -> (Vec<Row>, Option<Request>, bool) {
    let Some(nav) = state.navigation.get(&pane) else {
        return (Vec::new(), None, false);
    };
    let Some(root) = nav.root.clone().filter(|_| nav.source.is_some()) else {
        return (Vec::new(), None, false);
    };
    enum Visit {
        Directory(PathBuf, usize),
        Entry(Entry, usize),
    }
    let mut stack = vec![Visit::Directory(root, 0)];
    let mut rows = Vec::new();
    let mut pending = None;
    let mut truncated = false;
    let row_limit = nav.row_limit;
    let visible_start = (state.navigation[&pane].scroll / state.row_stride) as usize;
    let visible_end = visible_start.saturating_add(state.viewport_rows);
    while let Some(visit) = stack.pop() {
        if rows.len() >= row_limit {
            truncated = true;
            break;
        }
        match visit {
            Visit::Entry(entry, depth) => {
                if entry.kind == EntryKind::Directory
                    && state.navigation[&pane].expanded.contains(&entry.path)
                {
                    stack.push(Visit::Directory(entry.path.clone(), depth + 1));
                }
                rows.push(Row {
                    entry,
                    depth,
                    message: None,
                });
            }
            Visit::Directory(path, depth) => {
                let key = state.request(pane, path.clone());
                state.clock = state.clock.wrapping_add(1);
                let root = state.navigation[&pane].root.as_ref() == Some(&path);
                let near_view =
                    rows.len() >= visible_start.saturating_sub(1) && rows.len() <= visible_end;
                let Some(cached) = state.cache.get_mut(&key) else {
                    if pending.is_none() && (root || near_view) {
                        pending = Some(key);
                    }
                    rows.push(Row {
                        entry: Entry {
                            path,
                            kind: EntryKind::Other,
                        },
                        depth,
                        message: Some("Loading…".into()),
                    });
                    continue;
                };
                if root || near_view {
                    cached.used = state.clock;
                }
                match &cached.result {
                    Err(error) => rows.push(Row {
                        entry: Entry {
                            path,
                            kind: EntryKind::Other,
                        },
                        depth,
                        message: Some(error.clone()),
                    }),
                    Ok(listing) => {
                        for entry in listing.entries.iter().rev() {
                            stack.push(Visit::Entry(entry.clone(), depth));
                        }
                    }
                }
            }
        }
    }
    (rows, pending, truncated)
}

fn activate(state: &mut Explorer, pane: usize, entry: &Entry) -> Option<(usize, PathBuf)> {
    match entry.kind {
        EntryKind::Directory => {
            let expanded = &mut state.navigation.get_mut(&pane).unwrap().expanded;
            if !expanded.remove(&entry.path) {
                expanded.insert(entry.path.clone());
            }
            None
        }
        EntryKind::File
            if entry
                .path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md")) =>
        {
            if entry.path.to_str().is_none() {
                state.message = "This path cannot be opened as Unicode text.".into();
                None
            } else {
                Some((pane, entry.path.clone()))
            }
        }
        _ => None,
    }
}

pub(super) fn keys(state: &mut Explorer, pane: usize, ctx: &egui::Context) {
    let (mut rows, _, _) = rows(state, pane);
    rows.retain(|row| row.message.is_none());
    if rows.is_empty() {
        return;
    }
    let nav = state.navigation.get_mut(&pane).unwrap();
    let current = rows
        .iter()
        .position(|row| Some(&row.entry.path) == nav.selected.as_ref());
    let key = |key| ctx.input(|i| i.key_pressed(key));
    if key(egui::Key::ArrowDown) || key(egui::Key::ArrowUp) {
        let index = if key(egui::Key::ArrowDown) {
            current.map_or(0, |index| (index + 1).min(rows.len() - 1))
        } else {
            current.unwrap_or(0).saturating_sub(1)
        };
        nav.selected = Some(rows[index].entry.path.clone());
        nav.scroll = (index as f32 * state.row_stride - 48.0).max(0.0);
    }
    if let Some(index) = current {
        let entry = &rows[index].entry;
        if key(egui::Key::ArrowRight) && entry.kind == EntryKind::Directory {
            nav.expanded.insert(entry.path.clone());
        }
        if key(egui::Key::ArrowLeft) {
            nav.expanded.remove(&entry.path);
        }
        if key(egui::Key::Enter) {
            state.activate = true;
        }
    }
}

pub(super) fn show(
    state: &mut Explorer,
    parent: &mut egui::Ui,
    area: egui::Rect,
    target: Option<usize>,
    label: Option<&str>,
    theme: crate::theme::Theme,
) -> Option<(usize, PathBuf)> {
    let rect = state.rect(area);
    let mut open = None;
    egui::Area::new(egui::Id::new("file-explorer"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .movable(false)
        .show(parent.ctx(), |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            ui.set_clip_rect(rect);
            ui.painter()
                .rect_filled(rect, 0.0, theme.panel_background());
            ui.painter().line_segment(
                [rect.right_top(), rect.right_bottom()],
                egui::Stroke::new(1.0, theme.divider()),
            );
            ui.style_mut().visuals.override_text_color = Some(theme.foreground());
            let inner = rect.shrink2(egui::vec2(10.0, 7.0));
            let mut body = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            body.set_clip_rect(inner);
            body.horizontal(|ui| {
                ui.strong("Files");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let available = target.is_some_and(|pane| {
                        state
                            .navigation
                            .get(&pane)
                            .is_some_and(|nav| nav.source.is_some())
                    });
                    if refresh_button(ui, available) {
                        state.navigation.get_mut(&target.unwrap()).unwrap().reload();
                        state.message.clear();
                    }
                });
            });
            let Some(pane) = target else {
                body.label("No active console.");
                return;
            };
            let source_path = state
                .navigation
                .get(&pane)
                .and_then(|nav| nav.source.as_deref())
                .map(display)
                .unwrap_or_else(|| "Local directory unavailable or unsupported.".into());
            body.horizontal(|ui| {
                let source_label = format!("#{} {}", pane, label.unwrap_or("Console"));
                ui.add(egui::Label::new(source_label).truncate())
                    .on_hover_text(source_path.clone());
            });
            let Some(nav) = state.navigation.get_mut(&pane) else {
                return;
            };
            let available = nav.source.is_some();
            let root = nav.root.clone();
            if let Some(root) = &root {
                body.horizontal(|ui| {
                    let (icon, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    paint_entry_icon(ui.painter(), icon, EntryKind::Directory, theme.focus());
                    let name = root
                        .file_name()
                        .map(|name| display(Path::new(name)))
                        .unwrap_or_else(|| display(root));
                    ui.add(egui::Label::new(name).truncate())
                        .on_hover_text(display(root));
                });
            }
            if !available {
                body.label("Local directory unavailable or unsupported.");
                return;
            }
            if nav.source != nav.seeded_source {
                body.label("Console directory changed; refresh to follow it.");
            }
            if !state.message.is_empty() {
                body.label(&state.message);
            }
            let scroll = nav.scroll;
            state.row_stride = 22.0 + body.spacing().item_spacing.y;
            state.viewport_rows =
                ((body.available_height() - 32.0).max(22.0) / state.row_stride).ceil() as usize + 1;
            let (rows, pending, truncated) = rows(state, pane);
            let activate_key = std::mem::take(&mut state.activate);
            let mut action = if activate_key {
                rows.iter()
                    .find(|row| {
                        row.message.is_none()
                            && state.navigation[&pane].selected.as_ref() == Some(&row.entry.path)
                    })
                    .map(|row| row.entry.clone())
            } else {
                None
            };
            let mut toggle = None;
            let mut selected = None;
            let root_loaded = state.navigation[&pane]
                .root
                .as_ref()
                .is_some_and(|path| state.cache.contains_key(&state.request(pane, path.clone())));
            let footer_height = if truncated { 28.0 } else { 0.0 }
                + if state.navigation[&pane].selected.is_some() {
                    28.0
                } else {
                    0.0
                };
            let result = egui::ScrollArea::both()
                .id_salt(("explorer-tree", pane))
                .max_height((body.available_height() - footer_height).max(22.0))
                .vertical_scroll_offset(scroll)
                .auto_shrink([false, false])
                .show_rows(&mut body, 22.0, rows.len(), |ui, range| {
                    for row in &rows[range] {
                        if let Some(message) = &row.message {
                            ui.label(message).on_hover_text(display(&row.entry.path));
                            continue;
                        }
                        let (row_rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 22.0),
                            egui::Sense::click(),
                        );
                        let chosen =
                            state.navigation[&pane].selected.as_ref() == Some(&row.entry.path);
                        if chosen {
                            ui.painter().rect_filled(
                                row_rect,
                                2.0,
                                theme.focus().gamma_multiply(0.22),
                            );
                        } else if response.hovered() {
                            ui.painter().rect_filled(
                                row_rect,
                                2.0,
                                theme.foreground().gamma_multiply(0.07),
                            );
                        }
                        let indent = row.depth as f32 * 12.0;
                        let center_y = row_rect.center().y;
                        if row.entry.kind == EntryKind::Directory {
                            let expanded =
                                state.navigation[&pane].expanded.contains(&row.entry.path);
                            let chevron = egui::Rect::from_min_size(
                                egui::pos2(row_rect.left() + indent + 3.0, center_y - 5.0),
                                egui::vec2(10.0, 10.0),
                            );
                            let points = if expanded {
                                vec![
                                    chevron.left_top(),
                                    chevron.center_bottom(),
                                    chevron.right_top(),
                                ]
                            } else {
                                vec![
                                    chevron.left_top(),
                                    chevron.right_center(),
                                    chevron.left_bottom(),
                                ]
                            };
                            ui.painter().add(egui::Shape::line(
                                points,
                                egui::Stroke::new(1.4, theme.foreground().gamma_multiply(0.8)),
                            ));
                            let toggle_rect = egui::Rect::from_min_size(
                                egui::pos2(row_rect.left() + indent, row_rect.top()),
                                egui::vec2(18.0, row_rect.height()),
                            );
                            if ui
                                .interact(
                                    toggle_rect,
                                    egui::Id::new(("explorer-toggle", pane, &row.entry.path)),
                                    egui::Sense::click(),
                                )
                                .clicked()
                            {
                                toggle = Some(row.entry.path.clone());
                            }
                        }
                        let icon_rect = egui::Rect::from_min_size(
                            egui::pos2(row_rect.left() + indent + 18.0, center_y - 8.0),
                            egui::vec2(16.0, 16.0),
                        );
                        let icon_color = if row.entry.kind == EntryKind::Directory {
                            theme.focus()
                        } else {
                            theme.foreground().gamma_multiply(0.78)
                        };
                        paint_entry_icon(ui.painter(), icon_rect, row.entry.kind, icon_color);
                        let name =
                            display(Path::new(row.entry.path.file_name().unwrap_or_default()));
                        let suffix = if row.entry.kind == EntryKind::Link {
                            " ↗"
                        } else {
                            ""
                        };
                        ui.painter().text(
                            egui::pos2(icon_rect.right() + 5.0, center_y),
                            egui::Align2::LEFT_CENTER,
                            format!("{name}{suffix}"),
                            egui::TextStyle::Body.resolve(ui.style()),
                            if chosen {
                                theme.foreground()
                            } else {
                                theme.foreground().gamma_multiply(0.9)
                            },
                        );
                        response.clone().on_hover_text(display(&row.entry.path));
                        if response.clicked() {
                            selected = Some(row.entry.path.clone());
                            if row.entry.kind == EntryKind::File {
                                action = Some(row.entry.clone());
                            }
                        }
                        if response.double_clicked()
                            && row.entry.kind == EntryKind::Directory
                            && toggle.as_ref() != Some(&row.entry.path)
                        {
                            action = Some(row.entry.clone());
                        }
                    }
                });
            let incomplete = rows
                .iter()
                .any(|row| row.message.as_deref() == Some("Loading…"));
            let scroll_gesture =
                parent.input(|i| i.pointer.any_down() || i.smooth_scroll_delta != egui::Vec2::ZERO);
            if root_loaded && (!incomplete || scroll_gesture) {
                state.navigation.get_mut(&pane).unwrap().scroll = result.state.offset.y;
            }
            if let Some(path) = toggle {
                let expanded = &mut state.navigation.get_mut(&pane).unwrap().expanded;
                if !expanded.remove(&path) {
                    expanded.insert(path);
                }
            }
            if let Some(path) = selected {
                state.navigation.get_mut(&pane).unwrap().selected = Some(path);
            }
            if truncated {
                body.horizontal(|ui| {
                    ui.label("More entries hidden.");
                    if ui.small_button("Show more").clicked() {
                        let nav = state.navigation.get_mut(&pane).unwrap();
                        nav.row_limit = nav.row_limit.saturating_add(ROW_LIMIT);
                        ui.ctx().request_repaint();
                    }
                });
            }
            if let Some(path) = state.navigation[&pane].selected.clone() {
                body.horizontal_wrapped(|ui| {
                    if ui.small_button("Copy").on_hover_text("Copy path").clicked() {
                        if let Some(path) = path.to_str() {
                            ui.ctx().copy_text(path.into());
                        } else {
                            state.message =
                                "This path is unavailable for copying as Unicode text.".into();
                        }
                    }
                    if ui.small_button("Open").on_hover_text("Open").clicked() {
                        action = rows
                            .iter()
                            .find(|row| row.entry.path == path)
                            .map(|row| row.entry.clone());
                    }
                });
            }
            if state.loading.is_none()
                && let Some(request) = pending
            {
                state.loader.submit(request.clone(), parent.ctx());
                state.loading = Some(request);
            }
            if let Some(entry) = action {
                open = activate(state, pane, &entry);
            }
        });
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(state: &mut Explorer, path: &str, entries: Vec<Entry>) {
        let key = state.request(1, path.into());
        state.cache.insert(
            key,
            Cached {
                result: Ok(Listing { entries, bytes: 0 }),
                used: 0,
            },
        );
    }

    fn entry(path: &str, kind: EntryKind) -> Entry {
        Entry {
            path: path.into(),
            kind,
        }
    }

    #[test]
    fn flattening_keeps_siblings_after_nested_expansion_and_bounds_rows() {
        let mut state = Explorer::default();
        state.reconcile(&[(1, Some("/root".into()))]);
        listing(
            &mut state,
            "/root",
            vec![
                entry("/root/a", EntryKind::Directory),
                entry("/root/b.md", EntryKind::File),
            ],
        );
        listing(
            &mut state,
            "/root/a",
            vec![entry("/root/a/c.md", EntryKind::File)],
        );
        state
            .navigation
            .get_mut(&1)
            .unwrap()
            .expanded
            .insert("/root/a".into());
        let (visible, _, _) = rows(&mut state, 1);
        assert_eq!(
            visible
                .iter()
                .map(|row| row.entry.path.clone())
                .collect::<Vec<_>>(),
            ["/root/a", "/root/a/c.md", "/root/b.md"].map(PathBuf::from)
        );
        assert_eq!(
            visible.iter().map(|row| row.depth).collect::<Vec<_>>(),
            [0, 1, 0]
        );
        listing(
            &mut state,
            "/root",
            (0..ROW_LIMIT + 1)
                .map(|i| entry(&format!("/root/{i}"), EntryKind::File))
                .collect(),
        );
        let (visible, _, truncated) = rows(&mut state, 1);
        assert_eq!(visible.len(), ROW_LIMIT);
        assert!(truncated);
    }

    #[test]
    fn activation_is_explicit_and_never_follows_a_symlink() {
        let mut state = Explorer::default();
        state.reconcile(&[(1, Some("/root".into()))]);
        assert!(activate(&mut state, 1, &entry("/root/link.md", EntryKind::Link)).is_none());
        assert!(state.message.is_empty());
        assert!(activate(&mut state, 1, &entry("/root/code.rs", EntryKind::File)).is_none());
        assert_eq!(
            activate(&mut state, 1, &entry("/root/README.MD", EntryKind::File)),
            Some((1, "/root/README.MD".into()))
        );
        activate(&mut state, 1, &entry("/root/src", EntryKind::Directory));
        assert_eq!(state.navigation[&1].root, Some("/root".into()));
        assert!(
            state.navigation[&1]
                .expanded
                .contains(Path::new("/root/src"))
        );
        activate(&mut state, 1, &entry("/root/src", EntryKind::Directory));
        assert!(
            !state.navigation[&1]
                .expanded
                .contains(Path::new("/root/src"))
        );
    }

    #[test]
    fn failed_children_have_inline_errors_without_removing_siblings() {
        let mut state = Explorer::default();
        state.reconcile(&[(1, Some("/root".into()))]);
        listing(
            &mut state,
            "/root",
            vec![
                entry("/root/a", EntryKind::Directory),
                entry("/root/b.md", EntryKind::File),
            ],
        );
        let key = state.request(1, "/root/a".into());
        state.cache.insert(
            key,
            Cached {
                result: Err("Directory unavailable.".into()),
                used: 0,
            },
        );
        state
            .navigation
            .get_mut(&1)
            .unwrap()
            .expanded
            .insert("/root/a".into());
        let (visible, pending, _) = rows(&mut state, 1);
        assert!(pending.is_none());
        assert!(
            visible
                .iter()
                .any(|row| row.message.as_deref() == Some("Directory unavailable."))
        );
        assert_eq!(
            visible.last().unwrap().entry.path,
            PathBuf::from("/root/b.md")
        );
    }
}
