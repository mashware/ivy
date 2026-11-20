use super::*;
use crate::ui::icons::{FileKind, Icon as Glyph};

struct Row {
    entry: Entry,
    depth: usize,
    message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Icon {
    Search,
    Refresh,
    Collapse,
}

impl Icon {
    fn hint(self) -> &'static str {
        match self {
            Self::Search => "Search files and folders",
            Self::Refresh => "Refresh",
            Self::Collapse => "Collapse all",
        }
    }
}

/// One of the header's icon buttons, on the roles: hover on `hover`, ink on `tx2`, a dimmed ink
/// while no local directory backs it.
fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    (available, on): (bool, bool),
    roles: &crate::theme::Roles,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    let response = response
        .on_hover_text(icon.hint())
        .on_hover_cursor(if available {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::NotAllowed
        });
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, available, icon.hint())
    });
    // The search button stays lit while its list is open, as the kit draws it.
    if on {
        ui.painter().rect_filled(rect, 5.0, roles.accent_soft);
    } else if available && (response.hovered() || response.is_pointer_button_down_on()) {
        ui.painter().rect_filled(rect, 5.0, roles.hover);
    }
    let color = if on {
        roles.accent
    } else if available {
        roles.tx2
    } else {
        roles.tx3
    };
    let shown = match icon {
        Icon::Search => crate::ui::icons::Icon::Search,
        Icon::Refresh => crate::ui::icons::Icon::Refresh,
        Icon::Collapse => crate::ui::icons::Icon::CollapseAll,
    };
    crate::ui::icons::paint(ui.painter(), rect.center(), shown, 13.0, color);
    #[cfg(test)]
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<(Icon, egui::Rect)>>(egui::Id::new("explorer-icons"))
            .retain(|(which, _)| *which != icon);
        data.get_temp_mut_or_default::<Vec<(Icon, egui::Rect)>>(egui::Id::new("explorer-icons"))
            .push((icon, rect));
    });
    available && response.clicked()
}

/// Where the header painted `icon` on its last pass, for tests that click it as a person would.
#[cfg(test)]
pub(super) fn icon_at(ctx: &egui::Context, icon: Icon) -> Option<egui::Pos2> {
    ctx.data(|data| {
        data.get_temp::<Vec<(Icon, egui::Rect)>>(egui::Id::new("explorer-icons"))
            .and_then(|icons| {
                icons
                    .iter()
                    .find(|(which, _)| *which == icon)
                    .map(|(_, r)| r.center())
            })
    })
}

fn rows(state: &mut Explorer, pane: usize) -> (Vec<Row>, Option<Request>, bool) {
    let Some(nav) = state.navigation.get(&pane) else {
        return (Vec::new(), None, false);
    };
    let Some(root) = nav.root.clone().filter(|_| nav.source.is_some()) else {
        return (Vec::new(), None, false);
    };
    let row_limit = nav.row_limit;
    enum Visit {
        Directory(PathBuf, usize),
        Entry(Entry, usize),
    }
    // The name search lists its matches in a floating list of its own; the tree stays the tree.
    let mut stack = vec![Visit::Directory(root, 0)];
    let mut rows = Vec::new();
    let mut pending = None;
    let mut truncated = false;
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
                let mut key = state.request(pane, path.clone());
                key.query.clear();
                key.regex = false;
                state.clock = state.clock.wrapping_add(1);
                let root = state.navigation[&pane].root.as_ref() == Some(&path);
                // A revealed match loads the folders on its way, wherever they fall.
                let revealing = state.navigation[&pane].reveal_selected
                    && state.navigation[&pane]
                        .selected
                        .as_ref()
                        .is_some_and(|selected| selected.starts_with(&path));
                let near_view = revealing
                    || (rows.len() >= visible_start.saturating_sub(1) && rows.len() <= visible_end);
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

pub(super) fn activate(
    state: &mut Explorer,
    pane: usize,
    entry: &Entry,
) -> Option<(usize, PathBuf)> {
    match entry.kind {
        EntryKind::Directory => {
            let expanded = &mut state.navigation.get_mut(&pane).unwrap().expanded;
            if !expanded.remove(&entry.path) {
                expanded.insert(entry.path.clone());
            }
            None
        }
        // Any file opens in the drawer's File tab: Markdown formatted, anything else as code.
        EntryKind::File | EntryKind::Link => {
            if entry.path.to_str().is_none() {
                state.message = "This path cannot be opened as Unicode text.".into();
                None
            } else {
                // An earlier refusal is about another row; it would be misread under this one.
                state.message.clear();
                Some((pane, entry.path.clone()))
            }
        }
        EntryKind::Other => {
            state.message = "Only files and folders open here.".into();
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
            // Files is a side list: `panel`, or the content's `base` where a `panel` set apart
            // from the text would leave the names unreadable (D98).
            let roles = crate::ui::style::roles(ui.ctx(), &theme);
            let surface = roles.panel_or(roles.tx, roles.base);
            let grounds = crate::ui::icons::row_grounds(surface, roles.list_hover, roles.list_sel);
            ui.painter().rect_filled(rect, 0.0, surface);
            ui.painter().line_segment(
                [rect.right_top(), rect.right_bottom()],
                egui::Stroke::new(1.0, roles.line),
            );
            ui.style_mut().visuals.override_text_color = Some(roles.tx);
            let inner = rect.shrink2(egui::vec2(10.0, 7.0));
            let mut body = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            body.set_clip_rect(inner);
            let available = target.is_some_and(|pane| {
                state
                    .navigation
                    .get(&pane)
                    .is_some_and(|nav| nav.source.is_some())
            });
            body.horizontal(|ui| {
                ui.strong("Files");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, Icon::Collapse, (available, false), &roles) {
                        let nav = state.navigation.get_mut(&target.unwrap()).unwrap();
                        nav.expanded.clear();
                        nav.scroll = 0.0;
                    }
                    if icon_button(ui, Icon::Refresh, (available, false), &roles) {
                        state.navigation.get_mut(&target.unwrap()).unwrap().reload();
                        state.message.clear();
                    }
                    if icon_button(ui, Icon::Search, (available, state.searching), &roles) {
                        if state.searching {
                            state.close_search(target);
                        } else {
                            state.open_search(target);
                        }
                    }
                });
            });
            let Some(pane) = target else {
                body.label("No active console.");
                return;
            };
            let Some(nav) = state.navigation.get_mut(&pane) else {
                return;
            };
            let root = nav.root.clone();
            if let Some(root) = &root {
                body.horizontal(|ui| {
                    let (icon, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    crate::ui::icons::paint(
                        ui.painter(),
                        icon.center(),
                        Glyph::Folder { open: true },
                        16.0,
                        roles.accent,
                    );
                    let name = root
                        .file_name()
                        .map(|name| display(Path::new(name)))
                        .unwrap_or_else(|| display(root));
                    ui.add(egui::Label::new(name).truncate())
                        .on_hover_text(display(root));
                });
                body.label(
                    egui::RichText::new("Follows the focused console")
                        .size(11.0)
                        .color(roles.tx3),
                );
            }
            crate::settings::editor::view::style(&mut body, theme);
            if !available {
                body.label("Local directory unavailable or unsupported.");
                return;
            }
            if !state.message.is_empty() {
                body.label(&state.message);
            }
            let mut scroll = state.navigation[&pane].scroll;
            state.row_stride = 22.0 + body.spacing().item_spacing.y;
            state.viewport_rows =
                ((body.available_height() - 32.0).max(22.0) / state.row_stride).ceil() as usize + 1;
            let (rows, pending, truncated) = rows(state, pane);
            let nav = state.navigation.get_mut(&pane).unwrap();
            if nav.reveal_selected
                && let Some(index) = rows
                    .iter()
                    .position(|row| nav.selected.as_ref() == Some(&row.entry.path))
            {
                scroll = (index as f32 * state.row_stride - body.available_height() * 0.5).max(0.0);
                nav.scroll = scroll;
                nav.reveal_selected = false;
            } else if nav.reveal_selected && pending.is_none() {
                // Every folder on the way has answered and the row is not there (a folder over
                // its limit, or the file gone): stop waiting for it.
                nav.reveal_selected = false;
            }
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
                            ui.painter().rect_filled(row_rect, 5.0, roles.list_sel);
                        } else if response.hovered() {
                            ui.painter().rect_filled(row_rect, 5.0, roles.list_hover);
                        }
                        let indent = row.depth as f32 * 12.0;
                        let center_y = row_rect.center().y;
                        let directory = row.entry.kind == EntryKind::Directory;
                        let expanded =
                            directory && state.navigation[&pane].expanded.contains(&row.entry.path);
                        if directory {
                            crate::ui::icons::paint(
                                ui.painter(),
                                egui::pos2(row_rect.left() + indent + 8.0, center_y),
                                Glyph::Caret { open: expanded },
                                12.0,
                                roles.tx3,
                            );
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
                        // The caret already says a row is a folder, so a folder carries no icon
                        // and its name follows the caret; a file keeps its type's icon (#159).
                        let name_left = if directory {
                            row_rect.left() + indent + 18.0
                        } else {
                            let icon_rect = egui::Rect::from_min_size(
                                egui::pos2(row_rect.left() + indent + 18.0, center_y - 8.0),
                                egui::vec2(16.0, 16.0),
                            );
                            let kind = FileKind::of(&row.entry.path);
                            crate::ui::icons::paint(
                                ui.painter(),
                                icon_rect.center(),
                                Glyph::File(kind),
                                16.0,
                                kind.color(&theme, &roles, &grounds),
                            );
                            icon_rect.right() + 5.0
                        };
                        let name =
                            display(Path::new(row.entry.path.file_name().unwrap_or_default()));
                        let suffix = if row.entry.kind == EntryKind::Link {
                            " ↗"
                        } else {
                            ""
                        };
                        ui.painter().text(
                            egui::pos2(name_left, center_y),
                            egui::Align2::LEFT_CENTER,
                            format!("{name}{suffix}"),
                            egui::TextStyle::Body.resolve(ui.style()),
                            roles.tx,
                        );
                        response.clone().on_hover_text(display(&row.entry.path));
                        if response.clicked() {
                            selected = Some(row.entry.path.clone());
                            // A special file goes through activation too, which says why it
                            // does not open.
                            if row.entry.kind != EntryKind::Directory {
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

    #[test]
    fn header_icons_and_their_hover_use_the_effective_themes_roles() {
        use egui::epaint::Shape;
        for named in [
            crate::theme::NamedTheme::GithubLightDefault,
            crate::theme::NamedTheme::TokyoNight,
        ] {
            let theme = named.theme();
            let ctx = crate::ui::icons::installed_for_tests(1.0);
            let mut center = egui::Pos2::ZERO;
            for hover in [false, true] {
                let events = if hover {
                    vec![egui::Event::PointerMoved(center)]
                } else {
                    vec![]
                };
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(200.0, 100.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        center = ui.cursor().min + egui::vec2(12.0, 12.0);
                        let roles = crate::ui::style::roles(ui.ctx(), &theme);
                        icon_button(ui, Icon::Refresh, (true, false), &roles);
                    },
                );
                let strokes: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| crate::ui::icons::painted_icon(&shape.shape))
                    .map(|(_, color, _)| color)
                    .collect();
                let roles = crate::theme::Roles::of(&theme);
                let hover_fill = output.shapes.iter().any(|shape| matches!(&shape.shape, Shape::Rect(rect) if rect.rect.size() == egui::vec2(24.0, 24.0) && rect.fill == roles.hover));
                output.drop_without_applying_deltas();
                assert_eq!(strokes, vec![roles.tx2], "{named:?}, hover={hover}");
                if hover {
                    assert!(hover_fill, "hover did not paint the effective theme's fill");
                }
            }
        }
    }

    fn listing(state: &mut Explorer, path: &str, entries: Vec<Entry>) {
        let key = state.request(1, path.into());
        state.cache.insert(
            key,
            Cached {
                result: Ok(Listing {
                    entries,
                    bytes: 0,
                    ..Default::default()
                }),
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
    fn activation_opens_any_file_and_images_and_keeps_folders_in_the_tree() {
        let mut state = Explorer::default();
        state.reconcile(&[(1, Some("/root".into()))]);
        assert_eq!(
            activate(&mut state, 1, &entry("/root/link.md", EntryKind::Link)),
            Some((1, "/root/link.md".into()))
        );
        assert!(state.message.is_empty());
        for code in [
            "/root/code.rs",
            "/root/index.php",
            "/root/ci.yml",
            "/root/notes.txt",
        ] {
            assert_eq!(
                activate(&mut state, 1, &entry(code, EntryKind::File)),
                Some((1, code.into()))
            );
        }
        assert!(activate(&mut state, 1, &entry("/root/fifo", EntryKind::Other)).is_none());
        assert_eq!(state.message, "Only files and folders open here.");
        state.message.clear();
        for image in ["/root/shot.png", "/root/photo.JPEG"] {
            assert_eq!(
                activate(&mut state, 1, &entry(image, EntryKind::File)),
                Some((1, image.into()))
            );
        }
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

    #[test]
    fn colliding_explorer_rows_paint_the_composited_shared_roles() {
        for source in ["focus=#171d21\n", "foreground=#171d21\n", "panel=#1a372e\n"] {
            let (settings, _) = crate::settings::interpret(source).unwrap();
            let theme = crate::theme::Theme::default().with_colors(&settings.colors);
            let roles = crate::theme::Roles::of(&theme);
            let ctx = egui::Context::default();
            ctx.global_style_mut(|s| s.animation_time = 0.0);
            let mut state = Explorer::default();
            state.reconcile(&[(1, Some("/root".into()))]);
            listing(
                &mut state,
                "/root",
                vec![entry("/root/a.md", EntryKind::File)],
            );
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
            let mut at = egui::Pos2::ZERO;
            for step in 0..4 {
                state.navigation.get_mut(&1).unwrap().selected =
                    (step == 3).then(|| PathBuf::from("/root/a.md"));
                let output = ctx.run_ui(
                    egui::RawInput {
                        time: Some(step as f64),
                        screen_rect: Some(area),
                        events: if step > 0 {
                            vec![egui::Event::PointerMoved(at)]
                        } else {
                            vec![]
                        },
                        ..Default::default()
                    },
                    |ui| {
                        show(&mut state, ui, area, Some(1), theme);
                    },
                );
                for s in &output.shapes {
                    if let egui::Shape::Text(t) = &s.shape
                        && t.galley.text() == "a.md"
                    {
                        at = t.pos + t.galley.size() / 2.0;
                    }
                }
                if step >= 2 {
                    let expected = if step == 3 {
                        roles.list_sel
                    } else {
                        roles.list_hover
                    };
                    assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == expected && r.rect.height() == 22.0)), "{source} step {step}");
                    assert!(
                        crate::theme::project_ink_contrast(
                            roles.panel.blend(expected),
                            roles.panel
                        ) >= 1.03
                    );
                }
                output.drop_without_applying_deltas();
            }
        }
    }

    /// One click on a special file says why it does not open, and a click that opens a file takes
    /// that sentence away.
    #[test]
    fn a_single_click_on_a_special_file_says_why_and_an_open_clears_it() {
        const WHY: &str = "Only files and folders open here.";
        let theme = crate::theme::Theme::default();
        let ctx = egui::Context::default();
        ctx.global_style_mut(|s| s.animation_time = 0.0);
        let mut state = Explorer::default();
        state.reconcile(&[(1, Some("/root".into()))]);
        listing(
            &mut state,
            "/root",
            vec![
                entry("/root/fifo", EntryKind::Other),
                entry("/root/a.rs", EntryKind::File),
            ],
        );
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let mut time = 0.0;
        let mut pass = |state: &mut Explorer, events: Vec<egui::Event>| {
            time += 0.1;
            let mut opened = None;
            let output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(area),
                    events,
                    ..Default::default()
                },
                |ui| opened = show(state, ui, area, Some(1), theme),
            );
            let texts: Vec<(String, egui::Pos2)> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => {
                        Some((t.galley.text().to_owned(), t.pos + t.galley.size() / 2.0))
                    }
                    _ => None,
                })
                .collect();
            output.drop_without_applying_deltas();
            (texts, opened)
        };
        let click = |at: egui::Pos2| {
            let button = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            vec![
                vec![egui::Event::PointerMoved(at)],
                vec![button(true)],
                vec![button(false)],
            ]
        };
        let at = |texts: &[(String, egui::Pos2)], name: &str| {
            texts
                .iter()
                .find(|(t, _)| t == name)
                .unwrap_or_else(|| panic!("{name} not painted: {texts:?}"))
                .1
        };
        pass(&mut state, vec![]);
        let (texts, _) = pass(&mut state, vec![]);
        assert!(!texts.iter().any(|(t, _)| t == WHY));
        for events in click(at(&texts, "fifo")) {
            pass(&mut state, events);
        }
        let (texts, _) = pass(&mut state, vec![]);
        assert!(texts.iter().any(|(t, _)| t == WHY), "{texts:?}");
        let mut opened = None;
        for events in click(at(&texts, "a.rs")) {
            opened = opened.or(pass(&mut state, events).1);
        }
        assert_eq!(opened, Some((1, PathBuf::from("/root/a.rs"))));
        let (texts, _) = pass(&mut state, vec![]);
        assert!(!texts.iter().any(|(t, _)| t == WHY), "{texts:?}");
    }

    /// The tree shows what each row is: a folder by the caret alone, closed or open, with its name
    /// right after the caret; a file by its type's icon in its type's colour.
    #[test]
    fn tree_rows_show_folders_by_their_caret_and_files_by_type() {
        use egui_phosphor::regular as glyph;
        let theme = crate::theme::Theme::default();
        let ctx = crate::ui::icons::installed_for_tests(1.0);
        ctx.global_style_mut(|s| s.animation_time = 0.0);
        let mut state = Explorer::default();
        state.reconcile(&[(1, Some("/root".into()))]);
        listing(
            &mut state,
            "/root",
            vec![
                entry("/root/src", EntryKind::Directory),
                entry("/root/main.rs", EntryKind::File),
                entry("/root/Cargo.lock", EntryKind::File),
            ],
        );
        listing(&mut state, "/root/src", vec![]);
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        // A surface's first pass only measures itself and paints nothing: the second is read.
        // The icons painted, and where each row's name begins.
        type Painted = (Vec<(String, egui::Color32)>, Vec<(String, f32)>);
        let painted = |state: &mut Explorer| -> Painted {
            let pass = |state: &mut Explorer| {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(area),
                        ..Default::default()
                    },
                    |ui| {
                        show(state, ui, area, Some(1), theme);
                    },
                )
            };
            pass(state).drop_without_applying_deltas();
            let output = pass(state);
            let icons: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
                .map(|(g, color, _)| (g, color))
                .collect();
            let names = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some((t.galley.text().to_owned(), t.pos.x)),
                    _ => None,
                })
                .collect();
            output.drop_without_applying_deltas();
            (icons, names)
        };
        let roles = crate::ui::style::roles(&ctx, &theme);
        let has = |icons: &[(String, egui::Color32)], g: &str, color: egui::Color32| {
            icons.iter().any(|(painted, c)| painted == g && *c == color)
        };
        let left_of = |names: &[(String, f32)], name: &str| {
            names
                .iter()
                .find(|(text, _)| text == name)
                .map(|(_, x)| *x)
                .unwrap_or_else(|| panic!("{name} not painted: {names:?}"))
        };
        let folder = |icons: &[(String, egui::Color32)]| {
            icons
                .iter()
                .filter(|(g, _)| g == glyph::FOLDER || g == glyph::FOLDER_OPEN)
                .count()
        };
        let (closed, names) = painted(&mut state);
        assert!(has(&closed, glyph::CARET_RIGHT, roles.tx3), "{closed:?}");
        assert!(!has(&closed, glyph::CARET_DOWN, roles.tx3));
        // Files keep their icons in the same frame, so the absence below is not an empty paint.
        let red = theme.color(term_core::TermColor::Indexed(1));
        assert!(has(&closed, glyph::FILE_RS, red), "{closed:?}");
        assert!(
            closed.iter().any(|(g, _)| g == glyph::FILE_LOCK),
            "{closed:?}"
        );
        // Only the root caption above the tree draws a folder; no tree row does.
        assert_eq!(folder(&closed), 1, "{closed:?}");
        // A folder's name takes the place its icon had; a file's name follows its icon.
        let (src, main) = (left_of(&names, "src"), left_of(&names, "main.rs"));
        assert!(
            (main - src - 21.0).abs() < 0.5,
            "src at {src}, main.rs at {main}"
        );
        state
            .navigation
            .get_mut(&1)
            .unwrap()
            .expanded
            .insert("/root/src".into());
        let (open, _) = painted(&mut state);
        assert!(has(&open, glyph::CARET_DOWN, roles.tx3), "{open:?}");
        assert!(!has(&open, glyph::CARET_RIGHT, roles.tx3), "{open:?}");
        assert_eq!(folder(&open), 1, "{open:?}");
    }
}
