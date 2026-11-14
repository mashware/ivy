use super::*;

struct Row {
    entry: Entry,
    depth: usize,
    message: Option<String>,
}

fn rows(state: &mut Explorer, pane: usize) -> (Vec<Row>, Option<Request>, Vec<String>) {
    let Some(nav) = state.navigation.get(&pane) else {
        return (Vec::new(), None, Vec::new());
    };
    let Some(root) = nav.root.clone().filter(|_| nav.source.is_some()) else {
        return (Vec::new(), None, Vec::new());
    };
    enum Visit {
        Directory(PathBuf, usize),
        Entry(Entry, usize),
    }
    let mut stack = vec![Visit::Directory(root, 0)];
    let mut rows = Vec::new();
    let mut pending = None;
    let mut messages = Vec::new();
    let visible_start = (state.navigation[&pane].scroll / 24.0) as usize;
    let visible_end = visible_start.saturating_add(40);
    while let Some(visit) = stack.pop() {
        if rows.len() >= ROW_LIMIT {
            messages.push("More entries hidden; collapse folders or open a directory.".into());
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
    (rows, pending, messages)
}

fn activate(state: &mut Explorer, pane: usize, entry: &Entry) -> Option<(usize, PathBuf)> {
    match entry.kind {
        EntryKind::Directory => {
            state
                .navigation
                .get_mut(&pane)
                .unwrap()
                .enter(entry.path.clone());
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
        _ => {
            state.message = "Preview unavailable for this file type.".into();
            None
        }
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
        nav.scroll = (index as f32 * 24.0 - 48.0).max(0.0);
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
            let inner = rect.shrink(8.0);
            let mut body = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            body.set_clip_rect(inner);
            body.horizontal_wrapped(|ui| {
                ui.strong("Files");
                if ui.button("Hide").clicked() {
                    state.hide();
                }
            });
            let Some(pane) = target else {
                body.label("No active console.");
                return;
            };
            body.add(
                egui::Label::new(format!(
                    "{}: {} (#{pane})",
                    if state.pinned.is_some() {
                        "Pinned"
                    } else {
                        "Following"
                    },
                    label.unwrap_or("Console")
                ))
                .truncate(),
            )
            .on_hover_text(
                state
                    .navigation
                    .get(&pane)
                    .and_then(|nav| nav.source.as_deref())
                    .map(display)
                    .unwrap_or_else(|| "Local directory unavailable or unsupported.".into()),
            );
            body.horizontal_wrapped(|ui| {
                if state.pinned.is_some() {
                    if ui.button("Follow active pane").clicked() {
                        state.pinned = None;
                    }
                } else if ui.button("Pin this console").clicked() {
                    state.pinned = Some(pane);
                }
            });
            let Some(nav) = state.navigation.get_mut(&pane) else {
                return;
            };
            let available = nav.source.is_some();
            let root = nav.root.clone();
            if let Some(root) = &root {
                body.add(egui::Label::new(display(root)).truncate())
                    .on_hover_text(display(root));
            }
            body.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        available && root.as_ref().and_then(|path| path.parent()).is_some(),
                        egui::Button::new("Up"),
                    )
                    .clicked()
                {
                    nav.enter(root.as_ref().unwrap().parent().unwrap().to_path_buf());
                }
                if ui
                    .add_enabled(available, egui::Button::new("Use console directory"))
                    .clicked()
                {
                    nav.enter(nav.source.clone().unwrap());
                }
                if ui
                    .add_enabled(available, egui::Button::new("Refresh"))
                    .clicked()
                {
                    nav.refresh();
                }
            });
            if !available {
                body.label("Local directory unavailable or unsupported.");
                return;
            }
            if nav.source != nav.root {
                body.label("Console directory changed");
            }
            if !state.message.is_empty() {
                body.label(&state.message);
            }
            let scroll = nav.scroll;
            let (rows, pending, messages) = rows(state, pane);
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
            let result = egui::ScrollArea::both()
                .id_salt(("explorer-tree", pane))
                .max_height((body.available_height() - 110.0).max(24.0))
                .vertical_scroll_offset(scroll)
                .auto_shrink([false, false])
                .show_rows(&mut body, 24.0, rows.len(), |ui, range| {
                    for row in &rows[range] {
                        if let Some(message) = &row.message {
                            ui.label(message).on_hover_text(display(&row.entry.path));
                            continue;
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(row.depth as f32 * 14.0);
                            if row.entry.kind == EntryKind::Directory {
                                let expanded =
                                    state.navigation[&pane].expanded.contains(&row.entry.path);
                                if ui.small_button(if expanded { "v" } else { ">" }).clicked() {
                                    toggle = Some(row.entry.path.clone());
                                }
                            } else {
                                ui.add_space(18.0);
                            }
                            let chosen =
                                state.navigation[&pane].selected.as_ref() == Some(&row.entry.path);
                            let suffix = if row.entry.kind == EntryKind::Link {
                                " ↗"
                            } else {
                                ""
                            };
                            let name =
                                display(Path::new(row.entry.path.file_name().unwrap_or_default()));
                            let response = ui
                                .selectable_label(chosen, format!("{name}{suffix}"))
                                .on_hover_text(display(&row.entry.path));
                            if response.clicked() {
                                selected = Some(row.entry.path.clone());
                            }
                            if response.double_clicked() {
                                action = Some(row.entry.clone());
                            }
                        });
                    }
                });
            if root_loaded {
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
            for message in messages.iter().take(3) {
                body.label(message);
            }
            if let Some(path) = state.navigation[&pane].selected.clone() {
                body.horizontal_wrapped(|ui| {
                    if ui.button("Copy path").clicked() {
                        if let Some(path) = path.to_str() {
                            ui.ctx().copy_text(path.into());
                        } else {
                            state.message =
                                "This path is unavailable for copying as Unicode text.".into();
                        }
                    }
                    if ui.button("Open").clicked() {
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
        let (visible, _, messages) = rows(&mut state, 1);
        assert_eq!(visible.len(), ROW_LIMIT);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("More entries hidden"))
        );
    }

    #[test]
    fn activation_is_explicit_and_never_follows_a_symlink() {
        let mut state = Explorer::default();
        state.reconcile(&[(1, Some("/root".into()))]);
        assert!(activate(&mut state, 1, &entry("/root/link.md", EntryKind::Link)).is_none());
        assert!(state.message.contains("Preview unavailable"));
        assert!(activate(&mut state, 1, &entry("/root/code.rs", EntryKind::File)).is_none());
        assert_eq!(
            activate(&mut state, 1, &entry("/root/README.MD", EntryKind::File)),
            Some((1, "/root/README.MD".into()))
        );
        activate(&mut state, 1, &entry("/root/src", EntryKind::Directory));
        assert_eq!(state.navigation[&1].root, Some("/root/src".into()));
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
