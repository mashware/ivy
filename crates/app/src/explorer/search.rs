//! Files' name search as a floating list over the window (`Files.dc.html`, D72): the existing
//! worker finds the names below the displayed root; Enter reveals a match in the tree and
//! Ctrl+Enter opens it.
use super::{Entry, EntryKind, Explorer, Request, display};
use eframe::egui;
use std::path::{Path, PathBuf};

const WIDTH: f32 = 360.0;
const ROWS: usize = 8;

/// What a person chose in the list.
pub(super) enum Pick {
    /// Show the match in the tree, its folders expanded, without opening it.
    Reveal(PathBuf),
    /// Open it: a Markdown file opens in the drawer and a folder opens or closes in the tree;
    /// any other file is revealed, and Files says why it did not open.
    Open(Entry),
}

/// The matches for the pane's query, what to say instead, and the request to run when the
/// worker has not answered yet.
struct Matches {
    entries: Vec<Entry>,
    said: Option<String>,
    pending: Option<Request>,
}

fn matches(state: &mut Explorer, pane: usize) -> Matches {
    let nav = &state.navigation[&pane];
    let (Some(root), false) = (nav.root.clone(), nav.query.trim().is_empty()) else {
        return Matches {
            entries: Vec::new(),
            said: None,
            pending: None,
        };
    };
    let request = state.request(pane, root);
    state.clock = state.clock.wrapping_add(1);
    let Some(cached) = state.cache.get_mut(&request) else {
        return Matches {
            entries: Vec::new(),
            said: Some("Searching…".into()),
            pending: Some(request),
        };
    };
    cached.used = state.clock;
    match &cached.result {
        Ok(listing) if listing.entries.is_empty() => Matches {
            entries: Vec::new(),
            said: Some("No matching files or folders.".into()),
            pending: None,
        },
        Ok(listing) => Matches {
            entries: listing.entries.clone(),
            said: listing.notice.clone(),
            pending: None,
        },
        Err(error) => Matches {
            entries: Vec::new(),
            said: Some(error.clone()),
            pending: None,
        },
    }
}

/// Where the list floats: just below Files' header, overhanging the consoles, inside the window.
pub(super) fn place(files: egui::Rect, screen: egui::Rect, rows: usize) -> egui::Rect {
    let width = WIDTH.min((screen.right() - files.left() - 8.0).max(0.0));
    let height = 6.0 * 2.0 + 30.0 + 8.0 + rows.min(ROWS) as f32 * ROW + 22.0;
    egui::Rect::from_min_size(
        egui::pos2(files.left() + 8.0, files.top() + 34.0),
        egui::vec2(
            width,
            height.min((screen.bottom() - files.top() - 40.0).max(0.0)),
        ),
    )
}

/// The keys the open list answers before the text field sees them.
pub(super) fn keys(state: &mut Explorer, pane: usize, ctx: &egui::Context) -> Option<Pick> {
    let pressed = |key| ctx.input(|i| i.key_pressed(key));
    let (down, up) = (pressed(egui::Key::ArrowDown), pressed(egui::Key::ArrowUp));
    if !down && !up && !pressed(egui::Key::Enter) {
        return None;
    }
    let found = matches(state, pane).entries;
    if down {
        state.search_index = (state.search_index + 1).min(found.len().saturating_sub(1));
    }
    if up {
        state.search_index = state.search_index.saturating_sub(1);
    }
    state.search_scroll |= down || up;
    let enter = ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                modifiers,
                ..
            } => Some(modifiers.command || i.modifiers.command),
            _ => None,
        })
    })?;
    let entry = found.get(state.search_index)?.clone();
    if enter {
        Some(Pick::Open(entry))
    } else {
        Some(Pick::Reveal(entry.path))
    }
}

/// Paint the list and its field; returns what a click chose. The measures are
/// `Files.dc.html`'s: a 30-point field outlined in the accent with the search icon and an Esc
/// key cap, 30-point rows with the match marked, and a footer under a line.
pub(super) fn show(
    state: &mut Explorer,
    ctx: &egui::Context,
    pane: usize,
    files: egui::Rect,
    theme: &crate::theme::Theme,
) -> Option<Pick> {
    let found = matches(state, pane);
    if state.loading.is_none()
        && let Some(request) = found.pending.clone()
    {
        state.loader.submit(request.clone(), ctx);
        state.loading = Some(request);
    }
    state.search_index = state
        .search_index
        .min(found.entries.len().saturating_sub(1));
    let screen = ctx.content_rect();
    let rect = place(files, screen, found.entries.len().max(1));
    let roles = crate::ui::style::roles(ctx, theme);
    let root = state.navigation[&pane].root.clone().unwrap_or_default();
    let query = state.navigation[&pane].query.trim().to_lowercase();
    let mut pick = None;
    let scroll = std::mem::take(&mut state.search_scroll);
    let shown = egui::Area::new(egui::Id::new(("explorer-search", pane)))
        .order(egui::Order::Tooltip)
        .fixed_pos(rect.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            crate::ui::floating::style(ui, theme);
            ui.set_clip_rect(
                egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), screen.bottom()))
                    .expand(12.0),
            );
            crate::ui::floating::menu_frame(&roles)
                .corner_radius(10)
                .inner_margin(6)
                .show(ui, |ui| {
                    let inner = rect.width() - 2.0 * 6.0 - 2.0;
                    ui.set_width(inner);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    field(state, ui, pane, inner, &roles);
                    let rows = found.entries.len().min(ROWS);
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        egui::ScrollArea::vertical()
                            .id_salt(("explorer-search-results", pane))
                            .max_height(rows as f32 * ROW)
                            .auto_shrink([false, true])
                            .show_rows(ui, ROW, found.entries.len(), |ui, range| {
                                for index in range {
                                    let entry = &found.entries[index];
                                    let chosen = index == state.search_index;
                                    let (choice, response) =
                                        row(ui, entry, (&root, &query), chosen, &roles);
                                    if chosen && scroll {
                                        response.scroll_to_me(None);
                                    }
                                    if let Some(choice) = choice {
                                        state.search_index = index;
                                        pick = Some(choice);
                                    }
                                }
                            });
                    });
                    let count = match &found.said {
                        Some(said) if found.entries.is_empty() => said.clone(),
                        Some(said) => format!("{} · {said}", count(found.entries.len())),
                        None => format!("{} · names", count(found.entries.len())),
                    };
                    footer(ui, &count, &roles);
                });
        });
    // What a press claims is the list as laid out, not an estimate of it.
    state.search_rect = Some(shown.response.rect);
    pick
}

const ROW: f32 = 30.0;

/// The field: the search icon, the query, and the Esc key cap that closes the list.
fn field(
    state: &mut Explorer,
    ui: &mut egui::Ui,
    pane: usize,
    width: f32,
    roles: &crate::theme::Roles,
) {
    use crate::ui::icons::{Icon, paint};
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::hover());
    ui.painter().rect(
        rect,
        6,
        roles.base,
        egui::Stroke::new(2.0, roles.accent),
        egui::StrokeKind::Inside,
    );
    paint(
        ui.painter(),
        egui::pos2(rect.left() + 8.0 + 7.0, rect.center().y),
        Icon::Search,
        14.0,
        roles.tx3,
    );
    let key = ui
        .painter()
        .layout_no_wrap("Esc".into(), egui::FontId::monospace(10.5), roles.tx3);
    let cap = egui::Rect::from_center_size(
        egui::pos2(
            rect.right() - 8.0 - (key.size().x + 10.0) / 2.0,
            rect.center().y,
        ),
        key.size() + egui::vec2(10.0, 2.0),
    );
    ui.painter().rect_stroke(
        cap,
        4,
        egui::Stroke::new(1.0, roles.line2),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(cap.center() - key.size() / 2.0, key, roles.tx3);
    let input = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 30.0, rect.top() + 2.0),
        egui::pos2(cap.left() - 6.0, rect.bottom() - 2.0),
    );
    let nav = state.navigation.get_mut(&pane).unwrap();
    let mut query = nav.query.clone();
    let field = ui.put(
        input,
        egui::TextEdit::singleline(&mut query)
            .id(egui::Id::new(("explorer-name-search", pane)))
            .frame(egui::Frame::NONE)
            .hint_text("Search files and folders")
            .font(egui::FontId::proportional(13.0))
            .text_color(roles.tx)
            .char_limit(256)
            .vertical_align(egui::Align::Center)
            .desired_width(input.width()),
    );
    if std::mem::take(&mut state.focus_search) {
        field.request_focus();
    }
    // A singleline field gives the keys up on Enter; the list keeps them while open, or what is
    // typed next would go nowhere.
    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        field.request_focus();
    }
    if field.changed() {
        // The query is part of each search's request, so the tree's listings stay cached; only
        // a search still running for the old query is let go.
        nav.query = query;
        if state
            .loading
            .as_ref()
            .is_some_and(|r| r.pane == pane && !r.query.is_empty())
        {
            state.loader.cancel();
            state.loading = None;
        }
        state.search_index = 0;
        state.message.clear();
    }
}

fn footer(ui: &mut egui::Ui, count: &str, roles: &crate::theme::Roles) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, roles.line),
    );
    let font = egui::FontId::proportional(11.0);
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y + 1.0),
        egui::Align2::LEFT_CENTER,
        count,
        font.clone(),
        roles.tx3,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y + 1.0),
        egui::Align2::RIGHT_CENTER,
        if cfg!(target_os = "macos") {
            "Enter reveals · Cmd+Enter opens"
        } else {
            "Enter reveals · Ctrl+Enter opens"
        },
        font,
        roles.tx3,
    );
}

fn count(n: usize) -> String {
    match n {
        1 => "1 match".into(),
        n => format!("{n} matches"),
    }
}

fn row(
    ui: &mut egui::Ui,
    entry: &Entry,
    (root, query): (&Path, &str),
    chosen: bool,
    roles: &crate::theme::Roles,
) -> (Option<Pick>, egui::Response) {
    use crate::ui::icons::{Icon, paint};
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), egui::Sense::click());
    if chosen {
        ui.painter().rect_filled(rect, 6, roles.sel);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 6, roles.hover);
    }
    let directory = entry.kind == EntryKind::Directory;
    let icon = egui::pos2(rect.left() + 8.0 + 7.0, rect.center().y);
    paint(
        ui.painter(),
        icon,
        if directory { Icon::Folder } else { Icon::File },
        14.0,
        roles.tx2,
    );
    let name = display(Path::new(entry.path.file_name().unwrap_or_default()));
    let folder = entry
        .path
        .parent()
        .and_then(|parent| parent.strip_prefix(root).ok())
        .map(|parent| {
            let shown = display(parent);
            if shown.is_empty() {
                "./".to_owned()
            } else {
                format!("{shown}/")
            }
        })
        .unwrap_or_default();
    // The part of the name the query matched, marked as the kit marks it.
    let font = egui::FontId::proportional(13.0);
    let plain = egui::TextFormat::simple(font.clone(), roles.tx);
    let mut job = egui::text::LayoutJob::default();
    match (!query.is_empty())
        .then(|| name.to_lowercase().find(query))
        .flatten()
        .filter(|at| name.is_char_boundary(*at) && name.is_char_boundary(at + query.len()))
    {
        Some(at) => {
            job.append(&name[..at], 0.0, plain.clone());
            job.append(
                &name[at..at + query.len()],
                0.0,
                egui::TextFormat {
                    background: roles.wait_soft,
                    color: roles.wait,
                    ..plain.clone()
                },
            );
            job.append(&name[at + query.len()..], 0.0, plain);
        }
        None => job.append(&name, 0.0, plain),
    }
    let name_galley = ui.painter().layout_job(job);
    let at = egui::pos2(
        icon.x + 7.0 + 8.0,
        rect.center().y - name_galley.size().y / 2.0,
    );
    let name_right = at.x + name_galley.size().x;
    ui.painter().galley(at, name_galley, roles.tx);
    let enter = if chosen { 22.0 } else { 0.0 };
    let mut folder_job =
        egui::text::LayoutJob::simple_singleline(folder, egui::FontId::monospace(11.0), roles.tx3);
    folder_job.wrap = egui::text::TextWrapping::truncate_at_width(
        (rect.right() - 8.0 - enter - name_right - 8.0).max(0.0),
    );
    let folder = ui.painter().layout_job(folder_job);
    ui.painter().galley(
        egui::pos2(name_right + 8.0, rect.center().y - folder.size().y / 2.0),
        folder,
        roles.tx3,
    );
    if chosen {
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            "Enter",
            egui::FontId::monospace(10.0),
            roles.tx3,
        );
    }
    let response = response.on_hover_text(display(&entry.path));
    // The click's own modifiers, as the desktop sent them with the press.
    let command = ui.input(|i| {
        i.modifiers.command
            || i.events.iter().any(|event| {
                matches!(event, egui::Event::PointerButton { modifiers, .. } if modifiers.command)
            })
    });
    let pick = response.clicked().then(|| {
        if command {
            Pick::Open(entry.clone())
        } else {
            Pick::Reveal(entry.path.clone())
        }
    });
    (pick, response)
}

/// Show `path` in the tree: every folder between the root and it expanded, it selected and
/// scrolled into view. The root stays where it is.
pub(super) fn reveal(state: &mut Explorer, pane: usize, path: &Path) {
    let Some(nav) = state.navigation.get_mut(&pane) else {
        return;
    };
    let Some(root) = nav.root.clone() else {
        return;
    };
    let mut folder = path.parent();
    while let Some(parent) = folder.filter(|parent| parent.starts_with(&root) && *parent != root) {
        nav.expanded.insert(parent.to_path_buf());
        folder = parent.parent();
    }
    nav.selected = Some(path.to_path_buf());
    nav.reveal_selected = true;
    nav.query.clear();
    state.searching = false;
    state.search_rect = None;
}
